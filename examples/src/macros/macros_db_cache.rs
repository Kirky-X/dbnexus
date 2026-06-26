// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in the project root for full license information.

//! db_entity 宏 cache 子参数示例
//!
//! 演示 `#[db_entity(..., cache(...))]` 生成的缓存配置常量与方法：
//! - 宏生成的常量：`CACHE_TTL` / `CACHE_STRATEGY` / `CACHE_MAX_CAPACITY` / `CACHE_ENABLED`
//! - 宏生成的方法：`cache_key(id)` / `cache_config()`
//! - 结合 CRUD 操作展示缓存键的生成模式
//! - 通过 `cache_config()` 生成 `CacheConfig` 用于初始化缓存
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example macros_db_cache --features "sqlite,permission,macros,cache"
//! ```

#[path = "../common/mod.rs"]
mod common;

use dbnexus::{CacheConfig, db_entity};
use sea_orm::entity::prelude::*;

// ============================================
// 定义 Article 实体（带 cache 子参数）
// ============================================

/// 文章实体
///
/// `#[db_entity(..., cache(ttl = 60, strategy = "lru", max_capacity = 5000))]` 生成缓存配置：
/// - `CACHE_TTL`           缓存 TTL（秒），默认 300
/// - `CACHE_STRATEGY`      缓存策略名称，默认 "lru"
/// - `CACHE_MAX_CAPACITY`  缓存最大容量，默认 10000
/// - `CACHE_ENABLED`       缓存是否启用，始终为 true
/// - `cache_key(id: i64)`  生成缓存键，格式为 "{table_name}:{id}"
/// - `cache_config()`      生成 `CacheConfig` 配置实例
#[db_entity(
    table_name = "articles",
    primary_key = "id",
    cache(ttl = 60, strategy = "lru", max_capacity = 5000)
)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "articles")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub title: String,
    pub content: String,
    pub author: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

// ============================================
// 主函数
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("💾 DBNexus db_cache 宏示例");
    println!("========================================\n");

    // ============================================
    // 1. 展示宏生成的缓存常量
    // ============================================
    println!("--- 1. 宏生成的缓存常量 ---\n");
    println!("  CACHE_TTL          = {} 秒", Model::CACHE_TTL);
    println!("  CACHE_STRATEGY     = \"{}\"", Model::CACHE_STRATEGY);
    println!("  CACHE_MAX_CAPACITY = {} 条", Model::CACHE_MAX_CAPACITY);
    println!("  CACHE_ENABLED      = {}", Model::CACHE_ENABLED);

    // ============================================
    // 2. 展示 cache_key() 方法
    // ============================================
    println!("\n--- 2. cache_key() 方法 ---\n");
    let ids: Vec<i64> = vec![1, 2, 3, 100, 9999];
    println!("  生成各 ID 的缓存键:");
    for id in &ids {
        println!("    id={:<5} → key=\"{}\"", id, Model::cache_key(*id));
    }

    // ============================================
    // 3. 展示 cache_config() 方法
    // ============================================
    println!("\n--- 3. cache_config() 方法 ---\n");
    let cache_config: CacheConfig = Model::cache_config();
    println!("  生成的 CacheConfig:");
    println!("    policy_cache_capacity   = {}", cache_config.policy_cache_capacity);
    println!("    sql_parse_cache_capacity = {}", cache_config.sql_parse_cache_capacity);
    println!("    query_cache_capacity    = {}", cache_config.query_cache_capacity);
    println!("    default_ttl             = {} 秒", cache_config.default_ttl);

    // ============================================
    // 4. 创建 DbPool + Session
    // ============================================
    println!("\n--- 4. 创建 DbPool + Session ---\n");
    let (_pool, session) = common::db::setup_shared_sqlite_session().await?;
    println!("  ✓ Session 创建成功 (角色: admin)");

    // 建表
    session
        .execute_raw_ddl(
            "CREATE TABLE articles (
                id INTEGER PRIMARY KEY,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                author TEXT NOT NULL
            )",
        )
        .await?;
    println!("  ✓ articles 表创建成功");

    // ============================================
    // 5. CRUD 操作 + 缓存键模式
    // ============================================
    println!("\n--- 5. CRUD 操作 + 缓存键模式 ---\n");

    // CREATE
    println!("[CREATE]");
    let article1 = Model {
        id: 1,
        title: "Rust 异步编程指南".to_string(),
        content: "本文介绍 Rust async/await 的核心概念...".to_string(),
        author: "Alice".to_string(),
    };
    let created = Model::insert(&session, article1).await?;
    println!("  ✓ 插入文章: id={}, title=\"{}\"", created.id, created.title);
    println!("    缓存键: \"{}\" (可用于写入缓存)", Model::cache_key(created.id));

    let article2 = Model {
        id: 2,
        title: "Sea-ORM 实战教程".to_string(),
        content: "Sea-ORM 是 Rust 的异步 ORM 框架...".to_string(),
        author: "Bob".to_string(),
    };
    let created2 = Model::insert(&session, article2).await?;
    println!("  ✓ 插入文章: id={}, title=\"{}\"", created2.id, created2.title);
    println!("    缓存键: \"{}\"", Model::cache_key(created2.id));

    // READ
    println!("\n[READ]");
    let found = Model::find_by_id(&session, 1).await?;
    if let Some(ref a) = found {
        println!("  ✓ 查询文章: id={}, title=\"{}\", author=\"{}\"", a.id, a.title, a.author);
        println!("    缓存键: \"{}\" (可用于读取/回填缓存)", Model::cache_key(a.id));
    }

    // UPDATE
    println!("\n[UPDATE]");
    let before = found.unwrap();
    let updated = Model::update(&session, Model {
        title: "Rust 异步编程指南（第二版）".to_string(),
        ..before
    }).await?;
    println!("  ✓ 更新文章: id={}, 新 title=\"{}\"", updated.id, updated.title);
    println!("    缓存键: \"{}\" (更新后应失效缓存)", Model::cache_key(updated.id));

    // DELETE
    println!("\n[DELETE]");
    let deleted = Model::delete(&session, 2).await?;
    println!("  ✓ 删除文章 id=2: 影响 {} 行", deleted);
    println!("    缓存键: \"{}\" (删除后应清除缓存)", Model::cache_key(2));

    // ============================================
    // 6. 缓存键批量生成模式
    // ============================================
    println!("\n--- 6. 缓存键批量生成模式 ---\n");
    println!("  批量查询时生成缓存键列表:");
    let query_ids: Vec<i64> = vec![1, 2, 3, 4, 5];
    let cache_keys: Vec<String> = query_ids.iter().map(|id| Model::cache_key(*id)).collect();
    for (id, key) in query_ids.iter().zip(cache_keys.iter()) {
        println!("    id={} → \"{}\"", id, key);
    }

    // ============================================
    // 7. CacheConfig 应用场景
    // ============================================
    println!("\n--- 7. CacheConfig 应用场景 ---\n");
    let cfg = Model::cache_config();
    println!("  基于 db_cache 宏生成的 CacheConfig:");
    println!("    - 策略缓存容量:     {} (来自 max_capacity={})",
        cfg.policy_cache_capacity, Model::CACHE_MAX_CAPACITY);
    println!("    - SQL 解析缓存容量: {} (来自 max_capacity={})",
        cfg.sql_parse_cache_capacity, Model::CACHE_MAX_CAPACITY);
    println!("    - 查询缓存容量:     {} (来自 max_capacity={})",
        cfg.query_cache_capacity, Model::CACHE_MAX_CAPACITY);
    println!("    - 默认 TTL:         {} 秒 (来自 CACHE_TTL={})",
        cfg.default_ttl, Model::CACHE_TTL);
    println!("\n  该配置可用于初始化 oxcache::Cache 或其他缓存后端。");

    println!("\n========================================");
    println!("✨ db_entity 宏 cache 示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - #[db_entity(..., cache(ttl=60, strategy=\"lru\", max_capacity=5000))]  生成缓存配置");
    println!("  - Model::CACHE_TTL           缓存 TTL（秒）");
    println!("  - Model::CACHE_STRATEGY      缓存策略名称");
    println!("  - Model::CACHE_MAX_CAPACITY  缓存最大容量");
    println!("  - Model::CACHE_ENABLED       缓存是否启用");
    println!("  - Model::cache_key(id)       生成缓存键 \"{{table_name}}:{{id}}\"");
    println!("  - Model::cache_config()      生成 CacheConfig 实例");
    println!("\n⚠️  注意: #[db_entity] 的 cache 子参数仅生成配置常量和辅助方法，不自动执行缓存读写。");
    println!("   开发者需在 CRUD 操作前后手动调用缓存 API 进行读写/失效。");
    println!("   常量用于统一管理缓存策略，避免硬编码。");

    Ok(())
}
