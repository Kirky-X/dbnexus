// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in the project root for full license information.

//! DbNexusKit 高级示例：同时注册 pool + permission + metrics 三个能力
//!
//! 本示例在 [`kit_usage`] 基础上演示更复杂的 Kit 组合场景：
//! - 同时注册 `ConnectionPool`、`PermissionProvider`、`MetricsCollector` 三种能力
//! - 通过 Kit 统一发现并组合使用三者（执行 SQL 前先做权限检查，并记录查询指标）
//! - 演示 `MetricsCollectorTrait` trait 对象注册（`Arc<dyn MetricsCollectorTrait>`）
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example kit_advanced --features "sqlite,permission,permission-engine,metrics,macros"
//! ```

#[path = "../common/mod.rs"]
mod common;

use std::sync::Arc;

use async_trait::async_trait;
use dbnexus::database::pool::PoolStatus;
use dbnexus::domain::permission::{self, PermissionAction};
use dbnexus::foundation::error::DbResult;
use dbnexus::{
    ConnectionPool, DatabaseSession, DbConfig, DbNexusKit, DbPool, MetricsCollector,
    MetricsCollectorTrait, PermissionProvider, Session,
};

// ============================================
// 适配器：DbPool → ConnectionPool trait
// ============================================

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
// 适配器：Session → DatabaseSession trait
// ============================================

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
    println!("🧰 DBNexus DbNexusKit 高级示例：pool + permission + metrics 三能力组合");
    println!("========================================\n");

    // ============================================
    // 1. 创建 DbNexusKit
    // ============================================
    let kit = DbNexusKit::new();
    println!("--- 1. 创建空 DbNexusKit ---");
    println!(
        "  初始状态: pool={}, permission={}, metrics={}",
        kit.has_connection_pool(),
        kit.has_permission(),
        kit.has_metrics_collector(),
    );

    // ============================================
    // 2. 注册 ConnectionPool 能力
    // ============================================
    println!("\n--- 2. 注册 ConnectionPool 能力 ---");
    let (db_pool_inner, session) = common::db::setup_shared_sqlite_session().await?;
    let db_pool = Arc::new(db_pool_inner);
    let pool_adapter = Arc::new(PoolAdapter { inner: db_pool });
    kit.provide_connection_pool(pool_adapter)?;
    println!("  ✓ ConnectionPool 已注册（max_connections=5）");

    // 准备测试表
    session
        .execute_raw_ddl(
            "CREATE TABLE IF NOT EXISTS orders (
                id INTEGER PRIMARY KEY,
                user_id INTEGER NOT NULL,
                amount REAL NOT NULL
            )",
        )
        .await?;
    session
        .execute_raw("INSERT INTO orders (id, user_id, amount) VALUES (1, 42, 99.5)")
        .await?;
    session
        .execute_raw("INSERT INTO orders (id, user_id, amount) VALUES (2, 42, 12.3)")
        .await?;
    session
        .execute_raw("INSERT INTO orders (id, user_id, amount) VALUES (3, 7, 50.0)")
        .await?;
    println!("  ✓ 已准备 orders 测试数据（3 行）");

    // 注册 DatabaseSession 能力
    let session_adapter = Arc::new(SessionAdapter { inner: session });
    kit.provide_database_session(session_adapter)?;
    println!("  ✓ DatabaseSession 已注册");

    // ============================================
    // 3. 注册 Permission 能力（内存实现）
    // ============================================
    println!("\n--- 3. 注册 Permission 能力 ---");
    let perm_provider: Arc<dyn PermissionProvider> = Arc::new(permission::new_in_memory());
    kit.provide_permission(perm_provider)?;
    println!("  ✓ Permission 已注册");
    println!("  has_permission() = {}", kit.has_permission());

    // ============================================
    // 4. 注册 Metrics 能力（通过 trait 对象）
    // ============================================
    println!("\n--- 4. 注册 MetricsCollector 能力 ---");
    let metrics: Arc<dyn MetricsCollectorTrait> = Arc::new(MetricsCollector::new());
    kit.provide_metrics_collector(metrics)?;
    println!("  ✓ MetricsCollector 已注册");
    println!("  has_metrics_collector() = {}", kit.has_metrics_collector());

    // ============================================
    // 5. 通过 Kit 组合使用三种能力
    //    业务流程：权限检查 → SQL 执行 → 指标记录
    // ============================================
    println!("\n--- 5. 组合使用：权限检查 → SQL 执行 → 指标记录 ---\n");

    let pool = kit.connection_pool()?;
    let permission = kit.permission()?;
    let metrics_collector = kit.metrics_collector()?;

    // 同步连接池状态到指标
    let status = pool.status();
    metrics_collector.record_pool_usage(status.total as u32, status.active as u32, status.idle as u32);

    let test_role = "admin";
    let test_table = "orders";

    // 模拟 3 次查询请求，每次先做权限检查
    for i in 1..=3 {
        let start = std::time::Instant::now();
        let allowed = permission.check(test_role, test_table, PermissionAction::Select).await?;
        let duration = start.elapsed();

        if allowed {
            // 通过 DatabaseSession 执行 SQL
            let db_session = kit.database_session()?;
            let _ = db_session
                .execute_raw("SELECT COUNT(*) FROM orders")
                .await?;
            metrics_collector.record_query(duration);
            println!("  请求 #{}: ✓ 权限通过，查询完成，耗时 {:?}", i, duration);
        } else {
            println!("  请求 #{}: ✗ 权限被拒绝，耗时 {:?}", i, duration);
        }
    }

    // ============================================
    // 6. 查看汇总指标
    // ============================================
    println!("\n--- 6. 汇总指标 ---");
    let pool_metrics = metrics_collector.pool_metrics();
    println!(
        "  连接池: total={}, active={}, idle={}",
        pool_metrics.total, pool_metrics.active, pool_metrics.idle
    );
    let stats = metrics_collector.query_stats();
    println!(
        "  查询: count={}, errors={}, p50={:?}, p99={:?}",
        stats.count,
        stats.error_count,
        stats.latency_percentiles.p50(),
        stats.latency_percentiles.p99(),
    );

    // ============================================
    // 7. 通过 trait 对象验证跨 Kit 一致性
    // ============================================
    println!("\n--- 7. trait 对象验证 ---");
    let cloned_kit = kit.clone();
    let metrics_again = cloned_kit.metrics_collector()?;
    let pool_metrics_again = metrics_again.pool_metrics();
    println!(
        "  克隆 kit 后 pool_metrics.total = {} (应等于 {})",
        pool_metrics_again.total, pool_metrics.total
    );

    println!("\n========================================");
    println!("✨ DbNexusKit 高级示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - DbNexusKit 同时注册多种能力，业务层通过单一入口获取");
    println!("  - ConnectionPool + DatabaseSession + Permission + Metrics 四能力组合");
    println!("  - MetricsCollector 通过 MetricsCollectorTrait trait 对象注册");
    println!("  - kit.clone() 共享底层注册项，适用于跨任务共享");
    println!("\n💡 业务编排模式:");
    println!("  permission.check(role, table, action).await?");
    println!("    → db_session.execute_raw(sql).await?");
    println!("    → metrics.record_query(type, dur, ok, bytes)");

    Ok(())
}
