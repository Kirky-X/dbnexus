// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! DDL 辅助函数
//!
//! 提供 `users` / `articles` 表的快捷创建函数，统一示例中的 `CREATE TABLE` 样板。
//! 使用 `session.execute_raw_ddl()` 执行 DDL（仅 admin 角色可调用）。

use dbnexus::Session;

/// 创建 `users` 表
///
/// 表结构：`id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT NOT NULL`
pub async fn create_users_table(session: &Session) -> Result<(), Box<dyn std::error::Error>> {
    session
        .execute_raw_ddl(
            "CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                email TEXT NOT NULL
            )",
        )
        .await?;
    Ok(())
}

/// 创建 `articles` 表
///
/// 表结构：`id INTEGER PRIMARY KEY, title TEXT NOT NULL, content TEXT NOT NULL, author TEXT NOT NULL`
pub async fn create_articles_table(session: &Session) -> Result<(), Box<dyn std::error::Error>> {
    session
        .execute_raw_ddl(
            "CREATE TABLE IF NOT EXISTS articles (
                id INTEGER PRIMARY KEY,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                author TEXT NOT NULL
            )",
        )
        .await?;
    Ok(())
}
