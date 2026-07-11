// Copyright (c) 2026 Kirky.X
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

use dbnexus::{
    db_entity, AuditConfig, AuditEvent, AuditLogger, AuditOperation, AuditQueryFilters, AuditSeverity,
    MemoryAuditStorage,
};
use sea_orm::entity::prelude::*;
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
    println!("🔍 DBNexus db_audit 宏示例");
    println!("========================================\n");

    // ============================================
    // 1. 展示宏生成的审计常量
    // ============================================
    println!("--- 1. 宏生成的审计常量 ---\n");
    println!("  AUDIT_TABLE_NAME  = {}", Model::AUDIT_TABLE_NAME);
    println!("  AUDIT_OPERATIONS  = {:?}", Model::AUDIT_OPERATIONS);
    println!("  AUDIT_ROLES       = {:?}", Model::AUDIT_ROLES);
    println!("  AUDIT_LOG_VALUES  = {}", Model::AUDIT_LOG_VALUES);
    println!("  AUDIT_ENABLED     = {}", Model::AUDIT_ENABLED);

    // ============================================
    // 2. 创建 AuditLogger + MemoryAuditStorage
    // ============================================
    println!("\n--- 2. 创建 AuditLogger ---\n");
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
    println!("  ✓ AuditLogger 创建成功 (storage 容量=1000)");

    // ============================================
    // 3. 创建 DbPool + Session
    // ============================================
    println!("\n--- 3. 创建 DbPool + Session ---\n");
    let (_pool, session) = common::db::setup_shared_sqlite_session()
        .await
        .map_err(|e| Box::<dyn std::error::Error + Send + Sync>::from(e.to_string()))?;
    println!("  ✓ Session 创建成功 (角色: admin)");

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
    println!("  ✓ products 表创建成功");

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
    println!("  ✓ {} 表创建成功 (使用 AUDIT_TABLE_NAME)", Model::AUDIT_TABLE_NAME);

    // ============================================
    // 4. CRUD 操作 + 审计日志记录
    // ============================================
    println!("\n--- 4. CRUD 操作 + 审计日志 ---\n");
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
    println!("  ✓ 插入产品: id={}, name={}", created.id, created.name);

    if should_audit("CREATE", role) {
        let event = AuditEvent::create("products", &created.id.to_string(), role).with_after_value(&format!(
            r#"{{"name":"{}","price":{},"stock":{}}}"#,
            created.name, created.price, created.stock
        ));
        logger.log(event).await?;
        println!("  ✓ 审计日志已记录 (operation=CREATE, role={})", role);
    } else {
        println!("  - 跳过审计 (CREATE 不在 AUDIT_OPERATIONS 或角色不匹配)");
    }

    // READ
    println!("\n[READ]");
    let found = Model::find_by_id(&session, 1).await?;
    if let Some(p) = &found {
        println!("  ✓ 查询产品: id={}, name={}", p.id, p.name);
    }
    // READ 不在 AUDIT_OPERATIONS 中，不审计
    if should_audit("READ", role) {
        let event = AuditEvent::read("products", "1", role);
        logger.log(event).await?;
        println!("  ✓ 审计日志已记录 (operation=READ)");
    } else {
        println!(
            "  - 跳过审计 (READ 不在 AUDIT_OPERATIONS={:?})",
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
        "  ✓ 更新产品: id={}, 新 price={:.2}, 新 stock={}",
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
        println!("  ✓ 审计日志已记录 (operation=UPDATE, 含前后值)");
    } else {
        println!("  - 跳过审计 (UPDATE 不在审计范围或 AUDIT_LOG_VALUES=false)");
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
    println!("  ✓ 删除产品 id=2: 影响 {} 行", deleted);

    if should_audit("DELETE", role) {
        let event = AuditEvent::delete("products", "2", role).with_severity(AuditSeverity::High);
        logger.log(event).await?;
        println!("  ✓ 审计日志已记录 (operation=DELETE, severity=High)");
    } else {
        println!("  - 跳过审计 (DELETE 不在 AUDIT_OPERATIONS)");
    }

    // ============================================
    // 5. 审计历史查询
    // ============================================
    println!("\n--- 5. 审计历史查询 ---\n");
    let total_events = storage_for_query.event_count().await;
    println!("  存储中共 {} 条审计事件", total_events);

    // 查询所有审计事件
    let all_filters = AuditQueryFilters::default();
    let all_events = logger.query(&all_filters).await?;
    println!("\n  [全部审计事件]");
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
    println!("\n  [operation=Create] 共 {} 条", create_events.len());

    let delete_filters = AuditQueryFilters {
        operation: Some(AuditOperation::Delete),
        ..Default::default()
    };
    let delete_events = logger.query(&delete_filters).await?;
    println!("  [operation=Delete] 共 {} 条", delete_events.len());

    // 按 severity 查询
    let high_filters = AuditQueryFilters {
        severity: Some(AuditSeverity::High),
        ..Default::default()
    };
    let high_events = logger.query(&high_filters).await?;
    println!("  [severity=High] 共 {} 条", high_events.len());

    // ============================================
    // 6. 验证 should_audit 逻辑（角色不匹配场景）
    // ============================================
    println!("\n--- 6. 角色不匹配场景 ---\n");
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
        "  should_audit(\"CREATE\", \"guest\")   = {} (角色不在 AUDIT_ROLES)",
        should_audit("CREATE", "guest")
    );
    println!(
        "  should_audit(\"READ\", \"admin\")     = {} (READ 不在 AUDIT_OPERATIONS)",
        should_audit("READ", "admin")
    );

    println!("\n========================================");
    println!("✨ db_entity 宏 audit 示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - #[db_entity(..., audit(table_name=\"...\", log_values=true))]  生成审计配置常量");
    println!("  - Model::AUDIT_TABLE_NAME   审计日志表名");
    println!("  - Model::AUDIT_OPERATIONS   需要审计的操作列表 (CREATE/UPDATE/DELETE)");
    println!("  - Model::AUDIT_ROLES        需要审计的角色列表");
    println!("  - Model::AUDIT_LOG_VALUES   是否记录变更值");
    println!("  - Model::AUDIT_ENABLED      审计是否启用");
    println!("  - AuditLogger + AuditEvent  审计日志记录");
    println!("  - AuditQueryFilters         审计日志查询过滤");
    println!("\n⚠️  注意: #[db_entity] 的 audit 子参数仅生成配置常量，不自动记录审计日志。");
    println!("   开发者需在 CRUD 操作后手动调用 AuditLogger.log() 记录事件。");
    println!("   常量用于在运行时判断哪些操作/角色需要审计。");

    Ok(())
}
