// Copyright (c) 2026 Kirky.X
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
//! `DbNexusModule` 是 trait-kit 0.2.2 `AsyncKit` 集成模块，
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

use dbnexus::database::ConnectionPool;
use dbnexus::foundation::{DbConfig, PoolConfig};
use dbnexus::DbNexusModule;
use oxcache::integrations::kit::{OxcacheConfig, OxcacheModule};
use trait_kit::prelude::*;

// ============================================
// 主函数
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🧰 DBNexus AsyncKit + DbNexusModule 依赖注入示例");
    println!("========================================\n");

    // ============================================
    // 1. 创建 AsyncKit 并配置
    // ============================================
    println!("--- 1. 创建 AsyncKit 并设置配置 ---\n");

    let mut kit = AsyncKit::new();
    println!("  ✓ AsyncKit 创建成功");

    // 设置 OxcacheConfig（OxcacheModule 需要）
    kit.set_config(OxcacheConfig::default());
    println!("  ✓ OxcacheConfig 已设置（默认配置）");

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
    println!("  ✓ DbConfig 已设置（sqlite 共享内存, max_connections=5）");

    // ============================================
    // 2. 注册模块
    // ============================================
    println!("\n--- 2. 注册模块 ---\n");

    // OxcacheModule 必须先注册（DbNexusModule 依赖它）
    kit.register::<OxcacheModule>()
        .map_err(|e| format!("register OxcacheModule: {e}"))?;
    println!("  ✓ OxcacheModule 已注册（缓存模块）");

    kit.register::<DbNexusModule>()
        .map_err(|e| format!("register DbNexusModule: {e}"))?;
    println!("  ✓ DbNexusModule 已注册（数据库连接池模块）");
    println!("  依赖关系: DbNexusModule → OxcacheModule");

    // ============================================
    // 3. 构建 Kit 并获取连接池
    // ============================================
    println!("\n--- 3. 构建 Kit ---\n");

    let kit = kit.build().await.map_err(|e| format!("AsyncKit::build: {e}"))?;
    println!("  ✓ AsyncKit 构建成功");

    // 从 kit 中获取 DbNexusModule 的能力（Arc<dyn ConnectionPool + Send + Sync>）
    let pool: Arc<dyn ConnectionPool + Send + Sync> = kit
        .require::<DbNexusModule>()
        .map_err(|e| format!("require DbNexusModule: {e}"))?;
    println!("  ✓ 获取 Arc<dyn ConnectionPool> 成功");

    // ============================================
    // 4. 使用连接池
    // ============================================
    println!("\n--- 4. 使用连接池 ---\n");

    // 查看连接池状态
    let status = pool.status();
    println!(
        "  连接池状态: total={}, active={}, idle={}",
        status.total, status.active, status.idle
    );
    println!("  配置 URL: {}", pool.config().url);

    // 获取 Session 并执行 DDL
    let session = pool.get_session("admin").await?;
    println!("  ✓ 获取 Session 成功 (角色: {})", session.role());

    session
        .execute_raw_ddl(
            "CREATE TABLE IF NOT EXISTS kit_demo (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                created_at TEXT DEFAULT (datetime('now'))
            )",
        )
        .await?;
    println!("  ✓ 执行 DDL 成功: CREATE TABLE kit_demo");

    // ============================================
    // 5. INSERT 数据
    // ============================================
    println!("\n--- 5. INSERT 数据 ---\n");

    session
        .execute_raw("INSERT INTO kit_demo (id, name) VALUES (1, 'async-kit')")
        .await?;
    session
        .execute_raw("INSERT INTO kit_demo (id, name) VALUES (2, 'dbnexus-module')")
        .await?;
    session
        .execute_raw("INSERT INTO kit_demo (id, name) VALUES (3, 'oxcache-module')")
        .await?;
    println!("  ✓ 插入 3 行测试数据");

    // ============================================
    // 6. SELECT 查询
    // ============================================
    println!("\n--- 6. SELECT 查询 ---\n");

    session.execute_raw("SELECT id, name FROM kit_demo ORDER BY id").await?;
    println!("  ✓ 查询 kit_demo 表成功");

    // ============================================
    // 7. 事务操作
    // ============================================
    println!("\n--- 7. 事务操作 ---\n");

    session.begin_transaction().await?;
    println!("  ✓ 开始事务");

    session
        .execute_raw("INSERT INTO kit_demo (id, name) VALUES (4, 'transaction-test')")
        .await?;
    println!("  ✓ 事务内插入: transaction-test");

    println!("  is_in_transaction() = {}", session.is_in_transaction().await);

    session.commit().await?;
    println!("  ✓ 事务已提交");

    // ============================================
    // 8. 错误处理演示
    // ============================================
    println!("\n--- 8. 错误处理 ---\n");

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
        Ok(_) => println!("  ✗ 预期应该失败"),
        Err(e) => {
            let msg = e.to_string();
            println!("  ✓ 未注册 OxcacheModule 时 build 失败（预期行为）");
            println!("  错误信息: {}", msg);
            assert!(msg.contains("oxcache"), "错误应提及 oxcache 依赖, got: {msg}");
        }
    }

    // ============================================
    // 9. 多 Session 使用
    // ============================================
    println!("\n--- 9. 多 Session ---\n");

    let admin_session = pool.get_session("admin").await?;
    let system_session = pool.get_session("system").await?;
    println!("  ✓ admin session (role: {})", admin_session.role());
    println!("  ✓ system session (role: {})", system_session.role());

    let final_status = pool.status();
    println!(
        "\n  最终连接池状态: total={}, active={}, idle={}",
        final_status.total, final_status.active, final_status.idle
    );

    println!("\n========================================");
    println!("✨ AsyncKit + DbNexusModule 示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - AsyncKit::new()                    创建 DI 容器");
    println!("  - kit.set_config(T)                  设置模块配置");
    println!("  - kit.register::<M>()                注册模块");
    println!("  - kit.build().await                  构建（拓扑排序 + 依赖注入）");
    println!("  - kit.require::<M>()                 获取模块能力");
    println!("  - DbNexusModule                      构建 DbPool 的 AsyncKit 模块");
    println!("  - OxcacheModule                      构建缓存后端的 AsyncKit 模块");
    println!("\n💡 设计要点:");
    println!("  - DbNexusModule 依赖 OxcacheModule（自动拓扑排序）");
    println!("  - OxcacheDbCacheAdapter 在 build() 内自动创建并注入");
    println!("  - 返回 Arc<dyn ConnectionPool + Send + Sync> trait 对象");
    println!("  - 未注册依赖模块时 build() 返回清晰错误");

    Ok(())
}
