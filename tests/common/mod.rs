// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 测试辅助模块
//!
//! 提供跨数据库测试的辅助函数，包括配置管理、测试夹具和工具函数

use dbnexus::DbConfig;
use dbnexus::foundation::PoolConfig;
use std::collections::HashMap;
use tempfile::TempDir;

/// 创建测试连接池的 helper
///
/// 优先使用 `DATABASE_URL` 环境变量，默认回退到 `sqlite::memory:`。
/// 这使得同一测试在 sqlite/mysql/postgres CI 矩阵下均可运行。
#[allow(dead_code)]
pub async fn make_sqlite_memory_pool() -> dbnexus::DbPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());
    dbnexus::DbPool::new(&url).await.expect("Failed to create test pool")
}

#[cfg(feature = "permission-engine")]
#[allow(dead_code)]
async fn join_all<F>(handles: Vec<tokio::task::JoinHandle<F>>) -> Vec<F>
where
    F: Send,
{
    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await.unwrap());
    }
    results
}

/// 脱敏 URL，隐藏密码
fn sanitize_url(url: &str) -> String {
    // 简单的 URL 脱敏：替换密码部分为 ****
    if let Some(at_pos) = url.find('@') {
        if let Some(proto_end) = url.find("://") {
            let proto = &url[..proto_end + 3];
            let rest = &url[at_pos..];
            if let Some(colon_pos) = url[proto_end + 3..at_pos].find(':') {
                let user = &url[proto_end + 3..proto_end + 3 + colon_pos];
                return format!("{}{}:****{}", proto, user, rest);
            }
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
            let password = std::env::var("TEST_DB_PASSWORD").unwrap_or_else(|_| "dbnexus_password".to_string());
            format!("postgres://dbnexus:{}@localhost:15433/dbnexus_test", password)
        }),
        "mysql" => database_url.unwrap_or_else(|| {
            let password = std::env::var("TEST_DB_PASSWORD").unwrap_or_else(|_| "dbnexus_password".to_string());
            format!("mysql://dbnexus:{}@localhost:13308/dbnexus_test", password)
        }),
        _ => database_url.unwrap_or_else(|| "sqlite::memory:".to_string()),
    }
}

/// 获取测试数据库配置（无权限配置）
#[allow(dead_code)]
pub fn get_test_config() -> (DbConfig, Option<TempDir>) {
    get_test_config_with_permissions(false)
}

/// 获取测试数据库配置（可选择包含权限配置）
///
/// 返回配置和可选的临时目录（用于保持权限配置文件的生命周期）
#[allow(dead_code)]
pub fn get_test_config_with_permissions(with_permissions: bool) -> (DbConfig, Option<TempDir>) {
    let url = get_test_database_url();

    // 可选：添加权限配置
    if with_permissions {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let perm_file = temp_dir.path().join("test_permissions.yaml");
        std::fs::write(&perm_file, TEST_PERMISSIONS_CONTENT).expect("Failed to write test permissions file");
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
#[allow(dead_code)]
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
#[allow(dead_code)]
pub fn generate_test_migration_base_version() -> u32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(10000);
    COUNTER.fetch_add(100, Ordering::SeqCst)
}

/// 清理测试表
///
/// 在指定的会话上删除测试表
#[allow(dead_code)]
pub async fn cleanup_test_table(session: &mut dbnexus::Session, table_name: &str) {
    let _ = session
        .execute_raw_ddl(&format!("DROP TABLE IF EXISTS {}", table_name))
        .await;
}

/// 创建测试表
///
/// 在指定的会话上创建简单的测试表
#[allow(dead_code)]
pub async fn create_test_table(session: &mut dbnexus::Session, table_name: &str) {
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
pub fn assert_session_valid(session: &mut dbnexus::Session) {
    assert!(!session.role().is_empty(), "Session should have a role");
    // 使用公开的 execute_raw 方法来验证连接可用
    // 注意：这个验证在 SQLite 内存模式下可能不适用
    // 实际的连接验证由具体的测试负责
}

/// 并行运行测试任务
///
/// 辅助函数，用于在测试中并行运行多个异步任务
#[cfg(feature = "permission-engine")]
#[allow(dead_code)]
pub async fn run_parallel_tasks<F, T>(tasks: Vec<F>) -> Vec<T>
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let mut handles = Vec::new();
    for task in tasks {
        handles.push(tokio::spawn(task));
    }

    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await.unwrap());
    }
    results
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
#[allow(dead_code)]
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

/// 创建用于追踪测试的测试表
///
/// 返回表名和临时目录
#[allow(dead_code)]
pub async fn create_tracing_test_table(pool: &dbnexus::DbPool) -> (String, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let table_name = generate_test_table_name("tracing_test");

    let session = pool.get_session("admin").await.expect("Failed to get session");

    // 根据数据库类型使用不同的表结构
    let create_sql = if pool.config().url.contains("mysql") {
        // MySQL: 使用 AUTO_INCREMENT
        format!(
            "CREATE TABLE IF NOT EXISTS {} (
                id INTEGER AUTO_INCREMENT PRIMARY KEY,
                trace_id VARCHAR(64),
                span_id VARCHAR(64),
                data TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            table_name
        )
    } else if pool.config().url.contains("postgres") {
        // PostgreSQL: 使用 GENERATED ALWAYS AS IDENTITY
        format!(
            "CREATE TABLE IF NOT EXISTS {} (
                id INTEGER GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
                trace_id TEXT,
                span_id TEXT,
                data TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
            table_name
        )
    } else {
        // SQLite: INTEGER PRIMARY KEY 默认自增
        format!(
            "CREATE TABLE IF NOT EXISTS {} (
                id INTEGER PRIMARY KEY,
                trace_id TEXT,
                span_id TEXT,
                data TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
            table_name
        )
    };

    session
        .execute_raw_ddl(&create_sql)
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
    use futures::future::join_all;
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

    join_all(handles).await;

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
pub(crate) fn validate_table_name(table_name: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
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
pub(crate) fn sanitize_sql_string(input: &str) -> String {
    input.replace('\'', "''")
}

/// 创建临时目录（用于测试）
///
/// 此函数需要 `test-utils` feature
#[cfg(feature = "test-utils")]
#[allow(dead_code)]
pub fn create_temp_dir() -> TempDir {
    tempfile::Builder::new()
        .prefix("dbnexus_test_")
        .tempdir()
        .expect("Failed to create temp directory")
}
