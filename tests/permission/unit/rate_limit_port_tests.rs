// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 限流端口统一测试
//!
//! 覆盖 R-dbnexus-003 验收面：
//! - Limiter 端口契约（令牌桶实现端口、外部后端注入）
//! - 双后端切换配置生效（令牌桶默认 / 外部端口实现）
//! - 429 响应语义（RateLimited 决策携带 Retry-After，区别于策略拒绝）
//! - 限流拒绝审计事件（audit feature）

use dbnexus::access::{PermissionAction, PermissionConfig, RateLimiter, RolePolicy};
use dbnexus::access::{PermissionContext, RateLimitBackend, TableAccessDecision, TablePermission};
use dbnexus::{DbError, Limiter, RateLimitDecision, RateLimitError};
use std::sync::Arc;
use std::time::Duration;

/// 构造允许 test_role 查询 users 表的策略
fn policy_allowing_select() -> PermissionConfig {
    PermissionConfig {
        roles: [(
            "test_role".to_string(),
            RolePolicy {
                tables: vec![TablePermission {
                    name: "users".to_string(),
                    operations: vec![PermissionAction::Select],
                }],
            },
        )]
        .into_iter()
        .collect(),
    }
}

/// 固定判定的外部限流后端（测试桩）
struct FixedLimiter {
    decision: Result<RateLimitDecision, RateLimitError>,
}

#[async_trait::async_trait]
impl Limiter for FixedLimiter {
    async fn check(&self, _key: &str) -> Result<RateLimitDecision, RateLimitError> {
        self.decision.clone()
    }
}

// ============================================================================
// Limiter 端口契约
// ============================================================================

/// 旧令牌桶实现 Limiter 端口：作为 Arc<dyn Limiter> 消费，拒绝时携带 Retry-After
#[tokio::test]
async fn test_token_bucket_impls_limiter_port() {
    let limiter: Arc<dyn Limiter> = Arc::new(RateLimiter::new(1, Duration::from_secs(60), 16, 1));

    let first = limiter.check("test_role").await.unwrap();
    assert!(first.allowed, "first request should consume the token");

    let second = limiter.check("test_role").await.unwrap();
    assert!(!second.allowed, "bucket exhausted -> denied");
    assert!(
        second.retry_after.is_some(),
        "denial should carry a Retry-After hint"
    );
}

// ============================================================================
// 双后端切换配置生效
// ============================================================================

/// 默认令牌桶后端：配额耗尽产生 RateLimited 决策（429 语义），而非策略 Denied
#[tokio::test]
async fn test_token_bucket_backend_rate_limited_decision() {
    let ctx = PermissionContext::with_cache_size_and_backend(
        "test_role".to_string(),
        16,
        RateLimitBackend::TokenBucket {
            max_requests: 1,
            window_secs: 60,
        },
    )
    .await
    .unwrap();
    ctx.load_policy(&policy_allowing_select()).await.unwrap();

    let first = ctx
        .check_table_access_decision("users", &PermissionAction::Select)
        .await;
    assert_eq!(first, TableAccessDecision::Allowed);

    let second = ctx
        .check_table_access_decision("users", &PermissionAction::Select)
        .await;
    match second {
        TableAccessDecision::RateLimited { retry_after } => {
            assert!(
                retry_after.is_some(),
                "token bucket should hint Retry-After"
            );
        }
        other => panic!("expected RateLimited, got {other:?}"),
    }

    // 限流拒绝计入统计
    assert_eq!(ctx.check_stats().snapshot().rate_limited_checks, 1);

    // 旧 bool API 兼容：限流拒绝同样返回 false
    assert!(
        !ctx.check_table_access("users", &PermissionAction::Select)
            .await
    );
}

/// 外部端口后端注入生效：拒绝判定与 Retry-After 透传自外部实现
#[tokio::test]
async fn test_external_backend_deny_passthrough() {
    let ctx = PermissionContext::with_cache_size_and_backend(
        "test_role".to_string(),
        16,
        RateLimitBackend::External(Arc::new(FixedLimiter {
            decision: Ok(RateLimitDecision::deny(Some(Duration::from_secs(7)))),
        })),
    )
    .await
    .unwrap();
    ctx.load_policy(&policy_allowing_select()).await.unwrap();

    let decision = ctx
        .check_table_access_decision("users", &PermissionAction::Select)
        .await;
    assert_eq!(
        decision,
        TableAccessDecision::RateLimited {
            retry_after: Some(Duration::from_secs(7))
        }
    );
    assert_eq!(ctx.check_stats().snapshot().rate_limited_checks, 1);
}

/// 外部端口后端放行后仍走策略判定：限流(429)与权限拒绝(403)语义可区分
#[tokio::test]
async fn test_external_backend_allow_then_policy_applies() {
    let ctx = PermissionContext::with_cache_size_and_backend(
        "test_role".to_string(),
        16,
        RateLimitBackend::External(Arc::new(FixedLimiter {
            decision: Ok(RateLimitDecision::allow()),
        })),
    )
    .await
    .unwrap();
    ctx.load_policy(&policy_allowing_select()).await.unwrap();

    let allowed = ctx
        .check_table_access_decision("users", &PermissionAction::Select)
        .await;
    assert_eq!(allowed, TableAccessDecision::Allowed);

    let denied = ctx
        .check_table_access_decision("secrets", &PermissionAction::Select)
        .await;
    assert_eq!(denied, TableAccessDecision::Denied);
    assert_eq!(ctx.check_stats().snapshot().rate_limited_checks, 0);
}

// ============================================================================
// 429 响应语义
// ============================================================================

/// DbError::RateLimited：429 语义载体，携带 Retry-After 秒数
#[test]
fn test_db_error_rate_limited_semantics() {
    let err = DbError::RateLimited {
        retry_after_secs: Some(7),
    };
    assert!(err.to_string().contains("Rate limited"));
    assert!(err.to_string().contains("7"));

    let err = DbError::RateLimited {
        retry_after_secs: None,
    };
    assert!(err.to_string().contains("Rate limited"));
}

/// 后端故障 fail-closed：Err 上报为限流拒绝（无 Retry-After），绝不静默放行
#[tokio::test]
async fn test_backend_error_fails_closed() {
    let ctx = PermissionContext::with_cache_size_and_backend(
        "test_role".to_string(),
        16,
        RateLimitBackend::External(Arc::new(FixedLimiter {
            decision: Err(RateLimitError::new("storage unavailable")),
        })),
    )
    .await
    .unwrap();
    ctx.load_policy(&policy_allowing_select()).await.unwrap();

    let decision = ctx
        .check_table_access_decision("users", &PermissionAction::Select)
        .await;
    assert_eq!(
        decision,
        TableAccessDecision::RateLimited { retry_after: None }
    );
    assert_eq!(ctx.check_stats().snapshot().rate_limited_checks, 1);
}

// ============================================================================
// 审计事件（audit feature）
// ============================================================================

/// 限流拒绝产生审计事件：operation=rate_limit_exceeded，result=Failure，
/// 事件携带表名/操作/Retry-After 上下文
#[cfg(feature = "audit")]
#[tokio::test]
async fn test_rate_limit_denial_emits_audit_event() {
    use dbnexus::{
        AuditConfig, AuditLogger, AuditQueryFilters, AuditSeverity, AuditStatus, AuditStorage,
        MemoryAuditStorage,
    };

    let storage = Arc::new(MemoryAuditStorage::new(16));
    let logger = AuditLogger::with_config(AuditConfig::default(), storage.clone());

    let mut ctx = PermissionContext::with_cache_size_and_backend(
        "test_role".to_string(),
        16,
        RateLimitBackend::External(Arc::new(FixedLimiter {
            decision: Ok(RateLimitDecision::deny(Some(Duration::from_secs(7)))),
        })),
    )
    .await
    .unwrap();
    ctx.load_policy(&policy_allowing_select()).await.unwrap();
    ctx.set_audit_logger(Arc::new(logger));
    let decision = ctx
        .check_table_access_decision("users", &PermissionAction::Select)
        .await;
    assert!(matches!(decision, TableAccessDecision::RateLimited { .. }));

    let events = storage.query(&AuditQueryFilters::default()).await.unwrap();
    assert_eq!(events.len(), 1, "rate limit denial should emit one event");
    let event = &events[0];
    assert_eq!(event.entity_type, "table_access");
    assert_eq!(event.entity_id, "users");
    assert_eq!(event.user_role, "test_role");
    assert_eq!(event.result, AuditStatus::Failure);
    assert_eq!(event.severity, AuditSeverity::Medium);
    let extra = event.extra.as_deref().unwrap_or_default();
    assert!(extra.contains("rate_limit_exceeded"), "extra: {extra}");
    assert!(
        extra.contains("SELECT"),
        "extra should carry operation: {extra}"
    );
    assert!(
        extra.contains("7"),
        "extra should carry retry_after: {extra}"
    );
}

/// 未挂审计器时限流拒绝不产生事件、不影响判定
#[cfg(feature = "audit")]
#[tokio::test]
async fn test_rate_limit_without_audit_logger_still_decides() {
    let ctx = PermissionContext::with_cache_size_and_backend(
        "test_role".to_string(),
        16,
        RateLimitBackend::External(Arc::new(FixedLimiter {
            decision: Ok(RateLimitDecision::deny(None)),
        })),
    )
    .await
    .unwrap();
    ctx.load_policy(&policy_allowing_select()).await.unwrap();

    let decision = ctx
        .check_table_access_decision("users", &PermissionAction::Select)
        .await;
    assert!(matches!(decision, TableAccessDecision::RateLimited { .. }));
}
