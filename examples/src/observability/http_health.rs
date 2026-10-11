// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! HTTP 健康端点示例
//!
//! 演示 `HealthRouterBuilder` 生成挂载三个标准健康端点的 axum Router：
//! - `GET /healthz`：liveness（进程存活标记，恒 200）
//! - `GET /readyz`：readiness（池快照 + 熔断器判定，healthy/degraded → 200）
//! - `GET /metrics`：Prometheus 抓取（注入 MetricsCollector 后输出文本指标）
//!
//! 生成器只产出 Router，监听/优雅停机由消费方编排（本例用 `axum::serve`
//! 起本地端口演示探测输出）。
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --bin http_health
//! ```
//!
//! 另开终端探测：
//!
//! ```bash
//! curl -s localhost:8090/healthz
//! curl -s localhost:8090/readyz
//! curl -s localhost:8090/metrics | head
//! ```

use std::sync::Arc;
use std::time::Duration;

use dbnexus::database::DbPool;
use dbnexus::integrations::http_health::HealthRouterBuilder;
use dbnexus::observability::MetricsCollector;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🌐 DBNexus HTTP health endpoint example");
    println!("========================================\n");

    // 池数据源：sqlite 临时库并预热一条查询（total > 0 → healthy）
    let pool = Arc::new(DbPool::new("sqlite::memory:").await?);
    let session = pool.get_session("admin").await?;
    session.execute_raw("SELECT 1").await?;
    println!("✓ the pool is ready (preheat query completed)");

    // 可选数据源：metrics 采集器（/metrics 输出 Prometheus 文本）
    let collector = Arc::new(MetricsCollector::new());
    pool.set_metrics_collector(Some(collector.clone())).await;

    // 生成 Router：三端点一次装配；服务生命周期由消费方编排
    let app = HealthRouterBuilder::new(pool)
        .with_metrics_collector(collector)
        .build();
    println!("✓ Router generated: /healthz /readyz /metrics");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8090").await?;
    println!("\nListening on http://127.0.0.1:8090 — exits automatically after 10 seconds\n");
    println!("  curl -s localhost:8090/healthz   # liveness (always 200)");
    println!("  curl -s localhost:8090/readyz    # readiness (pool snapshot verdict)");
    println!("  curl -s localhost:8090/metrics   # Prometheus text\n");

    // 演示进程：后台限时退出，期间可直接探测
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(10)).await;
        println!("\n(the example is limited to 10 seconds; it has exited)");
        std::process::exit(0);
    });

    axum::serve(listener, app).await?;
    Ok(())
}
