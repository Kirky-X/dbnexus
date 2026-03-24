// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 权限引擎示例
//!
//! 展示如何使用 dbnexus 的高级权限引擎功能：
//! - 策略决策点 (PolicyDecisionPoint)
//! - YAML 权限提供者
//! - RBAC 权限提供者
//! - 权限上下文和决策
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example permission_engine --features "sqlite,permission-engine,cache"
//! ```

use dbnexus::permission_engine::{
    PermissionAction, PermissionContext, PermissionDecision, PermissionProvider, PermissionResource, PermissionRule,
    PermissionSubject, PolicyDecisionPoint, RbacPermissionProvider, Role, YamlPermissionProvider,
};
use dbnexus::{DbConfig, DbPool};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔐 DBNexus 权限引擎示例\n");
    println!("========================================");

    // 1. 初始化数据库连接池
    println!("\n1️⃣ 初始化数据库连接池");
    println!("------------------------------------------");
    let config = DbConfig {
        url: "sqlite:file::memory:?cache=shared".to_string(),
        permissions_path: Some("examples/demo/permissions.yaml".to_string()),
        admin_role: "admin".to_string(),
        ..Default::default()
    };
    let _pool = DbPool::with_config(config).await?;
    println!("✓ 连接池创建成功");

    // 2. 创建 YAML 权限提供者
    println!("\n2️⃣ 创建 YAML 权限提供者");
    println!("------------------------------------------");
    let yaml_provider = YamlPermissionProvider::new("examples/demo/permissions.yaml")?;
    println!("✓ YAML 权限提供者创建成功");
    println!("  提供者名称: {}", yaml_provider.name());

    // 3. 创建策略决策点
    println!("\n3️⃣ 创建策略决策点");
    println!("------------------------------------------");
    let pdp = PolicyDecisionPoint::new(Arc::new(yaml_provider));
    println!("✓ 策略决策点创建成功");
    println!("  缓存 TTL: 300 秒");
    println!("  速率限制: 100 请求/分钟");

    // 4. 创建权限上下文
    println!("\n4️⃣ 创建权限上下文");
    println!("------------------------------------------");
    let subject = PermissionSubject::role("admin");
    let resource = PermissionResource::new("users");
    let action = PermissionAction::Select;

    let context = PermissionContext::new(subject.clone(), resource.clone(), action.clone())
        .with_attribute("department", "engineering")
        .with_environment("source", "web");

    println!("✓ 权限上下文创建成功");
    println!("  主体: {} (role)", subject.id);
    println!("  资源: {} ({})", resource.name, resource.resource_type);
    println!("  操作: {}", action);

    // 5. 检查权限
    println!("\n5️⃣ 检查权限");
    println!("------------------------------------------");
    let decision = pdp.check_permission(&context).await;
    match decision {
        PermissionDecision::Allow => println!("  ✅ 允许访问"),
        PermissionDecision::Deny => println!("  ❌ 拒绝访问"),
        PermissionDecision::NotApplicable => println!("  ⚠️ 未找到策略"),
        PermissionDecision::Error(e) => println!("  ❌ 错误: {}", e),
    }

    // 6. 测试不同角色
    println!("\n6️⃣ 测试不同角色");
    println!("------------------------------------------");

    let roles = vec!["admin", "manager", "viewer"];
    let actions = vec![
        PermissionAction::Select,
        PermissionAction::Insert,
        PermissionAction::Update,
        PermissionAction::Delete,
    ];

    for role in roles {
        println!("\n  角色: {}", role);
        let subject = PermissionSubject::role(role);

        for action in &actions {
            let context = PermissionContext::new(subject.clone(), PermissionResource::new("users"), action.clone());
            let decision = pdp.check_permission(&context).await;
            let symbol = match decision {
                PermissionDecision::Allow => "✅",
                PermissionDecision::Deny => "❌",
                _ => "⚠️",
            };
            println!("    {} {}: {:?}", symbol, action, decision);
        }
    }

    // 7. 创建 RBAC 权限提供者
    println!("\n7️⃣ 创建 RBAC 权限提供者");
    println!("------------------------------------------");
    let rbac_provider = RbacPermissionProvider::new();

    // 添加角色
    rbac_provider.add_role(Role {
        name: "admin".to_string(),
        description: "Administrator role".to_string(),
        enabled: true,
        extends: vec![],
    });
    rbac_provider.add_role(Role {
        name: "editor".to_string(),
        description: "Editor role".to_string(),
        enabled: true,
        extends: vec![],
    });
    rbac_provider.add_role(Role {
        name: "viewer".to_string(),
        description: "Viewer role".to_string(),
        enabled: true,
        extends: vec![],
    });

    // 添加权限规则
    rbac_provider.add_permission(
        "admin",
        PermissionRule {
            name: "admin-all".to_string(),
            priority: 100,
            subject: "admin".to_string(),
            resource: "*".to_string(),
            // admin 拥有所有基本操作权限
            allow: vec![
                PermissionAction::Select,
                PermissionAction::Insert,
                PermissionAction::Update,
                PermissionAction::Delete,
            ],
            deny: vec![],
            condition: None,
            enabled: true,
        },
    );
    rbac_provider.add_permission(
        "editor",
        PermissionRule {
            name: "editor-articles".to_string(),
            priority: 50,
            subject: "editor".to_string(),
            resource: "articles".to_string(),
            allow: vec![
                PermissionAction::Select,
                PermissionAction::Insert,
                PermissionAction::Update,
            ],
            deny: vec![],
            condition: None,
            enabled: true,
        },
    );
    rbac_provider.add_permission(
        "viewer",
        PermissionRule {
            name: "viewer-articles".to_string(),
            priority: 10,
            subject: "viewer".to_string(),
            resource: "articles".to_string(),
            allow: vec![PermissionAction::Select],
            deny: vec![],
            condition: None,
            enabled: true,
        },
    );

    println!("✓ RBAC 权限提供者创建成功");
    println!("  提供者名称: {}", rbac_provider.name());

    // 8. 使用 RBAC 提供者
    println!("\n8️⃣ 使用 RBAC 提供者检查权限");
    println!("------------------------------------------");
    let rbac_pdp = PolicyDecisionPoint::new(Arc::new(rbac_provider));

    let test_cases = vec![
        ("admin", "articles", PermissionAction::Delete),
        ("editor", "articles", PermissionAction::Update),
        ("editor", "articles", PermissionAction::Delete),
        ("viewer", "articles", PermissionAction::Select),
        ("viewer", "articles", PermissionAction::Insert),
    ];

    for (role, resource, action) in test_cases {
        let context = PermissionContext::new(
            PermissionSubject::role(role),
            PermissionResource::new(resource),
            action.clone(),
        );
        let decision = rbac_pdp.check_permission(&context).await;
        let symbol = match decision {
            PermissionDecision::Allow => "✅",
            PermissionDecision::Deny => "❌",
            _ => "⚠️",
        };
        println!("  {} {} -> {} / {}: {:?}", symbol, role, resource, action, decision);
    }

    // 9. 获取允许的资源
    println!("\n9️⃣ 获取允许的资源");
    println!("------------------------------------------");
    let resources = rbac_pdp.get_allowed_resources("editor").await;
    println!("  editor 可访问的资源:");
    for res in resources {
        println!("    - {} ({})", res.name, res.resource_type);
    }

    // 10. 刷新权限缓存
    println!("\n🔟 刷新权限缓存");
    println!("------------------------------------------");
    rbac_pdp.refresh_cache().await;
    println!("✓ 权限缓存已刷新");

    println!("\n=== 所有权限引擎示例完成 ===");
    Ok(())
}
