// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 审计日志示例
//!
//! 展示如何使用 dbnexus 的审计日志功能：
//! - 配置审计日志记录器
//! - 记录 CRUD 操作审计
//! - 使用 #[db_audit] 宏自动审计
//! - 查询和导出审计日志
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example audit --features sqlite,audit
//! ```

use dbnexus::audit::{
    AuditConfig, AuditEvent, AuditLogger, AuditOperation, AuditResult, AuditSeverity,
};
use dbnexus::{DbPool, DbEntity, db_crud, db_audit};
use std::time::Duration;

/// 定义 User Entity（带审计支持）
///
/// #[db_audit] 宏自动为 CRUD 操作添加审计日志
#[derive(DbEntity)]
#[db_entity]
#[table_name = "users")]
#[db_crud]
#[db_audit(operations = ["CREATE", "UPDATE", "DELETE"])]
struct User {
    #[primary_key]
    id: i64,
    name: String,
    email: String,
    role: String,
}

/// 定义 Order Entity（手动审计）
#[derive(DbEntity)]
#[db_entity]
#[table_name = "orders")]
#[db_crud]
struct Order {
    #[primary_key]
    id: i64,
    user_id: i64,
    amount: f64,
    status: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("📋 DBNexus 审计日志示例\n");
    println!("========================================");

    // 1. 创建审计配置
    println!("\n1️⃣ 创建审计配置");
    println!("------------------------------------------");
    let audit_config = AuditConfig::builder()
        .enabled(true)
        .log_level("info")
        .async_buffer_size(1000)
        .flush_interval(Duration::from_secs(30))
        .enable_file_output(true)
        .log_file_path("/tmp/dbnexus_audit.log")
        .enable_console_output(true)
        .build()?;
    println!("✓ 审计配置创建成功");

    // 2. 创建审计日志记录器
    println!("\n2️⃣ 创建审计日志记录器");
    println!("------------------------------------------");
    let audit_logger = AuditLogger::new(audit_config);
    println!("✓ 审计日志记录器创建成功");

    // 3. 初始化数据库连接池
    println!("\n3️⃣ 初始化数据库连接池");
    println!("------------------------------------------");
    let pool = DbPool::new("sqlite::memory:").await?;
    println!("✓ 连接池创建成功");

    // 4. 创建测试数据
    println!("\n4️⃣ 创建测试数据");
    println!("------------------------------------------");
    let mut session = pool.get_session("admin").await?;

    User::insert(
        &mut session,
        User {
            id: 1,
            name: "Alice".to_string(),
            email: "alice@example.com".to_string(),
            role: "admin".to_string(),
        },
    )
    .await?;
    println!("  ✓ 创建用户: Alice");

    User::insert(
        &mut session,
        User {
            id: 2,
            name: "Bob".to_string(),
            email: "bob@example.com".to_string(),
            role: "user".to_string(),
        },
    )
    .await?;
    println!("  ✓ 创建用户: Bob");

    Order::insert(
        &mut session,
        Order {
            id: 1,
            user_id: 1,
            amount: 99.99,
            status: "pending".to_string(),
        },
    )
    .await?;
    println!("  ✓ 创建订单: #1");

    // 5. 演示手动审计日志记录
    println!("\n5️⃣ 手动记录审计日志");
    println!("------------------------------------------");

    // 记录登录事件
    let login_event = AuditEvent::builder()
        .operation(AuditOperation::Login)
        .entity_type("session")
        .entity_id("session_123")
        .user_id("alice")
        .user_role("admin")
        .client_ip("192.168.1.100")
        .severity(AuditSeverity::Info)
        .result(AuditResult::Success)
        .build()?;
    audit_logger.log(&login_event).await;
    println!("  ✓ 记录登录审计日志");

    // 记录敏感操作
    let sensitive_event = AuditEvent::builder()
        .operation(AuditOperation::ConfigChange)
        .entity_type("system_config")
        .entity_id("config_1")
        .user_id("alice")
        .user_role("admin")
        .client_ip("192.168.1.100")
        .severity(AuditSeverity::High)
        .result(AuditResult::Success)
        .details(r#"{"changed": "max_connections", "old_value": 10, "new_value": 20}"#)
        .build()?;
    audit_logger.log(&sensitive_event).await;
    println!("  ✓ 记录配置变更审计日志");

    // 记录失败的权限变更尝试
    let failed_event = AuditEvent::builder()
        .operation(AuditOperation::PermissionChange)
        .entity_type("permissions")
        .entity_id("user_2")
        .user_id("bob")
        .user_role("user")
        .client_ip("192.168.1.101")
        .severity(AuditSeverity::Medium)
        .result(AuditResult::Failure)
        .details("Attempted to grant admin permissions without authorization")
        .build()?;
    audit_logger.log(&failed_event).await;
    println!("  ✓ 记录失败的权限变更审计日志");

    // 6. 演示使用 #[db_audit] 宏的自动审计
    println!("\n6️⃣ 自动审计日志（使用 #[db_audit] 宏）");
    println!("------------------------------------------");

    // 更新用户（自动审计）
    let mut alice = User::find_by_id(&session, 1)
        .await?
        .expect("Alice not found");
    alice.email = "alice_new@example.com".to_string();
    User::update(&session, alice).await?;
    println!("  ✓ 更新用户 Alice（自动审计）");

    // 删除用户（自动审计）
    User::delete(&session, 2).await?;
    println!("  ✓ 删除用户 Bob（自动审计）");

    // 7. 查询审计日志
    println!("\n7️⃣ 查询审计日志");
    println!("------------------------------------------");

    let logs = audit_logger.query().await?;
    println!("  📊 总审计日志数: {}", logs.len());

    // 按操作类型统计
    let mut op_counts = std::collections::HashMap::new();
    for log in &logs {
        *op_counts.entry(log.operation.clone()).or_insert(0) += 1;
    }

    println!("  📈 按操作类型统计:");
    for (op, count) in op_counts {
        println!("    - {}: {}", op, count);
    }

    // 按严重级别统计
    let mut severity_counts = std::collections::HashMap::new();
    for log in &logs {
        *severity_counts.entry(log.severity.clone()).or_insert(0) += 1;
    }

    println!("  📈 按严重级别统计:");
    for (sev, count) in severity_counts {
        println!("    - {}: {}", sev, count);
    }

    // 8. 导出审计日志
    println!("\n8️⃣ 导出审计日志");
    println!("------------------------------------------");

    // 导出为 JSON
    let json_output = audit_logger.export_json(&logs).await?;
    println!("  ✓ 审计日志已导出为 JSON 格式");
    println!("  📄 JSON 长度: {} 字符", json_output.len());

    // 导出为 CSV
    let csv_output = audit_logger.export_csv(&logs).await?;
    println!("  ✓ 审计日志已导出为 CSV 格式");
    println!("  📄 CSV 长度: {} 字符", csv_output.len());

    // 9. 审计日志统计
    println!("\n9️⃣ 审计日志统计信息");
    println!("------------------------------------------");

    let stats = audit_logger.get_stats().await?;
    println!("  📊 总事件数: {}", stats.total_events);
    println!("  📊 成功事件: {}", stats.success_count);
    println!("  📊 失败事件: {}", stats.failure_count);
    println!("  📊 严重事件: {}", stats.critical_count);

    println!("\n========================================");
    println!("✨ 审计日志示例运行完成！");

    // 注意：实际应用中应定期清理和归档审计日志
    println!("\n💡 提示: 生产环境中应定期备份和清理审计日志");

    Ok(())
}
