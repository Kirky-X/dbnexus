// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! YAML 权限策略示例
//!
//! 演示 `YamlPermissionProvider` 的使用，包括：
//! - 从 YAML 字符串解析权限配置
//! - 使用 `YamlPermissionProvider::from_config` 加载策略
//! - 不同角色（admin / manager / guest）的权限差异
//! - 权限检查通过和拒绝场景
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example permission_yaml --features "sqlite,permission,yaml"
//! ```

use dbnexus::access::{PermissionAction, PermissionProvider, YamlPermissionProvider};
use dbnexus::{DbConfig, DbPool};
use std::collections::HashMap;

// ============================================
// YAML 权限策略配置（文档参考）
// ============================================
//
// 以下是等价的 YAML 配置文件内容：
//
// ```yaml
// roles:
//   admin:
//     tables:
//       - name: "*"
//         operations:
//           - select
//           - insert
//           - update
//           - delete
//   manager:
//     tables:
//       - name: users
//         operations:
//           - select
//           - insert
//           - update
//       - name: orders
//         operations:
//           - select
//           - insert
//   guest:
//     tables:
//       - name: users
//         operations:
//           - select
// ```
//
// 在生产环境中，可以使用 `YamlPermissionProvider::new("path/to/permissions.yaml")`
// 从文件加载（需要启用 `yaml` feature）。

// ============================================
// 主函数
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("📄 DBNexus YAML permission policy example");
    println!("========================================\n");

    // ============================================
    // 构建权限配置
    // ============================================
    // 本示例通过编程方式构建 PermissionConfig（与 YAML 结构完全一致）。
    // 生产环境推荐使用 serde_saphyr::from_str 或 YamlPermissionProvider::new(path)。

    let yaml_content = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations:
          - select
          - insert
          - update
          - delete
  manager:
    tables:
      - name: users
        operations:
          - select
          - insert
          - update
      - name: orders
        operations:
          - select
          - insert
  guest:
    tables:
      - name: users
        operations:
          - select
"#;

    println!("--- YAML configuration content ---");
    println!("{}", yaml_content);

    // 使用 serde_saphyr 解析 YAML（yaml feature 提供）
    let config: dbnexus::access::PermissionConfig =
        serde_saphyr::from_str(yaml_content).expect("Failed to parse YAML config");

    println!("✓ YAML configuration parsed\n");

    // 也可以通过编程方式构建相同的配置：
    let _manual_config = dbnexus::access::PermissionConfig {
        roles: HashMap::from([
            (
                "admin".to_string(),
                dbnexus::access::RolePolicy {
                    tables: vec![dbnexus::access::TablePermission {
                        name: "*".to_string(),
                        operations: vec![
                            PermissionAction::Select,
                            PermissionAction::Insert,
                            PermissionAction::Update,
                            PermissionAction::Delete,
                        ],
                    }],
                },
            ),
            (
                "guest".to_string(),
                dbnexus::access::RolePolicy {
                    tables: vec![dbnexus::access::TablePermission {
                        name: "users".to_string(),
                        operations: vec![PermissionAction::Select],
                    }],
                },
            ),
        ]),
    };

    // ============================================
    // 创建 YamlPermissionProvider
    // ============================================
    let provider = YamlPermissionProvider::from_config(config);

    println!("--- configured roles ---");
    let mut roles = provider.get_roles();
    roles.sort();
    for role in &roles {
        println!("  • {}", role);
    }
    println!();

    // ============================================
    // 演示不同角色的权限差异
    // ============================================
    println!("--- permission check results ---\n");

    let test_cases = [
        // admin 拥有通配符权限，可以操作任何表
        ("admin", "users", PermissionAction::Select, true),
        ("admin", "users", PermissionAction::Delete, true),
        ("admin", "orders", PermissionAction::Insert, true),
        ("admin", "logs", PermissionAction::Update, true),
        // manager 仅限 users 和 orders 表，无 DELETE
        ("manager", "users", PermissionAction::Select, true),
        ("manager", "users", PermissionAction::Insert, true),
        ("manager", "users", PermissionAction::Update, true),
        ("manager", "users", PermissionAction::Delete, false),
        ("manager", "orders", PermissionAction::Select, true),
        ("manager", "orders", PermissionAction::Insert, true),
        ("manager", "orders", PermissionAction::Delete, false),
        ("manager", "logs", PermissionAction::Select, false),
        // guest 仅限 users 表 SELECT
        ("guest", "users", PermissionAction::Select, true),
        ("guest", "users", PermissionAction::Insert, false),
        ("guest", "orders", PermissionAction::Select, false),
        // 未知角色
        ("unknown", "users", PermissionAction::Select, false),
    ];

    let mut passed = 0;
    let mut failed = 0;

    for (role, table, action, expected) in test_cases {
        let result = provider.check_access(role, table, action.clone())?;
        let mark = if result == expected { "✔" } else { "✘" };
        let status = if result { "allowed" } else { "rejected" };

        if result == expected {
            passed += 1;
        } else {
            failed += 1;
        }

        println!(
            "  {} [{}] {}.{} → {} (expected {})",
            mark, role, table, action, status, expected
        );
    }

    println!("\n  statistics: {} passed, {} failed", passed, failed);

    // ============================================
    // 展示角色策略详情
    // ============================================
    println!("\n--- role policy details ---\n");

    for role in &["admin", "manager", "guest"] {
        match provider.get_role_policy(role) {
            Some(policy) => {
                println!("  [{}] policy:", role);
                for tp in &policy.tables {
                    let ops: Vec<String> = tp.operations.iter().map(|o| o.to_string()).collect();
                    println!("    table {} → [{}]", tp.name, ops.join(", "));
                }
            }
            None => {
                println!("  [{}] has no policy", role);
            }
        }
    }

    // ============================================
    // 结合 DbPool + Session（展示集成场景）
    // ============================================
    println!("\n--- DbPool integration ---\n");

    let db_config = DbConfig {
        url: "sqlite::memory:".to_string(),
        admin_role: "admin".to_string(),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 3,
            min_connections: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    let pool = DbPool::with_config(db_config).await?;
    let session = pool.get_session("admin").await?;
    println!("✓ pool created; current Session role: {}", session.role());

    // 在实际应用中，可将 YamlPermissionProvider 注入到 PermissionContext，
    // 实现缓存未命中时自动重新加载策略：
    //   let ctx = PermissionContext::new_with_provider(role, cache, Arc::new(provider));

    println!("\n========================================");
    println!("✨ YAML permission policy example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - YamlPermissionProvider::from_config(config)  create a provider from config");
    println!(
        "  - YamlPermissionProvider::new(path)           load from a YAML file (needs the yaml feature)"
    );
    println!(
        "  - serde_saphyr::from_str(yaml)              parse a YAML string into PermissionConfig"
    );
    println!("  - PermissionConfig.roles                     the HashMap of roles to policies");
    println!("  - RolePolicy.tables                          the table permission list");
    println!("  - TablePermission.name = \"*\"                 wildcard matching all tables");

    Ok(())
}
