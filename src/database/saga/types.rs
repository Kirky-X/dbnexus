// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 分布式事务 Saga 编排器
//!
//! 每分片独立事务 + 补偿操作，应用层协调，无跨分片锁。

use std::fmt;

use async_trait::async_trait;

use crate::database::Session;

// ============================================================================
// Saga 核心类型（纯移动自 saga.rs）
// ============================================================================

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
    /// 持久化用小写标识
    pub fn as_str(&self) -> &'static str {
        match self {
            SagaStatus::Running => "running",
            SagaStatus::Completed => "completed",
            SagaStatus::Compensating => "compensating",
            SagaStatus::Failed => "failed",
            SagaStatus::CompensationFailed => "compensation_failed",
        }
    }

    /// 从存储标识解析
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::error_ext::LocalizedMsg;

    #[test]
    fn test_saga_error_display() {
        assert_eq!(
            SagaError::ExecutionFailed("boom".to_string()).to_string(),
            "Saga execution failed: boom"
        );
        assert_eq!(
            SagaError::CompensationFailed("undo".to_string()).to_string(),
            "Saga compensation failed: undo"
        );
        assert_eq!(
            SagaError::Timeout("30s".to_string()).to_string(),
            "Saga timeout: 30s"
        );
    }

    #[test]
    fn test_saga_error_localized_msg() {
        let cases = [
            (
                SagaError::ExecutionFailed("boom".to_string()),
                "saga-execution-failed",
            ),
            (
                SagaError::CompensationFailed("undo".to_string()),
                "saga-compensation-failed",
            ),
            (SagaError::Timeout("30s".to_string()), "saga-timeout"),
        ];
        for (err, key) in cases {
            assert_eq!(err.message_key(), key);
            assert_eq!(err.message_args(), vec![("reason", err_arg(&err))]);
        }
    }

    fn err_arg(err: &SagaError) -> String {
        match err {
            SagaError::ExecutionFailed(r)
            | SagaError::CompensationFailed(r)
            | SagaError::Timeout(r) => r.clone(),
        }
    }

    #[test]
    fn test_saga_status_str_round_trip() {
        let cases = [
            (SagaStatus::Running, "running"),
            (SagaStatus::Completed, "completed"),
            (SagaStatus::Compensating, "compensating"),
            (SagaStatus::Failed, "failed"),
            (SagaStatus::CompensationFailed, "compensation_failed"),
        ];
        for (status, s) in cases {
            assert_eq!(status.as_str(), s);
            assert_eq!(SagaStatus::from_str_kind(s), status);
        }
        // 连字符历史形态与未知值回退
        assert_eq!(
            SagaStatus::from_str_kind("compensation-failed"),
            SagaStatus::CompensationFailed
        );
        assert_eq!(SagaStatus::from_str_kind("nonsense"), SagaStatus::Running);
    }
}
