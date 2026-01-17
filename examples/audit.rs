// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 审计日志示例
//!
//! 展示如何使用 dbnexus 的审计日志功能：
//! - 配置审计日志记录器
//! - 记录 CRUD 操作审计
//! - 手动记录审计事件
//! - 查询和导出审计日志
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example audit --features sqlite,audit
//! ```

use dbnexus::DbPool;
use dbnexus::audit::{
    AuditConfig, AuditEvent, AuditLogger, AuditOperation, AuditQueryFilters, AuditResult, AuditSeverity,
    MemoryAuditStorage,
};
use std::sync::Arc;

/// 定义 User 结构体（简化版，用于演示）
#[derive(Debug, Clone, PartialEq)]
struct User {
    id: i64,
    name: String,
    email: String,
    role: String,
}

/// 定义 Order 结构体
#[derive(Debug, Clone, PartialEq)]
struct Order {
    id: i64,
    user_id: i64,
    amount: f64,
    status: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("📋 DBNexus 审计日志示例\n");
    println!("========================================");

    // 1. 创建审计配置
    println!("\n1️⃣ 创建审计配置");
    println!("------------------------------------------");
    let audit_config = AuditConfig::default();
    println!("✓ 审计配置创建成功");
    println!("  - 启用审计: {}", audit_config.enabled);
    println!("  - 同步写入: {}", audit_config.sync_write);
    println!("  - 最大文件大小: {} MB", audit_config.max_file_size / (1024 * 1024));

    // 2. 创建审计日志记录器
    println!("\n2️⃣ 创建审计日志记录器");
    println!("------------------------------------------");
    let storage = Arc::new(MemoryAuditStorage::new(10000));
    let audit_logger = AuditLogger::new(audit_config, storage);
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

    // 创建表
    session
        .execute_raw_ddl(
            "CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                email TEXT NOT NULL,
                role TEXT NOT NULL
            )",
        )
        .await?;

    session
        .execute_raw_ddl(
            "CREATE TABLE orders (
                id INTEGER PRIMARY KEY,
                user_id INTEGER NOT NULL,
                amount REAL NOT NULL,
                status TEXT NOT NULL
            )",
        )
        .await?;

    // 插入用户并记录审计日志
    session
        .execute_raw("INSERT INTO users (id, name, email, role) VALUES (1, 'Alice', 'alice@example.com', 'admin')")
        .await?;
    println!("  ✓ 创建用户: Alice");
    audit_logger
        .log_create(
            "users",
            "1",
            "admin",
            Some(r#"{"name":"Alice","email":"alice@example.com","role":"admin"}"#.to_string()),
        )
        .await?;

    session
        .execute_raw("INSERT INTO users (id, name, email, role) VALUES (2, 'Bob', 'bob@example.com', 'user')")
        .await?;
    println!("  ✓ 创建用户: Bob");
    audit_logger
        .log_create(
            "users",
            "2",
            "admin",
            Some(r#"{"name":"Bob","email":"bob@example.com","role":"user"}"#.to_string()),
        )
        .await?;

    // 插入订单并记录审计日志
    session
        .execute_raw("INSERT INTO orders (id, user_id, amount, status) VALUES (1, 1, 99.99, 'pending')")
        .await?;
    println!("  ✓ 创建订单: #1");
    audit_logger
        .log_create(
            "orders",
            "1",
            "admin",
            Some(r#"{"user_id":1,"amount":99.99,"status":"pending"}"#.to_string()),
        )
        .await?;

    // 5. 演示手动审计日志记录
    println!("\n5️⃣ 手动记录审计日志");
    println!("------------------------------------------");

    // 记录登录事件
    let login_event = AuditEvent::new(
        AuditOperation::Login,
        "session",
        "session_123",
        "alice",
        "admin",
        "192.168.1.100",
    )
    .with_result(AuditResult::Success)
    .with_severity(AuditSeverity::Info);
    audit_logger.log(login_event).await?;
    println!("  ✓ 记录登录审计日志");

    // 记录敏感操作
    let sensitive_event = AuditEvent::new(
        AuditOperation::ConfigChange,
        "system_config",
        "config_1",
        "alice",
        "admin",
        "192.168.1.100",
    )
    .with_result(AuditResult::Success)
    .with_severity(AuditSeverity::High)
    .with_extra(r#"{"changed": "max_connections", "old_value": 10, "new_value": 20}"#);
    audit_logger.log(sensitive_event).await?;
    println!("  ✓ 记录配置变更审计日志");

    // 记录失败的权限变更尝试
    let failed_event = AuditEvent::new(
        AuditOperation::PermissionChange,
        "permissions",
        "user_2",
        "bob",
        "user",
        "192.168.1.101",
    )
    .with_result(AuditResult::Failure)
    .with_severity(AuditSeverity::Medium)
    .with_extra("Attempted to grant admin permissions without authorization");
    audit_logger.log(failed_event).await?;
    println!("  ✓ 记录失败的权限变更审计日志");

    // 6. 演示手动审计日志记录（UPDATE）
    println!("\n6️⃣ 手动审计日志记录（UPDATE）");
    println!("------------------------------------------");

    // 更新用户
    session
        .execute_raw("UPDATE users SET email = 'alice_new@example.com' WHERE id = 1")
        .await?;
    println!("  ✓ 更新用户 Alice");
    audit_logger
        .log_update(
            "users",
            "1",
            "admin",
            Some(r#"{"email":"alice@example.com"}"#.to_string()),
            Some(r#"{"email":"alice_new@example.com"}"#.to_string()),
        )
        .await?;
    println!("  ✓ 记录更新审计日志");

    // 删除用户
    session.execute_raw("DELETE FROM users WHERE id = 2").await?;
    println!("  ✓ 删除用户 Bob");
    audit_logger
        .log_delete(
            "users",
            "2",
            "admin",
            Some(r#"{"name":"Bob","email":"bob@example.com","role":"user"}"#.to_string()),
        )
        .await?;
    println!("  ✓ 记录删除审计日志");

    // 7. 查询审计日志
    println!("\n7️⃣ 查询审计日志");
    println!("------------------------------------------");

    let filters = AuditQueryFilters::default();
    let logs = audit_logger.query(&filters).await?;
    println!("  📊 总审计日志数: {}", logs.len());

    // 按操作类型统计（使用字符串作为 key）
    let mut op_counts = std::collections::HashMap::new();
    for log in &logs {
        *op_counts.entry(log.operation.to_string()).or_insert(0) += 1;
    }

    println!("  📈 按操作类型统计:");
    for (op, count) in op_counts {
        println!("    - {}: {}", op, count);
    }

    // 按严重级别统计
    let mut severity_counts = std::collections::HashMap::new();
    for log in &logs {
        *severity_counts.entry(log.severity.to_string()).or_insert(0) += 1;
    }

    println!("  📈 按严重级别统计:");
    for (sev, count) in severity_counts {
        println!("    - {}: {}", sev, count);
    }

    // 8. 导出审计日志
    println!("\n8️⃣ 导出审计日志");
    println!("------------------------------------------");

    // 导出为 JSON
    let json_output: Vec<String> = logs.iter().filter_map(|log| log.to_json().ok()).collect();
    let json_string = format!("[{}]", json_output.join(","));
    println!("  ✓ 审计日志已导出为 JSON 格式");
    println!("  📄 JSON 长度: {} 字符", json_string.len());

    // 9. 清理旧日志
    println!("\n9️⃣ 清理旧日志");
    println!("------------------------------------------");

    let removed = audit_logger.cleanup(1).await?;
    println!("  ✓ 清理了 {} 条旧日志", removed);

    println!("\n========================================");
    println!("✨ 审计日志示例运行完成！");

    // 注意：实际应用中应定期清理和归档审计日志
    println!("\n💡 提示: 生产环境中应定期备份和清理审计日志");

    Ok(())
}
