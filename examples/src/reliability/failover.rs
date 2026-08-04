// Copyright (c) 2026 Kirky.X
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
    println!("🔌 DBNexus 连接故障转移示例");
    println!("========================================\n");

    // ============================================
    // 1. FailoverConfig 基础配置
    // ============================================
    println!("--- 1. FailoverConfig 基础配置 ---\n");

    let failover_config = FailoverConfig {
        urls: vec![
            "postgres://primary:5432/mydb".to_string(),
            "postgres://standby1:5432/mydb".to_string(),
            "postgres://standby2:5432/mydb".to_string(),
        ],
        health_check_query: Some("SELECT 1".to_string()),
        failover_threshold: 3,
    };

    println!("  故障转移链:");
    for (i, url) in failover_config.urls.iter().enumerate() {
        let role = match i {
            0 => "primary (主库)",
            1 => "standby1 (备库1)",
            2 => "standby2 (备库2)",
            _ => "unknown",
        };
        println!("  [{}] {} : {}", i, role, url);
    }
    println!();
    println!("  - 健康检查 SQL : {:?}", failover_config.health_check_query);
    println!(
        "  - 触发阈值     : 连续失败 {} 次后切换",
        failover_config.failover_threshold
    );
    println!();

    // ============================================
    // 2. 与 DbConfig 集成
    // ============================================
    println!("--- 2. 与 DbConfig 集成 ---\n");

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

    println!("  DbConfig 配置:");
    println!("  - 主库 URL     : {}", db_config.url);
    #[cfg(feature = "failover")]
    if let Some(ref fc) = db_config.failover_config {
        println!("  - 故障转移链   : {} 个节点", fc.urls.len());
        println!("  - 触发阈值     : 连续失败 {} 次", fc.failover_threshold);
    }
    println!();

    // ============================================
    // 3. 故障转移流程
    // ============================================
    println!("--- 3. 故障转移流程 ---\n");

    println!("  正常状态:");
    println!("  ┌──────────┐         ┌──────────┐");
    println!("  │ 应用层   │ ──────→ │ primary  │ ✅");
    println!("  └──────────┘         └──────────┘");
    println!();

    println!("  主库故障（连续失败 >= threshold）:");
    println!("  ┌──────────┐         ┌──────────┐");
    println!("  │ 应用层   │ ──X───→ │ primary  │ ❌ (3 次失败)");
    println!("  └──────────┘         └──────────┘");
    println!("       │");
    println!("       │ 触发故障转移");
    println!("       ▼");
    println!("  ┌──────────┐         ┌──────────┐");
    println!("  │ 应用层   │ ──────→ │ standby1 │ ✅");
    println!("  └──────────┘         └──────────┘");
    println!();

    // ============================================
    // 4. CircuitBreaker 协同
    // ============================================
    println!("--- 4. CircuitBreaker 状态机 ---\n");

    println!("  ┌─────────┐  失败 >= N  ┌─────────┐  超时后探测  ┌──────────────┐");
    println!("  │ Closed  │ ──────────→ │  Open   │ ───────────→ │ HalfOpen     │");
    println!("  │ (正常)  │             │ (断开)  │              │ (探测恢复)   │");
    println!("  └─────────┘             └─────────┘              └──────┬───────┘");
    println!("       ▲                                                  │");
    println!("       │              探测成功                             │");
    println!("       └──────────────────────────────────────────────────┘");
    println!();

    println!("  与故障转移的协同:");
    println!("  1. CircuitBreaker 检测连续失败，状态 Closed → Open");
    println!("  2. 触发 FailoverConfig 切换到下一个 URL");
    println!("  3. 新 URL 的 CircuitBreaker 初始为 Closed");
    println!("  4. 原主库恢复后，可手动或自动切回");
    println!();

    // ============================================
    // 5. 默认配置
    // ============================================
    println!("--- 5. 默认配置 ---\n");

    let default_config = FailoverConfig::default();
    println!("  FailoverConfig::default():");
    println!("  - urls              : (空)");
    println!("  - health_check_query: None");
    println!("  - failover_threshold: {}", default_config.failover_threshold);
    println!();

    // ============================================
    // 6. 生产环境最佳实践
    // ============================================
    println!("--- 6. 生产环境最佳实践 ---\n");

    println!("  ┌────────────────────────┬──────────────────────────────────────────┐");
    println!("  │ 实践                   │ 说明                                     │");
    println!("  ├────────────────────────┼──────────────────────────────────────────┤");
    println!("  │ 至少 2 个节点          │ 1 主 + 1 备，避免单点故障                │");
    println!("  │ threshold = 3-5        │ 避免网络抖动导致误切换                   │");
    println!("  │ 自定义 health_check    │ 使用轻量 SQL（SELECT 1）探活             │");
    println!("  │ 监控故障转移事件       │ 切换时告警，及时排查主库问题             │");
    println!("  │ 定期演练               │ 验证故障转移链可用性                     │");
    println!("  └────────────────────────┴──────────────────────────────────────────┘");
    println!();

    println!("========================================");
    println!("✨ 连接故障转移示例完成！");
    println!("========================================");
    println!("\n📚 关键 API:");
    println!("  - FailoverConfig {{ urls, health_check_query, failover_threshold }}");
    println!("  - DbConfig {{ failover_config: Some(FailoverConfig {{ .. }}), .. }}");
    println!("  - CircuitBreaker + HealthChecker 协同自动故障转移");
}
