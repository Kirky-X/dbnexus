// Copyright (c) 2026 Kirky.X
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

use dbnexus::{FailoverConfig, ReplicaConfig};

fn main() {
    println!("========================================");
    println!("🔀 DBNexus 副本路由（读写分离）示例");
    println!("========================================\n");

    // ============================================
    // 1. ReplicaConfig 配置
    // ============================================
    println!("--- 1. ReplicaConfig 配置 ---\n");

    let replica_config = ReplicaConfig {
        replica_urls: vec![
            "postgres://replica1:5432/mydb".to_string(),
            "postgres://replica2:5432/mydb".to_string(),
        ],
        max_lag_seconds: 5.0,
        lag_check_interval_secs: 10,
    };

    println!("  副本路由配置:");
    println!("  - 副本数量      : {}", replica_config.replica_urls.len());
    for (i, url) in replica_config.replica_urls.iter().enumerate() {
        println!("    - replica {}   : {}", i + 1, url);
    }
    println!("  - 最大延迟阈值  : {:.1} 秒", replica_config.max_lag_seconds);
    println!("  - 检测间隔      : {} 秒", replica_config.lag_check_interval_secs);
    println!();

    // 默认配置
    let default_config = ReplicaConfig::default();
    println!("  默认配置:");
    println!("  - max_lag_seconds      : {:.1}", default_config.max_lag_seconds);
    println!(
        "  - lag_check_interval   : {} 秒",
        default_config.lag_check_interval_secs
    );
    println!("  - replica_urls         : (空)",);
    println!();

    // ============================================
    // 2. ReplicationLag 结构体
    // ============================================
    println!("--- 2. ReplicationLag 结构体 ---\n");

    println!("  ReplicationLag 字段:");
    println!("  ┌────────────────┬──────────────────────────────────────────┐");
    println!("  │ 字段           │ 说明                                     │");
    println!("  ├────────────────┼──────────────────────────────────────────┤");
    println!("  │ lag_bytes      │ 字节级延迟（PostgreSQL pg_wal_lsn_diff） │");
    println!("  │ lag_seconds    │ 秒级延迟（MySQL Seconds_Behind_Master）  │");
    println!("  │ is_caught_up   │ 是否已追上主库（lag < 阈值）             │");
    println!("  └────────────────┴──────────────────────────────────────────┘");
    println!();

    // ============================================
    // 3. ReplicationLagDetector 各后端实现
    // ============================================
    println!("--- 3. ReplicationLagDetector 后端实现 ---\n");

    println!("  ┌──────────────────────┬───────────────────────────────────────────────┐");
    println!("  │ 检测器               │ 实现方式                                      │");
    println!("  ├──────────────────────┼───────────────────────────────────────────────┤");
    println!("  │ PostgresLagDetector  │ pg_wal_lsn_diff(pg_current_wal_lsn(), ...)    │");
    println!("  │                      │ 字节级精度，默认阈值 10MB                      │");
    println!("  ├──────────────────────┼───────────────────────────────────────────────┤");
    println!("  │ MySqlLagDetector     │ SHOW SLAVE STATUS → Seconds_Behind_Master     │");
    println!("  │                      │ 秒级精度，默认阈值 5.0 秒                      │");
    println!("  ├──────────────────────┼───────────────────────────────────────────────┤");
    println!("  │ SqliteLagDetector    │ SQLite 无副本语义，始终返回 is_caught_up=true  │");
    println!("  └──────────────────────┴───────────────────────────────────────────────┘");
    println!();

    // ============================================
    // 4. ReplicaPool 路由逻辑
    // ============================================
    println!("--- 4. ReplicaPool 路由逻辑 ---\n");

    println!("  读请求路由流程:");
    println!();
    println!("  ┌──────────┐   get_read_session()   ┌──────────────┐");
    println!("  │ 客户端   │ ──────────────────────→ │ ReplicaPool  │");
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
    println!("                              │ 返回副本  │  │ 返回 None│");
    println!("                              │ Session  │  │ (回退主库)│");
    println!("                              └──────────┘  └──────────┘");
    println!();

    // ============================================
    // 5. FailoverConfig 故障转移配置
    // ============================================
    println!("--- 5. FailoverConfig 故障转移配置 ---\n");

    let failover_config = FailoverConfig {
        urls: vec![
            "postgres://primary:5432/mydb".to_string(),
            "postgres://standby1:5432/mydb".to_string(),
            "postgres://standby2:5432/mydb".to_string(),
        ],
        health_check_query: Some("SELECT 1".to_string()),
        failover_threshold: 3,
    };

    println!("  故障转移配置:");
    println!("  - 故障转移链:");
    for (i, url) in failover_config.urls.iter().enumerate() {
        let role = if i == 0 { "primary" } else { "standby" };
        println!("    [{}] {} : {}", i, role, url);
    }
    println!("  - 健康检查 SQL : {:?}", failover_config.health_check_query);
    println!("  - 触发阈值     : 连续失败 {} 次", failover_config.failover_threshold);
    println!();

    // 默认配置
    let default_failover = FailoverConfig::default();
    println!("  默认配置:");
    println!("  - urls              : (空)");
    println!("  - health_check_query: None (默认 SELECT 1)");
    println!("  - failover_threshold: {}", default_failover.failover_threshold);
    println!();

    // ============================================
    // 6. Failover + CircuitBreaker 协同
    // ============================================
    println!("--- 6. 故障转移 + CircuitBreaker 协同 ---\n");

    println!("  ┌──────────┐   查询失败    ┌────────────────┐");
    println!("  │ 查询请求  │ ────────────→ │ CircuitBreaker │");
    println!("  └──────────┘               └───────┬────────┘");
    println!("                                     │");
    println!("                        连续失败 >= threshold");
    println!("                                     │");
    println!("                                     ▼");
    println!("                          ┌──────────────────┐");
    println!("                          │ 触发故障转移      │");
    println!("                          │ 切换到下一个 URL  │");
    println!("                          └──────────────────┘");
    println!();

    // ============================================
    // 7. 生产环境建议
    // ============================================
    println!("--- 7. 生产环境建议 ---\n");
    println!("  ┌────────────────────┬──────────────────────────────────────────────┐");
    println!("  │ 参数               │ 建议值                                       │");
    println!("  ├────────────────────┼──────────────────────────────────────────────┤");
    println!("  │ max_lag_seconds    │ 1-10 秒（根据业务容忍度）                    │");
    println!("  │ lag_check_interval │ 5-30 秒（避免频繁检测增加主库压力）          │");
    println!("  │ failover_threshold │ 3-5 次（避免瞬时网络抖动触发误切换）         │");
    println!("  │ health_check_query │ SELECT 1（轻量级探活）                       │");
    println!("  └────────────────────┴──────────────────────────────────────────────┘");
    println!();

    println!("========================================");
    println!("✨ 副本路由示例完成！");
    println!("========================================");
    println!("\n📚 关键 API:");
    println!("  - ReplicaConfig {{ replica_urls, max_lag_seconds, lag_check_interval_secs }}");
    println!("  - FailoverConfig {{ urls, health_check_query, failover_threshold }}");
    println!("  - ReplicaPool::new(pool, lag_detector, max_lag_seconds)");
    println!("  - replica_pool.get_read_session(role) -> Option<Session>");
}
