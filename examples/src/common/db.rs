// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 数据库连接池与会话辅助函数
//!
//! 提供共享内存 SQLite（`sqlite:file::memory:?cache=shared`）的快捷构造方式，
//! 统一示例中的 DbPool/Session 样板代码：多连接共享同一份内存库，适用于需要在
//! 多个 Session 间看到同一张表的示例（如 CRUD + 事务演示）。

use dbnexus::{DbConfig, DbPool, Session};

/// 构造一份默认的 SQLite `DbConfig`
///
/// `admin_role = "admin"`、`max_connections = 5`、`min_connections = 1`。
fn sqlite_config(url: &str) -> DbConfig {
    DbConfig {
        url: url.to_string(),
        admin_role: "admin".to_string(),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 5,
            min_connections: 1,
            ..Default::default()
        },
        ..Default::default()
    }
}
/// 创建共享内存 SQLite 连接池 + admin Session
///
/// 使用 `sqlite:file::memory:?cache=shared`，多个连接共享同一份内存数据库，
/// 适用于需要在多个 Session 间看到同一张表的示例（如 CRUD + 事务演示）。
pub async fn setup_shared_sqlite_session() -> Result<(DbPool, Session), Box<dyn std::error::Error>>
{
    let pool = DbPool::with_config(sqlite_config("sqlite:file::memory:?cache=shared")).await?;
    let session = pool.get_session("admin").await?;
    Ok((pool, session))
}
