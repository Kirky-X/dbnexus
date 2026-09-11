// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 分布式事务 Saga 编排器
//!
//! 每分片独立事务 + 补偿操作，应用层协调，无跨分片锁。

use std::collections::HashMap;
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

impl crate::i18n::error_ext::LocalizedMsg for SagaError {
    fn message_key(&self) -> &'static str {
        match self {
            Self::ExecutionFailed(_) => "saga-execution-failed",
            Self::CompensationFailed(_) => "saga-compensation-failed",
            Self::Timeout(_) => "saga-timeout",
        }
    }

    fn message_args(&self) -> Vec<(&str, String)> {
        match self {
            Self::ExecutionFailed(reason) => vec![("reason", reason.clone())],
            Self::CompensationFailed(reason) => vec![("reason", reason.clone())],
            Self::Timeout(reason) => vec![("reason", reason.clone())],
        }
    }
}

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
    /// 已失败（正向动作失败，补偿全部成功）
    Failed,
    /// 补偿失败（正向动作失败且至少一个补偿操作也失败）
    CompensationFailed,
}

impl SagaStatus {
    /// T402：持久化用小写标识
    pub fn as_str(&self) -> &'static str {
        match self {
            SagaStatus::Running => "running",
            SagaStatus::Completed => "completed",
            SagaStatus::Compensating => "compensating",
            SagaStatus::Failed => "failed",
            SagaStatus::CompensationFailed => "compensation_failed",
        }
    }

    /// T402：从存储标识解析
    pub fn from_str_kind(s: &str) -> SagaStatus {
        match s {
            "completed" => SagaStatus::Completed,
            "compensating" => SagaStatus::Compensating,
            "failed" => SagaStatus::Failed,
            "compensation_failed" | "compensation-failed" => SagaStatus::CompensationFailed,
            _ => SagaStatus::Running,
        }
    }
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
        Self {
            logs: DashMap::new(),
        }
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
// SagaLogStore trait（T402：日志存储端口，内存/DB 实现）
// ============================================================================

/// Saga 日志存储端口（T402）
///
/// 内存实现为默认；`DbSagaLog`（`sql-parser` feature）提供基于数据库的持久化。
#[async_trait]
pub trait SagaLogStore: Send + Sync {
    /// 插入/覆盖指定 saga 的整条日志（幂等 upsert）
    async fn persist(&self, log: &SagaLog) -> Result<(), String>;

    /// 加载未终结（Running/Compensating）的 saga 日志（启动恢复用）
    async fn load_pending(&self) -> Result<Vec<SagaLog>, String>;

    /// 按 saga_id 获取日志
    async fn get(&self, saga_id: &str) -> Result<Option<SagaLog>, String>;
}

#[async_trait]
impl SagaLogStore for InMemorySagaLog {
    async fn persist(&self, log: &SagaLog) -> Result<(), String> {
        self.logs.insert(log.saga_id.clone(), log.clone());
        Ok(())
    }

    async fn load_pending(&self) -> Result<Vec<SagaLog>, String> {
        Ok(self
            .logs
            .iter()
            .map(|r| r.value().clone())
            .filter(|l| matches!(l.status, SagaStatus::Running | SagaStatus::Compensating))
            .collect())
    }

    async fn get(&self, saga_id: &str) -> Result<Option<SagaLog>, String> {
        Ok(InMemorySagaLog::get(self, saga_id))
    }
}

/// DB 持久化 Saga 日志存储（T402，`sql-parser` feature）
///
/// 复用 dbnexus 自身的连接池执行 DDL/UPSERT，行查询经统一 API `query_rows`。
/// 步骤日志以 JSON 文本列存储，无需为公共类型引入 serde 依赖。
#[cfg(feature = "sql-parser")]
pub struct DbSagaLog {
    pool: Arc<crate::database::DbPool>,
}

#[cfg(feature = "sql-parser")]
impl DbSagaLog {
    /// 创建存储（表按需幂等创建：首次 persist/get/load 前调用 `init`）
    pub fn new(pool: Arc<crate::database::DbPool>) -> Self {
        Self { pool }
    }

    /// 建表（幂等）
    pub async fn init(&self) -> Result<(), String> {
        let session = self
            .pool
            .get_session("admin")
            .await
            .map_err(|e| e.to_string())?;
        session
            .execute_raw_ddl(
                "CREATE TABLE IF NOT EXISTS saga_logs (\
                 saga_id TEXT PRIMARY KEY, status TEXT NOT NULL, \
                 steps TEXT NOT NULL, updated_at TEXT NOT NULL)",
            )
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

#[cfg(feature = "sql-parser")]
#[async_trait]
impl SagaLogStore for DbSagaLog {
    async fn persist(&self, log: &SagaLog) -> Result<(), String> {
        self.init().await?;
        let steps_text = saga_steps_to_json(&log.steps).to_string();
        let status = log.status.as_str();
        // updated_at 用客户端时间戳（跨 sqlite/postgres 方言安全）
        let updated_at = chrono::Utc::now().to_rfc3339();
        let sql = format!(
            "INSERT INTO saga_logs (saga_id, status, steps, updated_at) \
             VALUES ('{}', '{}', '{}', '{}') \
             ON CONFLICT(saga_id) DO UPDATE SET status = excluded.status, \
             steps = excluded.steps, updated_at = excluded.updated_at",
            log.saga_id.replace('\'', "''"),
            status,
            steps_text.replace('\'', "''"),
            updated_at,
        );
        let session = self.pool.get_session("admin").await.map_err(|e| e.to_string())?;
        session.execute_raw(&sql).await.map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn load_pending(&self) -> Result<Vec<SagaLog>, String> {
        let rows = self
            .pool
            .query_rows(
                "SELECT saga_id, status, steps FROM saga_logs \
                 WHERE status IN ('running', 'compensating')",
                "admin",
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(rows.iter().filter_map(saga_log_from_row).collect())
    }

    async fn get(&self, saga_id: &str) -> Result<Option<SagaLog>, String> {
        let rows = self
            .pool
            .query_rows(
                &format!(
                    "SELECT saga_id, status, steps FROM saga_logs WHERE saga_id = '{}'",
                    saga_id.replace('\'', "''")
                ),
                "admin",
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(rows.first().and_then(saga_log_from_row))
    }
}

/// T402：步骤日志 → JSON（避免公共类型引入 serde derive）
#[cfg(feature = "sql-parser")]
fn saga_steps_to_json(steps: &[SagaStepLog]) -> serde_json::Value {
    serde_json::Value::Array(
        steps
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "shard_id": s.shard_id,
                    "action_success": s.action_success,
                    "compensation_success": s.compensation_success,
                    "error": s.error,
                })
            })
            .collect(),
    )
}

/// T401 行 → SagaLog（postgres/sqlite 通用列名约定）
#[cfg(feature = "sql-parser")]
fn saga_log_from_row(row: &serde_json::Value) -> Option<SagaLog> {
    let saga_id = row.get("saga_id")?.as_str()?.to_string();
    let status = row
        .get("status")
        .and_then(|v| v.as_str())
        .map(SagaStatus::from_str_kind)
        .unwrap_or(SagaStatus::Running);
    let steps = row
        .get("steps")
        .and_then(|v| v.as_str())
        .and_then(|txt| serde_json::from_str::<serde_json::Value>(txt).ok())
        .and_then(|v| v.as_array().cloned())
        .map(|arr| {
            arr.iter()
                .filter_map(|s| {
                    Some(SagaStepLog {
                        name: s.get("name")?.as_str()?.to_string(),
                        shard_id: s.get("shard_id")?.as_u64()? as u32,
                        action_success: s
                            .get("action_success")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                        compensation_success: s
                            .get("compensation_success")
                            .and_then(|v| v.as_bool()),
                        error: s.get("error").and_then(|v| v.as_str()).map(str::to_string),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Some(SagaLog {
        saga_id,
        status,
        steps,
    })
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
// SagaRecovery（T402：启动恢复）
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

    /// 创建编排器并注入自定义日志存储（T402：`DbSagaLog` 持久化 + 启动恢复）
    pub fn new_with_log_store(router: Arc<ShardRouter>, saga_log: Arc<dyn SagaLogStore>) -> Self {
        Self { router, saga_log }
    }

    /// T402：对已持久化的未完成 saga 重放补偿
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
                continue;
            };
            if let Ok(Some(session)) = self.router.get_session(step_log.shard_id).await {
                match steps[idx].compensation.execute(&session).await {
                    Ok(()) => compensated.push(step_log.name.clone()),
                    Err(comp_err) => {
                        replay_failed = true;
                        log.steps.push(SagaStepLog {
                            name: step_log.name.clone(),
                            shard_id: step_log.shard_id,
                            action_success: true,
                            compensation_success: Some(false),
                            error: Some(format!("compensation replay failed: {comp_err}")),
                        });
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
        // T402：初始状态持久化（best-effort，失败不阻断 saga 执行）
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
                        // T402：每步落盘（best-effort，持久化失败不中断 saga）
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
                                            // 补偿失败：记录结构化事件，不吞错
                                            log.steps.push(SagaStepLog {
                                                name: completed_name.clone(),
                                                shard_id: *completed_shard_id,
                                                action_success: true,
                                                compensation_success: Some(false),
                                                error: Some(format!(
                                                    "compensation failed: {comp_err}"
                                                )),
                                            });
                                            compensation_failed = true;
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

    /// 获取 Saga 日志（T402：经日志存储异步获取）
    pub async fn get_saga_log(&self, saga_id: &str) -> Option<SagaLog> {
        self.saga_log.get(saga_id).await.ok().flatten()
    }
}
