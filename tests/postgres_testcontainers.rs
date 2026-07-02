// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in the project root for full license information.

//! PostgreSQL testcontainers 集成测试
//!
//! 使用 testcontainers 启动真实的 PostgreSQL 容器，验证 dbnexus 在真实数据库环境下的
//! 连接、CRUD、事务等核心功能。需要 Docker 环境，且本地需有 `postgres:16-alpine` 镜像。
//!
//! # 运行方式
//!
//! ```bash
//! cargo test --test postgres_testcontainers --features postgres
//! ```

#![cfg(feature = "postgres")]

use dbnexus::{DbConfig, DbPool};
use testcontainers::GenericImage;
use testcontainers::core::{ContainerAsync, ImageExt, WaitFor};
use testcontainers::runners::AsyncRunner;

/// 启动一个 PostgreSQL 容器并返回 (容器, 连接 URL)。
///
/// 使用本地 `postgres:16-alpine` 镜像（需预先 `docker pull postgres:16-alpine`）。
/// 容器必须在测试期间保持存活，否则 Docker 会回收它。
async fn setup_postgres() -> (ContainerAsync<GenericImage>, String) {
    let container = GenericImage::new("postgres", "16-alpine")
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ))
        .with_wait_for(WaitFor::seconds(2))
        .with_env_var("POSTGRES_USER", "dbnexus")
        .with_env_var("POSTGRES_PASSWORD", "dbnexus")
        .with_env_var("POSTGRES_DB", "dbnexus_test")
        .start()
        .await
        .expect("Failed to start PostgreSQL container");

    let host = container.get_host().await.expect("Failed to get host");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get host port");

    let url = format!("postgres://dbnexus:dbnexus@{}:{}/dbnexus_test", host, port);

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
async fn test_postgres_connection() {
    let (_container, url) = setup_postgres().await;
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
async fn test_postgres_crud_insert() {
    let (_container, url) = setup_postgres().await;
    let pool = DbPool::with_config(make_config(url))
        .await
        .expect("Failed to create pool");

    let session = pool.get_session("admin").await.expect("Failed to get session");

    session
        .execute_raw_ddl(
            "CREATE TABLE users (
                id SERIAL PRIMARY KEY,
                name VARCHAR(100) NOT NULL,
                email VARCHAR(200) NOT NULL UNIQUE
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
async fn test_postgres_crud_update_delete() {
    let (_container, url) = setup_postgres().await;
    let pool = DbPool::with_config(make_config(url))
        .await
        .expect("Failed to create pool");

    let session = pool.get_session("admin").await.expect("Failed to get session");

    session
        .execute_raw_ddl(
            "CREATE TABLE products (
                id SERIAL PRIMARY KEY,
                name VARCHAR(100) NOT NULL,
                stock INTEGER NOT NULL DEFAULT 0
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
async fn test_postgres_transaction_rollback() {
    let (_container, url) = setup_postgres().await;
    let pool = DbPool::with_config(make_config(url))
        .await
        .expect("Failed to create pool");

    let session = pool.get_session("admin").await.expect("Failed to get session");

    session
        .execute_raw_ddl(
            "CREATE TABLE accounts (
                id SERIAL PRIMARY KEY,
                email VARCHAR(200) NOT NULL UNIQUE
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
async fn test_postgres_transaction_commit() {
    let (_container, url) = setup_postgres().await;
    let pool = DbPool::with_config(make_config(url))
        .await
        .expect("Failed to create pool");

    let session = pool.get_session("admin").await.expect("Failed to get session");

    session
        .execute_raw_ddl(
            "CREATE TABLE orders (
                id SERIAL PRIMARY KEY,
                order_no VARCHAR(100) NOT NULL UNIQUE
            )",
        )
        .await
        .expect("Failed to create table");

    session.begin_transaction().await.expect("Failed to begin transaction");

    session
        .execute_raw("INSERT INTO orders (order_no) VALUES ('ORD-001')")
        .await
        .expect("Failed to insert in transaction");

    session.commit().await.expect("Failed to commit");

    assert!(
        !session.is_in_transaction().await,
        "Should not be in transaction after commit"
    );

    let conflict_result = session
        .execute_raw("INSERT INTO orders (order_no) VALUES ('ORD-001') ON CONFLICT (order_no) DO NOTHING")
        .await
        .expect("Failed to execute conflict insert");
    assert_eq!(
        conflict_result.rows_affected(),
        0,
        "Insert should conflict with committed data (rows_affected=0)"
    );
}
