// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! DbNexusKit 统一能力管理示例
//!
//! 演示如何使用 [`DbNexusKit`] 进行模块能力的注册、发现和替换：
//! - 通过 `provide_*` 注册连接池、数据库会话、权限提供者等能力
//! - 通过 `has_*` 检查能力是否已注册
//! - 通过 `connection_pool()` / `database_session()` / `permission()` 获取能力
//! - 通过 `replace_*` 热替换已注册的能力
//! - 展示未注册能力获取时的错误处理
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example kit_usage --features "sqlite,permission,macros"
//! ```

#[path = "../common/mod.rs"]
mod common;

use std::sync::Arc;

use async_trait::async_trait;
use dbnexus::database::PoolStatus;
use dbnexus::domain::permission;
use dbnexus::foundation::DbResult;
use dbnexus::{ConnectionPool, DatabaseSession, DbConfig, DbNexusKit, DbPool, PermissionProvider, Session};

// ============================================
// 适配器：将 DbPool 适配为 ConnectionPool trait
// ============================================

/// DbPool → ConnectionPool 适配器
///
/// `DbPool` 拥有与 `ConnectionPool` trait 相同的方法签名，但未直接实现该 trait。
/// 通过此适配器，可将真实的 `DbPool` 注册到 `DbNexusKit` 中。
struct PoolAdapter {
    inner: Arc<DbPool>,
}

#[async_trait]
impl ConnectionPool for PoolAdapter {
    async fn get_session(&self, role: &str) -> DbResult<Session> {
        self.inner.get_session(role).await
    }

    fn status(&self) -> PoolStatus {
        self.inner.status()
    }

    fn config(&self) -> &DbConfig {
        self.inner.config()
    }
}

// ============================================
// 适配器：将 Session 适配为 DatabaseSession trait
// ============================================

/// Session → DatabaseSession 适配器
///
/// `Session` 拥有与 `DatabaseSession` trait 相同的方法签名，但未直接实现该 trait。
/// 通过此适配器，可将真实的 `Session` 注册到 `DbNexusKit` 中。
struct SessionAdapter {
    inner: Session,
}

#[async_trait]
impl DatabaseSession for SessionAdapter {
    async fn execute(&self, sql: &str) -> DbResult<sea_orm::ExecResult> {
        self.inner.execute(sql).await
    }

    async fn execute_raw(&self, sql: &str) -> DbResult<sea_orm::ExecResult> {
        self.inner.execute_raw(sql).await
    }

    async fn execute_raw_ddl(&self, sql: &str) -> DbResult<sea_orm::ExecResult> {
        self.inner.execute_raw_ddl(sql).await
    }

    async fn begin_transaction(&self) -> DbResult<()> {
        self.inner.begin_transaction().await
    }

    async fn commit(&self) -> DbResult<()> {
        self.inner.commit().await
    }

    async fn rollback(&self) -> DbResult<()> {
        self.inner.rollback().await
    }

    fn role(&self) -> &str {
        self.inner.role()
    }

    async fn is_in_transaction(&self) -> bool {
        self.inner.is_in_transaction().await
    }
}

// ============================================
// 主函数
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🧰 DBNexus DbNexusKit 统一能力管理示例");
    println!("========================================\n");

    // ============================================
    // 1. 创建空的 DbNexusKit
    // ============================================
    println!("--- 1. 创建空的 DbNexusKit ---\n");
    let kit = DbNexusKit::new();
    println!("  ✓ DbNexusKit 创建成功");
    println!("  has_connection_pool()  = {}", kit.has_connection_pool());
    println!("  has_database_session() = {}", kit.has_database_session());
    println!("  has_permission()       = {}", kit.has_permission());

    // ============================================
    // 2. 创建真实 DbPool 并注册为 ConnectionPool 能力
    // ============================================
    println!("\n--- 2. 注册 ConnectionPool 能力 ---\n");
    let (db_pool_inner, _) = common::db::setup_shared_sqlite_session().await?;
    let db_pool = Arc::new(db_pool_inner);
    println!("  ✓ DbPool 创建成功");

    let pool_adapter = Arc::new(PoolAdapter { inner: db_pool });
    kit.provide_connection_pool(pool_adapter)?;
    println!("  ✓ ConnectionPool 能力已注册");
    println!("  has_connection_pool()  = {}", kit.has_connection_pool());

    // ============================================
    // 3. 获取 ConnectionPool 能力并使用
    // ============================================
    println!("\n--- 3. 获取并使用 ConnectionPool 能力 ---\n");
    let pool = kit.connection_pool()?;
    let status = pool.status();
    println!("  ✓ 获取 ConnectionPool 成功");
    println!(
        "  连接池状态: total={}, active={}, idle={}",
        status.total, status.active, status.idle
    );
    println!("  配置 URL:   {}", pool.config().url);

    // 通过 ConnectionPool trait 获取 Session
    let session = pool.get_session("admin").await?;
    println!(
        "  ✓ 通过 ConnectionPool trait 获取 Session 成功 (角色: {})",
        session.role()
    );

    // ============================================
    // 4. 注册 DatabaseSession 能力
    // ============================================
    println!("\n--- 4. 注册 DatabaseSession 能力 ---\n");
    let session_adapter = Arc::new(SessionAdapter { inner: session });
    kit.provide_database_session(session_adapter)?;
    println!("  ✓ DatabaseSession 能力已注册");
    println!("  has_database_session() = {}", kit.has_database_session());

    // ============================================
    // 5. 获取 DatabaseSession 能力并执行 SQL
    // ============================================
    println!("\n--- 5. 获取并使用 DatabaseSession 能力 ---\n");
    let db_session = kit.database_session()?;
    println!("  ✓ 获取 DatabaseSession 成功 (角色: {})", db_session.role());

    db_session
        .execute_raw_ddl(
            "CREATE TABLE IF NOT EXISTS kit_demo (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL
            )",
        )
        .await?;
    println!("  ✓ 通过 DatabaseSession trait 执行 DDL 成功");

    db_session
        .execute_raw("INSERT INTO kit_demo (id, name) VALUES (1, 'hello-kit')")
        .await?;
    println!("  ✓ 通过 DatabaseSession trait 执行 INSERT 成功");

    println!("  is_in_transaction() = {}", db_session.is_in_transaction().await);

    // ============================================
    // 6. 注册 Permission 能力
    // ============================================
    println!("\n--- 6. 注册 Permission 能力 ---\n");
    let perm_provider: Arc<dyn PermissionProvider> = Arc::new(permission::new_in_memory());
    kit.provide_permission(perm_provider)?;
    println!("  ✓ Permission 能力已注册");
    println!("  has_permission()       = {}", kit.has_permission());

    let _perm = kit.permission()?;
    println!("  ✓ 获取 Permission 能力成功");

    // ============================================
    // 7. 错误处理：获取未注册的能力
    // ============================================
    println!("\n--- 7. 错误处理：获取未注册的能力 ---\n");

    // 创建一个新 kit，不注册任何能力
    let empty_kit = DbNexusKit::new();
    let err = empty_kit.connection_pool();
    println!("  未注册时 connection_pool() 返回: {:?}", err.err());

    let err = empty_kit.database_session();
    println!("  未注册时 database_session() 返回: {:?}", err.err());

    // ============================================
    // 8. 热替换能力 (replace_*)
    // ============================================
    println!("\n--- 8. 热替换 ConnectionPool 能力 ---\n");
    let config2 = DbConfig {
        url: "sqlite:file::memory:?cache=shared".to_string(),
        admin_role: "admin".to_string(),
        max_connections: 10,
        min_connections: 2,
        ..Default::default()
    };
    let db_pool2 = Arc::new(DbPool::with_config(config2).await?);
    let pool_adapter2 = Arc::new(PoolAdapter { inner: db_pool2 });
    kit.replace_connection_pool(pool_adapter2);
    println!("  ✓ ConnectionPool 已热替换 (max_connections: 5 → 10)");

    let new_pool = kit.connection_pool()?;
    println!("  替换后 max_connections = {}", new_pool.config().max_connections);

    // ============================================
    // 9. Clone 与 Debug
    // ============================================
    println!("\n--- 9. Clone 与 Debug 行为 ---\n");
    let cloned_kit = kit.clone();
    println!("  ✓ kit.clone() 成功");
    println!(
        "  克隆 kit has_connection_pool()  = {}",
        cloned_kit.has_connection_pool()
    );
    println!(
        "  克隆 kit has_database_session() = {}",
        cloned_kit.has_database_session()
    );
    println!("  克隆 kit has_permission()       = {}", cloned_kit.has_permission());
    println!("  Debug 格式: {:?}", kit);

    // ============================================
    // 10. as_inner / into_inner 访问底层 Kit
    // ============================================
    println!("\n--- 10. 底层 Kit 访问 ---\n");
    let _inner_ref = kit.as_inner();
    println!("  ✓ as_inner() 获取底层 &Kit 引用成功");

    // 注意：into_inner 会消费 kit，放最后执行
    let _inner = kit.into_inner();
    println!("  ✓ into_inner() 消费 kit 并返回底层 Kit 成功");

    println!("\n========================================");
    println!("✨ DbNexusKit 统一能力管理示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - DbNexusKit::new()                    创建空 kit");
    println!("  - kit.provide_connection_pool(pool)    注册连接池能力");
    println!("  - kit.connection_pool()                获取连接池能力");
    println!("  - kit.has_connection_pool()            检查能力是否已注册");
    println!("  - kit.replace_connection_pool(pool)    热替换能力");
    println!("  - kit.provide_database_session(sess)   注册数据库会话能力");
    println!("  - kit.provide_permission(provider)     注册权限提供者能力");
    println!("  - kit.clone()                          克隆 kit（共享注册项）");
    println!("\n💡 设计要点:");
    println!("  - DbNexusKit 基于 trait-kit，提供类型安全的能力注册和发现");
    println!("  - 每个能力通过 CapabilityKey 标识，编译期类型检查");
    println!("  - replace_* 支持运行时热替换，适用于动态重新配置场景");
    println!("  - 未注册能力返回 TraitKitError，调用方需显式处理");

    Ok(())
}
