// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in the project root for full license information.

//! 数据库连接池与会话辅助函数
//!
//! 提供三种典型场景的快捷构造方式，统一示例中的 DbPool/Session 样板代码：
//! - 内存 SQLite（`sqlite::memory:`）—— 单连接独占，适用于无需跨会话共享表的场景
//! - 共享内存 SQLite（`sqlite:file::memory:?cache=shared`）—— 多连接共享同一内存库
//! - 文件 SQLite（`sqlite://<path>`）—— 持久化到磁盘

use dbnexus::{DbConfig, DbPool, Session};

/// 构造一份默认的 SQLite `DbConfig`
///
/// `admin_role = "admin"`、`max_connections = 5`、`min_connections = 1`。
fn sqlite_config(url: &str) -> DbConfig {
    DbConfig {
        url: url.to_string(),
        admin_role: "admin".to_string(),
        max_connections: 5,
        min_connections: 1,
        ..Default::default()
    }
}

/// 同步创建一个 SQLite 内存连接池（`sqlite::memory:`）
///
/// `DbPool::with_config` 实际为 async，因此本函数仅作 API 占位；
/// 请使用 [`setup_sqlite_pool_async`]。
pub fn setup_sqlite_pool() -> Result<DbPool, Box<dyn std::error::Error>> {
    Err("use setup_sqlite_pool_async() instead — DbPool::with_config is async".into())
}

/// 异步创建一个 SQLite 内存连接池（`sqlite::memory:`）
pub async fn setup_sqlite_pool_async() -> Result<DbPool, Box<dyn std::error::Error>> {
    let pool = DbPool::with_config(sqlite_config("sqlite::memory:")).await?;
    Ok(pool)
}

/// 创建 SQLite 内存连接池 + admin Session（`sqlite::memory:`）
///
/// 适用于不需要跨 Session 共享表的简单示例。
pub async fn setup_sqlite_session() -> Result<(DbPool, Session), Box<dyn std::error::Error>> {
    let pool = setup_sqlite_pool_async().await?;
    let session = pool.get_session("admin").await?;
    Ok((pool, session))
}

/// 创建共享内存 SQLite 连接池 + admin Session
///
/// 使用 `sqlite:file::memory:?cache=shared`，多个连接共享同一份内存数据库，
/// 适用于需要在多个 Session 间看到同一张表的示例（如 CRUD + 事务演示）。
pub async fn setup_shared_sqlite_session() -> Result<(DbPool, Session), Box<dyn std::error::Error>> {
    let pool = DbPool::with_config(sqlite_config("sqlite:file::memory:?cache=shared")).await?;
    let session = pool.get_session("admin").await?;
    Ok((pool, session))
}

/// 创建文件型 SQLite 连接池 + admin Session
///
/// 在系统临时目录下创建 `dbnexus_example.db` 文件。sqlx 默认 `create_if_missing=false`，
/// 因此本函数会预创建空文件。返回 `(pool, session, file_path)`，调用方可在结束时
/// 用 `file_path` 删除数据库文件。
pub async fn setup_file_sqlite_session() -> Result<(DbPool, Session, std::path::PathBuf), Box<dyn std::error::Error>> {
    let path = std::env::temp_dir().join("dbnexus_example.db");
    // 清理旧文件以确保示例干净
    let _ = std::fs::remove_file(&path);
    // 预创建空文件
    std::fs::File::create(&path)?;

    let url = format!("sqlite://{}", path.display());
    let pool = DbPool::with_config(sqlite_config(&url)).await?;
    let session = pool.get_session("admin").await?;
    Ok((pool, session, path))
}
