// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! DbNexusModule + AsyncKit 依赖注入示例
//!
//! 演示如何使用 [`DbNexusModule`] 与 trait-kit 的 [`AsyncKit`] 进行模块注册、
//! 构建和获取数据库连接池能力：
//! - 通过 `AsyncKit::register` 注册 `OxcacheModule` 和 `DbNexusModule`
//! - 通过 `AsyncKit::set_config` 配置 `OxcacheConfig` 和 `DbConfig`
//! - 通过 `AsyncKit::build` 构建并获取 `Arc<dyn ConnectionPool>`
//! - 使用连接池执行 DDL、INSERT、SELECT 和事务操作
//!
//! `DbNexusModule` 是 trait-kit 0.4 `AsyncKit` 集成模块，
//! 它将 dbnexus 的数据库连接池注入到 AsyncKit 依赖发现框架中。
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --bin kit_usage --features "kit"
//! ```

#[path = "../common/mod.rs"]
mod common;

use std::sync::Arc;

use dbnexus::DbNexusModule;
use dbnexus::database::ConnectionPool;
use dbnexus::foundation::{DbConfig, PoolConfig};
use oxcache::integrations::kit::{OxcacheConfig, OxcacheModule};
use trait_kit::prelude::*;

// ============================================
// 主函数
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🧰 DBNexus AsyncKit + DbNexusModule dependency injection example");
    println!("========================================\n");

    // ============================================
    // 1. 创建 AsyncKit 并配置
    // ============================================
    println!("--- 1. create AsyncKit and set the configuration ---\n");

    let mut kit = AsyncKit::new();
    println!("  ✓ AsyncKit created");

    // 设置 OxcacheConfig（OxcacheModule 需要）
    kit.set_config(OxcacheConfig::default());
    println!("  ✓ OxcacheConfig set (default configuration)");

    // 设置 DbConfig（DbNexusModule 需要）
    let db_config = DbConfig {
        url: "sqlite:file::memory:?cache=shared".to_string(),
        admin_role: "admin".to_string(),
        pool_config: PoolConfig {
            max_connections: 5,
            min_connections: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    kit.set_config(db_config);
    println!("  ✓ DbConfig set (sqlite shared memory, max_connections=5)");

    // ============================================
    // 2. 注册模块
    // ============================================
    println!("\n--- 2. register modules ---\n");

    // OxcacheModule 必须先注册（DbNexusModule 依赖它）
    kit.register::<OxcacheModule>()
        .map_err(|e| format!("register OxcacheModule: {e}"))?;
    println!("  ✓ OxcacheModule registered (the cache module)");

    kit.register::<DbNexusModule>()
        .map_err(|e| format!("register DbNexusModule: {e}"))?;
    println!("  ✓ DbNexusModule registered (the database pool module)");
    println!("  dependencies: DbNexusModule → OxcacheModule");

    // ============================================
    // 3. 构建 Kit 并获取连接池
    // ============================================
    println!("\n--- 3. build the Kit ---\n");

    let kit = kit
        .build()
        .await
        .map_err(|e| format!("AsyncKit::build: {e}"))?;
    println!("  ✓ AsyncKit built successfully");

    // 从 kit 中获取 DbNexusModule 的能力（Arc<dyn ConnectionPool + Send + Sync>）
    let pool: Arc<dyn ConnectionPool + Send + Sync> = kit
        .require::<DbNexusModule>()
        .map_err(|e| format!("require DbNexusModule: {e}"))?;
    println!("  ✓ Arc<dyn ConnectionPool> acquired successfully");

    // ============================================
    // 4. 使用连接池
    // ============================================
    println!("\n--- 4. use the connection pool ---\n");

    let status = pool.status();
    println!(
        "  pool status: total={}, active={}, idle={}",
        status.total, status.active, status.idle
    );
    println!("  configuration URL: {}", pool.config().url);

    let session = pool.get_session("admin").await?;
    println!("  ✓ Session acquired (role: {})", session.role());

    session
        .execute_raw_ddl(
            "CREATE TABLE IF NOT EXISTS kit_demo (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                created_at TEXT DEFAULT (datetime('now'))
            )",
        )
        .await?;
    println!("  ✓ DDL executed: CREATE TABLE kit_demo");

    // ============================================
    // 5. INSERT 数据
    // ============================================
    println!("\n--- 5. INSERT data ---\n");

    session
        .execute_raw("INSERT INTO kit_demo (id, name) VALUES (1, 'async-kit')")
        .await?;
    session
        .execute_raw("INSERT INTO kit_demo (id, name) VALUES (2, 'dbnexus-module')")
        .await?;
    session
        .execute_raw("INSERT INTO kit_demo (id, name) VALUES (3, 'oxcache-module')")
        .await?;
    println!("  ✓ inserted 3 test rows");

    // ============================================
    // 6. SELECT 查询
    // ============================================
    println!("\n--- 6. SELECT query ---\n");

    session
        .execute_raw("SELECT id, name FROM kit_demo ORDER BY id")
        .await?;
    println!("  ✓ queried the kit_demo table successfully");

    // ============================================
    // 7. 事务操作
    // ============================================
    println!("\n--- 7. transactions ---\n");

    session.begin_transaction().await?;
    println!("  ✓ transaction started");

    session
        .execute_raw("INSERT INTO kit_demo (id, name) VALUES (4, 'transaction-test')")
        .await?;
    println!("  ✓ inserted inside the transaction: transaction-test");

    println!(
        "  is_in_transaction() = {}",
        session.is_in_transaction().await
    );

    session.commit().await?;
    println!("  ✓ transaction committed");

    // ============================================
    // 8. 错误处理演示
    // ============================================
    println!("\n--- 8. error handling ---\n");

    // 演示：未注册 OxcacheModule 时 build 会失败
    let mut incomplete_kit = AsyncKit::new();
    incomplete_kit.set_config(DbConfig {
        url: "sqlite::memory:".to_string(),
        ..Default::default()
    });
    incomplete_kit
        .register::<DbNexusModule>()
        .map_err(|e| format!("register: {e}"))?;

    match incomplete_kit.build().await {
        Ok(_) => println!("  ✗ expected a failure"),
        Err(e) => {
            let msg = e.to_string();
            println!("  ✓ build fails when OxcacheModule is not registered (expected behavior)");
            println!("  error message: {}", msg);
            assert!(
                msg.contains("oxcache"),
                "the error should mention the oxcache dependency, got: {msg}"
            );
        }
    }

    // ============================================
    // 9. 多 Session 使用
    // ============================================
    println!("\n--- 9. multiple Sessions ---\n");

    let admin_session = pool.get_session("admin").await?;
    let system_session = pool.get_session("system").await?;
    println!("  ✓ admin session (role: {})", admin_session.role());
    println!("  ✓ system session (role: {})", system_session.role());

    let final_status = pool.status();
    println!(
        "\n  final pool status: total={}, active={}, idle={}",
        final_status.total, final_status.active, final_status.idle
    );

    println!("\n========================================");
    println!("✨ AsyncKit + DbNexusModule example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - AsyncKit::new()                    create a DI container");
    println!("  - kit.set_config(T)                  set a module configuration");
    println!("  - kit.register::<M>()                register a module");
    println!(
        "  - kit.build().await                  build (topological sort + dependency injection)"
    );
    println!("  - kit.require::<M>()                 acquire a module capability");
    println!("  - DbNexusModule              the AsyncKit module that builds the DbPool");
    println!("  - OxcacheModule              the AsyncKit module that builds the cache backend");
    println!("\n💡 Design notes:");
    println!("  - DbNexusModule depends on OxcacheModule (automatic topological sort)");
    println!("  - OxcacheDbCacheAdapter created and injected automatically inside build()");
    println!("  - returns an Arc<dyn ConnectionPool + Send + Sync> trait object");
    println!("  - build() returns a clear error when a dependency module is not registered");

    Ok(())
}
