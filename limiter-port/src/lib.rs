// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! dbnexus 限流端口（Limiter port）
//!
//! 层级抽取的防循环依赖接缝：limiteron 已 optional 依赖 dbnexus（作存储后端），
//! Cargo 禁止包级循环依赖（optional 亦然），dbnexus 无法直接反向依赖 limiteron。
//! 故将限流端口独立成本 crate——dbnexus 消费端口（内置令牌桶实现端口，权限
//! 上下文经端口调用），外部限流后端（如 limiteron，经其集成 crate）实现端口，
//! 装配发生在应用组合根。本 crate 不依赖 dbnexus 或 limiteron 任何一方。
//!
//! 端口契约刻意保持最小：单个异步 `check`（`async_trait` 保证对象安全，
//! `Arc<dyn Limiter>` 可注入），判定结果携带 HTTP 429 语义所需的
//! Retry-After 建议，后端故障经 [`RateLimitError`] 显性上报——是否
//! fail-open/fail-closed 由消费方策略决定，端口不擅自放行或拒绝。

use async_trait::async_trait;
use std::fmt;
use std::time::Duration;

/// 限流判定结果
///
/// 携带 HTTP 429 语义所需的最小信息：是否放行 + 拒绝时的 Retry-After 建议。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitDecision {
    /// 是否允许通过
    pub allowed: bool,
    /// 拒绝时的建议等待时间（`Retry-After` 语义）；`None` = 后端未提供
    pub retry_after: Option<Duration>,
}

impl RateLimitDecision {
    /// 允许通过
    pub fn allow() -> Self {
        Self {
            allowed: true,
            retry_after: None,
        }
    }

    /// 拒绝通过（可附 Retry-After 建议）
    pub fn deny(retry_after: Option<Duration>) -> Self {
        Self {
            allowed: false,
            retry_after,
        }
    }
}

/// 限流后端故障
///
/// 后端不可用（如分布式存储断连）时经 `Err` 显性上报，禁止包装成
/// "允许/拒绝" 的判定结果掩盖故障。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitError {
    message: String,
}

impl RateLimitError {
    /// 创建后端故障（`message` 应包含可供排障的上下文）
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// 故障描述
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rate limiter backend error: {}", self.message)
    }
}

impl std::error::Error for RateLimitError {}

impl From<String> for RateLimitError {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

impl From<&str> for RateLimitError {
    fn from(message: &str) -> Self {
        Self::new(message)
    }
}

/// 限流端口
///
/// 所有可接入 dbnexus 权限检查链路的限流后端（内置令牌桶、limiteron 等）
/// 都实现此 trait。`check` 语义为"检查并消费一次配额"。
#[async_trait]
pub trait Limiter: Send + Sync {
    /// 检查 `key`（如角色/用户 ID/IP）是否允许通过，并消费一次配额
    ///
    /// # Errors
    ///
    /// 后端故障时返回 [`RateLimitError`]，消费方按自身策略决定放行或拒绝。
    async fn check(&self, key: &str) -> Result<RateLimitDecision, RateLimitError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Arc;

    #[test]
    fn test_decision_allow() {
        let decision = RateLimitDecision::allow();
        assert!(decision.allowed);
        assert_eq!(decision.retry_after, None);
    }

    #[test]
    fn test_decision_deny_with_retry_after() {
        let decision = RateLimitDecision::deny(Some(Duration::from_secs(7)));
        assert!(!decision.allowed);
        assert_eq!(decision.retry_after, Some(Duration::from_secs(7)));
    }

    #[test]
    fn test_decision_deny_without_retry_after() {
        let decision = RateLimitDecision::deny(None);
        assert!(!decision.allowed);
        assert_eq!(decision.retry_after, None);
    }

    #[test]
    fn test_error_display_and_accessors() {
        let err = RateLimitError::new("storage unavailable");
        assert_eq!(err.message(), "storage unavailable");
        assert_eq!(
            err.to_string(),
            "rate limiter backend error: storage unavailable"
        );
    }

    #[test]
    fn test_error_from_string() {
        let err: RateLimitError = "boom".into();
        assert_eq!(err.message(), "boom");
    }

    /// 端口对象安全：`Arc<dyn Limiter>` 可注入、可分发调用
    #[tokio::test]
    async fn test_trait_object_dispatch() {
        struct AlwaysDeny;

        #[async_trait]
        impl Limiter for AlwaysDeny {
            async fn check(&self, _key: &str) -> Result<RateLimitDecision, RateLimitError> {
                Ok(RateLimitDecision::deny(Some(Duration::from_millis(250))))
            }
        }

        let limiter: Arc<dyn Limiter> = Arc::new(AlwaysDeny);
        let decision = limiter.check("role-a").await.unwrap();
        assert!(!decision.allowed);
        assert_eq!(decision.retry_after, Some(Duration::from_millis(250)));
    }

    /// 端口故障显性上报：`Err` 不得被端口层吞掉
    #[tokio::test]
    async fn test_backend_error_surfaces() {
        struct Broken;

        #[async_trait]
        impl Limiter for Broken {
            async fn check(&self, _key: &str) -> Result<RateLimitDecision, RateLimitError> {
                Err(RateLimitError::new("connection refused"))
            }
        }

        let limiter: Arc<dyn Limiter> = Arc::new(Broken);
        let err = limiter.check("role-a").await.unwrap_err();
        assert!(err.message().contains("connection refused"));
    }
}
