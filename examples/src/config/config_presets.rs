// Copyright (c) 2025-2026 Kirky.X🌠
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
    println!("🏗️  DBNexus preset configuration comparison example");
    println!("========================================\n");

    // ============================================
    // 1. 打印当前启用的特性
    // ============================================
    println!("📋 Currently enabled features (detected via cfg!):");
    print_feature_status();

    // ============================================
    // 2. 预设配置对比表
    // ============================================
    println!("\n📊 Preset configuration comparison table:");
    println!("┌─────────────┬──────────┬───────────────┬────────────┬─────────────┐");
    println!("│ Feature      │ embedded │ microservice  │ monolith   │ enterprise  │");
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
    println!("\n🎯 Preset use cases:");
    println!("\n  embedded (embedded/edge devices):");
    println!("    - ultra-lightweight configuration, sqlite + config-env only");
    println!("    - for IoT devices, edge computing, and embedded systems");
    println!("    - smallest resource footprint, no permission/observability overhead");

    println!("\n  microservice (microservices):");
    println!("    - postgres + permissions + SQL parsing + observability");
    println!("    - for cloud-native microservice architectures");
    println!("    - environment variable configuration, fit for K8s/containerized deployment");

    println!("\n  monolith (monolithic application):");
    println!("    - full features: data management + security + observability");
    println!("    - for medium and large monolithic applications");
    println!("    - YAML configuration, supports sharding/global index/audit");

    println!("\n  enterprise (enterprise-grade):");
    println!("    - adds permission-engine on top of monolith");
    println!("    - for high-security scenarios such as finance and government");
    println!("    - advanced RBAC permission engine + policy decision point");

    // ============================================
    // 4. 条件编译演示
    // ============================================
    println!("\n🔧 Conditional compilation capability demo:");

    #[cfg(feature = "sqlite")]
    println!("  ✓ SQLite driver enabled — supports sqlite::memory: and file mode");

    #[cfg(feature = "postgres")]
    println!("  ✓ PostgreSQL driver enabled — supports postgres:// connections");

    #[cfg(feature = "mysql")]
    println!("  ✓ MySQL driver enabled — supports mysql:// connections");

    #[cfg(feature = "permission")]
    println!("  ✓ Permission control enabled — supports RBAC role policies");

    #[cfg(feature = "macros")]
    println!(
        "  ✓ Procedural macros enabled — supports the #[db_entity(...)] unified attribute macro"
    );

    #[cfg(feature = "sql-parser")]
    println!("  ✓ SQL parsing enabled — supports SQL injection detection and DDL protection");

    #[cfg(feature = "migration")]
    println!("  ✓ Database migration enabled — supports MigrationExecutor");

    #[cfg(feature = "sharding")]
    println!("  ✓ Data sharding enabled — supports ShardRouter multi-strategy sharding");

    #[cfg(feature = "global-index")]
    println!("  ✓ Global index enabled — supports GlobalIndex cross-shard queries");

    #[cfg(feature = "cache")]
    println!("  ✓ Cache enabled — supports oxcache query result caching");

    #[cfg(feature = "metrics")]
    println!("  ✓ Metrics collection enabled — supports Prometheus metrics");

    #[cfg(feature = "health-check")]
    println!("  ✓ Health check enabled — supports pool health monitoring");

    #[cfg(feature = "audit")]
    println!("  ✓ Audit logging enabled — supports AuditLogger event recording");

    println!("\n========================================");
    println!("✨ Preset configuration comparison example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - presets are feature combinations defined in the [features] table of Cargo.toml");
    println!(
        "  - embedded = minimal, microservice = cloud-native, monolith = full, enterprise = all features"
    );
    println!("  - at runtime, use cfg!(feature = \"...\") to detect whether a feature is enabled");
    println!(
        "  - choose a preset instead of enabling features one by one, reducing misconfiguration"
    );
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
        let status = if enabled { "enabled" } else { "disabled" };
        println!("  {} {:<20} - {}", mark, name, status);
    }
}
