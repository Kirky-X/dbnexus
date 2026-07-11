// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 预设配置对比示例
//!
//! 演示 dbnexus 不同预设（embedded / microservice / monolith / enterprise）
//! 之间的 feature 差异，通过条件编译展示当前启用的特性。
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example config_presets --features "sqlite,permission,macros"
//! ```

fn main() {
    println!("========================================");
    println!("🏗️  DBNexus 预设配置对比示例");
    println!("========================================\n");

    // ============================================
    // 1. 打印当前启用的特性
    // ============================================
    println!("📋 当前启用的特性 (通过 cfg! 检测):");
    print_feature_status();

    // ============================================
    // 2. 预设配置对比表
    // ============================================
    println!("\n📊 预设配置对比表:");
    println!("┌─────────────┬──────────┬───────────────┬────────────┬─────────────┐");
    println!("│ 特性        │ embedded │ microservice  │ monolith   │ enterprise  │");
    println!("├─────────────┼──────────┼───────────────┼────────────┼─────────────┤");
    println!("│ sqlite      │    ✓     │       -       │     -      │     -       │");
    println!("│ postgres    │    -     │       ✓       │     ✓      │     ✓       │");
    println!("│ permission  │    -     │       ✓       │     ✓      │     ✓       │");
    println!("│ sql-parser  │    -     │       ✓       │     ✓      │     ✓       │");
    println!("│ config-env  │    ✓     │       ✓       │     -      │     -       │");
    println!("│ yaml        │    -     │       -       │     ✓      │     ✓       │");
    println!("│ data-mgmt   │    -     │       -       │     ✓      │     ✓       │");
    println!("│ security    │    -     │       -       │     ✓      │     ✓       │");
    println!("│ observability│   -     │       ✓       │     ✓      │     ✓       │");
    println!("│ perm-engine │    -     │       -       │     -      │     ✓       │");
    println!("└─────────────┴──────────┴───────────────┴────────────┴─────────────┘");

    // ============================================
    // 3. 各预设的适用场景
    // ============================================
    println!("\n🎯 预设适用场景:");
    println!("\n  embedded (嵌入式/边缘设备):");
    println!("    - 超轻量配置，仅 sqlite + config-env");
    println!("    - 适用于 IoT 设备、边缘计算、嵌入式系统");
    println!("    - 资源占用最小，无权限/可观测性开销");

    println!("\n  microservice (微服务):");
    println!("    - postgres + 权限 + SQL 解析 + 可观测性");
    println!("    - 适用于云原生微服务架构");
    println!("    - 环境变量配置，适配 K8s/容器化部署");

    println!("\n  monolith (单体应用):");
    println!("    - 完整功能：数据管理 + 安全 + 可观测性");
    println!("    - 适用于中大型单体应用");
    println!("    - YAML 配置，支持分片/全局索引/审计");

    println!("\n  enterprise (企业级):");
    println!("    - 在 monolith 基础上增加 permission-engine");
    println!("    - 适用于金融、政务等高安全场景");
    println!("    - 高级 RBAC 权限引擎 + 策略决策点");

    // ============================================
    // 4. 条件编译演示
    // ============================================
    println!("\n🔧 条件编译能力演示:");

    #[cfg(feature = "sqlite")]
    println!("  ✓ SQLite 驱动已启用 — 支持 sqlite::memory: 和文件模式");

    #[cfg(feature = "postgres")]
    println!("  ✓ PostgreSQL 驱动已启用 — 支持 postgres:// 连接");

    #[cfg(feature = "mysql")]
    println!("  ✓ MySQL 驱动已启用 — 支持 mysql:// 连接");

    #[cfg(feature = "permission")]
    println!("  ✓ 权限控制已启用 — 支持 RBAC 角色策略");

    #[cfg(feature = "macros")]
    println!("  ✓ 过程宏已启用 — 支持 #[db_entity(...)] 统一属性宏");

    #[cfg(feature = "sql-parser")]
    println!("  ✓ SQL 解析已启用 — 支持 SQL 注入检测和 DDL 防护");

    #[cfg(feature = "migration")]
    println!("  ✓ 数据库迁移已启用 — 支持 MigrationExecutor");

    #[cfg(feature = "sharding")]
    println!("  ✓ 数据分片已启用 — 支持 ShardRouter 多策略分片");

    #[cfg(feature = "global-index")]
    println!("  ✓ 全局索引已启用 — 支持 GlobalIndex 跨分片查询");

    #[cfg(feature = "cache")]
    println!("  ✓ 缓存已启用 — 支持 oxcache 查询结果缓存");

    #[cfg(feature = "metrics")]
    println!("  ✓ 指标收集已启用 — 支持 Prometheus metrics");

    #[cfg(feature = "health-check")]
    println!("  ✓ 健康检查已启用 — 支持连接池健康监控");

    #[cfg(feature = "audit")]
    println!("  ✓ 审计日志已启用 — 支持 AuditLogger 事件记录");

    println!("\n========================================");
    println!("✨ 预设配置对比示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - 预设是 feature 的组合，通过 Cargo.toml 的 [features] 定义");
    println!("  - embedded = 最小化，microservice = 云原生，monolith = 完整，enterprise = 全功能");
    println!("  - 运行时可通过 cfg!(feature = \"...\") 检测特性是否启用");
    println!("  - 选择预设而非逐个启用 feature，减少配置出错概率");
}

fn print_feature_status() {
    let features: Vec<(&str, bool)> = vec![
        ("sqlite", cfg!(feature = "sqlite")),
        ("postgres", cfg!(feature = "postgres")),
        ("mysql", cfg!(feature = "mysql")),
        ("permission", cfg!(feature = "permission")),
        ("permission-engine", cfg!(feature = "permission-engine")),
        ("sql-parser", cfg!(feature = "sql-parser")),
        ("macros", cfg!(feature = "macros")),
        ("cache", cfg!(feature = "cache")),
        ("config-env", cfg!(feature = "config-env")),
        ("yaml", cfg!(feature = "yaml")),
        ("config-toml", cfg!(feature = "config-toml")),
        ("migration", cfg!(feature = "migration")),
        ("sharding", cfg!(feature = "sharding")),
        ("global-index", cfg!(feature = "global-index")),
        ("metrics", cfg!(feature = "metrics")),
        ("health-check", cfg!(feature = "health-check")),
        ("audit", cfg!(feature = "audit")),
        ("authentication", cfg!(feature = "authentication")),
        ("observability", cfg!(feature = "observability")),
        ("security", cfg!(feature = "security")),
    ];

    for (name, enabled) in features {
        let mark = if enabled { "✓" } else { "✗" };
        let status = if enabled { "启用" } else { "未启用" };
        println!("  {} {:<20} - {}", mark, name, status);
    }
}
