// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 审计日志示例
//!
//! 演示 `AuditLogger` 的完整使用流程：
//! - 配置 `AuditConfig`（敏感字段、告警操作、容量等）
//! - 使用 `AuditEventBuilder` 链式构建审计事件
//! - 展示不同 `AuditSeverity` 级别与 `AuditOperation` 类型
//! - 通过 `MemoryAuditStorage` 存储审计日志
//! - 使用 `AuditQueryFilters` 查询审计日志
//! - 展示敏感数据自动脱敏
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example audit_logging --features "sqlite,audit"
//! ```

use dbnexus::{
    AuditConfig, AuditEvent, AuditEventBuilder, AuditLogger, AuditOperation, AuditQueryFilters, AuditSeverity,
    AuditStatus, MemoryAuditStorage,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("========================================");
    println!("📝 DBNexus 审计日志示例");
    println!("========================================\n");

    // ============================================
    // 1. 配置 AuditConfig
    // ============================================
    println!("--- 1. AuditConfig 配置 ---");
    let config = AuditConfig {
        enabled: true,
        storage_path: None,
        sync_write: false,
        max_file_size: 10 * 1024 * 1024, // 10MB
        retention_count: 7,
        sensitive_fields: vec![
            "password".to_string(),
            "token".to_string(),
            "secret".to_string(),
            "api_key".to_string(),
        ],
        alert_operations: vec![
            AuditOperation::Delete,
            AuditOperation::PermissionChange,
            AuditOperation::ConfigChange,
        ],
        alert_severity: AuditSeverity::High,
    };
    println!("  enabled         = {}", config.enabled);
    println!("  max_file_size   = {} bytes", config.max_file_size);
    println!("  retention_count = {} days", config.retention_count);
    println!("  sensitive_fields = {:?}", config.sensitive_fields);
    println!(
        "  alert_operations = {:?}",
        config
            .alert_operations
            .iter()
            .map(|o| o.to_string())
            .collect::<Vec<_>>()
    );

    // ============================================
    // 2. 创建 AuditLogger + MemoryAuditStorage
    // ============================================
    println!("\n--- 2. 创建 AuditLogger 与 MemoryAuditStorage ---");
    let storage = Arc::new(MemoryAuditStorage::new(1000));
    let storage_for_query = storage.clone();
    let logger = AuditLogger::with_config(config, storage);

    println!("  ✓ MemoryAuditStorage 容量 = 1000");
    println!("  ✓ AuditLogger 创建成功（with_config）");

    // ============================================
    // 3. AuditOperation 全类型展示
    // ============================================
    println!("\n--- 3. AuditOperation 类型 ---");
    let operations = [
        AuditOperation::Create,
        AuditOperation::Read,
        AuditOperation::Update,
        AuditOperation::Delete,
        AuditOperation::Login,
        AuditOperation::Logout,
        AuditOperation::PermissionChange,
        AuditOperation::ConfigChange,
        AuditOperation::Other("custom_export".to_string()),
    ];
    for op in &operations {
        println!("  - {}", op.to_string());
    }

    // ============================================
    // 4. AuditSeverity 级别展示
    // ============================================
    println!("\n--- 4. AuditSeverity 级别 ---");
    let severities = [
        AuditSeverity::Info,
        AuditSeverity::Low,
        AuditSeverity::Medium,
        AuditSeverity::High,
        AuditSeverity::Critical,
    ];
    for sev in &severities {
        println!("  - {}", sev.to_string());
    }

    // ============================================
    // 5. 使用 AuditEventBuilder 链式构建审计事件
    // ============================================
    println!("\n--- 5. AuditEventBuilder 链式构建 ---");
    let event = AuditEventBuilder::new()
        .operation(AuditOperation::Update)
        .entity_type("users")
        .entity_id("u_10086")
        .user_id("admin")
        .user_role("administrator")
        .client_ip("192.168.1.100")
        .severity(AuditSeverity::Medium)
        .result(AuditStatus::Success)
        .before_value(r#"{"name":"old_name","email":"old@example.com"}"#)
        .after_value(r#"{"name":"new_name","email":"new@example.com"}"#)
        .extra(r#"{"reason":"user profile update"}"#)
        .request_id("req-abc-123")
        .session_id("sess-xyz-789")
        .build()?;
    println!("  ✓ 构建审计事件:");
    println!("    operation    = {}", event.operation);
    println!("    entity_type  = {}", event.entity_type);
    println!("    entity_id    = {}", event.entity_id);
    println!("    user_id      = {}", event.user_id);
    println!("    severity     = {}", event.severity);
    println!("    result       = {}", event.result);
    println!("    request_id   = {}", event.request_id);

    // ============================================
    // 6. 使用快捷构造方法记录各类审计事件
    // ============================================
    println!("\n--- 6. 记录各类审计事件 ---");

    // CREATE
    logger
        .log_create("users", "u_001", "admin", Some(r#"{"name":"alice"}"#.to_string()))
        .await?;
    println!("  ✓ log_create: users/u_001 by admin");

    // READ
    logger.log_read("users", "u_001", "admin").await?;
    println!("  ✓ log_read: users/u_001 by admin");

    // UPDATE（带前后值）
    logger
        .log_update(
            "orders",
            "o_200",
            "alice",
            Some(r#"{"status":"pending"}"#.to_string()),
            Some(r#"{"status":"shipped"}"#.to_string()),
        )
        .await?;
    println!("  ✓ log_update: orders/o_200 by alice");

    // DELETE（高危操作，会触发告警）
    logger
        .log_delete("users", "u_002", "admin", Some(r#"{"name":"bob"}"#.to_string()))
        .await?;
    println!("  ✓ log_delete: users/u_002 by admin (高危，触发告警)");

    // LOGIN
    let login_event = AuditEvent::new(
        AuditOperation::Login,
        "auth",
        "session_001",
        "alice",
        "user",
        "10.0.0.1",
    )
    .with_severity(AuditSeverity::Low);
    logger.log(login_event).await?;
    println!("  ✓ Login 事件: session_001 by alice");

    // PERMISSION_CHANGE
    let perm_event = AuditEvent::new(
        AuditOperation::PermissionChange,
        "rbac",
        "role_editor",
        "admin",
        "admin",
        "127.0.0.1",
    )
    .with_severity(AuditSeverity::High)
    .with_extra(r#"{"granted":"delete"}"#);
    logger.log(perm_event).await?;
    println!("  ✓ PermissionChange 事件: role_editor by admin");

    // CONFIG_CHANGE
    let config_event = AuditEventBuilder::new()
        .operation(AuditOperation::ConfigChange)
        .entity_type("system")
        .entity_id("config_v2")
        .user_id("admin")
        .severity(AuditSeverity::Critical)
        .result(AuditStatus::Success)
        .build()?;
    logger.log(config_event).await?;
    println!("  ✓ ConfigChange 事件: config_v2 by admin (Critical)");

    // 记录 builder 构建的事件
    logger.log(event).await?;
    println!("  ✓ Builder 构建的事件已记录");

    // 验证存储数量
    let count = storage_for_query.event_count().await;
    println!("\n  存储中共 {} 条审计事件", count);

    // ============================================
    // 7. AuditQueryFilters 查询审计日志
    // ============================================
    println!("\n--- 7. AuditQueryFilters 查询 ---");

    // 查询所有
    let all_filters = AuditQueryFilters::default();
    let all_events = logger.query(&all_filters).await?;
    println!("  [全部查询] 共 {} 条", all_events.len());

    // 按 entity_type 查询
    let users_filters = AuditQueryFilters {
        entity_type: Some("users".to_string()),
        ..Default::default()
    };
    let users_events = logger.query(&users_filters).await?;
    println!("  [entity_type=users] 共 {} 条", users_events.len());

    // 按 operation 查询
    let delete_filters = AuditQueryFilters {
        operation: Some(AuditOperation::Delete),
        ..Default::default()
    };
    let delete_events = logger.query(&delete_filters).await?;
    println!("  [operation=Delete] 共 {} 条", delete_events.len());
    for ev in &delete_events {
        println!(
            "    - entity={}/{}, user={}, severity={}",
            ev.entity_type, ev.entity_id, ev.user_id, ev.severity
        );
    }

    // 按 severity 查询
    let critical_filters = AuditQueryFilters {
        severity: Some(AuditSeverity::Critical),
        ..Default::default()
    };
    let critical_events = logger.query(&critical_filters).await?;
    println!("  [severity=Critical] 共 {} 条", critical_events.len());

    // 按 user_id 查询
    let admin_filters = AuditQueryFilters {
        user_id: Some("admin".to_string()),
        ..Default::default()
    };
    let admin_events = logger.query(&admin_filters).await?;
    println!("  [user_id=admin] 共 {} 条", admin_events.len());

    // ============================================
    // 8. 敏感数据自动脱敏
    // ============================================
    println!("\n--- 8. 敏感数据自动脱敏 ---");
    let sensitive_event = AuditEvent::create("users", "u_999", "admin")
        .with_after_value(r#"{"name":"secret_user","password":"p@ssw0rd","api_key":"ak_12345"}"#);
    logger.log(sensitive_event).await?;

    let sanitized_filters = AuditQueryFilters {
        entity_type: Some("users".to_string()),
        ..Default::default()
    };
    let sanitized_results = logger.query(&sanitized_filters).await?;
    let ev = sanitized_results.iter().find(|e| e.entity_id == "u_999");
    if let Some(ev) = ev {
        if let Some(after) = &ev.after_value {
            println!("  原始 after_value 包含 password/api_key");
            println!("  存储后 after_value = {}", after);
            assert!(after.contains("***REDACTED_PASSWORD***"), "password 应被脱敏");
            assert!(after.contains("***REDACTED_API_KEY***"), "api_key 应被脱敏");
            println!("  ✓ password 和 api_key 已被自动脱敏");
        }
    }

    // ============================================
    // 9. 禁用审计的对比
    // ============================================
    println!("\n--- 9. 禁用审计 ---");
    let disabled_storage = Arc::new(MemoryAuditStorage::new(100));
    let disabled_logger = AuditLogger::with_config(
        AuditConfig {
            enabled: false,
            ..Default::default()
        },
        disabled_storage.clone(),
    );
    disabled_logger.log_create("test", "t_1", "u", None).await?;
    println!(
        "  enabled=false 时 log_create 后存储数量 = {} (应为 0)",
        disabled_storage.event_count().await
    );

    // ============================================
    // 10. 日志清理
    // ============================================
    println!("\n--- 10. 日志清理 ---");
    // 写入一条"过期"事件（手动调整时间戳）
    let mut old_event = AuditEvent::create("legacy", "old_1", "system");
    old_event.timestamp = chrono::Utc::now() - chrono::Duration::days(30);
    logger.log(old_event).await?;
    println!("  写入一条 30 天前的事件");

    let removed = logger.cleanup(7).await?;
    println!("  ✓ cleanup(7 days) 清理了 {} 条事件", removed);

    println!("\n========================================");
    println!("✨ 审计日志示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - AuditConfig                              - 审计配置");
    println!("  - AuditLogger::with_config(config, storage) - 创建日志器");
    println!("  - MemoryAuditStorage::new(capacity)         - 内存存储");
    println!("  - AuditEventBuilder                         - 链式构建事件");
    println!("  - AuditEvent::create/read/update/delete     - 快捷构造");
    println!("  - AuditOperation: Create/Read/Update/Delete/... - 操作类型");
    println!("  - AuditSeverity: Info/Low/Medium/High/Critical - 严重级别");
    println!("  - AuditStatus: Success/Failure/Partial      - 结果状态");
    println!("  - logger.log(event)                         - 记录事件");
    println!("  - logger.log_create/read/update/delete      - 快捷记录");
    println!("  - AuditQueryFilters                         - 查询过滤");
    println!("  - logger.query(&filters) -> Vec<AuditEvent> - 查询日志");
    println!("  - logger.cleanup(days)                      - 清理旧日志");
    println!("  - 自动敏感字段脱敏 (password/token/api_key)   - 安全特性");

    Ok(())
}
