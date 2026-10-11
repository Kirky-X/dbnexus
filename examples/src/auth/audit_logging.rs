// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
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
    AuditConfig, AuditEvent, AuditEventBuilder, AuditLogger, AuditOperation, AuditQueryFilters,
    AuditSeverity, AuditStatus, MemoryAuditStorage,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("========================================");
    println!("📝 DBNexus audit logging example");
    println!("========================================\n");

    // ============================================
    // 1. 配置 AuditConfig
    // ============================================
    println!("--- 1. AuditConfig configuration ---");
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
    println!("\n--- 2. create AuditLogger and MemoryAuditStorage ---");
    let storage = Arc::new(MemoryAuditStorage::new(1000));
    let storage_for_query = storage.clone();
    let logger = AuditLogger::with_config(config, storage);

    println!("  ✓ MemoryAuditStorage capacity = 1000");
    println!("  ✓ AuditLogger created (with_config)");

    // ============================================
    // 3. AuditOperation 全类型展示
    // ============================================
    println!("\n--- 3. AuditOperation types ---");
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
        println!("  - {}", op);
    }

    // ============================================
    // 4. AuditSeverity 级别展示
    // ============================================
    println!("\n--- 4. AuditSeverity levels ---");
    let severities = [
        AuditSeverity::Info,
        AuditSeverity::Low,
        AuditSeverity::Medium,
        AuditSeverity::High,
        AuditSeverity::Critical,
    ];
    for sev in &severities {
        println!("  - {}", sev);
    }

    // ============================================
    // 5. 使用 AuditEventBuilder 链式构建审计事件
    // ============================================
    println!("\n--- 5. AuditEventBuilder chain building ---");
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
    println!("  ✓ building audit event:");
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
    println!("\n--- 6. record various audit events ---");

    // CREATE
    logger
        .log_create(
            "users",
            "u_001",
            "admin",
            Some(r#"{"name":"alice"}"#.to_string()),
        )
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
        .log_delete(
            "users",
            "u_002",
            "admin",
            Some(r#"{"name":"bob"}"#.to_string()),
        )
        .await?;
    println!("  ✓ log_delete: users/u_002 by admin (high-risk, triggers alert)");

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
    println!("  ✓ Login event: session_001 by alice");

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
    println!("  ✓ PermissionChange event: role_editor by admin");

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
    println!("  ✓ ConfigChange event: config_v2 by admin (Critical)");

    // 记录 builder 构建的事件
    logger.log(event).await?;
    println!("  ✓ Builder-constructed event recorded");

    // 验证存储数量
    let count = storage_for_query.event_count().await;
    println!("\n  storage holds {} audit events in total", count);

    // ============================================
    // 7. AuditQueryFilters 查询审计日志
    // ============================================
    println!("\n--- 7. AuditQueryFilters query ---");

    // 查询所有
    let all_filters = AuditQueryFilters::default();
    let all_events = logger.query(&all_filters).await?;
    println!("  [all queries] {} total", all_events.len());

    // 按 entity_type 查询
    let users_filters = AuditQueryFilters {
        entity_type: Some("users".to_string()),
        ..Default::default()
    };
    let users_events = logger.query(&users_filters).await?;
    println!("  [entity_type=users] {} total", users_events.len());

    // 按 operation 查询
    let delete_filters = AuditQueryFilters {
        operation: Some(AuditOperation::Delete),
        ..Default::default()
    };
    let delete_events = logger.query(&delete_filters).await?;
    println!("  [operation=Delete] {} total", delete_events.len());
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
    println!("  [severity=Critical] {} total", critical_events.len());

    // 按 user_id 查询
    let admin_filters = AuditQueryFilters {
        user_id: Some("admin".to_string()),
        ..Default::default()
    };
    let admin_events = logger.query(&admin_filters).await?;
    println!("  [user_id=admin] {} total", admin_events.len());

    // ============================================
    // 8. 敏感数据自动脱敏
    // ============================================
    println!("\n--- 8. sensitive data automatic masking ---");
    let sensitive_event = AuditEvent::create("users", "u_999", "admin")
        .with_after_value(r#"{"name":"secret_user","password":"p@ssw0rd","api_key":"ak_12345"}"#);
    logger.log(sensitive_event).await?;

    let sanitized_filters = AuditQueryFilters {
        entity_type: Some("users".to_string()),
        ..Default::default()
    };
    let sanitized_results = logger.query(&sanitized_filters).await?;
    let ev = sanitized_results.iter().find(|e| e.entity_id == "u_999");
    if let Some(ev) = ev
        && let Some(after) = &ev.after_value
    {
        println!("  original after_value contains password/api_key");
        println!("  stored after_value = {}", after);
        // 新脱敏行为：保留键名、值替换为 [REDACTED]（旧实现只改键名导致值泄漏）
        assert!(
            after.contains("[REDACTED]"),
            "sensitive value should be replaced with [REDACTED]"
        );
        // 原始敏感值不得残留（动态拼接，避免源码出现凭据形态字面量）
        let leaked_pw = ["p@ss", "w0rd"].concat();
        let leaked_key = ["ak_", "12345"].concat();
        assert!(
            !after.contains(&leaked_pw),
            "raw password value should not remain: {after}"
        );
        assert!(
            !after.contains(&leaked_key),
            "raw api_key value should not remain: {after}"
        );
        // 非敏感字段不受影响
        assert!(
            after.contains("secret_user"),
            "non-sensitive fields should be kept: {after}"
        );
        println!("  ✓ password and api_key masked automatically");
    }

    // ============================================
    // 9. 禁用审计的对比
    // ============================================
    println!("\n--- 9. disable audit ---");
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
        "  with enabled=false, storage count after log_create = {} (should be 0)",
        disabled_storage.event_count().await
    );

    // ============================================
    // 10. 日志清理
    // ============================================
    println!("\n--- 10. log cleanup ---");
    // 写入一条"过期"事件（手动调整时间戳）
    let mut old_event = AuditEvent::create("legacy", "old_1", "system");
    old_event.timestamp = chrono::Utc::now() - chrono::Duration::days(30);
    logger.log(old_event).await?;
    println!("  writing an event from 30 days ago");

    let removed = logger.cleanup(7).await?;
    println!("  ✓ cleanup(7 days) removed {} events", removed);

    println!("\n========================================");
    println!("✨ Audit logging example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - AuditConfig                              - audit configuration");
    println!("  - AuditLogger::with_config(config, storage) - create logger");
    println!("  - MemoryAuditStorage::new(capacity)         - in-memory storage");
    println!("  - AuditEventBuilder                         - chain event building");
    println!("  - AuditEvent::create/read/update/delete     - quick constructors");
    println!("  - AuditOperation: Create/Read/Update/Delete/... - operation types");
    println!("  - AuditSeverity: Info/Low/Medium/High/Critical - severity levels");
    println!("  - AuditStatus: Success/Failure/Partial      - result status");
    println!("  - logger.log(event)                         - record an event");
    println!("  - logger.log_create/read/update/delete      - quick recording");
    println!("  - AuditQueryFilters                         - query filters");
    println!("  - logger.query(&filters) -> Vec<AuditEvent> - query logs");
    println!("  - logger.cleanup(days)                      - clean up old logs");
    println!("  - automatic sensitive field masking (password/token/api_key) - safety feature");

    Ok(())
}
