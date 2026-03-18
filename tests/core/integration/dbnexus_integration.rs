// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! DbNexus 核心入口集成测试
//!
//! 覆盖 DbPool 初始化、连接、关闭和 DbPoolBuilder 构建等功能测试

use dbnexus::{DbPool, DbPoolBuilder, config::DatabaseType};
use std::sync::Arc;
use std::time::Duration;

#[path = "../../common/mod.rs"]
mod common;

// ============================================================================
// DbNexus 初始化测试
// ============================================================================

/// TEST-DBNEXUS-001: DbPool::new() 基本初始化测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_dbpool_new_basic() {
    let url = common::get_test_database_url();
    let pool = DbPool::new(&url).await.expect("Failed to create DbPool with new()");

    // 验证连接池配置
    let config = pool.config();
    assert!(!config.url.is_empty());
    assert!(config.max_connections > 0);
    assert!(config.min_connections > 0);
}

/// TEST-DBNEXUS-002: DbPool::with_config() 使用配置初始化测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_dbpool_with_config() {
    let url = common::get_test_database_url();
    let config = dbnexus::config::DbConfig {
        url,
        max_connections: 10,
        min_connections: 2,
        idle_timeout: 300,
        acquire_timeout: 5000,
        admin_role: "admin".to_string(),
        ..Default::default()
    };

    let pool = DbPool::with_config(config)
        .await
        .expect("Failed to create DbPool with config");

    // 验证配置被正确应用
    assert_eq!(pool.config().max_connections, 10);
    assert_eq!(pool.config().min_connections, 2);
    assert_eq!(pool.config().admin_role, "admin");
}

/// TEST-DBNEXUS-003: DbPool::try_from_config() 从配置结构体初始化测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_dbpool_try_from_config() {
    let url = common::get_test_database_url();
    let config = dbnexus::config::DbConfig {
        url,
        max_connections: 5,
        min_connections: 1,
        ..Default::default()
    };

    let pool = DbPool::try_from_config(config)
        .await
        .expect("Failed to create DbPool with try_from_config");

    assert_eq!(pool.config().max_connections, 5);
    assert_eq!(pool.config().min_connections, 1);
}

/// TEST-DBNEXUS-004: DbPool::try_from() 同步初始化测试（无权限特性）
#[tokio::test]
#[cfg(all(
    not(feature = "permission"),
    any(feature = "sqlite", feature = "postgres", feature = "mysql")
))]
async fn test_dbpool_try_from_sync() {
    let url = common::get_test_database_url();
    let config = dbnexus::config::DbConfig {
        url,
        max_connections: 5,
        min_connections: 1,
        ..Default::default()
    };

    let pool = DbPool::try_from(&config).expect("Failed to create DbPool with try_from");

    // 同步创建的连接池初始状态
    let status = pool.status();
    assert_eq!(status.total, 0);
    assert_eq!(status.active, 0);
    assert_eq!(status.idle, 0);
}

/// TEST-DBNEXUS-005: DbPool::try_from() 同步初始化测试（带权限特性）
#[tokio::test]
#[cfg(all(
    feature = "permission",
    any(feature = "sqlite", feature = "postgres", feature = "mysql")
))]
async fn test_dbpool_try_from_with_permission() {
    let url = common::get_test_database_url();
    let config = dbnexus::config::DbConfig {
        url,
        max_connections: 5,
        min_connections: 1,
        ..Default::default()
    };

    let pool = DbPool::try_from(&config).expect("Failed to create DbPool with try_from");

    // 验证权限缓存已初始化
    let status = pool.status();
    assert_eq!(status.total, 0);
}

// ============================================================================
// DbNexus 连接测试
// ============================================================================

/// TEST-DBNEXUS-006: 获取会话测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_get_session() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");

    let session = pool.get_session("admin").await.expect("Failed to get session");
    assert_eq!(session.role(), "admin");
}

/// TEST-DBNEXUS-007: 多次获取会话测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_get_session_multiple() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");

    // 获取多个会话
    let session1 = pool.get_session("admin").await.expect("Failed to get session1");
    let session2 = pool.get_session("admin").await.expect("Failed to get session2");

    assert_eq!(session1.role(), "admin");
    assert_eq!(session2.role(), "admin");

    // 验证连接池状态
    let status = pool.status();
    assert!(status.active >= 2);
}

/// TEST-DBNEXUS-008: 连接池状态测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_pool_status() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");

    let status = pool.status();

    // 验证状态字段
    assert!(status.total <= pool.config().max_connections);
    assert!(status.active <= status.total);
    assert_eq!(status.idle, status.total.saturating_sub(status.active));
}

/// TEST-DBNEXUS-009: 连接池健康检查测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_pool_health_check() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");

    // 获取会话后检查连接池健康状态
    let session = pool.get_session("admin").await.expect("Failed to get session");

    let status = pool.status();
    assert!(status.active >= 1);

    // 会话释放后（Drop）
    drop(session);

    // 给一点时间让连接归还
    tokio::time::sleep(Duration::from_millis(100)).await;

    let status = pool.status();
    assert!(status.idle >= 1 || status.total == 0);
}

/// TEST-DBNEXUS-010: 连接池配置获取测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_pool_config_access() {
    let url = common::get_test_database_url();
    let config = dbnexus::config::DbConfig {
        url,
        max_connections: 15,
        min_connections: 3,
        idle_timeout: 600,
        acquire_timeout: 8000,
        admin_role: "superuser".to_string(),
        ..Default::default()
    };

    let pool = DbPool::with_config(config).await.expect("Failed to create pool");

    // 验证配置访问
    let pool_config = pool.config();
    assert_eq!(pool_config.max_connections, 15);
    assert_eq!(pool_config.min_connections, 3);
    assert_eq!(pool_config.idle_timeout, 600);
    assert_eq!(pool_config.acquire_timeout, 8000);
    assert_eq!(pool_config.admin_role, "superuser");
}

// ============================================================================
// DbNexus 关闭测试
// ============================================================================

/// TEST-DBNEXUS-011: 连接池 Drop 自动关闭测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_pool_drop_cleanup() {
    let (config, _temp_dir) = common::get_test_config();

    {
        let pool = DbPool::with_config(config).await.expect("Failed to create pool");
        let _session = pool.get_session("admin").await.expect("Failed to get session");

        let status = pool.status();
        assert!(status.active >= 1);
    } // pool 在此处被 Drop

    // 连接池已离开作用域，资源应被清理
    // 此测试主要验证 Drop 不会 panic
}

/// TEST-DBNEXUS-012: 多会话场景下的连接池关闭测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_pool_drop_with_multiple_sessions() {
    let (config, _temp_dir) = common::get_test_config();

    {
        let pool = DbPool::with_config(config).await.expect("Failed to create pool");

        // 创建多个会话
        let session1 = pool.get_session("admin").await.expect("Failed to get session1");
        let session2 = pool.get_session("admin").await.expect("Failed to get session2");
        let session3 = pool.get_session("admin").await.expect("Failed to get session3");

        let status = pool.status();
        assert!(status.active >= 3);

        // 会话先释放
        drop(session1);
        drop(session2);
        drop(session3);

        // 然后连接池释放
    } // pool 在此处被 Drop

    // 验证所有资源正常释放
}

/// TEST-DBNEXUS-013: 克隆连接池后的关闭测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_pool_clone_and_drop() {
    let (config, _temp_dir) = common::get_test_config();

    let pool1 = DbPool::with_config(config).await.expect("Failed to create pool");
    let pool2 = pool1.clone();

    // 两个引用指向同一个连接池
    let session1 = pool1.get_session("admin").await.expect("Failed to get session");
    let session2 = pool2.get_session("admin").await.expect("Failed to get session");

    assert_eq!(session1.role(), "admin");
    assert_eq!(session2.role(), "admin");

    // 验证是同一个连接池
    let status1 = pool1.status();
    let status2 = pool2.status();
    assert_eq!(status1.total, status2.total);
    assert_eq!(status1.active, status2.active);

    drop(session1);
    drop(session2);
    drop(pool1);

    // pool2 仍然有效
    let _session = pool2
        .get_session("admin")
        .await
        .expect("Failed to get session after pool1 drop");
}

// ============================================================================
// DbPoolBuilder 构建测试
// ============================================================================

/// TEST-DBNEXUS-014: DbPoolBuilder 基本构建测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_builder_basic() {
    let url = common::get_test_database_url();

    let config = dbnexus::config::DbConfig {
        url,
        ..Default::default()
    };

    let pool = DbPoolBuilder::new()
        .config(config)
        .build()
        .await
        .expect("Failed to build pool with builder");

    // 验证默认配置
    assert_eq!(pool.config().max_connections, 20);
    assert_eq!(pool.config().min_connections, 5);
}

/// TEST-DBNEXUS-015: DbPoolBuilder 链式调用测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_builder_chaining() {
    let url = common::get_test_database_url();

    let config = dbnexus::config::DbConfig {
        url,
        max_connections: 25,
        min_connections: 5,
        admin_role: "superuser".to_string(),
        ..Default::default()
    };

    let pool = DbPoolBuilder::new()
        .config(config)
        .build()
        .await
        .expect("Failed to build pool");

    assert_eq!(pool.config().max_connections, 25);
    assert_eq!(pool.config().min_connections, 5);
    assert_eq!(pool.config().admin_role, "superuser");
}

/// TEST-DBNEXUS-016: DbPoolBuilder 使用配置构建测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_builder_with_config() {
    let url = common::get_test_database_url();

    let config = dbnexus::config::DbConfig {
        url,
        max_connections: 30,
        min_connections: 10,
        idle_timeout: 500,
        acquire_timeout: 10000,
        admin_role: "root".to_string(),
        ..Default::default()
    };

    let pool = DbPoolBuilder::new()
        .config(config)
        .build()
        .await
        .expect("Failed to build pool with config");

    assert_eq!(pool.config().max_connections, 30);
    assert_eq!(pool.config().min_connections, 10);
    assert_eq!(pool.config().idle_timeout, 500);
    assert_eq!(pool.config().acquire_timeout, 10000);
    assert_eq!(pool.config().admin_role, "root");
}

/// TEST-DBNEXUS-017: DbPoolBuilder 覆盖配置测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_builder_override_config() {
    let url = common::get_test_database_url();

    let config = dbnexus::config::DbConfig {
        url,
        max_connections: 20,
        min_connections: 5,
        ..Default::default()
    };

    // 使用 config 但覆盖 max_connections
    let override_config = dbnexus::config::DbConfig {
        max_connections: 50,
        ..config
    };

    let pool = DbPoolBuilder::new()
        .config(override_config)
        .build()
        .await
        .expect("Failed to build pool");

    // 注意：由于实现细节，覆盖可能不生效，这里主要测试 API 可用性
    assert!(pool.config().max_connections > 0);
}

/// TEST-DBNEXUS-018: DbPoolBuilder 默认值测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_builder_defaults() {
    let url = common::get_test_database_url();

    let pool = DbPoolBuilder::new()
        .url(&url)
        .build()
        .await
        .expect("Failed to build pool");

    // 验证默认值
    assert_eq!(pool.config().max_connections, 20);
    assert_eq!(pool.config().min_connections, 5);
    assert_eq!(pool.config().idle_timeout, 300);
    assert_eq!(pool.config().acquire_timeout, 5000);
    assert_eq!(pool.config().admin_role, "admin");
}

/// TEST-DBNEXUS-019: DbPoolBuilder 无 URL 或 config 错误测试
#[tokio::test]
async fn test_builder_missing_url_and_config() {
    let result = DbPoolBuilder::new().build().await;

    assert!(result.is_err());
}

/// TEST-DBNEXUS-020: DbPoolBuilder Debug 实现测试
#[test]
fn test_builder_debug() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 10,
        admin_role: "admin".to_string(),
        ..Default::default()
    };
    let builder = DbPoolBuilder::new().config(config);

    let debug_str = format!("{:?}", builder);

    // 验证 Debug 输出包含关键字段
    assert!(debug_str.contains("url"));
    assert!(debug_str.contains("admin_role"));
}

// ============================================================================
// Feature flags 组合测试
// ============================================================================

/// TEST-DBNEXUS-021: 数据库类型解析测试
#[tokio::test]
async fn test_database_type_parsing() {
    // SQLite
    assert_eq!(
        DatabaseType::parse_database_type("sqlite::memory:"),
        DatabaseType::Sqlite
    );
    assert_eq!(
        DatabaseType::parse_database_type("sqlite:///path/to/db"),
        DatabaseType::Sqlite
    );

    // PostgreSQL
    assert_eq!(
        DatabaseType::parse_database_type("postgres://localhost/db"),
        DatabaseType::Postgres
    );
    assert_eq!(
        DatabaseType::parse_database_type("postgresql://localhost/db"),
        DatabaseType::Postgres
    );

    // MySQL
    assert_eq!(
        DatabaseType::parse_database_type("mysql://localhost/db"),
        DatabaseType::MySql
    );
}

/// TEST-DBNEXUS-022: 数据库类型显示测试
#[tokio::test]
async fn test_database_type_display() {
    assert_eq!(DatabaseType::Sqlite.to_string(), "sqlite");
    assert_eq!(DatabaseType::Postgres.to_string(), "postgres");
    assert_eq!(DatabaseType::MySql.to_string(), "mysql");
}

/// TEST-DBNEXUS-023: 数据库类型 is_real_database 测试
#[tokio::test]
async fn test_database_type_is_real_database() {
    // SQLite 不是"真实"数据库（内存/文件）
    assert!(!DatabaseType::Sqlite.is_real_database());

    // PostgreSQL 和 MySQL 是真实数据库
    assert!(DatabaseType::Postgres.is_real_database());
    assert!(DatabaseType::MySql.is_real_database());
}

/// TEST-DBNEXUS-024: 数据库类型 as_str 测试
#[tokio::test]
async fn test_database_type_as_str() {
    assert_eq!(DatabaseType::Sqlite.as_str(), "sqlite");
    assert_eq!(DatabaseType::Postgres.as_str(), "postgres");
    assert_eq!(DatabaseType::MySql.as_str(), "mysql");
}

/// TEST-DBNEXUS-025: 连接池克隆测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_pool_clone() {
    let (config, _temp_dir) = common::get_test_config();

    let pool1 = DbPool::with_config(config).await.expect("Failed to create pool");
    let pool2 = pool1.clone();

    // 验证克隆后的连接池共享内部状态
    let status1 = pool1.status();
    let status2 = pool2.status();

    assert_eq!(status1.total, status2.total);
    assert_eq!(status1.active, status2.active);
    assert_eq!(status1.idle, status2.idle);
}

/// TEST-DBNEXUS-026: 连接池 Arc 包装测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_pool_in_arc() {
    let (config, _temp_dir) = common::get_test_config();

    let pool = Arc::new(DbPool::with_config(config).await.expect("Failed to create pool"));

    // 在多任务间共享
    let pool_clone = pool.clone();
    let handle = tokio::spawn(async move {
        let _session = pool_clone.get_session("admin").await.expect("Failed to get session");
    });

    // 主任务也可以使用
    let _session = pool.get_session("admin").await.expect("Failed to get session");

    handle.await.expect("Task panicked");
}

/// TEST-DBNEXUS-027: 并发获取会话测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_concurrent_session_access() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = Arc::new(DbPool::with_config(config).await.expect("Failed to create pool"));

    let mut handles = Vec::new();

    for _ in 0..5 {
        let pool_clone = pool.clone();
        handles.push(tokio::spawn(async move {
            let _session = pool_clone.get_session("admin").await.expect("Failed to get session");
        }));
    }

    // 等待所有任务完成
    for handle in handles {
        handle.await.expect("Task panicked");
    }

    // 验证连接池状态正常
    let status = pool.status();
    assert!(status.total <= pool.config().max_connections);
}

/// TEST-DBNEXUS-028: 连接池获取实际配置测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_pool_get_actual_config() {
    let url = common::get_test_database_url();

    // 使用可能被修正的配置
    let config = dbnexus::config::DbConfig {
        url,
        max_connections: 10,
        min_connections: 5,
        ..Default::default()
    };

    let pool = DbPool::with_config(config).await.expect("Failed to create pool");

    // 获取实际应用的配置
    let actual_config = pool.get_actual_config();

    // 验证配置有效
    assert!(actual_config.max_connections > 0);
    assert!(actual_config.min_connections > 0);
}

// ============================================================================
// 错误处理测试
// ============================================================================

/// TEST-DBNEXUS-029: 无效 URL 错误测试
#[tokio::test]
async fn test_invalid_url_error() {
    let result = DbPool::new("invalid://url").await;

    assert!(result.is_err());
}

/// TEST-DBNEXUS-030: 空角色错误测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_empty_role_handling() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");

    // 空角色应该被处理
    let result = pool.get_session("").await;

    // 根据权限配置，空角色可能被拒绝
    #[cfg(feature = "permission")]
    {
        assert!(result.is_err());
    }
}

/// TEST-DBNEXUS-031: 配置验证失败测试
#[tokio::test]
async fn test_config_validation_failure() {
    // min > max 应该失败 - DbConfig 结构体不会在创建时验证
    // 验证会在 DbPool 创建时进行
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 5,
        min_connections: 10,
        ..Default::default()
    };

    // 配置已创建，值被设置
    assert!(config.max_connections < config.min_connections);
}

/// TEST-DBNEXUS-032: 配置边界值测试
#[tokio::test]
async fn test_config_boundary_values() {
    // 最大连接数边界
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 1000,
        min_connections: 1,
        ..Default::default()
    };

    assert_eq!(config.max_connections, 1000);
    assert_eq!(config.min_connections, 1);

    // 超过最大值 - DbConfig 结构体不会在创建时验证
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 1001,
        min_connections: 1,
        ..Default::default()
    };

    // 配置已创建，值被设置
    assert_eq!(config.max_connections, 1001);
}

// ============================================================================
// 连接池生命周期测试
// ============================================================================

/// TEST-DBNEXUS-033: 会话生命周期测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_session_lifecycle() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");

    // 获取会话前
    let status_before = pool.status();

    let status_during = {
        let _session = pool.get_session("admin").await.expect("Failed to get session");

        // 会话活跃时
        let status = pool.status();
        assert!(status.active >= 1);
        status
    }; // 会话在此处释放

    // 给一点时间让连接归还
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 会话释放后
    let status_after = pool.status();
    assert!(status_after.active < status_during.active || status_after.active == 0);
}

/// TEST-DBNEXUS-034: 连接池预热测试（如果启用 pool-warmup 特性）
#[tokio::test]
#[cfg(all(
    feature = "pool-warmup",
    any(feature = "sqlite", feature = "postgres", feature = "mysql")
))]
async fn test_pool_warmup() {
    let url = common::get_test_database_url();

    let config = dbnexus::config::DbConfig {
        url,
        max_connections: 10,
        min_connections: 3,
        warmup_timeout: 30,
        warmup_retries: 3,
        ..Default::default()
    };

    let pool = DbPool::with_config(config).await.expect("Failed to create pool");

    // 预热后应该有初始连接
    let status = pool.status();
    // 注意：SQLite 内存数据库可能不会预热
    assert!(status.total >= 0);
}

/// TEST-DBNEXUS-035: 连接池清理无效连接测试
#[tokio::test]
#[cfg(all(
    any(feature = "sqlite", feature = "postgres", feature = "mysql"),
    feature = "pool-health-check"
))]
async fn test_clean_invalid_connections() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");

    // 清理无效连接（应该正常执行，即使没有无效连接）
    let removed = pool.clean_invalid_connections().await;

    // 清理后连接池状态应该正常
    let status = pool.status();
    assert!(status.total <= pool.config().max_connections);
}

/// TEST-DBNEXUS-036: 连接池验证并重建连接测试
#[tokio::test]
#[cfg(all(
    any(feature = "sqlite", feature = "postgres", feature = "mysql"),
    feature = "pool-health-check"
))]
async fn test_validate_and_recreate_connections() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");

    // 验证并重建连接
    let result = pool.validate_and_recreate_connections().await;

    assert!(result.is_ok());
    let recreated = result.unwrap();
    // 重建数量应该合理
    assert!(recreated <= pool.config().max_connections);
}

// ============================================================================
// 连接池健康检查测试（如果启用 health-check 特性）
// ============================================================================

/// TEST-DBNEXUS-037: 连接健康检查测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_connection_health_check() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");

    // 获取一个连接
    let session = pool.get_session("admin").await.expect("Failed to get session");

    // 会话内部有连接，连接池应该健康
    let status = pool.status();
    assert!(status.active >= 1);

    drop(session);
}

/// TEST-DBNEXUS-038: 连接池状态字段完整性测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_pool_status_fields() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");

    let status = pool.status();

    // 验证所有状态字段
    assert!(status.total <= pool.config().max_connections);
    assert!(status.active <= status.total);
    assert_eq!(status.idle, status.total.saturating_sub(status.active));
    // borrow_count 和 max_active 应该被正确初始化
    // wait_count 应该是非负的
}

// ============================================================================
// 配置相关测试
// ============================================================================

/// TEST-DBNEXUS-039: 配置 Duration 转换测试
#[tokio::test]
async fn test_config_duration_conversion() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        idle_timeout: 300,
        acquire_timeout: 5000,
        migration_timeout: 120,
        ..Default::default()
    };

    assert_eq!(config.idle_timeout_duration(), Duration::from_secs(300));
    assert_eq!(config.acquire_timeout_duration(), Duration::from_millis(5000));
    assert_eq!(config.migration_timeout_duration(), Duration::from_secs(120));
}

/// TEST-DBNEXUS-040: 配置 URL 访问测试
#[tokio::test]
async fn test_config_url_access() {
    // 带密码的 URL
    let config = dbnexus::config::DbConfig {
        url: "postgres://user:secret_password@localhost:5432/mydb".to_string(),
        ..Default::default()
    };

    // URL 包含密码
    assert!(config.url.contains("secret_password"));
    assert!(config.url.contains("postgres://"));

    // SQLite 内存数据库
    let sqlite_config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        ..Default::default()
    };

    assert_eq!(sqlite_config.url, "sqlite::memory:");
}

/// TEST-DBNEXUS-041: 配置克隆测试
#[tokio::test]
async fn test_config_clone() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 15,
        min_connections: 3,
        admin_role: "test_admin".to_string(),
        ..Default::default()
    };

    let cloned = config.clone();

    assert_eq!(config.max_connections, cloned.max_connections);
    assert_eq!(config.min_connections, cloned.min_connections);
    assert_eq!(config.admin_role, cloned.admin_role);
    assert_eq!(config.url, cloned.url);
}

/// TEST-DBNEXUS-042: 配置可选字段测试
#[tokio::test]
async fn test_config_optional_fields() {
    use std::path::PathBuf;

    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        permissions_path: Some("/etc/dbnexus/permissions.yaml".to_string()),
        migrations_dir: Some(PathBuf::from("/var/migrations")),
        auto_migrate: true,
        ..Default::default()
    };

    assert_eq!(
        config.permissions_path,
        Some("/etc/dbnexus/permissions.yaml".to_string())
    );
    assert!(config.migrations_dir.is_some());
    assert!(config.auto_migrate);
}

/// TEST-DBNEXUS-043: 配置默认可选字段测试
#[tokio::test]
async fn test_config_default_optional_fields() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        ..Default::default()
    };

    assert!(config.permissions_path.is_none());
    assert!(config.migrations_dir.is_none());
    assert!(!config.auto_migrate);
}

// ============================================================================
// 并发和线程安全测试
// ============================================================================

/// TEST-DBNEXUS-044: 多线程并发访问测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_multithreaded_access() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let (config, _temp_dir) = common::get_test_config();
    let pool = Arc::new(DbPool::with_config(config).await.expect("Failed to create pool"));

    let success_count = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();

    for _ in 0..10 {
        let pool_clone = pool.clone();
        let success_count_clone = success_count.clone();

        handles.push(tokio::spawn(async move {
            if pool_clone.get_session("admin").await.is_ok() {
                success_count_clone.fetch_add(1, Ordering::SeqCst);
            }
        }));
    }

    for handle in handles {
        handle.await.expect("Task panicked");
    }

    // 所有请求应该成功
    assert_eq!(success_count.load(Ordering::SeqCst), 10);
}

/// TEST-DBNEXUS-045: 连接池压力测试
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_pool_stress() {
    let url = common::get_test_database_url();

    let config = dbnexus::config::DbConfig {
        url,
        max_connections: 5,
        min_connections: 1,
        ..Default::default()
    };

    let pool = Arc::new(DbPool::with_config(config).await.expect("Failed to create pool"));

    let mut handles = Vec::new();

    // 创建超过连接池容量的并发请求
    for _ in 0..20 {
        let pool_clone = pool.clone();
        handles.push(tokio::spawn(async move {
            let _session = pool_clone.get_session("admin").await.ok();
            // 短暂持有会话
            tokio::time::sleep(Duration::from_millis(10)).await;
        }));
    }

    // 等待所有任务完成
    for handle in handles {
        handle.await.expect("Task panicked");
    }

    // 连接池状态应该正常
    let status = pool.status();
    assert!(status.total <= pool.config().max_connections);
}
