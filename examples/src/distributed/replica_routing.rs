// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 副本路由（读写分离）示例
//!
//! 演示 `replica-routing` 功能的使用：
//! - `ReplicaConfig` 配置副本 URL 和延迟阈值
//! - `ReplicationLag` 结构体含义
//! - `ReplicationLagDetector` trait 及各后端实现
//! - `ReplicaPool` 的 lag 感知路由逻辑
//! - `FailoverConfig` 故障转移配置
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --bin replica_routing
//! ```

use dbnexus::{FailoverConfig, ReplicaConfig, t_simple};

fn main() {
    println!("========================================");
    println!("🔀 DBNexus replica routing (read/write splitting) example");
    println!("========================================\n");

    // ============================================
    // ReplicaConfig 配置
    // ============================================
    println!("--- 1. ReplicaConfig configuration ---\n");

    let replica_config = ReplicaConfig {
        replica_urls: vec![
            "postgres://replica1:5432/mydb".to_string(),
            "postgres://replica2:5432/mydb".to_string(),
        ],
        max_lag_seconds: 5.0,
        lag_check_interval_secs: 10,
    };

    println!("  replica routing configuration:");
    println!(
        "  - replica count      : {}",
        replica_config.replica_urls.len()
    );
    for (i, url) in replica_config.replica_urls.iter().enumerate() {
        println!("    - replica {}   : {}", i + 1, url);
    }
    println!(
        "  - max lag threshold  : {:.1} sec",
        replica_config.max_lag_seconds
    );
    println!(
        "  - check interval      : {} sec",
        replica_config.lag_check_interval_secs
    );
    println!();

    // 默认配置
    let default_config = ReplicaConfig::default();
    println!("  default configuration:");
    println!(
        "  - max_lag_seconds      : {:.1}",
        default_config.max_lag_seconds
    );
    println!(
        "  - lag_check_interval   : {} sec",
        default_config.lag_check_interval_secs
    );
    println!("  - replica_urls         : (empty)",);
    println!();

    // ============================================
    // ReplicationLag 结构体
    // ============================================
    println!("--- 2. ReplicationLag struct ---\n");

    println!("  ReplicationLag fields:");
    println!("  ┌────────────────┬──────────────────────────────────────────┐");
    println!("  │ field          │ description                              │");
    println!("  ├────────────────┼──────────────────────────────────────────┤");
    println!("  │ lag_bytes      │ byte-level lag (PostgreSQL pg_wal_lsn_diff) │");
    println!("  │ lag_seconds    │ second-level lag (MySQL Seconds_Behind_Master) │");
    println!("  │ is_caught_up   │ whether the replica has caught up (lag < threshold) │");
    println!("  └────────────────┴──────────────────────────────────────────┘");
    println!();

    // ============================================
    // ReplicationLagDetector 各后端实现
    // ============================================
    println!("--- 3. ReplicationLagDetector backend implementations ---\n");

    println!("  ┌──────────────────────┬───────────────────────────────────────────────┐");
    println!("  │ detector               │ implementation approach                 │");
    println!("  ├──────────────────────┼───────────────────────────────────────────────┤");
    println!("  │ PostgresLagDetector  │ pg_wal_lsn_diff(pg_current_wal_lsn(), ...)    │");
    println!("  │                      │ byte-level precision, default threshold 10MB        │");
    println!("  ├──────────────────────┼───────────────────────────────────────────────┤");
    println!("  │ MySqlLagDetector     │ SHOW SLAVE STATUS → Seconds_Behind_Master     │");
    println!("  │                      │ second-level precision, default threshold 5.0 sec  │");
    println!("  ├──────────────────────┼───────────────────────────────────────────────┤");
    println!(
        "  │ SqliteLagDetector    │ SQLite has no replica semantics; always is_caught_up=true │"
    );
    println!("  └──────────────────────┴───────────────────────────────────────────────┘");
    println!();

    // ============================================
    // ReplicaPool 路由逻辑
    // ============================================
    println!("--- 4. ReplicaPool routing logic ---\n");

    println!("  read request routing flow:");
    println!();
    println!("  ┌──────────┐   get_read_session()   ┌──────────────┐");
    println!("  │ client    │ ──────────────────────→ │ ReplicaPool  │");
    println!("  └──────────┘                         └──────┬───────┘");
    println!("                                              │");
    println!("                                    detect_lag()");
    println!("                                              │");
    println!("                                              ▼");
    println!("                                    ┌──────────────────┐");
    println!("                                    │ ReplicationLag   │");
    println!("                                    │ is_caught_up?    │");
    println!("                                    └──┬───────────┬───┘");
    println!("                                  Yes  │           │ No");
    println!("                                       ▼           ▼");
    println!("                              ┌──────────┐  ┌──────────┐");
    println!("                              │ return replica │  │ return None│");
    println!("                              │ Session  │  │ (fall back to primary)│");
    println!("                              └──────────┘  └──────────┘");
    println!();

    // ============================================
    // FailoverConfig 故障转移配置
    // ============================================
    println!("--- 5. FailoverConfig failover configuration ---\n");

    let failover_config = FailoverConfig {
        urls: vec![
            "postgres://primary:5432/mydb".to_string(),
            "postgres://standby1:5432/mydb".to_string(),
            "postgres://standby2:5432/mydb".to_string(),
        ],
        health_check_query: Some("SELECT 1".to_string()),
        failover_threshold: 3,
    };

    println!("  failover configuration:");
    println!("  - failover chain:");
    for (i, url) in failover_config.urls.iter().enumerate() {
        let role = if i == 0 { "primary" } else { "standby" };
        println!("    [{}] {} : {}", i, role, url);
    }
    println!(
        "  - health check SQL: {:?}",
        failover_config.health_check_query
    );
    println!(
        "  - trigger threshold     : {} consecutive failures",
        failover_config.failover_threshold
    );
    println!();

    // 默认配置
    let default_failover = FailoverConfig::default();
    println!("  default configuration:");
    println!("  - urls              : (empty)");
    println!("  - health_check_query: None (default SELECT 1)");
    println!(
        "  - failover_threshold: {}",
        default_failover.failover_threshold
    );
    println!();

    // ============================================
    // Failover + CircuitBreaker 协同
    // ============================================
    println!("--- 6. failover + CircuitBreaker cooperation ---\n");

    println!("  ┌──────────┐   query failed   ┌────────────────┐");
    println!("  │ query request │ ────────────→ │ CircuitBreaker │");
    println!("  └──────────┘               └───────┬────────┘");
    println!("                                     │");
    println!("                        consecutive failures >= threshold");
    println!("                                     │");
    println!("                                     ▼");
    println!("                          ┌──────────────────┐");
    println!("                          │ trigger failover      │");
    println!("                          │ switch to the next URL │");
    println!("                          └──────────────────┘");
    println!();

    // ============================================
    // 生产环境建议
    // ============================================
    println!("--- 7. production recommendations ---\n");
    println!("  ┌────────────────────┬──────────────────────────────────────────────┐");
    println!("  │ parameter           │ recommended value                        │");
    println!("  ├────────────────────┼──────────────────────────────────────────────┤");
    println!("  │ max_lag_seconds    │ 1-10 sec (per your business tolerance)      │");
    println!("  │ lag_check_interval │ 5-30 sec (avoid frequent checks that burden the primary) │");
    println!(
        "  │ failover_threshold │ 3-5 failures (avoids spurious switches from transient jitter) │"
    );
    println!("  │ health_check_query │ SELECT 1 (lightweight liveness probe)          │");
    println!("  └────────────────────┴──────────────────────────────────────────────┘");
    println!();

    println!("========================================");
    println!("✨ Replica routing example complete!");
    println!("========================================");
    println!("\n📚 Key APIs:");
    println!("  - ReplicaConfig {{ replica_urls, max_lag_seconds, lag_check_interval_secs }}");
    println!("  - FailoverConfig {{ urls, health_check_query, failover_threshold }}");
    println!(
        "  {}",
        t_simple("example-replica-routing-api-replica-pool-new")
    );
    println!("  - replica_pool.get_read_session(role) -> Option<Session>");
}
