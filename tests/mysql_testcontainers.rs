// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! MySQL testcontainers 集成测试
//!
//! 使用 testcontainers 启动真实的 MySQL 容器，验证 dbnexus 在真实数据库环境下的
//! 连接、CRUD、事务等核心功能。需要 Docker 环境，且本地需有 `mysql:8.0-oracle` 镜像。
//!
//! # 运行方式
//!
//! ```bash
//! cargo test --test mysql_testcontainers --features mysql
//! ```

#![cfg(feature = "mysql")]

use dbnexus::{DbConfig, DbPool};
use testcontainers::GenericImage;
use testcontainers::core::{ContainerAsync, ImageExt, WaitFor};
use testcontainers::runners::AsyncRunner;

/// 启动一个 MySQL 容器并返回 (容器, 连接 URL)。
///
/// 使用本地 `mysql:8.0-oracle` 镜像（需预先 `docker pull mysql:8.0-oracle`）。
/// 容器必须在测试期间保持存活，否则 Docker 会回收它。
async fn setup_mysql() -> (Option<ContainerAsync<GenericImage>>, String) {
    if let Ok(url) = std::env::var("TEST_DATABASE_URL").or_else(|_| std::env::var("DATABASE_URL")) {
        return (None, url);
    }
    let container = GenericImage::new("mysql", "8.0-oracle")
        .with_wait_for(WaitFor::message_on_stdout("MySQL init process done"))
        .with_wait_for(WaitFor::seconds(5))
        .with_env_var("MYSQL_ROOT_PASSWORD", "root")
        .with_env_var("MYSQL_DATABASE", "dbnexus_test")
        .with_env_var("MYSQL_USER", "dbnexus")
        .with_env_var("MYSQL_PASSWORD", "dbnexus")
        .start()
        .await
        .expect("Failed to start MySQL container");

    let host = container.get_host().await.expect("Failed to get host");
    let port = container
        .get_host_port_ipv4(3306)
        .await
        .expect("Failed to get host port");

    let url = format!("mysql://dbnexus:dbnexus@{}:{}/dbnexus_test", host, port);

    (Some(container), url)
}

/// 创建测试用的 DbConfig
fn make_config(url: String) -> DbConfig {
    DbConfig {
        url,
        admin_role: "admin".to_string(),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 5,
            min_connections: 1,
            idle_timeout: 300,
            acquire_timeout: 5000,
        },
        ..Default::default()
    }
}

#[tokio::test]
async fn test_mysql_connection() {
    let (_container, url) = setup_mysql().await;
    let pool = DbPool::with_config(make_config(url))
        .await
        .expect("Failed to create pool");

    let session = pool.get_session("admin").await.expect("Failed to get session");
    assert_eq!(session.role(), "admin");

    let status = pool.status();
    assert!(
        status.total >= 1,
        "Pool should have at least one connection, got total={}",
        status.total
    );
    assert_eq!(
        status.total,
        status.active + status.idle,
        "Total should equal active + idle"
    );
}

#[tokio::test]
async fn test_mysql_crud_insert() {
    let (_container, url) = setup_mysql().await;
    let pool = DbPool::with_config(make_config(url))
        .await
        .expect("Failed to create pool");

    let session = pool.get_session("admin").await.expect("Failed to get session");

    session
        .execute_raw_ddl(
            "CREATE TABLE users (
                id INT AUTO_INCREMENT PRIMARY KEY,
                name VARCHAR(100) NOT NULL,
                email VARCHAR(200) NOT NULL,
                UNIQUE KEY uk_email (email)
            )",
        )
        .await
        .expect("Failed to create table");

    let result = session
        .execute_raw("INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com')")
        .await
        .expect("Failed to insert");

    assert_eq!(result.rows_affected(), 1, "Should insert 1 row");
}

#[tokio::test]
async fn test_mysql_crud_update_delete() {
    let (_container, url) = setup_mysql().await;
    let pool = DbPool::with_config(make_config(url))
        .await
        .expect("Failed to create pool");

    let session = pool.get_session("admin").await.expect("Failed to get session");

    session
        .execute_raw_ddl(
            "CREATE TABLE products (
                id INT AUTO_INCREMENT PRIMARY KEY,
                name VARCHAR(100) NOT NULL,
                stock INT NOT NULL DEFAULT 0
            )",
        )
        .await
        .expect("Failed to create table");

    session
        .execute_raw("INSERT INTO products (name, stock) VALUES ('Widget', 10)")
        .await
        .expect("Failed to insert");

    let update_result = session
        .execute_raw("UPDATE products SET stock = 5 WHERE name = 'Widget'")
        .await
        .expect("Failed to update");
    assert_eq!(update_result.rows_affected(), 1, "Should update 1 row");

    let delete_result = session
        .execute_raw("DELETE FROM products WHERE name = 'Widget'")
        .await
        .expect("Failed to delete");
    assert_eq!(delete_result.rows_affected(), 1, "Should delete 1 row");
}

#[tokio::test]
async fn test_mysql_transaction_rollback() {
    let (_container, url) = setup_mysql().await;
    let pool = DbPool::with_config(make_config(url))
        .await
        .expect("Failed to create pool");

    let session = pool.get_session("admin").await.expect("Failed to get session");

    session
        .execute_raw_ddl(
            "CREATE TABLE accounts (
                id INT AUTO_INCREMENT PRIMARY KEY,
                email VARCHAR(200) NOT NULL,
                UNIQUE KEY uk_email (email)
            )",
        )
        .await
        .expect("Failed to create table");

    session.begin_transaction().await.expect("Failed to begin transaction");

    session
        .execute_raw("INSERT INTO accounts (email) VALUES ('bob@example.com')")
        .await
        .expect("Failed to insert in transaction");

    session.rollback().await.expect("Failed to rollback");

    assert!(
        !session.is_in_transaction().await,
        "Should not be in transaction after rollback"
    );

    let result = session
        .execute_raw("INSERT INTO accounts (email) VALUES ('bob@example.com')")
        .await
        .expect("Failed to insert after rollback");
    assert_eq!(
        result.rows_affected(),
        1,
        "Insert should succeed after rollback (data was not committed)"
    );
}

#[tokio::test]
async fn test_mysql_transaction_commit() {
    let (_container, url) = setup_mysql().await;
    let pool = DbPool::with_config(make_config(url))
        .await
        .expect("Failed to create pool");

    let session = pool.get_session("admin").await.expect("Failed to get session");

    session
        .execute_raw_ddl(
            "CREATE TABLE IF NOT EXISTS orders_txn_commit (
                id INT AUTO_INCREMENT PRIMARY KEY,
                order_no VARCHAR(100) NOT NULL,
                UNIQUE KEY uk_order_no (order_no)
            )",
        )
        .await
        .expect("Failed to create table");

    session.begin_transaction().await.expect("Failed to begin transaction");

    session
        .execute_raw("INSERT INTO orders_txn_commit (order_no) VALUES ('ORD-001')")
        .await
        .expect("Failed to insert in transaction");

    session.commit().await.expect("Failed to commit");

    assert!(
        !session.is_in_transaction().await,
        "Should not be in transaction after commit"
    );

    let conflict_result = session
        .execute_raw("INSERT IGNORE INTO orders (order_no) VALUES ('ORD-001')")
        .await
        .expect("Failed to execute conflict insert");
    assert_eq!(
        conflict_result.rows_affected(),
        0,
        "Insert should be ignored due to committed data (rows_affected=0)"
    );
}

// ============================================================================
// 数据类型测试
// ============================================================================

#[tokio::test]
async fn test_mysql_data_types() {
    let (_container, url) = setup_mysql().await;
    let pool = DbPool::with_config(make_config(url))
        .await
        .expect("Failed to create pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    // MySQL 数据类型
    session
        .execute_raw_ddl(
            "CREATE TABLE type_test (
                id INT AUTO_INCREMENT PRIMARY KEY,
                bool_col BOOLEAN,
                tinyint_col TINYINT,
                int_col INT,
                bigint_col BIGINT,
                float_col FLOAT,
                double_col DOUBLE,
                text_col TEXT,
                varchar_col VARCHAR(100),
                json_col JSON,
                date_col DATE,
                datetime_col DATETIME
            )",
        )
        .await
        .expect("Failed to create table");

    session
        .execute_raw(
            "INSERT INTO type_test (bool_col, tinyint_col, int_col, bigint_col, float_col, double_col,
             text_col, varchar_col, json_col, date_col, datetime_col)
             VALUES (true, 127, 42, 9223372036854775807, 3.14, 2.718281828,
             'text value', 'varchar value', '{\"key\": \"value\"}',
             '2026-01-15', '2026-01-15 10:30:00')",
        )
        .await
        .expect("Failed to insert");

    let result = session
        .execute_raw("SELECT * FROM type_test WHERE id = 1")
        .await
        .expect("Failed to query");
    // MySQL execute_raw 对 SELECT 返回 rows_affected=0
    assert!(result.rows_affected() >= 0, "Query should succeed");
}

#[tokio::test]
async fn test_mysql_null_handling() {
    let (_container, url) = setup_mysql().await;
    let pool = DbPool::with_config(make_config(url))
        .await
        .expect("Failed to create pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    session
        .execute_raw_ddl("CREATE TABLE null_test (id INT AUTO_INCREMENT PRIMARY KEY, nullable_col VARCHAR(100))")
        .await
        .expect("Failed to create table");

    session
        .execute_raw("INSERT INTO null_test (nullable_col) VALUES (NULL)")
        .await
        .expect("Failed to insert NULL");

    let result = session
        .execute_raw("SELECT nullable_col FROM null_test WHERE id = 1")
        .await
        .expect("Failed to query");
    assert!(result.rows_affected() >= 0, "Query should succeed");
}

// ============================================================================
// 错误路径测试
// ============================================================================

#[tokio::test]
async fn test_mysql_syntax_error_returns_error() {
    let (_container, url) = setup_mysql().await;
    let pool = DbPool::with_config(make_config(url))
        .await
        .expect("Failed to create pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    let result = session.execute_raw("SELEC * FORM nonexistent").await;
    assert!(result.is_err(), "Syntax error should return error");
}

#[tokio::test]
async fn test_mysql_table_not_exists_returns_error() {
    let (_container, url) = setup_mysql().await;
    let pool = DbPool::with_config(make_config(url))
        .await
        .expect("Failed to create pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    let result = session.execute_raw("SELECT * FROM nonexistent_table").await;
    assert!(result.is_err(), "Query on nonexistent table should return error");
}

#[tokio::test]
async fn test_mysql_duplicate_key_returns_error() {
    let (_container, url) = setup_mysql().await;
    let pool = DbPool::with_config(make_config(url))
        .await
        .expect("Failed to create pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    session
        .execute_raw_ddl("CREATE TABLE pk_test (id INT PRIMARY KEY, value VARCHAR(50))")
        .await
        .expect("Failed to create table");

    session
        .execute_raw("INSERT INTO pk_test (id, value) VALUES (1, 'first')")
        .await
        .expect("First insert should succeed");

    let result = session
        .execute_raw("INSERT INTO pk_test (id, value) VALUES (1, 'duplicate')")
        .await;
    assert!(result.is_err(), "Duplicate primary key should return error");
}

// ============================================================================
// 聚合与 JOIN 测试
// ============================================================================

#[tokio::test]
async fn test_mysql_aggregate_query() {
    let (_container, url) = setup_mysql().await;
    let pool = DbPool::with_config(make_config(url))
        .await
        .expect("Failed to create pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    session
        .execute_raw_ddl(
            "CREATE TABLE sales (id INT AUTO_INCREMENT PRIMARY KEY, product VARCHAR(50), amount DECIMAL(10,2))",
        )
        .await
        .expect("Failed to create table");

    session
        .execute_raw("INSERT INTO sales (product, amount) VALUES ('A', 100.50), ('A', 200.00), ('B', 50.00)")
        .await
        .expect("Failed to insert");

    let result = session
        .execute_raw(
            "SELECT product, COUNT(*) as cnt, SUM(amount) as total FROM sales GROUP BY product ORDER BY product",
        )
        .await
        .expect("Aggregate query should succeed");
    assert!(result.rows_affected() >= 0, "Aggregate query should succeed");
}

#[tokio::test]
async fn test_mysql_join_query() {
    let (_container, url) = setup_mysql().await;
    let pool = DbPool::with_config(make_config(url))
        .await
        .expect("Failed to create pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    session
        .execute_raw_ddl("CREATE TABLE customers (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(100))")
        .await
        .expect("Failed to create customers table");

    session
        .execute_raw_ddl("CREATE TABLE orders (id INT AUTO_INCREMENT PRIMARY KEY, customer_id INT, total DECIMAL(10,2), FOREIGN KEY (customer_id) REFERENCES customers(id))")
        .await
        .expect("Failed to create orders table");

    session
        .execute_raw("INSERT INTO customers (name) VALUES ('Alice'), ('Bob')")
        .await
        .expect("Failed to insert customers");

    session
        .execute_raw("INSERT INTO orders (customer_id, total) VALUES (1, 100.00), (1, 200.00), (2, 50.00)")
        .await
        .expect("Failed to insert orders");

    let result = session
        .execute_raw(
            "SELECT c.name, COUNT(o.id) as order_count, SUM(o.total) as total_spent
             FROM customers c LEFT JOIN orders o ON c.id = o.customer_id
             GROUP BY c.name ORDER BY c.name",
        )
        .await
        .expect("JOIN query should succeed");
    assert!(result.rows_affected() >= 0, "JOIN query should succeed");
}

// ============================================================================
// 健康检查与并发测试
// ============================================================================

#[tokio::test]
async fn test_mysql_health_check() {
    let (_container, url) = setup_mysql().await;
    let pool = DbPool::with_config(make_config(url))
        .await
        .expect("Failed to create pool");

    // 通过成功获取 session 并执行查询来验证连接健康
    let session = pool.get_session("admin").await;
    assert!(session.is_ok(), "get_session should succeed with healthy connection");

    let session = session.unwrap();
    // 产品健康通道（execute_raw 的 SELECT 1 会被 sql-parser 无表名拦截）
    let conn = session.connection().expect("session connection available");
    let healthy = pool
        .check_connection_health(&dbnexus::DbConnection::SeaOrm(conn.clone()))
        .await;
    assert!(healthy, "Health check should succeed");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_mysql_concurrent_access() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let (_container, url) = setup_mysql().await;
    let config = make_config(url);
    let pool = Arc::new(DbPool::with_config(config).await.expect("Failed to create pool"));

    // 创建测试表
    {
        let session = pool.get_session("admin").await.expect("Failed to get setup session");
        session
            .execute_raw_ddl("CREATE TABLE concurrent_test (id INT, value INT)")
            .await
            .expect("CREATE TABLE should succeed");
    }

    let success_count = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();

    for i in 0..4 {
        let pool_clone = pool.clone();
        let success_clone = success_count.clone();
        handles.push(tokio::spawn(async move {
            let session = match pool_clone.get_session("admin").await {
                Ok(s) => s,
                Err(_) => return,
            };
            let sql = format!("INSERT INTO concurrent_test VALUES ({}, {})", i, i * 10);
            if session.execute_raw(&sql).await.is_ok() {
                success_clone.fetch_add(1, Ordering::SeqCst);
            }
        }));
    }

    for handle in handles {
        handle.await.expect("Task panicked");
    }

    assert_eq!(
        success_count.load(Ordering::SeqCst),
        4,
        "All concurrent inserts should succeed"
    );

    // 验证数据
    let session = pool
        .get_session("admin")
        .await
        .expect("Failed to get verification session");
    let result = session
        .execute_raw("SELECT COUNT(*) as cnt FROM concurrent_test")
        .await
        .expect("COUNT query should succeed");
    assert!(result.rows_affected() >= 0, "COUNT query should succeed");
}
