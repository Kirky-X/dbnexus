// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 测试辅助模块
//!
//! 提供跨数据库测试的辅助函数，包括配置管理、测试夹具和工具函数
//!
//! 各测试目标经 `#[path = "..."] mod common;` 各自纳入本模块编译，单个
//! 目标只使用助手的一个子集，未被该目标引用的助手会触发 dead_code 误报；
//! 模块级伞形豁免避免逐项标注在数十个测试目标间重复，新增助手无需各自标注。

#![allow(dead_code)]

use dbnexus::DbConfig;
use dbnexus::foundation::PoolConfig;
use tempfile::TempDir;

/// 创建测试连接池的 helper
///
/// 优先使用 `DATABASE_URL` 环境变量，默认回退到 `sqlite::memory:`。
/// 这使得同一测试在 sqlite/mysql/postgres CI 矩阵下均可运行。
pub async fn make_sqlite_memory_pool() -> dbnexus::DbPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());
    dbnexus::DbPool::new(&url)
        .await
        .expect("Failed to create test pool")
}

/// 脱敏 URL，隐藏密码
fn sanitize_url(url: &str) -> String {
    // 简单的 URL 脱敏：替换密码部分为 ****
    if let Some(at_pos) = url.find('@')
        && let Some(proto_end) = url.find("://")
    {
        let proto = &url[..proto_end + 3];
        let rest = &url[at_pos..];
        if let Some(colon_pos) = url[proto_end + 3..at_pos].find(':') {
            let user = &url[proto_end + 3..proto_end + 3 + colon_pos];
            return format!("{}{}:****{}", proto, user, rest);
        }
    }
    url.to_string()
}

/// 测试用的权限配置内容
static TEST_PERMISSIONS_CONTENT: &str = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations:
          - select
          - insert
          - update
          - delete
  user:
    tables:
      - name: "users"
        operations:
          - select
          - insert
  test_role:
    tables:
      - name: "*"
        operations:
          - select
"#;

pub fn get_test_database_url() -> String {
    let test_db_type = std::env::var("TEST_DB_TYPE").unwrap_or_else(|_| "sqlite".to_string());
    let database_url = std::env::var("DATABASE_URL").ok();

    match test_db_type.as_str() {
        "postgres" => database_url.unwrap_or_else(|| {
            let password = std::env::var("TEST_DB_PASSWORD")
                .unwrap_or_else(|_| "dbnexus_password".to_string());
            format!(
                "postgres://dbnexus:{}@localhost:15433/dbnexus_test",
                password
            )
        }),
        "mysql" => database_url.unwrap_or_else(|| {
            let password = std::env::var("TEST_DB_PASSWORD")
                .unwrap_or_else(|_| "dbnexus_password".to_string());
            format!("mysql://dbnexus:{}@localhost:13308/dbnexus_test", password)
        }),
        _ => database_url.unwrap_or_else(|| "sqlite::memory:".to_string()),
    }
}

/// 获取测试数据库配置（无权限配置）
pub fn get_test_config() -> (DbConfig, Option<TempDir>) {
    get_test_config_with_permissions(false)
}

/// 获取测试数据库配置（可选择包含权限配置）
///
/// 返回配置和可选的临时目录（用于保持权限配置文件的生命周期）
pub fn get_test_config_with_permissions(with_permissions: bool) -> (DbConfig, Option<TempDir>) {
    let url = get_test_database_url();

    // 可选：添加权限配置
    if with_permissions {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let perm_file = temp_dir.path().join("test_permissions.yaml");
        std::fs::write(&perm_file, TEST_PERMISSIONS_CONTENT)
            .expect("Failed to write test permissions file");
        let perm_path = perm_file.to_string_lossy().to_string();

        // 使用结构体字面量构建配置
        let config = dbnexus::DbConfig {
            url,
            pool_config: PoolConfig {
                max_connections: 5,
                min_connections: 1,
                idle_timeout: 300,
                acquire_timeout: 5000,
            },
            admin_role: "admin".to_string(),
            permissions_path: Some(perm_path),
            ..Default::default()
        };

        // 返回 config 和 temp_dir，temp_dir 会保持配置文件存活
        return (config, Some(temp_dir));
    }

    // 无权限配置
    let config = dbnexus::DbConfig {
        url,
        pool_config: PoolConfig {
            max_connections: 5,
            min_connections: 1,
            idle_timeout: 300,
            acquire_timeout: 5000,
        },
        admin_role: "admin".to_string(),
        ..Default::default()
    };

    (config, None)
}

/// 生成测试用的表名（避免测试间的冲突）
///
/// 使用进程内单调递增的原子计数器，保证表名唯一且可预测。
pub fn generate_test_table_name(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("{}_test_{}", prefix, n)
}

/// 生成测试用的迁移版本号基址（避免测试间的冲突）
///
/// 使用进程内单调递增的原子计数器，保证版本号唯一。
/// 每次调用返回一个基址，测试可用 base, base+1, base+2... 作为版本号。
/// 起始值为 10000，每次递增 100，为单个测试留出足够空间。
pub fn generate_test_migration_base_version() -> u32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(10000);
    COUNTER.fetch_add(100, Ordering::SeqCst)
}

/// 清理迁移跟踪表中指定的版本记录（持久化测试数据库隔离）
///
/// `dbnexus_migrations` 跟踪表在共享的 postgres/mysql 测试库中跨运行持久存在，
/// 而测试迁移版本号在每个测试进程内从 10000 重新计数（见
/// `generate_test_migration_base_version`）。重跑同一测试时，上一轮写入的
/// 版本记录会让 `run_migrations` 误判为已应用（返回 0），导致
/// `applied == N` 断言失败。运行迁移前删除本测试将要使用的版本记录，
/// 保证每次运行幂等。
///
/// cfg 门控与 lib.rs 的 `pub use sea_orm` 一致（任一 db 驱动 feature）：
/// 函数体经 `dbnexus::sea_orm::ConnectionTrait` 操作真实数据库，
/// 无 db 驱动的测试目标（如 permission_engine_integration）下整段剔除，
/// 保证 `--features "permission-engine,test-utils"` 也能编译。
/// 调用方（migration_integration / migration_auto_migrate）的
/// required-features 均含 sqlite，不受影响。
#[cfg(any(
    feature = "sqlite",
    feature = "postgres",
    feature = "mysql",
    feature = "duckdb"
))]
pub async fn cleanup_migration_versions(pool: &dbnexus::DbPool, versions: &[u32]) {
    if versions.is_empty() {
        return;
    }
    use dbnexus::sea_orm::ConnectionTrait;
    let session = pool
        .get_session("admin")
        .await
        .expect("Failed to get session");
    let conn = session
        .connection()
        .expect("Connection should be available");
    let placeholders = versions
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let _ = conn
        .execute_unprepared(&format!(
            "DELETE FROM dbnexus_migrations WHERE version IN ({})",
            placeholders
        ))
        .await;
}

/// 创建 SQLite 文件数据库连接池（用于追踪测试）
///
/// 返回连接池和临时目录（用于自动清理）
pub async fn create_sqlite_file_pool() -> Result<(dbnexus::DbPool, TempDir), dbnexus::DbError> {
    // 使用 tempfile 创建临时目录
    let temp_dir = tempfile::Builder::new()
        .prefix("dbnexus_tracing_test_")
        .tempdir()
        .expect("Failed to create temp directory");

    // 获取数据库文件路径
    let db_path = temp_dir.path().join("test.db");
    let db_path_str = db_path.to_string_lossy();

    // 预先创建数据库文件（解决权限问题）
    std::fs::File::create(&db_path).expect("Failed to create database file");

    let perm_content = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations:
          - select
          - insert
          - update
          - delete
  system:
    tables:
      - name: "*"
        operations:
          - select
          - insert
          - update
          - delete
"#;
    let perm_file = temp_dir.path().join("permissions.yaml");
    std::fs::write(&perm_file, perm_content).expect("Failed to write permissions file");

    // 使用 sqlx 标准的 SQLite URL 格式
    let config = dbnexus::DbConfig {
        url: format!("sqlite://{}", db_path_str),
        pool_config: PoolConfig {
            max_connections: 5,
            min_connections: 1,
            idle_timeout: 300,
            acquire_timeout: 5000,
        },
        permissions_path: Some(perm_file.to_string_lossy().to_string()),
        ..Default::default()
    };

    let pool = dbnexus::DbPool::with_config(config).await?;
    Ok((pool, temp_dir))
}

/// 创建通用测试连接池（根据环境变量选择数据库类型）
///
/// 返回连接池和临时目录（用于自动清理，仅 SQLite 需要）
pub async fn create_test_pool() -> Result<(dbnexus::DbPool, Option<TempDir>), dbnexus::DbError> {
    let test_db_type = std::env::var("TEST_DB_TYPE").unwrap_or_else(|_| "sqlite".to_string());
    eprintln!("DEBUG: TEST_DB_TYPE = {}", test_db_type);

    match test_db_type.as_str() {
        "postgres" | "mysql" => {
            // PostgreSQL 和 MySQL: 启用权限配置
            let (config, temp_dir) = get_test_config_with_permissions(true);
            eprintln!(
                "DEBUG: Using {} database with URL: {}",
                test_db_type,
                sanitize_url(&config.url)
            );
            let pool = dbnexus::DbPool::with_config(config).await?;
            Ok((pool, temp_dir))
        }
        _ => {
            // SQLite 使用文件数据库
            eprintln!("DEBUG: Using SQLite file database");
            let (pool, temp_dir) = create_sqlite_file_pool().await?;
            Ok((pool, Some(temp_dir)))
        }
    }
}

/// 创建临时目录（用于测试）
///
/// 此函数需要 `test-utils` feature
#[cfg(feature = "test-utils")]
pub fn create_temp_dir() -> TempDir {
    tempfile::Builder::new()
        .prefix("dbnexus_test_")
        .tempdir()
        .expect("Failed to create temp directory")
}
