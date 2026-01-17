// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 权限引擎示例
//!
//! 展示如何使用 dbnexus 的权限引擎功能：
//! - 创建权限提供者
//! - 使用策略决策点（PDP）
//! - 定义权限规则
//! - 检查权限
//! - 使用 RBAC 权限提供者
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example permission_engine --features "sqlite,permission-engine"
//! ```

use dbnexus::{
    EnginePermissionAction as PermissionAction, PermissionDecision, PermissionEngineContext as PermissionContext,
    PermissionProvider, PermissionResource, PermissionRule, PermissionSubject, PolicyDecisionPoint,
    RbacPermissionProvider, Role,
};
use std::sync::Arc;
use std::sync::RwLock;

/// 自定义权限提供者示例
#[derive(Debug)]
struct CustomPermissionProvider {
    rules: RwLock<Vec<PermissionRule>>,
}

impl CustomPermissionProvider {
    fn new() -> Self {
        Self {
            rules: RwLock::new(Vec::new()),
        }
    }

    fn add_rule(&self, rule: PermissionRule) {
        if let Ok(mut rules) = self.rules.write() {
            rules.push(rule);
        }
    }

    fn list_rules(&self) -> Vec<PermissionRule> {
        if let Ok(rules) = self.rules.read() {
            rules.clone()
        } else {
            Vec::new()
        }
    }
}

#[async_trait::async_trait]
impl PermissionProvider for CustomPermissionProvider {
    async fn check_permission(&self, context: &PermissionContext) -> PermissionDecision {
        let rules = if let Ok(r) = self.rules.read() {
            r.clone()
        } else {
            return PermissionDecision::Error("Lock error".to_string());
        };

        // 按优先级排序规则
        let mut matching_rules: Vec<&PermissionRule> = rules
            .iter()
            .filter(|rule| {
                rule.enabled
                    && self.matches_subject(&rule.subject, &context.subject.id)
                    && self.matches_resource(&rule.resource, &context.resource.name)
            })
            .collect();

        matching_rules.sort_by(|a, b| b.priority.cmp(&a.priority));

        // 评估规则
        for rule in matching_rules {
            if rule.deny.contains(&context.action) || rule.deny.contains(&PermissionAction::All) {
                return PermissionDecision::Deny;
            }
            if rule.allow.contains(&context.action) || rule.allow.contains(&PermissionAction::All) {
                return PermissionDecision::Allow;
            }
        }

        PermissionDecision::NotApplicable
    }

    async fn get_allowed_resources(&self, subject: &str) -> Vec<PermissionResource> {
        let rules = if let Ok(r) = self.rules.read() {
            r.clone()
        } else {
            return Vec::new();
        };

        let mut resources = std::collections::HashSet::new();
        for rule in &rules {
            if rule.enabled && self.matches_subject(&rule.subject, subject) {
                resources.insert(PermissionResource::new(&rule.resource));
            }
        }

        resources.into_iter().collect()
    }

    async fn get_allowed_actions(&self, subject: &str, resource: &str) -> Vec<PermissionAction> {
        let rules = if let Ok(r) = self.rules.read() {
            r.clone()
        } else {
            return Vec::new();
        };

        let mut actions = std::collections::HashSet::new();
        for rule in &rules {
            if rule.enabled
                && self.matches_subject(&rule.subject, subject)
                && self.matches_resource(&rule.resource, resource)
            {
                for action in &rule.allow {
                    actions.insert(action.clone());
                }
            }
        }

        actions.into_iter().collect()
    }

    async fn refresh(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 自定义提供者不需要刷新
        Ok(())
    }

    fn name(&self) -> &str {
        "custom"
    }
}

impl CustomPermissionProvider {
    fn matches_subject(&self, pattern: &str, subject: &str) -> bool {
        pattern == "*" || pattern == subject
    }

    fn matches_resource(&self, pattern: &str, resource: &str) -> bool {
        pattern == "*" || pattern == resource
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔐 DBNexus 权限引擎示例\n");
    println!("========================================");

    // 1. 创建自定义权限提供者
    println!("\n1️⃣ 创建自定义权限提供者");
    println!("------------------------------------------");

    let custom_provider = Arc::new(CustomPermissionProvider::new());

    // 添加权限规则
    custom_provider.add_rule(PermissionRule {
        name: "admin_full_access".to_string(),
        priority: 100,
        subject: "admin".to_string(),
        resource: "*".to_string(),
        allow: vec![PermissionAction::All],
        deny: vec![],
        condition: None,
        enabled: true,
    });

    custom_provider.add_rule(PermissionRule {
        name: "manager_read_write".to_string(),
        priority: 50,
        subject: "manager".to_string(),
        resource: "users".to_string(),
        allow: vec![
            PermissionAction::Select,
            PermissionAction::Insert,
            PermissionAction::Update,
        ],
        deny: vec![PermissionAction::Delete],
        condition: None,
        enabled: true,
    });

    custom_provider.add_rule(PermissionRule {
        name: "user_read_only".to_string(),
        priority: 10,
        subject: "user".to_string(),
        resource: "users".to_string(),
        allow: vec![PermissionAction::Select],
        deny: vec![
            PermissionAction::Insert,
            PermissionAction::Update,
            PermissionAction::Delete,
        ],
        condition: None,
        enabled: true,
    });

    println!("✓ 自定义权限提供者创建成功");
    println!("  📋 已定义 {} 条规则", custom_provider.list_rules().len());

    // 2. 创建策略决策点（PDP）
    println!("\n2️⃣ 创建策略决策点（PDP）");
    println!("------------------------------------------");

    let pdp = PolicyDecisionPoint::new(custom_provider);
    println!("✓ 策略决策点创建成功");

    // 3. 测试权限检查
    println!("\n3️⃣ 测试权限检查");
    println!("------------------------------------------");

    let test_cases = vec![
        ("admin", "users", "SELECT"),
        ("admin", "orders", "INSERT"),
        ("admin", "products", "DELETE"),
        ("manager", "users", "SELECT"),
        ("manager", "users", "INSERT"),
        ("manager", "users", "UPDATE"),
        ("manager", "users", "DELETE"),
        ("user", "users", "SELECT"),
        ("user", "users", "INSERT"),
        ("user", "users", "UPDATE"),
        ("user", "users", "DELETE"),
        ("guest", "users", "SELECT"),
    ];

    for (subject, resource, action) in test_cases {
        let context = PermissionContext::new(
            PermissionSubject::user(subject),
            PermissionResource::new(resource),
            parse_action(action),
        );

        let decision = pdp.check_permission(&context).await;

        println!("  {} -> {} -> {}: {:?}", subject, resource, action, decision);
    }

    // 4. 使用 RBAC 权限提供者
    println!("\n4️⃣ 使用 RBAC 权限提供者");
    println!("------------------------------------------");

    let rbac_provider = Arc::new(RbacPermissionProvider::new());
    println!("✓ RBAC 权限提供者创建成功");

    // 添加角色
    rbac_provider.add_role(Role {
        name: "admin".to_string(),
        description: "管理员角色".to_string(),
        enabled: true,
        extends: vec![],
    });

    rbac_provider.add_role(Role {
        name: "manager".to_string(),
        description: "经理角色".to_string(),
        enabled: true,
        extends: vec![],
    });

    rbac_provider.add_role(Role {
        name: "user".to_string(),
        description: "普通用户角色".to_string(),
        enabled: true,
        extends: vec![],
    });
    println!("  ✓ 添加角色: admin, manager, user");

    // 定义角色权限
    rbac_provider.add_permission(
        "admin",
        PermissionRule {
            name: "admin_all_access".to_string(),
            priority: 100,
            subject: "*".to_string(),
            resource: "*".to_string(),
            allow: vec![PermissionAction::All],
            deny: vec![],
            condition: None,
            enabled: true,
        },
    );

    rbac_provider.add_permission(
        "manager",
        PermissionRule {
            name: "manager_users_access".to_string(),
            priority: 50,
            subject: "*".to_string(),
            resource: "users".to_string(),
            allow: vec![
                PermissionAction::Select,
                PermissionAction::Insert,
                PermissionAction::Update,
            ],
            deny: vec![PermissionAction::Delete],
            condition: None,
            enabled: true,
        },
    );

    rbac_provider.add_permission(
        "user",
        PermissionRule {
            name: "user_read_only".to_string(),
            priority: 10,
            subject: "*".to_string(),
            resource: "users".to_string(),
            allow: vec![PermissionAction::Select],
            deny: vec![
                PermissionAction::Insert,
                PermissionAction::Update,
                PermissionAction::Delete,
            ],
            condition: None,
            enabled: true,
        },
    );
    println!("  ✓ 定义角色权限");

    // 创建 PDP
    let rbac_pdp = PolicyDecisionPoint::new(rbac_provider);

    // 测试 RBAC 权限（在 RBAC 中，用户名即角色名）
    let rbac_test_cases = vec![
        ("admin", "users", "SELECT"),
        ("admin", "orders", "DELETE"),
        ("manager", "users", "SELECT"),
        ("manager", "users", "DELETE"),
        ("user", "users", "SELECT"),
        ("user", "users", "INSERT"),
    ];

    println!("\n  📋 RBAC 权限检查结果:");
    for (user, resource, action) in rbac_test_cases {
        let decision = rbac_pdp.check(user, resource, action).await;

        println!("    {} -> {} -> {}: {:?}", user, resource, action, decision);
    }

    // 5. 使用权限上下文
    println!("\n5️⃣ 使用权限上下文");
    println!("------------------------------------------");

    let context = PermissionContext::new(
        PermissionSubject::user("manager"),
        PermissionResource::new("orders"),
        PermissionAction::Insert,
    )
    .with_attribute("department", "sales")
    .with_attribute("region", "west")
    .with_environment("ip", "192.168.1.100")
    .with_environment("time", "2024-01-01T10:00:00Z");

    println!("  📋 权限上下文:");
    println!("    主体: {}", context.subject.id);
    println!(
        "    资源: {} ({})",
        context.resource.name, context.resource.resource_type
    );
    println!("    操作: {}", context.action);
    println!("    属性: {:?}", context.attributes);
    println!("    环境: {:?}", context.environment);

    // 6. 获取允许的资源
    println!("\n6️⃣ 获取允许的资源");
    println!("------------------------------------------");

    let allowed_resources = pdp.get_allowed_resources("admin").await;
    println!("  📋 admin 可访问的资源:");
    for resource in &allowed_resources {
        println!("    - {}", resource.name);
    }

    // 7. 获取允许的操作
    println!("\n7️⃣ 获取允许的操作");
    println!("------------------------------------------");

    let allowed_actions = pdp.get_allowed_resources("manager").await;
    println!("  📋 manager 可访问的资源:");
    for resource in &allowed_actions {
        println!("    - {}", resource.name);
    }

    // 8. 刷新权限缓存
    println!("\n8️⃣ 刷新权限缓存");
    println!("------------------------------------------");

    pdp.refresh_cache().await;
    println!("  ✓ 权限缓存已刷新");

    // 9. 演示权限决策流程
    println!("\n9️⃣ 演示权限决策流程");
    println!("------------------------------------------");

    println!("  📋 权限决策流程:");
    println!("     1. 接收权限检查请求");
    println!("     2. 构建权限上下文");
    println!("     3. 检查缓存（如果启用）");
    println!("     4. 调用权限提供者检查权限");
    println!("     5. 遍历权限规则（按优先级排序）");
    println!("     6. 匹配主体、资源和操作");
    println!("     7. 检查拒绝规则");
    println!("     8. 检查允许规则");
    println!("     9. 返回决策结果");
    println!("    10. 更新缓存（如果启用）");

    // 10. 演示权限决策结果
    println!("\n🔟 演示权限决策结果");
    println!("------------------------------------------");

    println!("  📋 权限决策结果类型:");
    println!("     - Allow: 允许操作");
    println!("     - Deny: 拒绝操作");
    println!("     - NotApplicable: 未找到相关策略");
    println!("     - Error: 检查过程中发生错误");

    // 11. 演示权限缓存
    println!("\n1️⃣1️⃣ 演示权限缓存");
    println!("------------------------------------------");

    println!("  💡 权限缓存机制:");
    println!("     - 缓存权限检查结果");
    println!("     - 减少重复计算");
    println!("     - 提高性能");
    println!("     - 默认 TTL: 5 分钟");
    println!("     - 支持缓存失效");

    println!("\n========================================");
    println!("✨ 权限引擎示例运行完成！");

    println!("\n💡 提示:");
    println!("  - 权限引擎支持多种权限提供者");
    println!("  - 可以自定义权限规则和策略");
    println!("  - 支持基于角色的访问控制（RBAC）");
    println!("  - 支持基于属性的访问控制（ABAC）");
    println!("  - 在生产环境中应该使用持久化的权限存储");
    println!("  - RBAC 中用户名即角色名");

    Ok(())
}

/// 解析操作字符串
fn parse_action(action: &str) -> PermissionAction {
    match action.to_uppercase().as_str() {
        "SELECT" => PermissionAction::Select,
        "INSERT" => PermissionAction::Insert,
        "UPDATE" => PermissionAction::Update,
        "DELETE" => PermissionAction::Delete,
        "ALL" | "*" => PermissionAction::All,
        _ => PermissionAction::Select,
    }
}
