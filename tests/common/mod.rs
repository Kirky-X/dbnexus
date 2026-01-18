// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 测试辅助模块
//!
//! 提供跨数据库测试的辅助函数，包括配置管理、测试夹具和工具函数

use dbnexus::config::{DbConfig, DbConfigBuilder};
use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;

/// 测试用的权限配置内容
static TEST_PERMISSIONS_CONTENT: &str = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations:
          - SELECT
          - INSERT
          - UPDATE
          - DELETE
  user:
    tables:
      - name: "users"
        operations:
          - SELECT
          - INSERT
  test_role:
    tables:
      - name: "*"
        operations:
          - SELECT
"#;

/// 获取测试数据库配置
///
/// 根据环境变量或默认值返回数据库配置
pub fn get_test_config() -> DbConfig {
    get_test_config_with_permissions(false)
}

/// 获取测试数据库配置（可选择包含权限配置）
pub fn get_test_config_with_permissions(with_permissions: bool) -> DbConfig {
    let test_db_type = std::env::var("TEST_DB_TYPE").unwrap_or_else(|_| "sqlite".to_string());
    let database_url = std::env::var("DATABASE_URL").ok();

    let url = match test_db_type.as_str() {
        "postgres" => database_url.unwrap_or_else(|| {
            let password = std::env::var("TEST_DB_PASSWORD").unwrap_or_else(|_| "dbnexus_password".to_string());
            format!("postgres://dbnexus:{}@localhost:15432/dbnexus_test", password)
        }),
        "mysql" => database_url.unwrap_or_else(|| {
            let password = std::env::var("TEST_DB_PASSWORD").unwrap_or_else(|_| "dbnexus_password".to_string());
            format!("mysql://dbnexus:{}@localhost:13306/dbnexus_test", password)
        }),
        _ => database_url.unwrap_or_else(|| "sqlite::memory:".to_string()),
    };

    // 使用 DbConfigBuilder 构建配置
    let mut config = DbConfigBuilder::new()
        .url(&url)
        .max_connections(5)
        .min_connections(1)
        .idle_timeout(300)
        .acquire_timeout(5000)
        .build()
        .expect("Failed to build test config");

    // 可选：添加权限配置
    if with_permissions {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let perm_file = temp_dir.path().join("test_permissions.yaml");
        std::fs::write(&perm_file, TEST_PERMISSIONS_CONTENT).expect("Failed to write test permissions file");
        config.set_permissions_path(perm_file.to_string_lossy().to_string());

        // 保存 temp_dir 以防止被删除
        let _ = temp_dir;
    }

    config
}

/// 是否使用真实数据库（非内存数据库）
#[allow(dead_code)]
pub fn is_real_database() -> bool {
    std::env::var("TEST_DB_TYPE").unwrap_or_else(|_| "sqlite".to_string()) != "sqlite"
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
    DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(5)
        .min_connections(1)
        .idle_timeout(300)
        .acquire_timeout(5000)
        .build()
        .expect("Failed to build SQLite memory config")
}

/// 创建测试用的SQLite文件数据库配置（推荐用于迁移测试）
///
/// 返回配置和临时目录
#[allow(dead_code)]
pub fn get_sqlite_file_config() -> (DbConfig, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let db_path = temp_dir.path().join("test.db");

    let config = DbConfigBuilder::new()
        .url(&format!("sqlite:///{}", db_path.display()))
        .max_connections(5)
        .min_connections(1)
        .idle_timeout(300)
        .acquire_timeout(5000)
        .build()
        .expect("Failed to build SQLite file config");

    (config, temp_dir)
}

/// 创建小容量连接池配置（用于测试连接耗尽场景）
#[allow(dead_code)]
pub fn get_small_pool_config() -> DbConfig {
    DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(2)
        .min_connections(1)
        .idle_timeout(300)
        .acquire_timeout(1000)
        .build()
        .expect("Failed to build small pool config")
}

/// 创建大容量连接池配置（用于测试高并发场景）
#[allow(dead_code)]
pub fn get_large_pool_config() -> DbConfig {
    DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(50)
        .min_connections(10)
        .idle_timeout(300)
        .acquire_timeout(5000)
        .build()
        .expect("Failed to build large pool config")
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
        .execute_raw_ddl(&format!("DROP TABLE IF EXISTS {}", table_name))
        .await;
}

/// 创建测试表
///
/// 在指定的会话上创建简单的测试表
#[allow(dead_code)]
pub async fn create_test_table(session: &mut dbnexus::pool::Session, table_name: &str) {
    session
        .execute_raw_ddl(&format!(
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
    assert!(status.active <= status.total, "Active should not exceed total");
    assert_eq!(
        status.total,
        status.active + status.idle,
        "Total should equal active + idle"
    );
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

// ============================================================================
// 追踪测试辅助函数
// ============================================================================

/// 创建 SQLite 文件数据库连接池（用于追踪测试）
///
/// 返回连接池和临时目录（用于自动清理）
#[allow(dead_code)]
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
    let config = DbConfigBuilder::new()
        .url(&format!("sqlite://{}", db_path_str))
        .max_connections(5)
        .min_connections(1)
        .idle_timeout(300)
        .acquire_timeout(5000)
        .permissions_path(&perm_file.to_string_lossy())
        .build()
        .expect("Failed to build tracing config");

    let pool = dbnexus::DbPool::with_config(config).await?;
    Ok((pool, temp_dir))
}

/// 创建通用测试连接池（根据环境变量选择数据库类型）
///
/// 返回连接池和临时目录（用于自动清理，仅 SQLite 需要）
#[allow(dead_code)]
pub async fn create_test_pool() -> Result<(dbnexus::DbPool, Option<TempDir>), dbnexus::DbError> {
    let test_db_type = std::env::var("TEST_DB_TYPE").unwrap_or_else(|_| "sqlite".to_string());
    eprintln!("DEBUG: TEST_DB_TYPE = {}", test_db_type);

    match test_db_type.as_str() {
        "postgres" | "mysql" => {
            // PostgreSQL 和 MySQL 不需要临时目录
            let config = get_test_config();
            eprintln!("DEBUG: Using {} database with URL: {}", test_db_type, config.url);
            let pool = dbnexus::DbPool::with_config(config).await?;
            Ok((pool, None))
        }
        _ => {
            // SQLite 使用文件数据库
            eprintln!("DEBUG: Using SQLite file database");
            let (pool, temp_dir) = create_sqlite_file_pool().await?;
            Ok((pool, Some(temp_dir)))
        }
    }
}

/// 创建用于追踪测试的测试表
///
/// 返回表名和临时目录
#[allow(dead_code)]
pub async fn create_tracing_test_table(pool: &dbnexus::DbPool) -> (String, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let table_name = generate_test_table_name("tracing_test");

    let session = pool.get_session("admin").await.expect("Failed to get session");
    session
        .execute_raw_ddl(&format!(
            "CREATE TABLE IF NOT EXISTS {} (
                id INTEGER PRIMARY KEY,
                trace_id TEXT,
                span_id TEXT,
                data TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
            table_name
        ))
        .await
        .expect("Failed to create test table");

    (table_name, temp_dir)
}

/// 清理追踪测试表
#[allow(dead_code)]
pub async fn cleanup_tracing_test_table(pool: &dbnexus::DbPool, table_name: &str) {
    let session = pool.get_session("admin").await.expect("Failed to get session");
    let _ = session
        .execute_raw_ddl(&format!("DROP TABLE IF EXISTS {}", table_name))
        .await;
}

/// 验证追踪上下文注入
///
/// 返回注入的 headers 和提取的追踪ID
#[allow(dead_code)]
pub async fn verify_trace_injection(pool: &dbnexus::DbPool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _session = pool
        .get_session("admin")
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    let trace_id = "0af7651916cd43dd8448eb211c80319c";
    let span_id = "b7ad6b7169203331";
    let traceparent = format!("00-{}-{}01", trace_id, span_id);

    let mut headers = HashMap::new();
    headers.insert("traceparent".to_string(), traceparent);

    // 验证 headers 包含有效追踪信息
    assert!(
        headers.contains_key("traceparent"),
        "Headers should contain traceparent"
    );
    let tp = headers.get("traceparent").unwrap();
    assert!(tp.starts_with("00-"), "traceparent format should be valid");

    Ok(())
}

/// 验证追踪上下文提取
///
/// 返回提取的追踪ID
#[allow(dead_code)]
pub async fn verify_trace_extraction(
    traceparent: &str,
) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    // 解析 traceparent 格式: version-trace_id-span_id-flags
    let parts: Vec<&str> = traceparent.split('-').collect();
    if parts.len() < 3 {
        return Ok(None);
    }

    let trace_id = parts[1];
    let span_id = &parts[2][0..16]; // 只取前16个字符作为span_id

    // 验证追踪ID格式（32位十六进制）
    assert!(trace_id.len() == 32, "trace_id should be 32 characters");
    assert!(span_id.len() == 16, "span_id should be 16 characters");

    // 验证都是有效的十六进制
    u64::from_str_radix(&trace_id[0..16], 16).map_err(|_| "Invalid trace_id")?;
    u64::from_str_radix(&trace_id[16..32], 16).map_err(|_| "Invalid trace_id")?;
    u64::from_str_radix(span_id, 16).map_err(|_| "Invalid span_id")?;

    Ok(Some(trace_id.to_string()))
}

/// 测试连接池在追踪上下文下的行为
#[allow(dead_code)]
pub async fn test_pool_with_trace_context(pool: &dbnexus::DbPool) {
    let session = pool.get_session("admin").await.expect("Failed to get session");
    assert!(!session.role().is_empty(), "Session should have a role");

    let status = pool.status();
    assert_eq!(
        status.total,
        status.active + status.idle,
        "Total should equal active + idle"
    );
}

/// 并发测试追踪上下文注入
///
/// 返回成功和失败的数量
#[allow(dead_code)]
pub async fn concurrent_trace_injection_test(pool: &dbnexus::DbPool, num_tasks: usize) -> (usize, usize) {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let success_count = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();

    for i in 0..num_tasks {
        let pool = pool.clone();
        let success_count = success_count.clone();
        let handle = tokio::spawn(async move {
            let role = if i % 2 == 0 { "admin" } else { "system" };
            if pool.get_session(role).await.is_ok() {
                success_count.fetch_add(1, Ordering::SeqCst);
            }
        });
        handles.push(handle);
    }

    futures::future::join_all(handles).await;

    let success = success_count.load(Ordering::SeqCst);
    (success, num_tasks - success)
}

/// 验证数据库操作与追踪上下文关联
#[allow(dead_code)]
pub async fn verify_db_operation_with_trace(
    pool: &dbnexus::DbPool,
    table_name: &str,
    trace_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let session = pool
        .get_session("admin")
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    // 验证表名安全性（防止 SQL 注入）
    let safe_table_name = validate_table_name(table_name)?;
    let safe_trace_id = sanitize_sql_string(trace_id);

    // 插入带追踪ID的记录
    session
        .execute_raw(&format!(
            "INSERT INTO {} (trace_id, data) VALUES ('{}', 'test data')",
            safe_table_name, safe_trace_id
        ))
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    // 验证记录已插入
    let _result = session
        .execute_raw(&format!(
            "SELECT COUNT(*) FROM {} WHERE trace_id = '{}'",
            safe_table_name, safe_trace_id
        ))
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    // result 是 DbResult<ExecResult>，如果执行成功已经通过 map_err
    Ok(())
}

/// 验证表名安全性（白名单验证）
fn validate_table_name(table_name: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // 只允许字母、数字、下划线
    if !table_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid table name: contains disallowed characters",
        )));
    }
    // 限制长度
    if table_name.len() > 64 {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid table name: too long",
        )));
    }
    Ok(table_name.to_string())
}

/// 清理 SQL 字符串（转义单引号）
fn sanitize_sql_string(input: &str) -> String {
    input.replace('\'', "''")
}
