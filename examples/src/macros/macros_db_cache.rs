// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
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

use dbnexus::sea_orm::entity::prelude::*;
use dbnexus::{CacheConfig, db_entity};

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
    println!("💾 DBNexus db_cache macro example");
    println!("========================================\n");

    // ============================================
    // 1. 展示宏生成的缓存常量
    // ============================================
    println!("--- 1. macro-generated cache constants ---\n");
    println!("  CACHE_TTL          = {} sec", Model::CACHE_TTL);
    println!("  CACHE_STRATEGY     = \"{}\"", Model::CACHE_STRATEGY);
    println!(
        "  CACHE_MAX_CAPACITY = {} entries",
        Model::CACHE_MAX_CAPACITY
    );
    println!("  CACHE_ENABLED      = {}", Model::CACHE_ENABLED);

    // ============================================
    // 2. 展示 cache_key() 方法
    // ============================================
    println!("\n--- 2. cache_key() method ---\n");
    let ids: Vec<i64> = vec![1, 2, 3, 100, 9999];
    println!("  generating the cache key for each ID:");
    for id in &ids {
        println!("    id={:<5} → key=\"{}\"", id, Model::cache_key(*id));
    }

    // ============================================
    // 3. 展示 cache_config() 方法
    // ============================================
    println!("\n--- 3. cache_config() method ---\n");
    let cache_config: CacheConfig = Model::cache_config();
    println!("  the generated CacheConfig:");
    println!(
        "    policy_cache_capacity   = {}",
        cache_config.policy_cache_capacity
    );
    println!(
        "    sql_parse_cache_capacity = {}",
        cache_config.sql_parse_cache_capacity
    );
    println!(
        "    query_cache_capacity    = {}",
        cache_config.query_cache_capacity
    );
    println!(
        "    default_ttl             = {} sec",
        cache_config.default_ttl
    );

    // ============================================
    // 4. 创建 DbPool + Session
    // ============================================
    println!("\n--- 4. create DbPool + Session ---\n");
    let (_pool, session) = common::db::setup_shared_sqlite_session().await?;
    println!("  ✓ Session created (role: admin)");

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
    println!("  ✓ articles table created");

    // ============================================
    // 5. CRUD 操作 + 缓存键模式
    // ============================================
    println!("\n--- 5. CRUD operations + cache key patterns ---\n");

    // CREATE
    println!("[CREATE]");
    let article1 = Model {
        id: 1,
        title: "Rust Async Programming Guide".to_string(),
        content: "This article introduces the core concepts of Rust async/await...".to_string(),
        author: "Alice".to_string(),
    };
    let created = Model::insert(&session, article1).await?;
    println!(
        "  ✓ inserted article: id={}, title=\"{}\"",
        created.id, created.title
    );
    println!(
        "    cache key: \"{}\" (usable for writing to the cache)",
        Model::cache_key(created.id)
    );

    let article2 = Model {
        id: 2,
        title: "Sea-ORM in Practice".to_string(),
        content: "Sea-ORM is an async ORM framework for Rust...".to_string(),
        author: "Bob".to_string(),
    };
    let created2 = Model::insert(&session, article2).await?;
    println!(
        "  ✓ inserted article: id={}, title=\"{}\"",
        created2.id, created2.title
    );
    println!("    cache key: \"{}\"", Model::cache_key(created2.id));

    // READ
    println!("\n[READ]");
    let found = Model::find_by_id(&session, 1).await?;
    if let Some(ref a) = found {
        println!(
            "  ✓ queried article: id={}, title=\"{}\", author=\"{}\"",
            a.id, a.title, a.author
        );
        println!(
            "    cache key: \"{}\" (usable for reading/backfilling the cache)",
            Model::cache_key(a.id)
        );
    }

    // UPDATE
    println!("\n[UPDATE]");
    let before = found.unwrap();
    let updated = Model::update(
        &session,
        Model {
            title: "Rust Async Programming Guide (2nd edition)".to_string(),
            ..before
        },
    )
    .await?;
    println!(
        "  ✓ updated article: id={}, new title=\"{}\"",
        updated.id, updated.title
    );
    println!(
        "    cache key: \"{}\" (the cache should be invalidated after update)",
        Model::cache_key(updated.id)
    );

    // DELETE
    println!("\n[DELETE]");
    let deleted = Model::delete(&session, 2).await?;
    println!("  ✓ deleted article id=2: {} rows affected", deleted);
    println!(
        "    cache key: \"{}\" (the cache should be cleared after delete)",
        Model::cache_key(2)
    );

    // ============================================
    // 6. 缓存键批量生成模式
    // ============================================
    println!("\n--- 6. batch cache key generation pattern ---\n");
    println!("  generating cache keys for batch queries:");
    let query_ids: Vec<i64> = vec![1, 2, 3, 4, 5];
    let cache_keys: Vec<String> = query_ids.iter().map(|id| Model::cache_key(*id)).collect();
    for (id, key) in query_ids.iter().zip(cache_keys.iter()) {
        println!("    id={} → \"{}\"", id, key);
    }

    // ============================================
    // 7. CacheConfig 应用场景
    // ============================================
    println!("\n--- 7. CacheConfig use cases ---\n");
    let cfg = Model::cache_config();
    println!("  CacheConfig generated by the db_cache macro:");
    println!(
        "    - policy cache capacity:    {} (from max_capacity={})",
        cfg.policy_cache_capacity,
        Model::CACHE_MAX_CAPACITY
    );
    println!(
        "    - SQL parse cache capacity: {} (from max_capacity={})",
        cfg.sql_parse_cache_capacity,
        Model::CACHE_MAX_CAPACITY
    );
    println!(
        "    - query cache capacity:     {} (from max_capacity={})",
        cfg.query_cache_capacity,
        Model::CACHE_MAX_CAPACITY
    );
    println!(
        "    - default TTL:         {} sec (from CACHE_TTL={})",
        cfg.default_ttl,
        Model::CACHE_TTL
    );
    println!("\n  this config can initialize an oxcache::Cache or another cache backend.");

    println!("\n========================================");
    println!("✨ db_entity macro cache example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!(
        "  - #[db_entity(..., cache(ttl=60, strategy=\"lru\", max_capacity=5000))]  generates the cache config"
    );
    println!("  - Model::CACHE_TTL           the cache TTL (seconds)");
    println!("  - Model::CACHE_STRATEGY      the cache strategy name");
    println!("  - Model::CACHE_MAX_CAPACITY  the max cache capacity");
    println!("  - Model::CACHE_ENABLED       whether the cache is enabled");
    println!("  - Model::cache_key(id)       generates the cache key \"{{table_name}}:{{id}}\"");
    println!("  - Model::cache_config()      generates a CacheConfig instance");
    println!(
        "\n⚠️  Note: the cache sub-parameter of #[db_entity] only generates config constants and helper methods, it does not perform cache reads/writes automatically."
    );
    println!(
        "   developers must call the cache API manually around CRUD operations to read/write/invalidate."
    );
    println!("   the constants manage the cache policy centrally, avoiding hardcoding.");

    Ok(())
}
