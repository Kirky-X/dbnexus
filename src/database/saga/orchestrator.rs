// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 分布式事务 Saga 编排器
//!
//! 每分片独立事务 + 补偿操作，应用层协调，无跨分片锁。

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::database::Session;
use crate::database::sharding::ShardRouter;

use super::store::{InMemorySagaLog, SagaLog, SagaLogStore};
use super::types::*;

/// Saga 执行结果
#[derive(Debug)]
pub struct SagaExecutionResult {
    /// Saga 唯一标识
    pub saga_id: String,
    /// 是否成功
    pub success: bool,
    /// 最终状态
    pub status: SagaStatus,
    /// 已完成的步骤名称
    pub completed_steps: Vec<String>,
    /// 已补偿的步骤名称
    pub compensated_steps: Vec<String>,
    /// 失败信息
    pub failure: Option<SagaFailure>,
    /// 日志持久化失败次数（显性化：persist 失败不再静默丢弃；
    /// >0 表示恢复日志不完整，进程崩溃后该 saga 可能无法精确恢复）
    pub persist_failures: u32,
}

/// Saga 失败信息
#[derive(Debug)]
pub struct SagaFailure {
    /// 失败步骤名称
    pub step_name: String,
    /// 错误信息
    pub error: String,
}

/// 将补偿失败结果原地写回该步骤的既有日志条目（保留单条记录，
/// 后续重放按 `action_success` 仍会重试该步骤）
fn mark_compensation_failed(log: &mut SagaLog, step_name: &str, error: &str) {
    if let Some(entry) = log
        .steps
        .iter_mut()
        .find(|s| s.name == step_name && s.action_success)
    {
        entry.compensation_success = Some(false);
        entry.error = Some(error.to_string());
    }
}

// ============================================================================
// SagaRecovery
// ============================================================================

/// Saga 启动恢复器：从日志存储加载未完成（Running/Compensating）的 saga。
///
/// 典型用法：进程启动时 `list_pending()` 找到中断的 saga，随后经
/// `SagaOrchestrator::compensate_recovered()` 重放补偿（步骤定义由调用方重供）。
pub struct SagaRecovery {
    store: Arc<dyn SagaLogStore>,
}

impl SagaRecovery {
    /// 创建恢复器
    pub fn new(store: Arc<dyn SagaLogStore>) -> Self {
        Self { store }
    }

    /// 列出未完成（Running/Compensating）的 saga 日志
    pub async fn list_pending(&self) -> Result<Vec<SagaLog>, String> {
        self.store.load_pending().await
    }
}

// ============================================================================
// SagaOrchestrator
// ============================================================================

/// Saga 编排器
///
/// 按顺序执行每个步骤的 action，失败时逆序执行补偿操作。
pub struct SagaOrchestrator {
    router: Arc<ShardRouter>,
    saga_log: Arc<dyn SagaLogStore>,
}

impl SagaOrchestrator {
    /// 创建编排器（默认内存日志存储）
    pub fn new(router: Arc<ShardRouter>) -> Self {
        Self {
            router,
            saga_log: Arc::new(InMemorySagaLog::new()),
        }
    }

    /// 创建编排器并注入自定义日志存储
    pub fn new_with_log_store(router: Arc<ShardRouter>, saga_log: Arc<dyn SagaLogStore>) -> Self {
        Self { router, saga_log }
    }

    /// 对已持久化的未完成 saga 重放补偿
    ///
    /// 调用方需重新提供步骤定义（`SagaStep` 携带不可序列化的 action）；
    /// 对日志中「正向已成功」的步骤逆序执行补偿，任一补偿失败进入
    /// `CompensationFailed` 终态（可再次调用本方法重放）。
    pub async fn compensate_recovered(
        &self,
        saga_id: &str,
        steps: &[SagaStep],
    ) -> SagaExecutionResult {
        let stored = match self.saga_log.get(saga_id).await {
            Ok(Some(log)) => log,
            _ => {
                return SagaExecutionResult {
                    persist_failures: 0,
                    saga_id: saga_id.to_string(),
                    success: false,
                    status: SagaStatus::Failed,
                    completed_steps: Vec::new(),
                    compensated_steps: Vec::new(),
                    failure: Some(SagaFailure {
                        step_name: saga_id.to_string(),
                        error: "recovered saga log not found".to_string(),
                    }),
                };
            }
        };

        let mut log = stored.clone();
        let mut compensated: Vec<String> = Vec::new();
        let mut replay_failed = false;
        let mut persist_failures = 0u32;
        log.status = SagaStatus::Compensating;
        self.persist_logged(&log, &mut persist_failures).await;

        let step_index_map: HashMap<&str, usize> = steps
            .iter()
            .enumerate()
            .map(|(i, s)| (s.name.as_str(), i))
            .collect();

        // 逆序对「正向成功且尚未成功补偿」的步骤执行补偿（快照后遍历，避免借用冲突）。
        // compensation_success == Some(true) 的步骤已被补偿过，重放时跳过，
        // 否则幂等性不足的补偿动作会被二次执行（资金级二次回滚）。
        let replay_list: Vec<SagaStepLog> = log
            .steps
            .iter()
            .filter(|s| s.action_success && s.compensation_success != Some(true))
            .rev()
            .cloned()
            .collect();
        for step_log in replay_list {
            let Some(&idx) = step_index_map.get(step_log.name.as_str()) else {
                // 日志中的步骤未在重供定义中找到：该步骤无法补偿，不能视为重放成功
                //（终态进入 CompensationFailed，调用方可补齐步骤定义后再次重放）
                replay_failed = true;
                continue;
            };
            // 会话获取失败同样显性化：置 replay_failed 并记录到日志条目，
            // 终态进入 CompensationFailed（调用方修复后可再次重放）
            match self.router.get_session(step_log.shard_id).await {
                Ok(Some(session)) => {
                    match steps[idx].compensation.execute(&session).await {
                        Ok(()) => compensated.push(step_log.name.clone()),
                        Err(comp_err) => {
                            replay_failed = true;
                            // 原地更新既有条目（补偿结果记录在原步骤上，避免重复条目
                            // 导致后续重放对同一步骤补偿多次）
                            mark_compensation_failed(
                                &mut log,
                                &step_log.name,
                                &format!("compensation replay failed: {comp_err}"),
                            );
                        }
                    }
                }
                Ok(None) => {
                    replay_failed = true;
                    mark_compensation_failed(
                        &mut log,
                        &step_log.name,
                        &format!(
                            "compensation session unavailable: no pool for shard {}",
                            step_log.shard_id
                        ),
                    );
                }
                Err(e) => {
                    replay_failed = true;
                    mark_compensation_failed(
                        &mut log,
                        &step_log.name,
                        &format!("compensation session unavailable: {e}"),
                    );
                }
            }
        }

        // 以「本次重放」的补偿结果定终态（重试语义：上次失败可被本次成功覆盖）
        let final_status = if replay_failed {
            SagaStatus::CompensationFailed
        } else {
            SagaStatus::Failed
        };
        log.status = final_status;
        self.persist_logged(&log, &mut persist_failures).await;

        SagaExecutionResult {
            persist_failures,
            saga_id: saga_id.to_string(),
            success: false,
            status: final_status,
            completed_steps: Vec::new(),
            compensated_steps: compensated,
            failure: None,
        }
    }

    /// persist 并累计失败次数（显性化：失败不再被 `let _ =` 静默吞掉，
    /// 由 `SagaExecutionResult::persist_failures` 暴露给调用方决策）
    async fn persist_logged(&self, log: &SagaLog, failures: &mut u32) {
        if self.saga_log.persist(log).await.is_err() {
            *failures += 1;
        }
    }

    /// 执行 Saga（saga_id 由库生成；需复用调用方 ID 时用 [`Self::execute_saga_with_id`]）
    pub async fn execute_saga(&self, steps: Vec<SagaStep>) -> SagaExecutionResult {
        let saga_id = uuid::Uuid::new_v4().to_string();
        self.execute_saga_inner(saga_id, steps).await
    }

    /// 执行 Saga（调用方提供 saga_id：幂等重试/跨服务追踪需跨进程复用同一 ID 时使用）
    ///
    /// 不做去重：同一 saga_id 再次执行会重跑全部步骤并整条覆盖旧日志
    /// （日志存储为幂等 upsert），action 级幂等与日志覆盖语义由调用方负责。
    /// 空 `saga_id` 视为未提供，回退为库生成 uuid v4。
    pub async fn execute_saga_with_id(
        &self,
        saga_id: &str,
        steps: Vec<SagaStep>,
    ) -> SagaExecutionResult {
        let saga_id = if saga_id.is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            saga_id.to_string()
        };
        self.execute_saga_inner(saga_id, steps).await
    }

    /// 内部执行体（saga_id 已确定）
    async fn execute_saga_inner(
        &self,
        saga_id: String,
        steps: Vec<SagaStep>,
    ) -> SagaExecutionResult {
        let mut persist_failures = 0u32;
        let mut log = SagaLog {
            saga_id: saga_id.clone(),
            status: SagaStatus::Running,
            steps: Vec::new(),
        };
        // 初始状态持久化（best-effort，失败不阻断 saga 执行）
        self.persist_logged(&log, &mut persist_failures).await;

        let mut completed_steps: Vec<(String, u32, Box<dyn SagaAction>)> = Vec::new();
        let mut completed_names: Vec<String> = Vec::new();

        // 预建步骤名→索引映射，补偿时 O(1) 查找替代线性扫描
        let step_index_map: HashMap<&str, usize> = steps
            .iter()
            .enumerate()
            .map(|(i, s)| (s.name.as_str(), i))
            .collect();

        // 顺序执行每个步骤
        for step in &steps {
            let session_result = self.router.get_session(step.shard_id).await;
            match session_result {
                Ok(Some(session)) => match step.action.execute(&session).await {
                    Ok(()) => {
                        log.steps.push(SagaStepLog {
                            name: step.name.clone(),
                            shard_id: step.shard_id,
                            action_success: true,
                            compensation_success: None,
                            error: None,
                        });
                        completed_names.push(step.name.clone());
                        // 每步落盘（best-effort，持久化失败不中断 saga）
                        self.persist_logged(&log, &mut persist_failures).await;
                    }
                    Err(e) => {
                        log.steps.push(SagaStepLog {
                            name: step.name.clone(),
                            shard_id: step.shard_id,
                            action_success: false,
                            compensation_success: None,
                            error: Some(e.to_string()),
                        });

                        // 逆序补偿已完成步骤
                        let mut compensated: Vec<String> = Vec::new();
                        let mut compensation_failed = false;
                        log.status = SagaStatus::Compensating;
                        self.persist_logged(&log, &mut persist_failures).await;

                        for (completed_name, completed_shard_id, _) in completed_steps.iter().rev()
                        {
                            // 会话获取失败同样不得静默跳过补偿——否则该步骤的
                            // 补偿缺口被吞掉，终态误报为 Failed（补偿实际未执行）
                            match self.router.get_session(*completed_shard_id).await {
                                Ok(Some(session)) => {
                                    // O(1) 查找原始步骤的 compensation
                                    if let Some(&idx) = step_index_map.get(completed_name.as_str())
                                    {
                                        match steps[idx].compensation.execute(&session).await {
                                            Ok(()) => {
                                                compensated.push(completed_name.clone());
                                            }
                                            Err(comp_err) => {
                                                // 补偿失败：原地更新既有条目，不吞错
                                                compensation_failed = true;
                                                mark_compensation_failed(
                                                    &mut log,
                                                    completed_name,
                                                    &format!("compensation failed: {comp_err}"),
                                                );
                                            }
                                        }
                                    }
                                }
                                Ok(None) => {
                                    compensation_failed = true;
                                    mark_compensation_failed(
                                        &mut log,
                                        completed_name,
                                        &format!(
                                            "compensation session unavailable: no pool for shard {completed_shard_id}"
                                        ),
                                    );
                                }
                                Err(e) => {
                                    compensation_failed = true;
                                    mark_compensation_failed(
                                        &mut log,
                                        completed_name,
                                        &format!("compensation session unavailable: {e}"),
                                    );
                                }
                            }
                        }

                        let final_status = if compensation_failed {
                            SagaStatus::CompensationFailed
                        } else {
                            SagaStatus::Failed
                        };
                        log.status = final_status;
                        self.persist_logged(&log, &mut persist_failures).await;

                        return SagaExecutionResult {
                            persist_failures,
                            saga_id,
                            success: false,
                            status: final_status,
                            completed_steps: completed_names,
                            compensated_steps: compensated,
                            failure: Some(SagaFailure {
                                step_name: step.name.clone(),
                                error: e.to_string(),
                            }),
                        };
                    }
                },
                Err(e) => {
                    log.status = SagaStatus::Failed;
                    self.persist_logged(&log, &mut persist_failures).await;
                    return SagaExecutionResult {
                        persist_failures,
                        saga_id,
                        success: false,
                        status: SagaStatus::Failed,
                        completed_steps: completed_names,
                        compensated_steps: Vec::new(),
                        failure: Some(SagaFailure {
                            step_name: step.name.clone(),
                            error: e.to_string(),
                        }),
                    };
                }
                Ok(None) => {
                    log.status = SagaStatus::Failed;
                    self.persist_logged(&log, &mut persist_failures).await;
                    return SagaExecutionResult {
                        persist_failures,
                        saga_id,
                        success: false,
                        status: SagaStatus::Failed,
                        completed_steps: completed_names,
                        compensated_steps: Vec::new(),
                        failure: Some(SagaFailure {
                            step_name: step.name.clone(),
                            error: format!("No session available for shard {}", step.shard_id),
                        }),
                    };
                }
            }
            // 占位：`Box<dyn SagaAction>` 无法从 `&step` move 出来，
            // 补偿操作在上方逆序循环中直接引用原始 `steps` 索引。
            // 此 NoopAction 仅用于填充 `completed_steps` 元组的第三字段，
            // 不参与实际补偿逻辑。
            completed_steps.push((step.name.clone(), step.shard_id, {
                struct NoopAction;
                #[async_trait]
                impl SagaAction for NoopAction {
                    async fn execute(&self, _session: &Session) -> Result<(), SagaError> {
                        Ok(())
                    }
                    fn name(&self) -> &str {
                        "noop"
                    }
                }
                Box::new(NoopAction) as Box<dyn SagaAction>
            }));
        }

        // 全部成功
        log.status = SagaStatus::Completed;
        self.persist_logged(&log, &mut persist_failures).await;
        SagaExecutionResult {
            persist_failures,
            saga_id,
            success: true,
            status: SagaStatus::Completed,
            completed_steps: completed_names,
            compensated_steps: Vec::new(),
            failure: None,
        }
    }

    /// 获取 Saga 日志
    pub async fn get_saga_log(&self, saga_id: &str) -> Option<SagaLog> {
        self.saga_log.get(saga_id).await.ok().flatten()
    }
}

#[cfg(test)]
mod persist_failure_tests {
    use super::*;
    use crate::database::sharding::ShardRouter;
    use std::sync::Arc;

    /// persist 恒失败的 mock 存储
    struct FailingStore;

    #[async_trait]
    impl SagaLogStore for FailingStore {
        async fn persist(&self, _log: &SagaLog) -> Result<(), String> {
            Err("disk full".to_string())
        }
        async fn load_pending(&self) -> Result<Vec<SagaLog>, String> {
            Ok(Vec::new())
        }
        async fn get(&self, _saga_id: &str) -> Result<Option<SagaLog>, String> {
            Ok(None)
        }
    }

    #[derive(Default)]
    struct NoopAction;

    #[async_trait]
    impl SagaAction for NoopAction {
        async fn execute(&self, _session: &Session) -> Result<(), SagaError> {
            Ok(())
        }
        fn name(&self) -> &str {
            "noop"
        }
    }

    fn single_step() -> Vec<SagaStep> {
        vec![SagaStep {
            name: "only".to_string(),
            shard_id: 0,
            action: Box::new(NoopAction),
            compensation: Box::new(NoopAction),
        }]
    }

    /// R-saga-001: persist 失败显性化——结果携带失败计数，saga 正常返回
    #[tokio::test]
    async fn persist_failures_surface_in_result() {
        let orchestrator = SagaOrchestrator::new_with_log_store(
            Arc::new(ShardRouter::default()),
            Arc::new(FailingStore),
        );
        let result = orchestrator.execute_saga(single_step()).await;
        assert!(
            result.persist_failures >= 1,
            "persist failures must be counted, got {}",
            result.persist_failures
        );
        assert_eq!(result.status, SagaStatus::Failed);
    }

    /// R-saga-001: persist 全部成功时计数为 0
    #[tokio::test]
    async fn persist_failures_zero_when_store_healthy() {
        let orchestrator = SagaOrchestrator::new(Arc::new(ShardRouter::default()));
        let result = orchestrator.execute_saga(single_step()).await;
        assert_eq!(result.persist_failures, 0);
        assert_eq!(result.status, SagaStatus::Failed);
    }
}

/// 外部 saga_id 注入契约：结果与持久化日志携带调用方 ID，空 ID 回退库生成
#[cfg(test)]
mod external_id_tests {
    use super::*;

    struct NoopAction;

    #[async_trait]
    impl SagaAction for NoopAction {
        async fn execute(&self, _session: &Session) -> Result<(), SagaError> {
            Ok(())
        }
        fn name(&self) -> &str {
            "noop"
        }
    }

    fn single_step() -> Vec<SagaStep> {
        vec![SagaStep {
            name: "only".to_string(),
            shard_id: 0,
            action: Box::new(NoopAction),
            compensation: Box::new(NoopAction),
        }]
    }

    #[tokio::test]
    async fn execute_saga_with_id_uses_provided_id() {
        let orchestrator = SagaOrchestrator::new(Arc::new(ShardRouter::default()));
        let result = orchestrator
            .execute_saga_with_id("caller-saga-001", single_step())
            .await;
        assert_eq!(result.saga_id, "caller-saga-001");
        match orchestrator.get_saga_log("caller-saga-001").await {
            Some(log) => assert_eq!(log.saga_id, "caller-saga-001"),
            None => panic!("saga 日志必须以调用方提供的 saga_id 持久化"),
        }
    }

    #[tokio::test]
    async fn execute_saga_with_id_falls_back_on_empty() {
        let orchestrator = SagaOrchestrator::new(Arc::new(ShardRouter::default()));
        let result = orchestrator.execute_saga_with_id("", single_step()).await;
        assert!(!result.saga_id.is_empty(), "空 saga_id 必须回退为库生成");
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod replay_compensation_tests {
    use super::*;
    use crate::database::sharding::ShardRouter;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// 可预置日志的内存存储
    struct PreloadedStore {
        log: std::sync::Mutex<Option<SagaLog>>,
    }

    #[async_trait]
    impl SagaLogStore for PreloadedStore {
        async fn persist(&self, log: &SagaLog) -> Result<(), String> {
            *self.log.lock().expect("store lock") = Some(log.clone());
            Ok(())
        }
        async fn load_pending(&self) -> Result<Vec<SagaLog>, String> {
            Ok(self
                .log
                .lock()
                .expect("store lock")
                .as_ref()
                .filter(|l| matches!(l.status, SagaStatus::Running | SagaStatus::Compensating))
                .cloned()
                .into_iter()
                .collect())
        }
        async fn get(&self, saga_id: &str) -> Result<Option<SagaLog>, String> {
            Ok(self
                .log
                .lock()
                .expect("store lock")
                .clone()
                .filter(|l| l.saga_id == saga_id))
        }
    }

    /// 计数补偿动作
    struct CountingCompensation {
        calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl SagaAction for CountingCompensation {
        async fn execute(&self, _session: &Session) -> Result<(), SagaError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn name(&self) -> &str {
            "counting-comp"
        }
    }

    #[derive(Default)]
    struct NoopForward;

    #[async_trait]
    impl SagaAction for NoopForward {
        async fn execute(&self, _session: &Session) -> Result<(), SagaError> {
            Ok(())
        }
        fn name(&self) -> &str {
            "noop-forward"
        }
    }

    /// R-saga-002: 重放跳过已成功补偿（Some(true)）的步骤，Some(false) 的重试
    #[tokio::test]
    async fn replay_skips_already_compensated_steps() {
        let saga_id = "saga-replay-test".to_string();
        let stored = SagaLog {
            saga_id: saga_id.clone(),
            status: SagaStatus::Compensating,
            steps: vec![
                SagaStepLog {
                    name: "step-a".to_string(),
                    shard_id: 0,
                    action_success: true,
                    compensation_success: Some(true), // 已成功补偿 → 必须跳过
                    error: None,
                },
                SagaStepLog {
                    name: "step-b".to_string(),
                    shard_id: 0,
                    action_success: true,
                    compensation_success: Some(false), // 补偿失败 → 重试
                    error: Some("boom".to_string()),
                },
            ],
        };
        let store = Arc::new(PreloadedStore {
            log: std::sync::Mutex::new(Some(stored)),
        });

        // permission feature 下 "default" 角色会被安全默认策略拒绝，
        // 用 admin 角色确保补偿会话可获取（与生产配置口径一致）
        let router = ShardRouter::default().with_session_role("admin");
        let pool = std::sync::Arc::new(
            crate::database::DbPool::new("sqlite::memory:")
                .await
                .expect("pool"),
        );
        router.add_shard(0, pool);

        let calls_a = Arc::new(AtomicUsize::new(0));
        let calls_b = Arc::new(AtomicUsize::new(0));
        let steps = vec![
            SagaStep {
                name: "step-a".to_string(),
                shard_id: 0,
                action: Box::new(NoopForward),
                compensation: Box::new(CountingCompensation {
                    calls: calls_a.clone(),
                }),
            },
            SagaStep {
                name: "step-b".to_string(),
                shard_id: 0,
                action: Box::new(NoopForward),
                compensation: Box::new(CountingCompensation {
                    calls: calls_b.clone(),
                }),
            },
        ];

        let orchestrator = SagaOrchestrator::new_with_log_store(std::sync::Arc::new(router), store);
        let result = orchestrator.compensate_recovered(&saga_id, &steps).await;

        assert_eq!(
            result.status,
            SagaStatus::Failed,
            "replay should now complete"
        );
        assert_eq!(
            calls_a.load(Ordering::SeqCst),
            0,
            "already-compensated step must be skipped (no double compensation)"
        );
        assert_eq!(
            calls_b.load(Ordering::SeqCst),
            1,
            "failed-compensation step must be retried exactly once"
        );
        assert_eq!(result.compensated_steps, vec!["step-b".to_string()]);
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod session_failure_tests {
    use super::*;
    use crate::database::sharding::ShardRouter;
    use std::sync::Arc;

    /// 可预置日志的内存存储
    struct PreloadedStore {
        log: std::sync::Mutex<Option<SagaLog>>,
    }

    #[async_trait]
    impl SagaLogStore for PreloadedStore {
        async fn persist(&self, log: &SagaLog) -> Result<(), String> {
            *self.log.lock().expect("store lock") = Some(log.clone());
            Ok(())
        }
        async fn load_pending(&self) -> Result<Vec<SagaLog>, String> {
            Ok(Vec::new())
        }
        async fn get(&self, saga_id: &str) -> Result<Option<SagaLog>, String> {
            Ok(self
                .log
                .lock()
                .expect("store lock")
                .clone()
                .filter(|l| l.saga_id == saga_id))
        }
    }

    #[derive(Default)]
    struct NoopAction;

    #[async_trait]
    impl SagaAction for NoopAction {
        async fn execute(&self, _session: &Session) -> Result<(), SagaError> {
            Ok(())
        }
        fn name(&self) -> &str {
            "noop"
        }
    }

    /// 补偿会话不可用（空 router 无池）必须显性进入 CompensationFailed，
    /// 不得静默跳过伪装成补偿成功的 Failed
    #[tokio::test]
    async fn compensation_with_unavailable_session_enters_compensation_failed() {
        let saga_id = "saga-session-failure".to_string();
        let stored = SagaLog {
            saga_id: saga_id.clone(),
            status: SagaStatus::Compensating,
            steps: vec![SagaStepLog {
                name: "step-x".to_string(),
                shard_id: 7, // 无池分片
                action_success: true,
                compensation_success: None,
                error: None,
            }],
        };
        let store = Arc::new(PreloadedStore {
            log: std::sync::Mutex::new(Some(stored)),
        });

        let steps = vec![SagaStep {
            name: "step-x".to_string(),
            shard_id: 7,
            action: Box::new(NoopAction),
            compensation: Box::new(NoopAction),
        }];

        // 空 router：分片 7 无池 → 会话获取必然失败
        let orchestrator =
            SagaOrchestrator::new_with_log_store(Arc::new(ShardRouter::default()), store);
        let result = orchestrator.compensate_recovered(&saga_id, &steps).await;

        assert_eq!(
            result.status,
            SagaStatus::CompensationFailed,
            "session-unavailable compensation must surface as CompensationFailed"
        );
        assert!(
            result.compensated_steps.is_empty(),
            "nothing was actually compensated"
        );
    }
}
