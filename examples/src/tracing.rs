// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! OpenTelemetry 分布式追踪示例
//!
//! 展示如何使用 dbnexus 的分布式追踪功能：
//! - 初始化 OpenTelemetry 追踪
//! - 配置 OTLP 导出器
//! - 使用 tracing instrument
//! - 导出追踪数据到 Jaeger
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example tracing --features "sqlite,tracing"
//! ```

use dbnexus::{DbConfig, DbPool};
use dbnexus::tracing::{extract, inject};
use std::collections::HashMap;
use tracing::{error, info, instrument};
use tracing_subscriber::{EnvFilter, prelude::*};

/// 定义 User 结构体（用于演示追踪）
#[derive(Debug, Clone, PartialEq)]
struct User {
    id: i64,
    name: String,
    email: String,
}

/// 定义 Order 结构体（用于演示跨服务追踪）
#[derive(Debug, Clone, PartialEq)]
struct Order {
    id: i64,
    user_id: i64,
    amount: f64,
    status: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 DBNexus OpenTelemetry 分布式追踪示例\n");
    println!("========================================");

    // 1. 初始化 OpenTelemetry 追踪
    println!("\n1️⃣ 初始化 OpenTelemetry 追踪");
    println!("------------------------------------------");

    // 使用 stdout 导出器（用于演示）
    let _tracing_guard = dbnexus::tracing::init("stdout", "unused").await?;
    println!("✓ OpenTelemetry 追踪初始化成功");

    // 初始化 tracing subscriber
    tracing_subscriber::registry()
        .with(EnvFilter::new("dbnexus=debug,info"))
        .with(tracing_opentelemetry::layer())
        .try_init()?;

    println!("✓ Tracing subscriber 初始化成功");

    // 2. 初始化数据库连接池
    println!("\n2️⃣ 初始化数据库连接池");
    println!("------------------------------------------");
    let config = DbConfigBuilder::new()
        .url("sqlite:file::memory:?cache=shared")
        .permissions_path("src/permissions.yaml")
        .admin_role("admin")
        .build()
        .expect("Failed to build config");
    let pool = DbPool::with_config(config).await?;
    println!("✓ 连接池创建成功");

    // 3. 创建测试数据
    println!("\n3️⃣ 创建测试数据");
    println!("------------------------------------------");

    let session = pool.get_session("admin").await?;

    // 创建表
    session
        .execute_raw_ddl(
            "CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                email TEXT NOT NULL
            )",
        )
        .await?;

    session
        .execute_raw_ddl(
            "CREATE TABLE orders (
                id INTEGER PRIMARY KEY,
                user_id INTEGER NOT NULL,
                amount REAL NOT NULL,
                status TEXT NOT NULL
            )",
        )
        .await?;

    // 插入用户
    session
        .execute_raw("INSERT INTO users (id, name, email) VALUES (1, 'Alice', 'alice@example.com')")
        .await?;
    println!("  ✓ 创建用户: Alice");

    session
        .execute_raw("INSERT INTO users (id, name, email) VALUES (2, 'Bob', 'bob@example.com')")
        .await?;
    println!("  ✓ 创建用户: Bob");

    // 4. 使用 instrument 宏自动追踪
    println!("\n4️⃣ 使用 instrument 宏自动追踪");
    println!("------------------------------------------");

    create_order_with_instrument(&session, 1, 99.99).await?;
    create_order_with_instrument(&session, 2, 149.99).await?;

    // 5. 演示跨服务追踪
    println!("\n5️⃣ 演示跨服务追踪");
    println!("------------------------------------------");

    let mut headers = HashMap::new();

    // 在服务 A 中注入追踪上下文
    inject(&mut headers);
    println!("  ✓ 注入追踪上下文到 headers");
    println!("  📋 Headers: {:?}", headers);

    // 在服务 B 中提取追踪上下文
    extract(&headers);
    println!("  ✓ 从 headers 提取追踪上下文");

    // 模拟服务 B 的操作
    process_order(&session, 1).await?;

    // 6. 演示错误追踪
    println!("\n6️⃣ 演示错误追踪");
    println!("------------------------------------------");

    error!("模拟一个错误: 用户不存在");

    // 7. 演示性能追踪
    println!("\n7️⃣ 演示性能追踪");
    println!("------------------------------------------");

    let start = std::time::Instant::now();

    for i in 1..=10 {
        let _ = session
            .execute_raw(&format!("SELECT * FROM users WHERE id = {}", i))
            .await?;
    }

    let duration = start.elapsed();

    info!("查询 10 个用户耗时: {:?}", duration);

    // 8. 演示 OTLP 导出器配置
    println!("\n8️⃣ 演示 OTLP 导出器配置");
    println!("------------------------------------------");

    println!("  💡 OTLP 导出器配置:");
    println!("     - 导出器类型: otlp");
    println!("     - 端点: http://localhost:4317");
    println!("     - 协议: gRPC");
    println!("     - 服务名: dbnexus");

    println!("\n  📝 初始化代码:");
    println!("     let guard = dbnexus::tracing::init(\"otlp\", \"http://localhost:4317\").await?;");

    // 9. 演示 Jaeger 集成
    println!("\n9️⃣ 演示 Jaeger 集成");
    println!("------------------------------------------");

    println!("  💡 Jaeger 集成步骤:");
    println!("     1. 启动 Jaeger:");
    println!("        docker run -d -p 16686:16686 -p 4317:4317 jaegertracing/all-in-one:latest");
    println!("     2. 配置 OTLP 导出器指向 Jaeger");
    println!("     3. 在 Jaeger UI (http://localhost:16686) 查看追踪数据");

    println!("\n  📊 Jaeger UI 功能:");
    println!("     - 查看分布式追踪图");
    println!("     - 分析性能瓶颈");
    println!("     - 查看错误和异常");
    println!("     - 搜索和过滤追踪");

    // 10. 演示追踪最佳实践
    println!("\n🔟 追踪最佳实践");
    println!("------------------------------------------");

    println!("  💡 最佳实践:");
    println!("     1. 为关键操作创建 span");
    println!("     2. 添加有意义的属性");
    println!("     3. 使用 instrument 宏自动追踪");
    println!("     4. 记录错误和异常");
    println!("     5. 保持 span 层级清晰");
    println!("     6. 合理设置采样率");

    println!("\n  📝 常用属性:");
    println!("     - operation: 操作名称");
    println!("     - user.id: 用户 ID");
    println!("     - db.table: 数据库表名");
    println!("     - db.query: SQL 查询语句");
    println!("     - error.type: 错误类型");
    println!("     - error.message: 错误消息");

    println!("\n========================================");
    println!("✨ OpenTelemetry 分布式追踪示例运行完成！");

    println!("\n💡 提示:");
    println!("  - 在生产环境中使用 OTLP 导出器");
    println!("  - 配置适当的采样率以控制追踪数据量");
    println!("  - 使用 Jaeger 或 Grafana Tempo 可视化追踪数据");
    println!("  - 定期分析追踪数据以发现性能问题");
    println!("  - 为关键路径添加追踪 span");

    Ok(())
}

/// 使用 instrument 宏的函数
#[instrument(skip(session))]
async fn create_order_with_instrument(
    session: &dbnexus::Session,
    user_id: i64,
    amount: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("创建订单: user_id={}, amount={}", user_id, amount);

    session
        .execute_raw(&format!(
            "INSERT INTO orders (id, user_id, amount, status) VALUES ({}, {}, {}, 'pending')",
            user_id * 100,
            user_id,
            amount
        ))
        .await?;

    info!("订单创建成功");

    Ok(())
}

/// 处理订单的函数
#[instrument(skip(session))]
async fn process_order(session: &dbnexus::Session, order_id: i64) -> Result<(), Box<dyn std::error::Error>> {
    info!("处理订单: order_id={}", order_id);

    let _result = session
        .execute_raw(&format!("SELECT * FROM orders WHERE id = {}", order_id))
        .await?;

    session
        .execute_raw(&format!(
            "UPDATE orders SET status = 'processed' WHERE id = {}",
            order_id
        ))
        .await?;

    info!("订单处理完成");

    Ok(())
}
