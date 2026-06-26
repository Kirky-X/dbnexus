// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in the project root for full license information.

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

use dbnexus::access::permission::{
    PermissionAction, PermissionProvider, YamlPermissionProvider,
};
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
    println!("📄 DBNexus YAML 权限策略示例");
    println!("========================================\n");

    // ============================================
    // 1. 构建权限配置
    // ============================================
    // 本示例通过编程方式构建 PermissionConfig（与 YAML 结构完全一致）。
    // 生产环境推荐使用 serde_yaml_ng::from_str 或 YamlPermissionProvider::new(path)。

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

    println!("--- YAML 配置内容 ---");
    println!("{}", yaml_content);

    // 使用 serde_yaml_ng 解析 YAML（yaml feature 提供）
    let config: dbnexus::access::permission::PermissionConfig =
        serde_yaml_ng::from_str(yaml_content).expect("Failed to parse YAML config");

    println!("✓ YAML 配置解析成功\n");

    // 也可以通过编程方式构建相同的配置：
    let _manual_config = dbnexus::access::permission::PermissionConfig {
        roles: HashMap::from([
            (
                "admin".to_string(),
                dbnexus::access::permission::RolePolicy {
                    tables: vec![dbnexus::access::permission::TablePermission {
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
                dbnexus::access::permission::RolePolicy {
                    tables: vec![dbnexus::access::permission::TablePermission {
                        name: "users".to_string(),
                        operations: vec![PermissionAction::Select],
                    }],
                },
            ),
        ]),
    };

    // ============================================
    // 2. 创建 YamlPermissionProvider
    // ============================================
    let provider = YamlPermissionProvider::from_config(config);

    println!("--- 已配置的角色 ---");
    let mut roles = provider.get_roles();
    roles.sort();
    for role in &roles {
        println!("  • {}", role);
    }
    println!();

    // ============================================
    // 3. 演示不同角色的权限差异
    // ============================================
    println!("--- 权限检查结果 ---\n");

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
        let status = if result { "允许" } else { "拒绝" };

        if result == expected {
            passed += 1;
        } else {
            failed += 1;
        }

        println!(
            "  {} [{}] {}.{} → {} (期望 {})",
            mark, role, table, action, status, expected
        );
    }

    println!("\n  统计: {} 通过, {} 失败", passed, failed);

    // ============================================
    // 4. 展示角色策略详情
    // ============================================
    println!("\n--- 角色策略详情 ---\n");

    for role in &["admin", "manager", "guest"] {
        match provider.get_role_policy(role) {
            Some(policy) => {
                println!("  [{}] 策略:", role);
                for tp in &policy.tables {
                    let ops: Vec<String> = tp.operations.iter().map(|o| o.to_string()).collect();
                    println!("    表 {} → [{}]", tp.name, ops.join(", "));
                }
            }
            None => {
                println!("  [{}] 无策略", role);
            }
        }
    }

    // ============================================
    // 5. 结合 DbPool + Session（展示集成场景）
    // ============================================
    println!("\n--- DbPool 集成 ---\n");

    let db_config = DbConfig {
        url: "sqlite::memory:".to_string(),
        admin_role: "admin".to_string(),
        max_connections: 3,
        min_connections: 1,
        ..Default::default()
    };
    let pool = DbPool::with_config(db_config).await?;
    let session = pool.get_session("admin").await?;
    println!("✓ 连接池创建成功，当前 Session 角色: {}", session.role());

    // 在实际应用中，可将 YamlPermissionProvider 注入到 PermissionContext，
    // 实现缓存未命中时自动重新加载策略：
    //   let ctx = PermissionContext::new_with_provider(role, cache, Arc::new(provider));

    println!("\n========================================");
    println!("✨ YAML 权限策略示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - YamlPermissionProvider::from_config(config)  从配置创建提供者");
    println!("  - YamlPermissionProvider::new(path)           从 YAML 文件加载（需 yaml feature）");
    println!("  - serde_yaml_ng::from_str(yaml)              解析 YAML 字符串为 PermissionConfig");
    println!("  - PermissionConfig.roles                     角色到策略的 HashMap 映射");
    println!("  - RolePolicy.tables                          表权限列表");
    println!("  - TablePermission.name = \"*\"                 通配符匹配所有表");

    Ok(())
}
