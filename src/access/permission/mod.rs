// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 权限控制模块
//!
//! 提供基于角色的表级权限控制功能
//!
//! **模块定位**：本模块为权限**运行时实现层**，提供 `PermissionContext`（缓存 + 速率限制 +
//! 缓存击穿防护）、`RateLimiter`、`RbacProvider`/`AdvancedRbacProvider`、统计等运行时能力。
//! [`crate::domain::permission`] 为权限**领域接口层**，提供 trait 定义和配置类型。
//! 两层互补共存，非"已弃用"关系。新代码如需运行时上下文/缓存/速率限制，使用本模块；
//! 如仅需接口或配置类型，使用 domain::permission。

// 子模块声明
pub mod advanced;
pub mod rbac;

pub mod cache;
mod context;
mod provider;
mod rate_limiter;
mod stats;
mod types;

// ============================================================================
// 公共类型重导出
// ============================================================================

// 从 types.rs 重导出
#[cfg(any(feature = "ladybug", feature = "neo4j"))]
pub use types::GraphPermissionContext;
pub use types::{PermissionAction, PermissionConfig, PermissionError, RolePolicy, TablePermission};

// 从 provider.rs 重导出
pub use provider::{
    MemoryPermissionProvider, PermissionProvider, PermissionProviderError, RefreshablePermissionProvider,
    YamlPermissionProvider,
};

// 从 stats.rs 重导出
pub use stats::{CacheStats, PermissionCheckStats, PermissionCheckStatsSnapshot};

// 从 rate_limiter.rs 重导出
pub use rate_limiter::RateLimiter;

// 从 cache.rs 重导出（TTL + SWR 缓存，0.3.0 新增）
pub use cache::{PermissionCache, PermissionCacheConfig};

// 从 context.rs 重导出（需要 cache feature）
#[cfg(feature = "cache")]
pub use context::PermissionContext;

// ============================================================================
// 内部使用（pub(crate)）
// ============================================================================

// ============================================================================
// Public API Re-exports
// ============================================================================

// Re-export AdvancedRbacProvider for easy access
pub use self::advanced::AdvancedRbacProvider;

// Re-export RbacProvider for easy access
pub use self::rbac::RbacProvider;

// ============================================================================
// 测试模块
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试公共 API 可访问性
    #[test]
    fn test_public_api_accessible() {
        // 验证所有公共类型都可以访问
        let _action = PermissionAction::Select;
        let _operation: PermissionAction = PermissionAction::Insert;

        // 验证 Display 实现
        assert_eq!(PermissionAction::Select.to_string(), "SELECT");
        assert_eq!(PermissionAction::Insert.to_string(), "INSERT");
        assert_eq!(PermissionAction::Update.to_string(), "UPDATE");
        assert_eq!(PermissionAction::Delete.to_string(), "DELETE");
    }

    /// 测试 PermissionConfig 公共 API
    #[test]
    fn test_permission_config_public_api() {
        let config = PermissionConfig::allow_all();
        assert!(config.check_access("admin", "users", PermissionAction::Select));

        let config = PermissionConfig::deny_all();
        assert!(!config.check_access("admin", "users", PermissionAction::Select));
    }

    /// 测试 RolePolicy 公共 API
    #[test]
    fn test_role_policy_public_api() {
        let policy = RolePolicy {
            tables: vec![TablePermission {
                name: "*".to_string(),
                operations: vec![PermissionAction::Select],
            }],
        };

        assert!(policy.allows("any_table", &PermissionAction::Select));
        assert!(!policy.allows("any_table", &PermissionAction::Insert));
    }

    /// 测试 PermissionCheckStats 公共 API
    #[test]
    fn test_permission_check_stats_public_api() {
        let stats = PermissionCheckStats::new();

        stats.record_allowed();
        stats.record_denied();
        stats.record_cache_hit();
        stats.record_cache_miss();

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.total_checks, 2);
        assert_eq!(snapshot.allowed_checks, 1);
        assert_eq!(snapshot.denied_checks, 1);
        assert_eq!(snapshot.cache_hits, 1);
        assert_eq!(snapshot.cache_misses, 1);
    }

    /// 测试 PermissionCheckStatsSnapshot 公共 API
    #[test]
    fn test_permission_check_stats_snapshot_public_api() {
        let snapshot = PermissionCheckStatsSnapshot {
            total_checks: 100,
            allowed_checks: 80,
            denied_checks: 20,
            rate_limited_checks: 5,
            cache_hits: 90,
            cache_misses: 10,
            stampede_events: 0,
        };

        // 缓存命中率
        assert!((snapshot.cache_hit_rate() - 0.9).abs() < 0.001);

        // 拒绝率
        assert!((snapshot.denial_rate() - 0.2).abs() < 0.001);
    }
}
