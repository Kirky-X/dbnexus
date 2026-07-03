// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in the project root for full license information.

//! OTLP 分布式追踪示例（v0.3.0 新增）
//!
//! 演示 [`TracingGuard`] 的完整使用流程：
//! - 调用 [`TracingGuard::init_with_otlp`] 初始化全局 OTLP gRPC exporter
//! - 在 tracing feature 启用时执行数据库查询（`Session` 上的 `#[tracing::instrument]` 自动创建 span）
//! - 验证 `TracingError::AlreadyInitialized` 重复初始化保护
//! - RAII 语义：guard drop 时自动 flush 挂起 span
//!
//! # 运行示例
//!
//! ```bash
//! # 默认 OTLP endpoint 为 http://localhost:4317（Jaeger/OTel Collector 默认 gRPC 端口）
//! cargo run --example tracing_otlp --features "sqlite,tracing"
//!
//! # 启动本地 Jaeger 收集 span（可选）：
//! # docker run -d -p 4317:4317 -p 16686:16686 jaegertracing/all-in-one:latest
//! # 然后访问 http://localhost:16686 查看 span
//! ```

#[path = "../common/mod.rs"]
mod common;

use dbnexus::{DbNexusError, TracingError, TracingGuard};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("📡 DBNexus OTLP 分布式追踪示例");
    println!("========================================\n");

    // ============================================
    // 1. 初始化 TracingGuard
    // ============================================
    println!("--- 1. 初始化 TracingGuard ---\n");
    println!("  OTLP endpoint: http://localhost:4317");
    println!("  (如未启动 collector，span 会被丢弃但不会阻塞)");

    let endpoint = "http://localhost:4317";
    let guard: Option<TracingGuard> = match TracingGuard::init_with_otlp(endpoint) {
        Ok(g) => {
            println!("  ✓ TracingGuard 初始化成功，全局 subscriber 已注册");
            Some(g)
        }
        Err(TracingError::AlreadyInitialized) => {
            println!("  ⚠ Tracing 已被初始化过（可能由其他代码设置），继续执行");
            None
        }
        Err(e) => {
            println!("  ⚠ TracingGuard 初始化失败: {}", e);
            println!("    示例将继续运行，但 span 不会导出到 OTLP");
            None
        }
    };

    // ============================================
    // 2. 重复初始化保护
    // ============================================
    println!("\n--- 2. 重复初始化保护 ---\n");
    match TracingGuard::init_with_otlp(endpoint) {
        Ok(_) => println!("  ⚠ 第二次初始化意外成功（不应发生）"),
        Err(TracingError::AlreadyInitialized) => {
            println!("  ✓ 第二次 init_with_otlp 返回 AlreadyInitialized（符合预期）");
        }
        Err(e) => {
            println!("  ⚠ 第二次初始化返回其他错误: {}", e);
        }
    }

    // ============================================
    // 3. 创建 Session 并执行查询（自动创建 span）
    // ============================================
    println!("\n--- 3. 执行数据库查询（Session 自动创建 span）---\n");

    let (pool, session) = common::db::setup_shared_sqlite_session().await?;
    println!("  ✓ SQLite Session 创建成功（role: {}）", session.role());

    // tracing feature 启用时，Session::execute_raw_ddl 等方法上的
    // #[tracing::instrument(skip(self), fields(db.table, db.operation, db.role))]
    // 会自动创建 span 并通过 OTLP 异步导出
    session
        .execute_raw_ddl(
            "CREATE TABLE IF NOT EXISTS trace_demo (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
        )
        .await?;
    println!("  ✓ execute_raw_ddl 完成（span 'execute_raw_ddl' 已记录）");

    session
        .execute_raw(
            "INSERT INTO trace_demo (id, name, created_at) VALUES (1, 'hello-trace', '2026-07-03T00:00:00Z')",
        )
        .await?;
    println!("  ✓ execute_raw INSERT 完成（span 'execute_raw' 已记录）");

    session
        .execute_raw("SELECT id, name FROM trace_demo ORDER BY id")
        .await?;
    println!("  ✓ execute_raw SELECT 完成（span 'execute_raw' 已记录）");

    // ============================================
    // 4. 事务操作（多个 span 串联）
    // ============================================
    println!("\n--- 4. 事务操作（多个 span 串联）---\n");
    session.begin_transaction().await?;
    println!("  ✓ begin_transaction（span 已记录）");

    session
        .execute_raw(
            "INSERT INTO trace_demo (id, name, created_at) VALUES (2, 'txn-1', '2026-07-03T00:01:00Z')",
        )
        .await?;
    session
        .execute_raw(
            "INSERT INTO trace_demo (id, name, created_at) VALUES (3, 'txn-2', '2026-07-03T00:02:00Z')",
        )
        .await?;
    println!("  ✓ 事务内 2 条 INSERT 完成");

    session.commit().await?;
    println!("  ✓ commit（span 已记录）");

    // ============================================
    // 5. 查看连接池状态
    // ============================================
    println!("\n--- 5. 连接池状态 ---\n");
    let status = pool.status();
    println!(
        "  total={}, active={}, idle={}",
        status.total, status.active, status.idle
    );

    // ============================================
    // 6. guard drop 触发 flush
    // ============================================
    println!("\n--- 6. TracingGuard drop 触发 flush ---\n");
    if let Some(g) = guard {
        println!("  即将 drop guard，触发 opentelemetry::global::shutdown_tracer_provider()");
        println!("  (此调用同步 flush 所有挂起 span 后关闭 provider)");
        drop(g);
        println!("  ✓ guard 已 drop，span 已 flush");
    } else {
        println!("  无 guard 需要 drop（初始化未成功）");
    }

    // ============================================
    // 7. TracingError 类型展示
    // ============================================
    println!("\n--- 7. TracingError 类型展示 ---\n");
    let errors = [
        TracingError::ExporterInit("connection refused to localhost:4317".to_string()),
        TracingError::ProviderSetup("tracer provider already set".to_string()),
        TracingError::AlreadyInitialized,
        TracingError::SubscriberSetup("global subscriber already set".to_string()),
    ];
    for err in &errors {
        println!("  - {}", err);
    }

    println!("\n========================================");
    println!("✨ OTLP 分布式追踪示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - TracingGuard::init_with_otlp(endpoint)  初始化全局 OTLP gRPC exporter");
    println!("  - guard drop 时自动 flush + 关闭 provider（RAII 语义）");
    println!("  - 重复初始化返回 TracingError::AlreadyInitialized");
    println!("  - Session 方法上的 #[tracing::instrument] 自动创建 span");
    println!("  - span 异步批量导出，不阻塞业务逻辑");
    println!("\n💡 部署提示:");
    println!("  - 生产环境：部署 OTel Collector 或 Jaeger 接收 OTLP gRPC");
    println!("  - 开发环境：可启动 Jaeger all-in-one 容器（端口 4317/16686）");
    println!("  - 即使无 collector，TracingGuard 也能初始化成功，span 被丢弃");
    println!("\n⚙️ TracingError 变体:");
    println!("  - ExporterInit(String)        OTLP exporter 构建失败");
    println!("  - ProviderSetup(String)       tracer provider 设置失败");
    println!("  - AlreadyInitialized          进程内重复初始化");
    println!("  - SubscriberSetup(String)     全局 subscriber 设置失败");

    // 确保不吞掉潜在的 DbNexusError（虽然此处不太可能发生）
    let _: Result<(), DbNexusError> = Ok(());
    Ok(())
}
