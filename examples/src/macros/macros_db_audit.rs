// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! db_entity 宏 audit 子参数示例
//!
//! 演示 `#[db_entity(..., audit(...))]` 生成的审计常量与审计日志集成：
//! - 宏生成的常量：`AUDIT_TABLE_NAME` / `AUDIT_OPERATIONS` / `AUDIT_ROLES` /
//!   `AUDIT_LOG_VALUES` / `AUDIT_ENABLED`
//! - 结合 `AuditLogger` 在 CRUD 操作前后记录审计事件
//! - 通过 `AUDIT_OPERATIONS` / `AUDIT_ROLES` 常量过滤需要审计的操作
//! - 查询审计历史日志
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example macros_db_audit --features "sqlite,permission,macros,audit"
//! ```

#[path = "../common/mod.rs"]
mod common;

use dbnexus::sea_orm::entity::prelude::*;
use dbnexus::{
    AuditConfig, AuditEvent, AuditLogger, AuditOperation, AuditQueryFilters, AuditSeverity,
    MemoryAuditStorage, db_entity,
};
use std::sync::Arc;

// ============================================
// 定义 Product 实体（带 audit 子参数）
// ============================================

/// 产品实体
///
/// `#[db_entity(..., audit(table_name = "product_audit_log", log_values = true))]` 生成审计配置常量：
/// - `AUDIT_TABLE_NAME`     审计日志表名
/// - `AUDIT_OPERATIONS`     需要审计的操作列表
/// - `AUDIT_ROLES`          需要审计的角色列表
/// - `AUDIT_LOG_VALUES`     是否记录变更前后的值
/// - `AUDIT_ENABLED`        审计是否启用
#[db_entity(
    table_name = "products",
    primary_key = "id",
    audit(table_name = "product_audit_log", log_values = true)
)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "products")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub price: f64,
    pub stock: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

// ============================================
// 辅助函数：根据宏常量判断是否需要审计
// ============================================

/// 检查给定操作和角色是否需要审计
///
/// 基于 `#[db_audit]` 生成的 `AUDIT_OPERATIONS` 和 `AUDIT_ROLES` 常量进行判断。
fn should_audit(operation: &str, role: &str) -> bool {
    if !Model::AUDIT_ENABLED {
        return false;
    }
    let op_match = Model::AUDIT_OPERATIONS.contains(&operation);
    let role_match = Model::AUDIT_ROLES.contains(&role);
    op_match && role_match
}

// ============================================
// 主函数
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("========================================");
    println!("🔍 DBNexus db_audit macro example");
    println!("========================================\n");

    // ============================================
    // 1. 展示宏生成的审计常量
    // ============================================
    println!("--- 1. macro-generated audit constants ---\n");
    println!("  AUDIT_TABLE_NAME  = {}", Model::AUDIT_TABLE_NAME);
    println!("  AUDIT_OPERATIONS  = {:?}", Model::AUDIT_OPERATIONS);
    println!("  AUDIT_ROLES       = {:?}", Model::AUDIT_ROLES);
    println!("  AUDIT_LOG_VALUES  = {}", Model::AUDIT_LOG_VALUES);
    println!("  AUDIT_ENABLED     = {}", Model::AUDIT_ENABLED);

    // ============================================
    // 2. 创建 AuditLogger + MemoryAuditStorage
    // ============================================
    println!("\n--- 2. create the AuditLogger ---\n");
    let audit_config = AuditConfig {
        enabled: true,
        storage_path: None,
        sync_write: false,
        max_file_size: 10 * 1024 * 1024,
        retention_count: 7,
        sensitive_fields: vec!["price".to_string()], // price 视为敏感字段
        alert_operations: vec![AuditOperation::Delete],
        alert_severity: AuditSeverity::High,
    };
    let storage = Arc::new(MemoryAuditStorage::new(1000));
    let storage_for_query = storage.clone();
    let logger = AuditLogger::with_config(audit_config, storage);
    println!("  ✓ AuditLogger created (storage capacity=1000)");

    // ============================================
    // 3. 创建 DbPool + Session
    // ============================================
    println!("\n--- 3. create DbPool + Session ---\n");
    let (_pool, session) = common::db::setup_shared_sqlite_session()
        .await
        .map_err(|e| Box::<dyn std::error::Error + Send + Sync>::from(e.to_string()))?;
    println!("  ✓ Session created (role: admin)");

    // 建表
    session
        .execute_raw_ddl(
            "CREATE TABLE products (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                price REAL NOT NULL,
                stock INTEGER NOT NULL
            )",
        )
        .await?;
    println!("  ✓ products table created");

    // 建审计日志表（使用宏生成的 AUDIT_TABLE_NAME）
    let audit_table_ddl = format!(
        "CREATE TABLE {} (
            id INTEGER PRIMARY KEY,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            operation TEXT NOT NULL,
            user_id TEXT,
            severity TEXT,
            result TEXT
        )",
        Model::AUDIT_TABLE_NAME
    );
    session.execute_raw_ddl(&audit_table_ddl).await?;
    println!(
        "  ✓ {} table created (using AUDIT_TABLE_NAME)",
        Model::AUDIT_TABLE_NAME
    );

    // ============================================
    // 4. CRUD 操作 + 审计日志记录
    // ============================================
    println!("\n--- 4. CRUD operations + audit log ---\n");
    let role = session.role();

    // CREATE
    println!("[CREATE]");
    let product = Model {
        id: 1,
        name: "Laptop".to_string(),
        price: 1299.99,
        stock: 15,
    };
    let created = Model::insert(&session, product).await?;
    println!(
        "  ✓ inserted product: id={}, name={}",
        created.id, created.name
    );

    if should_audit("CREATE", role) {
        let event = AuditEvent::create("products", &created.id.to_string(), role).with_after_value(
            &format!(
                r#"{{"name":"{}","price":{},"stock":{}}}"#,
                created.name, created.price, created.stock
            ),
        );
        logger.log(event).await?;
        println!("  ✓ audit log recorded (operation=CREATE, role={})", role);
    } else {
        println!(
            "  - skip auditing (CREATE is not in AUDIT_OPERATIONS or the role does not match)"
        );
    }

    // READ
    println!("\n[READ]");
    let found = Model::find_by_id(&session, 1).await?;
    if let Some(p) = &found {
        println!("  ✓ queried product: id={}, name={}", p.id, p.name);
    }
    // READ 不在 AUDIT_OPERATIONS 中，不审计
    if should_audit("READ", role) {
        let event = AuditEvent::read("products", "1", role);
        logger.log(event).await?;
        println!("  ✓ audit log recorded (operation=READ)");
    } else {
        println!(
            "  - skip auditing (READ is not in AUDIT_OPERATIONS={:?})",
            Model::AUDIT_OPERATIONS
        );
    }

    // UPDATE
    println!("\n[UPDATE]");
    let before = found.unwrap();
    let before_value = format!(
        r#"{{"name":"{}","price":{},"stock":{}}}"#,
        before.name, before.price, before.stock
    );
    let updated = Model::update(
        &session,
        Model {
            price: 1199.99,
            stock: 20,
            ..before
        },
    )
    .await?;
    println!(
        "  ✓ updated product: id={}, new price={:.2}, new stock={}",
        updated.id, updated.price, updated.stock
    );

    if should_audit("UPDATE", role) && Model::AUDIT_LOG_VALUES {
        let after_value = format!(
            r#"{{"name":"{}","price":{},"stock":{}}}"#,
            updated.name, updated.price, updated.stock
        );
        let event = AuditEvent::update(
            "products",
            &updated.id.to_string(),
            role,
            Some(before_value),
            Some(after_value),
        )
        .with_severity(AuditSeverity::Medium);
        logger.log(event).await?;
        println!("  ✓ audit log recorded (operation=UPDATE, with before/after values)");
    } else {
        println!("  - skip auditing (UPDATE is out of the audit scope or AUDIT_LOG_VALUES=false)");
    }

    // 再插入一条用于 DELETE 演示
    let product2 = Model {
        id: 2,
        name: "Mouse".to_string(),
        price: 29.99,
        stock: 100,
    };
    let created2 = Model::insert(&session, product2).await?;
    if should_audit("CREATE", role) {
        let event = AuditEvent::create("products", &created2.id.to_string(), role);
        logger.log(event).await?;
    }

    // DELETE
    println!("\n[DELETE]");
    let deleted = Model::delete(&session, 2).await?;
    println!("  ✓ deleted product id=2: {} rows affected", deleted);

    if should_audit("DELETE", role) {
        let event = AuditEvent::delete("products", "2", role).with_severity(AuditSeverity::High);
        logger.log(event).await?;
        println!("  ✓ audit log recorded (operation=DELETE, severity=High)");
    } else {
        println!("  - skip auditing (DELETE is not in AUDIT_OPERATIONS)");
    }

    // ============================================
    // 5. 审计历史查询
    // ============================================
    println!("\n--- 5. audit history query ---\n");
    let total_events = storage_for_query.event_count().await;
    println!("  storage holds {} audit events in total", total_events);

    // 查询所有审计事件
    let all_filters = AuditQueryFilters::default();
    let all_events = logger.query(&all_filters).await?;
    println!("\n  [all audit events]");
    for ev in &all_events {
        println!(
            "    - {} {}/{} by {} [{}]",
            ev.operation, ev.entity_type, ev.entity_id, ev.user_id, ev.severity
        );
    }

    // 按 operation 查询
    let create_filters = AuditQueryFilters {
        operation: Some(AuditOperation::Create),
        ..Default::default()
    };
    let create_events = logger.query(&create_filters).await?;
    println!("\n  [operation=Create] {} total", create_events.len());

    let delete_filters = AuditQueryFilters {
        operation: Some(AuditOperation::Delete),
        ..Default::default()
    };
    let delete_events = logger.query(&delete_filters).await?;
    println!("  [operation=Delete] {} total", delete_events.len());

    // 按 severity 查询
    let high_filters = AuditQueryFilters {
        severity: Some(AuditSeverity::High),
        ..Default::default()
    };
    let high_events = logger.query(&high_filters).await?;
    println!("  [severity=High] {} total", high_events.len());

    // ============================================
    // 6. 验证 should_audit 逻辑（角色不匹配场景）
    // ============================================
    println!("\n--- 6. role mismatch scenario ---\n");
    println!("  AUDIT_ROLES = {:?}", Model::AUDIT_ROLES);
    println!(
        "  should_audit(\"CREATE\", \"admin\")   = {}",
        should_audit("CREATE", "admin")
    );
    println!(
        "  should_audit(\"CREATE\", \"manager\") = {}",
        should_audit("CREATE", "manager")
    );
    println!(
        "  should_audit(\"CREATE\", \"guest\")   = {} (the role is not in AUDIT_ROLES)",
        should_audit("CREATE", "guest")
    );
    println!(
        "  should_audit(\"READ\", \"admin\")     = {} (READ is not in AUDIT_OPERATIONS)",
        should_audit("READ", "admin")
    );

    println!("\n========================================");
    println!("✨ db_entity macro audit example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!(
        "  - #[db_entity(..., audit(table_name=\"...\", log_values=true))]  generates audit config constants"
    );
    println!("  - Model::AUDIT_TABLE_NAME   the audit log table name");
    println!(
        "  - Model::AUDIT_OPERATIONS   the list of operations that need auditing (CREATE/UPDATE/DELETE)"
    );
    println!("  - Model::AUDIT_ROLES        the list of roles that need auditing");
    println!("  - Model::AUDIT_LOG_VALUES   whether to record change values");
    println!("  - Model::AUDIT_ENABLED      whether auditing is enabled");
    println!("  - AuditLogger + AuditEvent  audit log recording");
    println!("  - AuditQueryFilters         audit log query filters");
    println!(
        "\n⚠️  Note: the audit sub-parameter of #[db_entity] only generates config constants, it does not record audit logs automatically."
    );
    println!(
        "   developers must call AuditLogger.log() manually after CRUD operations to record events."
    );
    println!("   the constants let you decide at runtime which operations/roles need auditing.");

    Ok(())
}
