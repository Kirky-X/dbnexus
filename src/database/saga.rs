// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 分布式事务 Saga 编排器
//!
//! 每分片独立事务 + 补偿操作，应用层协调，无跨分片锁。

use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;

use crate::database::Session;
use crate::database::sharding::ShardRouter;

// ============================================================================
// SagaError
// ============================================================================

/// Saga 执行错误
#[derive(Debug)]
pub enum SagaError {
    /// 执行失败
    ExecutionFailed(String),
    /// 补偿失败
    CompensationFailed(String),
    /// 超时
    Timeout(String),
}

impl fmt::Display for SagaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExecutionFailed(msg) => write!(f, "Saga execution failed: {msg}"),
            Self::CompensationFailed(msg) => write!(f, "Saga compensation failed: {msg}"),
            Self::Timeout(msg) => write!(f, "Saga timeout: {msg}"),
        }
    }
}

impl std::error::Error for SagaError {}

// ============================================================================
// SagaAction trait
// ============================================================================

/// Saga 步骤动作 trait
#[async_trait]
pub trait SagaAction: Send + Sync {
    /// 执行动作
    async fn execute(&self, session: &Session) -> Result<(), SagaError>;
    /// 动作名称（用于日志）
    fn name(&self) -> &str;
}

// ============================================================================
// SagaStep
// ============================================================================

/// Saga 步骤定义
pub struct SagaStep {
    /// 步骤名称
    pub name: String,
    /// 目标分片 ID
    pub shard_id: u32,
    /// 正向动作
    pub action: Box<dyn SagaAction>,
    /// 补偿动作
    pub compensation: Box<dyn SagaAction>,
}

// ============================================================================
// SagaStatus / SagaLog
// ============================================================================

/// Saga 执行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SagaStatus {
    /// 正在执行
    Running,
    /// 已完成
    Completed,
    /// 正在补偿
    Compensating,
    /// 已失败
    Failed,
}

/// 单步执行日志
#[derive(Debug, Clone)]
pub struct SagaStepLog {
    /// 步骤名称
    pub name: String,
    /// 目标分片 ID
    pub shard_id: u32,
    /// 正向动作是否成功
    pub action_success: bool,
    /// 补偿动作是否成功
    pub compensation_success: Option<bool>,
    /// 错误信息
    pub error: Option<String>,
}

/// Saga 执行日志
#[derive(Debug, Clone)]
pub struct SagaLog {
    /// Saga 唯一标识
    pub saga_id: String,
    /// 执行状态
    pub status: SagaStatus,
    /// 各步骤日志
    pub steps: Vec<SagaStepLog>,
}

/// 内存 Saga 日志存储
pub struct InMemorySagaLog {
    logs: DashMap<String, SagaLog>,
}

impl InMemorySagaLog {
    /// 创建内存日志存储
    pub fn new() -> Self {
        Self { logs: DashMap::new() }
    }

    /// 获取指定 saga 的日志
    pub fn get(&self, saga_id: &str) -> Option<SagaLog> {
        self.logs.get(saga_id).map(|r| r.value().clone())
    }

    /// 插入日志
    pub fn insert(&self, log: SagaLog) {
        self.logs.insert(log.saga_id.clone(), log);
    }

    /// 更新 saga 状态
    pub fn update_status(&self, saga_id: &str, status: SagaStatus) {
        if let Some(mut log) = self.logs.get_mut(saga_id) {
            log.status = status;
        }
    }
}

impl Default for InMemorySagaLog {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// SagaExecutionResult
// ============================================================================

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

// ============================================================================
// SagaOrchestrator
// ============================================================================

/// Saga 编排器
///
/// 按顺序执行每个步骤的 action，失败时逆序执行补偿操作。
pub struct SagaOrchestrator {
    router: Arc<ShardRouter>,
    saga_log: Arc<InMemorySagaLog>,
}

impl SagaOrchestrator {
    /// 创建编排器
    pub fn new(router: Arc<ShardRouter>) -> Self {
        Self {
            router,
            saga_log: Arc::new(InMemorySagaLog::new()),
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
        self.saga_log.insert(log.clone());

        let mut completed_steps: Vec<(String, u32, Box<dyn SagaAction>)> = Vec::new();
        let mut completed_names: Vec<String> = Vec::new();

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
                        // 注意：由于 Box<dyn SagaAction> 不能 clone，补偿步骤在失败时由原始 steps 处理
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
                        self.saga_log.update_status(&saga_id, SagaStatus::Compensating);

                        for (completed_name, completed_shard_id, _) in completed_steps.iter().rev() {
                            if let Ok(Some(session)) = self.router.get_session(*completed_shard_id).await {
                                // 找到对应步骤的 compensation
                                if let Some(original_step) = steps.iter().find(|s| s.name == *completed_name) {
                                    if let Ok(()) = original_step.compensation.execute(&session).await {
                                        compensated.push(completed_name.clone());
                                    }
                                }
                            }
                        }

                        self.saga_log.update_status(&saga_id, SagaStatus::Failed);

                        return SagaExecutionResult {
                            saga_id,
                            success: false,
                            status: SagaStatus::Failed,
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
                    self.saga_log.update_status(&saga_id, SagaStatus::Failed);
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
                    self.saga_log.update_status(&saga_id, SagaStatus::Failed);
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
            // 由于无法 move Box<dyn SagaAction> out of &step，简化处理
            completed_steps.push((step.name.clone(), step.shard_id, {
                // 占位：实际补偿在上面的逆序循环中直接引用原始 steps
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
        self.saga_log.update_status(&saga_id, SagaStatus::Completed);
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
    pub fn get_saga_log(&self, saga_id: &str) -> Option<SagaLog> {
        self.saga_log.get(saga_id)
    }
}
