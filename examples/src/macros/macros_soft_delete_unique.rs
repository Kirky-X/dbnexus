// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 软删除与唯一约束示例
//!
//! 演示 `#[db_entity(..., soft_delete = true)]` 配合复合唯一约束 `UNIQUE(email, deleted_at)`
//! 解决"软删除后无法用同一邮箱重新注册"的问题。
//!
//! ## 问题背景
//!
//! 若使用 `UNIQUE(email)` 约束，软删除后记录仍占用 email，导致新用户无法用同一 email 注册。
//!
//! ## 解决方案
//!
//! 改用 `UNIQUE(email, deleted_at)` 复合唯一约束：
//! - 活跃记录：`deleted_at = NULL`，email 唯一性由 NULL 语义保证
//! - 软删除记录：`deleted_at = <timestamp>`，与活跃记录的 `(email, NULL)` 不冲突
//! - 同一 email 软删除后可重新注册（新记录 `deleted_at = NULL`，旧记录 `deleted_at = <timestamp>`）
//!
//! ## 生产环境建议
//!
//! SQLite/PostgreSQL 中 NULL 在唯一约束里互不相同，因此 `UNIQUE(email, deleted_at)` 不能
//! 阻止两条活跃记录（都是 `deleted_at = NULL`）拥有相同 email。生产环境应额外创建部分唯一索引：
//! ```sql
//! CREATE UNIQUE INDEX idx_members_email_active ON members(email) WHERE deleted_at IS NULL;
//! ```
//!
//! # 运行示例
//!
//! ```bash
//! cargo run -p dbnexus-examples --bin macros_soft_delete_unique
//! ```

#[path = "../common/mod.rs"]
mod common;

use dbnexus::db_entity;
use dbnexus::sea_orm::entity::prelude::*;
use dbnexus::sea_orm::{ActiveModelTrait, EntityTrait};

// ============================================
// Member 实体（启用 soft_delete）
// ============================================

/// 会员实体
///
/// `soft_delete = true` 自动注入 `deleted_at: Option<time::OffsetDateTime>` 字段，
/// 并重写 `find*`/`delete*` 方法：
/// - `find_all` / `find_by_id` 自动加 `WHERE deleted_at IS NULL`
/// - `delete` 变为 `UPDATE SET deleted_at = now WHERE ... AND deleted_at IS NULL`
/// - 新增 `find_with_deleted` / `find_only_deleted` / `force_delete` 显式方法
#[db_entity(table_name = "members", primary_key = "id", soft_delete = true)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "members")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub email: String,
    pub name: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

// ============================================
// 主函数
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🗑️  Soft delete + composite unique constraint example");
    println!("========================================\n");

    // ============================================
    // 1. 创建数据库 + 带复合唯一约束的表
    // ============================================
    println!("--- 1. create the members table (UNIQUE(email, deleted_at)) ---\n");

    let (_pool, session) = common::db::setup_shared_sqlite_session().await?;
    println!("  ✓ connected to the SQLite in-memory database (role: admin)");

    // 使用原始 DDL 创建带复合唯一约束的表
    //
    // 注意：schema() 方法生成的表不包含复合唯一约束，因此本示例使用原始 DDL。
    // 关键点：UNIQUE(email, deleted_at) 而非 UNIQUE(email)
    let ddl = r#"CREATE TABLE members (
        id INTEGER PRIMARY KEY,
        email TEXT NOT NULL,
        name TEXT NOT NULL,
        deleted_at TEXT,
        UNIQUE(email, deleted_at)
    )"#;
    session.execute_raw_ddl(ddl).await?;
    println!("  ✓ created the members table with the constraint: UNIQUE(email, deleted_at)");
    println!("    → the same email can be re-registered after a soft delete\n");

    // ============================================
    // 2. 注册第一个用户 alice@example.com
    // ============================================
    println!("--- 2. register the user alice@example.com ---\n");

    let alice_v1 = Model {
        id: 1,
        email: "alice@example.com".to_string(),
        name: "Alice (initial)".to_string(),
        deleted_at: None,
    };
    let am: ActiveModel = alice_v1.into();
    let inserted: Model = am.insert(session.connection()?).await?;
    println!(
        "  ✓ inserted record: id={}, email={}, name={}",
        inserted.id, inserted.email, inserted.name
    );

    // ============================================
    // 3. 软删除 alice
    // ============================================
    println!("\n--- 3. soft-delete alice (DELETE SET deleted_at = now) ---\n");

    let affected = Model::delete(&session, 1).await?;
    assert_eq!(affected, 1, "the soft delete should affect 1 row");
    println!("  ✓ soft-deleted id=1; deleted_at set (not a physical delete)");

    // 验证 find_all 不返回软删除记录
    let active = Model::find_all(&session).await?;
    assert!(
        active.is_empty(),
        "find_all should not return soft-deleted records"
    );
    println!(
        "  ✓ find_all() returned {} records (soft-deleted filtered out)",
        active.len()
    );

    // 验证 find_with_deleted 返回软删除记录
    let with_deleted = Model::find_with_deleted(&session).await?;
    assert_eq!(
        with_deleted.len(),
        1,
        "find_with_deleted should return 1 (including soft-deleted)"
    );
    assert!(
        with_deleted[0].deleted_at.is_some(),
        "the soft-deleted record's deleted_at should be set"
    );
    println!(
        "  ✓ find_with_deleted() returned {} records (including soft-deleted)",
        with_deleted.len()
    );

    // ============================================
    // 4. 用同一 email 重新注册（关键演示）
    // ============================================
    println!(
        "\n--- 4. re-register with the same email (UNIQUE(email, deleted_at) in effect) ---\n"
    );

    let alice_v2 = Model {
        id: 2,
        email: "alice@example.com".to_string(),
        name: "Alice (re-registered)".to_string(),
        deleted_at: None,
    };
    let am: ActiveModel = alice_v2.into();
    let inserted: Model = am.insert(session.connection()?).await?;
    println!(
        "  ✓ re-registered: id={}, email={}, name={}",
        inserted.id, inserted.email, inserted.name
    );
    println!(
        "    → if UNIQUE(email) were used instead of UNIQUE(email, deleted_at), this insert would fail!"
    );

    // ============================================
    // 5. 验证最终状态
    // ============================================
    println!("\n--- 5. verify the final state ---\n");

    // find_all 仅返回活跃记录（id=2）
    let active = Model::find_all(&session).await?;
    assert_eq!(active.len(), 1, "there should be 1 active record");
    assert_eq!(active[0].id, 2, "the active record should be id=2");
    assert_eq!(active[0].email, "alice@example.com");
    println!("  ✓ find_all() returned {} active records:", active.len());
    for m in &active {
        println!(
            "    - id={}, email={}, name={}, deleted_at={:?}",
            m.id, m.email, m.name, m.deleted_at
        );
    }

    // find_with_deleted 返回全部记录（id=1 软删除 + id=2 活跃）
    let all = Model::find_with_deleted(&session).await?;
    assert_eq!(
        all.len(),
        2,
        "there should be 2 records (including soft-deleted)"
    );
    println!(
        "\n  ✓ find_with_deleted() returned {} records (including soft-deleted):",
        all.len()
    );
    for m in &all {
        let status = if m.deleted_at.is_some() {
            "soft-deleted"
        } else {
            "active"
        };
        println!(
            "    - id={}, email={}, name={}, status={}",
            m.id, m.email, m.name, status
        );
    }

    // find_only_deleted 仅返回软删除记录（id=1）
    let deleted = Model::find_only_deleted(&session).await?;
    assert_eq!(deleted.len(), 1, "there should be 1 soft-deleted record");
    assert_eq!(deleted[0].id, 1, "the soft-deleted record should be id=1");
    println!(
        "\n  ✓ find_only_deleted() returned {} soft-deleted records:",
        deleted.len()
    );
    for m in &deleted {
        println!(
            "    - id={}, email={}, name={}, deleted_at={:?}",
            m.id, m.email, m.name, m.deleted_at
        );
    }

    // ============================================
    // 总结
    // ============================================
    println!("\n========================================");
    println!("✨ Example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - soft_delete = true     enable soft delete (auto-injects the deleted_at field)");
    println!(
        "  - UNIQUE(email, deleted_at)  composite unique constraint allowing re-registration with the same email"
    );
    println!(
        "  - find_all()             automatically filters records where deleted_at IS NOT NULL"
    );
    println!("  - find_with_deleted()    returns all records (including soft-deleted)");
    println!("  - find_only_deleted()    returns only the soft-deleted records");
    println!("  - delete()               soft delete (UPDATE SET deleted_at = now)");
    println!("  - force_delete()         physical delete (a real DELETE)");
    println!("\n⚠️  Production note:");
    println!("  in SQLite/PostgreSQL, NULLs in a unique constraint are treated as distinct,");
    println!(
        "  so UNIQUE(email, deleted_at) cannot stop two active records (deleted_at=NULL) from sharing an email."
    );
    println!("  a partial unique index should additionally be created:");
    println!(
        "    CREATE UNIQUE INDEX idx_members_email_active ON members(email) WHERE deleted_at IS NULL;"
    );

    Ok(())
}
