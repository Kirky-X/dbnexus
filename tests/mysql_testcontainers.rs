// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in the project root for full license information.

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
use testcontainers::core::{ContainerAsync, ImageExt, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::GenericImage;

/// 启动一个 MySQL 容器并返回 (容器, 连接 URL)。
///
/// 使用本地 `mysql:8.0-oracle` 镜像（需预先 `docker pull mysql:8.0-oracle`）。
/// 容器必须在测试期间保持存活，否则 Docker 会回收它。
async fn setup_mysql() -> (ContainerAsync<GenericImage>, String) {
    let container = GenericImage::new("mysql", "8.0-oracle")
        .with_wait_for(WaitFor::message_on_stdout(
            "MySQL init process done",
        ))
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

    (container, url)
}

/// 创建测试用的 DbConfig
fn make_config(url: String) -> DbConfig {
    DbConfig {
        url,
        admin_role: "admin".to_string(),
        max_connections: 5,
        min_connections: 1,
        idle_timeout: 300,
        acquire_timeout: 5000,
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
        .execute_raw(
            "INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com')",
        )
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

    session
        .begin_transaction()
        .await
        .expect("Failed to begin transaction");

    session
        .execute_raw("INSERT INTO accounts (email) VALUES ('bob@example.com')")
        .await
        .expect("Failed to insert in transaction");

    session
        .rollback()
        .await
        .expect("Failed to rollback");

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
            "CREATE TABLE orders (
                id INT AUTO_INCREMENT PRIMARY KEY,
                order_no VARCHAR(100) NOT NULL,
                UNIQUE KEY uk_order_no (order_no)
            )",
        )
        .await
        .expect("Failed to create table");

    session
        .begin_transaction()
        .await
        .expect("Failed to begin transaction");

    session
        .execute_raw("INSERT INTO orders (order_no) VALUES ('ORD-001')")
        .await
        .expect("Failed to insert in transaction");

    session
        .commit()
        .await
        .expect("Failed to commit");

    assert!(
        !session.is_in_transaction().await,
        "Should not be in transaction after commit"
    );

    let conflict_result = session
        .execute_raw(
            "INSERT IGNORE INTO orders (order_no) VALUES ('ORD-001')",
        )
        .await
        .expect("Failed to execute conflict insert");
    assert_eq!(
        conflict_result.rows_affected(),
        0,
        "Insert should be ignored due to committed data (rows_affected=0)"
    );
}
