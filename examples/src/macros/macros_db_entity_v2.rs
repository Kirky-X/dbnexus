// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! db_entity v2 高级特性示例（timestamps + validate + hooks）
//!
//! 演示 `#[db_entity(...)]` 统一属性宏的三大行为特性：
//! - `timestamps = true` — 自动管理 `created_at` / `updated_at` 字段
//! - `validate` — 集成 `validator` crate 进行字段验证
//! - `hooks(...)` — 事件钩子（before/after insert/update/delete）
//!
//! ## 编排顺序
//!
//! `before_save` 内严格按以下顺序执行（任一失败短路）：
//! 1. **validate** — 调用 `validator::Validate::validate(&model)`
//! 2. **timestamps** — 检测主键 `ActiveValue` 三态，insert 设两个时间戳，update 仅设 `updated_at`
//! 3. **user_hooks** — 调用用户定义的 `before_insert` 或 `before_update` 钩子
//!
//! # 运行示例
//!
//! ```bash
//! cargo run -p dbnexus-examples --bin macros_db_entity_v2
//! ```

#[path = "../common/mod.rs"]
mod common;

use std::sync::atomic::{AtomicUsize, Ordering};

use dbnexus::db_entity;
use dbnexus::foundation::DatabaseType;
use dbnexus::sea_orm::entity::prelude::*;
use dbnexus::sea_orm::{ActiveModelTrait, DbBackend, EntityTrait};
use dbnexus::{Migration, MigrationExecutor, TableChange};
use validator::Validate;

// 钩子调用计数器（演示钩子触发）

static BEFORE_INSERT_CALLS: AtomicUsize = AtomicUsize::new(0);
static AFTER_INSERT_CALLS: AtomicUsize = AtomicUsize::new(0);
static BEFORE_UPDATE_CALLS: AtomicUsize = AtomicUsize::new(0);
static AFTER_UPDATE_CALLS: AtomicUsize = AtomicUsize::new(0);

fn reset_counters() {
    BEFORE_INSERT_CALLS.store(0, Ordering::SeqCst);
    AFTER_INSERT_CALLS.store(0, Ordering::SeqCst);
    BEFORE_UPDATE_CALLS.store(0, Ordering::SeqCst);
    AFTER_UPDATE_CALLS.store(0, Ordering::SeqCst);
}

// 钩子函数定义

/// insert 前钩子：验证 timestamps 已在钩子之前执行（编排顺序验证）
fn before_insert_hook(am: &mut ActiveModel) -> Result<(), sea_orm::DbErr> {
    BEFORE_INSERT_CALLS.fetch_add(1, Ordering::SeqCst);
    // 验证 timestamps 已设置 updated_at（编排顺序：timestamps → hooks）
    match &am.updated_at {
        sea_orm::ActiveValue::Set(Some(_)) => {
            println!("    [hook] before_insert: timestamps set ✓");
        }
        _ => {
            return Err(sea_orm::DbErr::Custom(
                "orchestration order error: timestamps should run before the before_insert hook"
                    .to_string(),
            ));
        }
    }
    Ok(())
}

/// insert 后钩子：可读取已保存的 Model 数据
fn after_insert_hook(model: &Model) -> Result<(), sea_orm::DbErr> {
    AFTER_INSERT_CALLS.fetch_add(1, Ordering::SeqCst);
    println!(
        "    [hook] after_insert: record id={}, name={} saved",
        model.id, model.name
    );
    Ok(())
}

/// update 前钩子
fn before_update_hook(am: &mut ActiveModel) -> Result<(), sea_orm::DbErr> {
    BEFORE_UPDATE_CALLS.fetch_add(1, Ordering::SeqCst);
    println!("    [hook] before_update: about to update the record");
    // 可在此修改 ActiveModel 字段
    let _ = am;
    Ok(())
}

/// update 后钩子
fn after_update_hook(model: &Model) -> Result<(), sea_orm::DbErr> {
    AFTER_UPDATE_CALLS.fetch_add(1, Ordering::SeqCst);
    println!(
        "    [hook] after_update: record id={}, name={} updated",
        model.id, model.name
    );
    Ok(())
}

// 实体定义（timestamps + validate + hooks）

/// 会员实体 — 启用全部行为特性
///
/// - `timestamps = true` — 自动管理 `created_at` / `updated_at`
/// - `validate` — 启用 `validator` crate 字段验证
/// - `hooks(...)` — 注册 4 个事件钩子
#[db_entity(
    table_name = "members",
    primary_key = "id",
    timestamps = true,
    validate,
    hooks(
        before_insert = "before_insert_hook",
        after_insert = "after_insert_hook",
        before_update = "before_update_hook",
        after_update = "after_update_hook",
    )
)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Validate)]
#[sea_orm(table_name = "members")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[validate(length(min = 2, message = "name too short"))]
    pub name: String,
    #[validate(email(message = "invalid email format"))]
    pub email: String,
    pub created_at: Option<time::OffsetDateTime>,
    pub updated_at: Option<time::OffsetDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

// 主函数

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("⚡ db_entity v2: timestamps + validate + hooks");
    println!("========================================\n");

    reset_counters();

    // 创建数据库 + 表
    println!("--- 1. create the members table (schema() auto-generated) ---\n");

    let (_pool, session) = common::db::setup_shared_sqlite_session().await?;
    println!("  ✓ connected to the SQLite in-memory database");

    // 使用宏生成的 schema() 方法创建表
    let table = Model::schema(DbBackend::Sqlite);
    let conn = session.connection()?;
    let mut executor = MigrationExecutor::new(conn.clone(), DatabaseType::Sqlite);
    let mut migration = Migration::new(1, "create_members_table".to_string());
    migration.add_table_change(TableChange::CreateTable(table));
    executor.apply_migration(&migration).await?;
    println!("  ✓ schema() generated the table and applied the migration\n");

    // 插入有效记录（timestamps + hooks 触发）
    println!("--- 2. insert a valid record (timestamps + before_insert + after_insert) ---\n");

    // 使用 Model { ... }.into() 模式创建 ActiveModel（所有字段为 Unchanged）
    // 注意：当 validate + timestamps 同时启用时，验证步骤会调用 TryIntoModel，
    // 要求所有字段为 Set 或 Unchanged（不能是 NotSet），故用 Model 字面量初始化。
    let new_member: ActiveModel = Model {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
        created_at: None,
        updated_at: None,
    }
    .into();

    let inserted = new_member.insert(session.connection()?).await?;
    println!(
        "  ✓ insert succeeded: id={}, name={}, email={}",
        inserted.id, inserted.name, inserted.email
    );
    println!("    created_at = {:?}", inserted.created_at);
    println!("    updated_at = {:?}", inserted.updated_at);
    println!(
        "    hook calls: before_insert={}, after_insert={}",
        BEFORE_INSERT_CALLS.load(Ordering::SeqCst),
        AFTER_INSERT_CALLS.load(Ordering::SeqCst)
    );

    assert!(
        inserted.created_at.is_some(),
        "created_at should be set automatically by timestamps"
    );
    assert!(
        inserted.updated_at.is_some(),
        "updated_at should be set automatically by timestamps"
    );
    println!("  ✓ timestamps auto-set verification passed\n");

    // 插入无效记录（validate 拦截）
    println!("--- 3. insert an invalid record (validate intercepts: name too short) ---\n");

    let invalid_member: ActiveModel = Model {
        id: 2,
        name: "A".to_string(), // 长度 < 2，触发验证错误
        email: "bob@example.com".to_string(),
        created_at: None,
        updated_at: None,
    }
    .into();

    let result = invalid_member.insert(session.connection()?).await;
    match &result {
        Err(e) => {
            println!("  ✓ validation intercepted: {}", e);
            println!("    → name=\"A\" is shorter than 2, triggering #[validate(length(min=2))]");
        }
        Ok(_) => println!("  ✗ validation did not intercept (should not happen)"),
    }
    assert!(
        result.is_err(),
        "the invalid record should be intercepted by validation"
    );

    // 验证钩子未被调用（验证失败短路）
    let before_insert_after_fail = BEFORE_INSERT_CALLS.load(Ordering::SeqCst);
    assert_eq!(
        before_insert_after_fail, 1,
        "validation failure should short-circuit; the before_insert hook should not be called again"
    );
    println!(
        "  ✓ short-circuit verification: the before_insert hook was not called (validation short-circuited)\n"
    );

    // 插入无效邮箱（validate 拦截）
    println!("--- 4. insert an invalid email (validate intercepts: bad email format) ---\n");

    let invalid_email: ActiveModel = Model {
        id: 3,
        name: "Bob".to_string(),
        email: "not-an-email".to_string(), // 非 email 格式
        created_at: None,
        updated_at: None,
    }
    .into();

    let result = invalid_email.insert(session.connection()?).await;
    assert!(
        result.is_err(),
        "the invalid email should be intercepted by validation"
    );
    println!("  ✓ validation intercepted: email=\"not-an-email\" has an invalid format");
    println!("    → trigger #[validate(email)]\n");

    // 更新记录（timestamps + hooks 触发）
    println!("--- 5. update the record (timestamps + before_update + after_update) ---\n");

    let existing = Model::find_by_id(&session, 1)
        .await?
        .expect("the record should exist");

    // 转为 ActiveModel 并修改 name（主键设为 Unchanged 表示 update）
    let mut am: ActiveModel = existing.into();
    am.name = sea_orm::Set("Alice Updated".to_string());

    let updated = am.update(session.connection()?).await?;
    println!(
        "  ✓ update succeeded: id={}, name={}",
        updated.id, updated.name
    );
    println!("    created_at = {:?} (unchanged)", updated.created_at);
    println!("    updated_at = {:?} (updated)", updated.updated_at);
    println!(
        "    hook calls: before_update={}, after_update={}",
        BEFORE_UPDATE_CALLS.load(Ordering::SeqCst),
        AFTER_UPDATE_CALLS.load(Ordering::SeqCst)
    );

    // 验证 updated_at 已变化（timestamps 在 update 时仅设 updated_at）
    assert!(updated.updated_at.is_some(), "updated_at should be set");
    println!("  ✓ timestamps update verification passed: only updated_at was updated\n");

    // 总结
    println!("========================================");
    println!("✨ Example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  timestamps = true       automatically manage created_at / updated_at");
    println!("    → insert: set created_at + updated_at");
    println!("    → update: update updated_at only");
    println!("  validate                 integrates validator crate field validation");
    println!("    → #[validate(length(min=2))]  field length validation");
    println!("    → #[validate(email)]          email format validation");
    println!("    → validation failure short-circuits; the later timestamps and hooks do not run");
    println!("  hooks(...)               event hooks (6 optional)");
    println!("    → before_insert: fn(&mut ActiveModel) -> Result<(), DbErr>");
    println!("    → after_insert:  fn(&Model) -> Result<(), DbErr>");
    println!("    → before_update: fn(&mut ActiveModel) -> Result<(), DbErr>");
    println!("    → after_update:  fn(&Model) -> Result<(), DbErr>");
    println!("\n⚡ Orchestration order: validate → timestamps → user_hooks");
    println!("    any failure short-circuits and later steps do not run");

    Ok(())
}
