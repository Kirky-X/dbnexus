// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 测试辅助模块
//!
//! 提供跨数据库测试的辅助函数，包括配置管理、测试夹具和工具函数

use dbnexus::config::{DbConfig, PoolConfig};
use std::path::PathBuf;
use tempfile::TempDir;

/// 获取测试数据库配置
///
/// 根据环境变量或默认值返回数据库配置
pub fn get_test_config() -> DbConfig {
    // 使用统一的配置管理
    let pool_config = PoolConfig {
        max_connections: 5,
        min_connections: 1,
        idle_timeout: 300,
        acquire_timeout: 5000,
    };

    // 直接从环境变量创建配置
    let mut config = DbConfig::from_env().unwrap_or_else(|_| DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: pool_config.max_connections,
        min_connections: pool_config.min_connections,
        idle_timeout: pool_config.idle_timeout,
        acquire_timeout: pool_config.acquire_timeout,
        permissions_path: None,
        migrations_dir: None,
        auto_migrate: false,
        migration_timeout: 60,
    });

    // 应用池配置
    config.max_connections = pool_config.max_connections;
    config.min_connections = pool_config.min_connections;
    config.idle_timeout = pool_config.idle_timeout;
    config.acquire_timeout = pool_config.acquire_timeout;

    config
}

/// 获取当前测试的数据库类型
#[allow(dead_code)]
pub fn get_current_db_type() -> String {
    std::env::var("TEST_DB_TYPE").unwrap_or_else(|_| "sqlite".to_string())
}

/// 是否使用真实数据库（非内存数据库）
#[allow(dead_code)]
pub fn is_real_database() -> bool {
    get_current_db_type() != "sqlite"
}

/// 创建测试用的临时迁移目录
///
/// 返回临时目录路径和清理句柄
#[allow(dead_code)]
pub fn create_temp_migrations_dir() -> (PathBuf, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp migrations directory");
    let path = temp_dir.path().to_path_buf();
    (path, temp_dir)
}

/// 创建测试用的SQLite内存数据库配置（注意：每个连接是独立的数据库）
/// 对于需要共享状态的测试，请使用 get_sqlite_file_config()
#[allow(dead_code)]
pub fn get_sqlite_memory_config() -> DbConfig {
    DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 5,
        min_connections: 1,
        idle_timeout: 300,
        acquire_timeout: 5000,
        permissions_path: None,
        migrations_dir: None,
        auto_migrate: false,
        migration_timeout: 60,
    }
}

/// 创建测试用的SQLite文件数据库配置（推荐用于迁移测试）
///
/// 返回配置和临时目录
#[allow(dead_code)]
pub fn get_sqlite_file_config() -> (DbConfig, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let db_path = temp_dir.path().join("test.db");

    let config = DbConfig {
        url: format!("sqlite:///{}", db_path.display()),
        max_connections: 5,
        min_connections: 1,
        idle_timeout: 300,
        acquire_timeout: 5000,
        permissions_path: None,
        migrations_dir: None,
        auto_migrate: false,
        migration_timeout: 60,
    };

    (config, temp_dir)
}

/// 创建小容量连接池配置（用于测试连接耗尽场景）
#[allow(dead_code)]
pub fn get_small_pool_config() -> DbConfig {
    DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 2,
        min_connections: 1,
        idle_timeout: 300,
        acquire_timeout: 1000,
        permissions_path: None,
        migrations_dir: None,
        auto_migrate: false,
        migration_timeout: 60,
    }
}

/// 创建大容量连接池配置（用于测试高并发场景）
#[allow(dead_code)]
pub fn get_large_pool_config() -> DbConfig {
    DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 50,
        min_connections: 10,
        idle_timeout: 300,
        acquire_timeout: 5000,
        permissions_path: None,
        migrations_dir: None,
        auto_migrate: false,
        migration_timeout: 60,
    }
}

/// 获取测试超时时间（毫秒）
#[allow(dead_code)]
pub fn get_test_timeout_ms() -> u64 {
    std::env::var("TEST_TIMEOUT_MS")
        .unwrap_or_else(|_| "30000".to_string())
        .parse()
        .unwrap_or(30000)
}

/// 等待指定时间（用于测试中的同步）
#[allow(dead_code)]
pub async fn wait_for_ms(ms: u64) {
    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
}

/// 生成测试用的表名（避免测试间的冲突）
#[allow(dead_code)]
pub fn generate_test_table_name(prefix: &str) -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("{}_test_{}", prefix, timestamp)
}

/// 创建测试夹具 - 包含连接池和临时迁移目录
///
/// 返回池、迁移目录路径和临时目录清理句柄
#[allow(dead_code)]
pub async fn create_test_fixture() -> (dbnexus::DbPool, PathBuf, TempDir) {
    use dbnexus::DbPool;

    let config = get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let (migrations_dir, temp_dir) = create_temp_migrations_dir();

    (pool, migrations_dir, temp_dir)
}

/// 清理测试表
///
/// 在指定的会话上删除测试表
#[allow(dead_code)]
pub async fn cleanup_test_table(session: &mut dbnexus::pool::Session, table_name: &str) {
    let _ = session
        .execute_raw(&format!("DROP TABLE IF EXISTS {}", table_name))
        .await;
}

/// 创建测试表
///
/// 在指定的会话上创建简单的测试表
#[allow(dead_code)]
pub async fn create_test_table(session: &mut dbnexus::pool::Session, table_name: &str) {
    session
        .execute_raw(&format!(
            "CREATE TABLE IF NOT EXISTS {} (id INTEGER PRIMARY KEY, data TEXT)",
            table_name
        ))
        .await
        .expect("Failed to create test table");
}

/// 测试断言帮助 - 验证连接池状态
#[allow(dead_code)]
pub fn assert_pool_healthy(pool: &dbnexus::DbPool) {
    let status = pool.status();
    assert!(status.total >= 1, "Pool should have at least 1 connection");
    assert!(status.active <= status.total, "Active should not exceed total");
    assert_eq!(status.total, status.active + status.idle);
}

/// 测试断言帮助 - 验证会话有效
#[allow(dead_code)]
pub fn assert_session_valid(session: &mut dbnexus::pool::Session) {
    assert!(!session.role().is_empty(), "Session should have a role");
    assert!(session.connection().is_ok(), "Session should have a valid connection");
}

/// 并行运行测试任务
///
/// 辅助函数，用于在测试中并行运行多个异步任务
#[allow(dead_code)]
pub async fn run_parallel_tasks<F, T>(tasks: Vec<F>) -> Vec<T>
where
    F: std::future::Future<Output = T> + Send,
    T: Send,
{
    futures::future::join_all(tasks).await
}
