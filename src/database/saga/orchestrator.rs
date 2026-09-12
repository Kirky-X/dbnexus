// Copyright (c) 2026 Kirky.X
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
        log.status = SagaStatus::Compensating;
        let _ = self.saga_log.persist(&log).await;

        let step_index_map: HashMap<&str, usize> = steps
            .iter()
            .enumerate()
            .map(|(i, s)| (s.name.as_str(), i))
            .collect();

        // 逆序对「正向成功」的步骤执行补偿（快照后遍历，避免借用冲突）
        let replay_list: Vec<SagaStepLog> = log
            .steps
            .iter()
            .filter(|s| s.action_success)
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
            if let Ok(Some(session)) = self.router.get_session(step_log.shard_id).await {
                match steps[idx].compensation.execute(&session).await {
                    Ok(()) => compensated.push(step_log.name.clone()),
                    Err(comp_err) => {
                        replay_failed = true;
                        // 原地更新既有条目（补偿结果记录在原步骤上，避免重复条目
                        // 导致后续重放对同一步骤补偿多次）
                        mark_compensation_failed(&mut log, &step_log.name, &format!("compensation replay failed: {comp_err}"));
                    }
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
        let _ = self.saga_log.persist(&log).await;

        SagaExecutionResult {
            saga_id: saga_id.to_string(),
            success: false,
            status: final_status,
            completed_steps: Vec::new(),
            compensated_steps: compensated,
            failure: None,
        }
    }

    /// 执行 Saga
    pub async fn execute_saga(&self, steps: Vec<SagaStep>) -> SagaExecutionResult {
        let saga_id = uuid::Uuid::new_v4().to_string();
        let mut log = SagaLog {
            saga_id: saga_id.clone(),
            status: SagaStatus::Running,
            steps: Vec::new(),
        };
        // 初始状态持久化（best-effort，失败不阻断 saga 执行）
        let _ = self.saga_log.persist(&log).await;

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
                        let _ = self.saga_log.persist(&log).await;
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
                        let _ = self.saga_log.persist(&log).await;

                        for (completed_name, completed_shard_id, _) in completed_steps.iter().rev()
                        {
                            if let Ok(Some(session)) =
                                self.router.get_session(*completed_shard_id).await
                            {
                                // O(1) 查找原始步骤的 compensation
                                if let Some(&idx) = step_index_map.get(completed_name.as_str()) {
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
                        }

                        let final_status = if compensation_failed {
                            SagaStatus::CompensationFailed
                        } else {
                            SagaStatus::Failed
                        };
                        log.status = final_status;
                        let _ = self.saga_log.persist(&log).await;

                        return SagaExecutionResult {
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
                    let _ = self.saga_log.persist(&log).await;
                    return SagaExecutionResult {
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
                    let _ = self.saga_log.persist(&log).await;
                    return SagaExecutionResult {
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
        let _ = self.saga_log.persist(&log).await;
        SagaExecutionResult {
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

