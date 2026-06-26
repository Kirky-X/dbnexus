// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in the project root for full license information.

//! 权限引擎与策略决策点（PDP）示例
//!
//! 演示 `PolicyDecisionPoint` 的使用，包括：
//! - 配置 `PolicyDecisionPointConfig`
//! - 创建 `RbacPermissionProvider` 并注册角色与权限规则
//! - 执行权限决策（`PermissionDecision`）
//! - 展示 PDP 的 builder 模式
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example permission_engine --features "sqlite,permission-engine"
//! ```

use dbnexus::access::permission_engine::{
    PermissionAction, PermissionContext, PermissionDecision, PermissionProvider, PermissionResource,
    PermissionRule, PermissionSubject, PolicyDecisionPoint, PolicyDecisionPointConfig,
    RbacPermissionProvider, Role,
};
use std::sync::Arc;

// ============================================
// 辅助函数：创建权限规则
// ============================================

/// 快速创建一条权限规则
fn rule(name: &str, priority: i32, subject: &str, resource: &str, allow: &[PermissionAction]) -> PermissionRule {
    PermissionRule {
        name: name.to_string(),
        priority,
        subject: subject.to_string(),
        resource: resource.to_string(),
        allow: allow.to_vec(),
        deny: vec![],
        condition: None,
        enabled: true,
    }
}

// ============================================
// 主函数
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("⚙️  DBNexus 权限引擎与 PDP 示例");
    println!("========================================\n");

    // ============================================
    // 1. 创建 RbacPermissionProvider 并注册角色
    // ============================================
    let provider = Arc::new(RbacPermissionProvider::new());

    // admin 角色：通配符权限，最高优先级
    provider.add_role(Role {
        name: "admin".to_string(),
        description: "管理员，拥有全部权限".to_string(),
        enabled: true,
        extends: vec![],
    });
    provider.add_permission(
        "admin",
        rule("admin_all", 100, "*", "*", &[
            PermissionAction::Select,
            PermissionAction::Insert,
            PermissionAction::Update,
            PermissionAction::Delete,
        ]),
    );

    // editor 角色：可读写 articles，可读 users
    provider.add_role(Role {
        name: "editor".to_string(),
        description: "编辑，可读写 articles".to_string(),
        enabled: true,
        extends: vec![],
    });
    provider.add_permission(
        "editor",
        rule("editor_articles_rw", 50, "*", "articles", &[
            PermissionAction::Select,
            PermissionAction::Insert,
            PermissionAction::Update,
        ]),
    );
    provider.add_permission(
        "editor",
        rule("editor_users_ro", 50, "*", "users", &[PermissionAction::Select]),
    );

    // viewer 角色：只读
    provider.add_role(Role {
        name: "viewer".to_string(),
        description: "访客，只读".to_string(),
        enabled: true,
        extends: vec!["editor".to_string()], // 继承 editor 的部分权限
    });
    provider.add_permission(
        "viewer",
        rule("viewer_articles_ro", 10, "*", "articles", &[PermissionAction::Select]),
    );

    // 将用户映射到角色
    provider.add_role_to_subject("alice", "admin");
    provider.add_role_to_subject("bob", "editor");
    provider.add_role_to_subject("charlie", "viewer");

    println!("✓ 已创建 3 个角色: admin / editor / viewer");
    println!("✓ 已映射 3 个用户: alice(admin) / bob(editor) / charlie(viewer)\n");

    // ============================================
    // 2. 使用 PolicyDecisionPoint 进行权限决策
    // ============================================
    println!("--- PolicyDecisionPoint 权限决策 ---\n");

    let pdp_config = PolicyDecisionPointConfig {
        default_decision: PermissionDecision::Deny,
        log_denied: true,
        cache_ttl_seconds: 600,
        cache_enabled: true,
    };
    let pdp = PolicyDecisionPoint::with_config(provider.clone(), pdp_config);

    let test_cases = [
        // alice (admin) — 全部允许
        ("alice", "users", "SELECT", PermissionDecision::Allow),
        ("alice", "users", "DELETE", PermissionDecision::Allow),
        ("alice", "articles", "INSERT", PermissionDecision::Allow),
        // bob (editor) — articles 读写，users 只读
        ("bob", "articles", "SELECT", PermissionDecision::Allow),
        ("bob", "articles", "INSERT", PermissionDecision::Allow),
        ("bob", "articles", "DELETE", PermissionDecision::Deny),
        ("bob", "users", "SELECT", PermissionDecision::Allow),
        ("bob", "users", "DELETE", PermissionDecision::Deny),
        // charlie (viewer) — articles 只读
        ("charlie", "articles", "SELECT", PermissionDecision::Allow),
        ("charlie", "articles", "INSERT", PermissionDecision::Deny),
        ("charlie", "users", "SELECT", PermissionDecision::Deny),
    ];

    for (user, resource, action, expected) in test_cases {
        let decision = pdp.check(user, resource, action).await;
        let mark = if decision == expected { "✔" } else { "✘" };
        println!(
            "  {} {}.{} {} → {:?} (期望 {:?})",
            mark, user, resource, action, decision, expected
        );
    }

    // ============================================
    // 3. 使用 PolicyDecisionPoint builder 模式
    // ============================================
    println!("\n--- PolicyDecisionPoint Builder 模式 ---\n");

    let pdp = PolicyDecisionPoint::builder()
        .provider(provider.clone())
        .cache_ttl_seconds(300)
        .cache_enabled(true)
        .rate_limit(200, 60)
        .build();

    println!("✓ PDP 创建成功（cache_ttl=300s, rate_limit=200/min）\n");

    // 使用 PermissionContext 进行更细粒度的权限检查
    let context = PermissionContext::new(
        PermissionSubject::user("bob"),
        PermissionResource::new("articles"),
        PermissionAction::Insert,
    )
    .with_attribute("ip", "192.168.1.100")
    .with_environment("source", "web");

    println!("--- PermissionContext 权限检查 ---\n");
    println!("  上下文: subject={}, resource={}, action={:?}",
        context.subject.id, context.resource.name, context.action);
    println!("  属性: {:?}", context.attributes);
    println!("  环境: {:?}", context.environment);

    let decision = pdp.check_permission(&context).await;
    println!("  决策: {:?}\n", decision);

    // 批量权限检查
    println!("--- 批量权限检查 ---\n");
    let contexts = vec![
        PermissionContext::new(
            PermissionSubject::user("alice"),
            PermissionResource::new("users"),
            PermissionAction::Delete,
        ),
        PermissionContext::new(
            PermissionSubject::user("bob"),
            PermissionResource::new("users"),
            PermissionAction::Delete,
        ),
        PermissionContext::new(
            PermissionSubject::user("charlie"),
            PermissionResource::new("articles"),
            PermissionAction::Select,
        ),
    ];

    let results = pdp.check_batch(contexts).await;
    for (ctx, decision) in &results {
        println!(
            "  {} {}.{} → {:?}",
            ctx.subject.id, ctx.resource.name, ctx.action, decision
        );
    }

    // ============================================
    // 4. 获取可访问资源和操作
    // ============================================
    println!("\n--- 资源与操作查询 ---\n");

    for user in &["alice", "bob", "charlie"] {
        let resources = pdp.get_allowed_resources(user).await;
        let resource_names: Vec<&str> = resources.iter().map(|r| r.name.as_str()).collect();
        println!("  {} 可访问资源: {:?}", user, resource_names);

        if let Some(first_resource) = resources.first() {
            // get_allowed_actions 是 PermissionProvider trait 的方法，直接在 provider 上调用
            let actions = provider.get_allowed_actions(user, &first_resource.name).await;
            println!("    {} 对 {} 的操作: {:?}", user, first_resource.name, actions);
        }
    }

    // ============================================
    // 5. 速率限制演示
    // ============================================
    println!("\n--- 速率限制演示 ---\n");

    let limited_pdp = PolicyDecisionPoint::with_rate_limit(provider.clone(), 5, 60);
    println!("  速率限制: 5 次请求/分钟");

    let mut allowed_count = 0;
    let mut denied_count = 0;
    for i in 1..=7 {
        let decision = limited_pdp.check("alice", "users", "SELECT").await;
        match decision {
            PermissionDecision::Allow => allowed_count += 1,
            _ => denied_count += 1,
        }
        println!("  请求 {}: {:?}", i, decision);
    }
    println!("  统计: {} 允许, {} 拒绝（限流）", allowed_count, denied_count);

    println!("\n========================================");
    println!("✨ 权限引擎与 PDP 示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - RbacPermissionProvider::new()   创建 RBAC 权限提供者");
    println!("  - provider.add_role(Role{{...}})  添加角色（支持继承）");
    println!("  - provider.add_permission(role, rule)  添加权限规则");
    println!("  - provider.add_role_to_subject(user, role)  映射用户到角色");
    println!("  - PolicyDecisionPoint::with_config(provider, config)  创建带配置的策略决策点");
    println!("  - pdp.check(user, res, act)  获取详细决策");
    println!("  - PolicyDecisionPoint::builder()...build()  Builder 模式创建 PDP");
    println!("  - PermissionContext  包含主体/资源/操作/属性/环境的上下文");
    println!("  - PermissionDecision::Allow / Deny / NotApplicable / Error");

    Ok(())
}
