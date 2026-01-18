// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

use dbnexus::{config::DbConfigBuilder, DbPool};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = DbConfigBuilder::new()
        .url("sqlite:file::memory:?cache=shared")
        .permissions_path("src/permissions.yaml")
        .admin_role("admin")
        .max_connections(5)
        .min_connections(1)
        .build()?;
    let pool = DbPool::with_config(config).await?;

    let session = pool.get_session("admin").await?;

    // 测试 users 表
    session
        .execute_raw_ddl("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)")
        .await?;
    let _ = session
        .execute_raw("INSERT INTO users (id, name) VALUES (1, 'Alice')")
        .await?;
    println!("users table permission OK");

    // 测试 products 表
    session
        .execute_raw_ddl("CREATE TABLE products (id INTEGER PRIMARY KEY, name TEXT)")
        .await?;
    let _ = session
        .execute_raw("INSERT INTO products (id, name) VALUES (1, 'Product1')")
        .await?;
    println!("products table permission OK");

    Ok(())
}
