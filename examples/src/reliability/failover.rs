// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 连接故障转移示例
//!
//! 演示 `failover` 功能的配置与使用：
//! - `FailoverConfig` 配置故障转移链
//! - 与 `DbConfig` 集成
//! - CircuitBreaker + HealthCheck 协同机制
//! - 故障转移流程与阈值配置
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --bin failover
//! ```

use dbnexus::{DbConfig, FailoverConfig};

fn main() {
    println!("========================================");
    println!("🔌 DBNexus connection failover example");
    println!("========================================\n");

    // ============================================
    // FailoverConfig 基础配置
    // ============================================
    println!("--- 1. FailoverConfig basic configuration ---\n");

    let failover_config = FailoverConfig {
        urls: vec![
            "postgres://primary:5432/mydb".to_string(),
            "postgres://standby1:5432/mydb".to_string(),
            "postgres://standby2:5432/mydb".to_string(),
        ],
        health_check_query: Some("SELECT 1".to_string()),
        failover_threshold: 3,
    };

    println!("  failover chain:");
    for (i, url) in failover_config.urls.iter().enumerate() {
        let role = match i {
            0 => "primary (primary)",
            1 => "standby1 (standby 1)",
            2 => "standby2 (standby 2)",
            _ => "unknown",
        };
        println!("  [{}] {} : {}", i, role, url);
    }
    println!();
    println!(
        "  - health check SQL: {:?}",
        failover_config.health_check_query
    );
    println!(
        "  - trigger threshold     : switch after {} consecutive failures",
        failover_config.failover_threshold
    );
    println!();

    // ============================================
    // 与 DbConfig 集成
    // ============================================
    println!("--- 2. integration with DbConfig ---\n");

    let db_config = DbConfig {
        url: "postgres://primary:5432/mydb".to_string(),
        #[cfg(feature = "failover")]
        failover_config: Some(FailoverConfig {
            urls: vec![
                "postgres://primary:5432/mydb".to_string(),
                "postgres://standby1:5432/mydb".to_string(),
            ],
            health_check_query: None,
            failover_threshold: 5,
        }),
        ..Default::default()
    };

    println!("  DbConfig configuration:");
    println!("  - primary URL     : {}", db_config.url);
    #[cfg(feature = "failover")]
    if let Some(ref fc) = db_config.failover_config {
        println!("  - failover chain   : {} nodes", fc.urls.len());
        println!(
            "  - trigger threshold     : {} consecutive failures",
            fc.failover_threshold
        );
    }
    println!();

    // ============================================
    // 故障转移流程
    // ============================================
    println!("--- 3. failover flow ---\n");

    println!("  normal state:");
    println!("  ┌──────────┐         ┌──────────┐");
    println!("  │ app layer  │ ──────→ │ primary  │ ✅");
    println!("  └──────────┘         └──────────┘");
    println!();

    println!("  primary failure (consecutive failures >= threshold):");
    println!("  ┌──────────┐         ┌──────────┐");
    println!("  │ app layer  │ ──X───→ │ primary  │ ❌ (3 failures)");
    println!("  └──────────┘         └──────────┘");
    println!("       │");
    println!("       │ trigger failover");
    println!("       ▼");
    println!("  ┌──────────┐         ┌──────────┐");
    println!("  │ app layer  │ ──────→ │ standby1 │ ✅");
    println!("  └──────────┘         └──────────┘");
    println!();

    // ============================================
    // CircuitBreaker 协同
    // ============================================
    println!("--- 4. CircuitBreaker state machine ---\n");

    println!("  ┌─────────┐  failures >= N  ┌─────────┐  probe after timeout  ┌──────────────┐");
    println!("  │ Closed  │ ──────────→ │  Open   │ ───────────→ │ HalfOpen     │");
    println!("  │ (normal)  │             │ (down)    │              │ (probe recovery) │");
    println!("  └─────────┘             └─────────┘              └──────┬───────┘");
    println!("       ▲                                                  │");
    println!("       │              probe succeeded                         │");
    println!("       └──────────────────────────────────────────────────┘");
    println!();

    println!("  cooperation with failover:");
    println!("  1. CircuitBreaker detects consecutive failures; state Closed → Open");
    println!("  2. trigger FailoverConfig to switch to the next URL");
    println!("  3. the new URL's CircuitBreaker starts as Closed");
    println!("  4. once the original primary recovers, switch back manually or automatically");
    println!();

    // ============================================
    // 默认配置
    // ============================================
    println!("--- 5. default configuration ---\n");

    let default_config = FailoverConfig::default();
    println!("  FailoverConfig::default():");
    println!("  - urls              : (empty)");
    println!("  - health_check_query: None");
    println!(
        "  - failover_threshold: {}",
        default_config.failover_threshold
    );
    println!();

    // ============================================
    // 生产环境最佳实践
    // ============================================
    println!("--- 6. production best practices ---\n");

    println!("  ┌────────────────────────┬──────────────────────────────────────────┐");
    println!("  │ practice                │ description                                  │");
    println!("  ├────────────────────────┼──────────────────────────────────────────┤");
    println!(
        "  │ at least 2 nodes          │ 1 primary + 1 standby, avoiding single points of failure │"
    );
    println!("  │ threshold = 3-5        │ avoids spurious switches from network jitter    │");
    println!("  │ custom health_check    │ use a lightweight SQL (SELECT 1) probe       │");
    println!(
        "  │ monitor failover events      │ alert on switchover, investigate the primary promptly │"
    );
    println!("  │ periodic drills          │ verify the failover chain is usable          │");
    println!("  └────────────────────────┴──────────────────────────────────────────┘");
    println!();

    println!("========================================");
    println!("✨ Connection failover example complete!");
    println!("========================================");
    println!("\n📚 Key APIs:");
    println!("  - FailoverConfig {{ urls, health_check_query, failover_threshold }}");
    println!("  - DbConfig {{ failover_config: Some(FailoverConfig {{ .. }}), .. }}");
    println!("  - CircuitBreaker + HealthChecker cooperate for automatic failover");
}
