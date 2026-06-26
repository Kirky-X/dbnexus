// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

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
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelTrait, EntityTrait};

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
    println!("🗑️  软删除 + 复合唯一约束示例");
    println!("========================================\n");

    // ============================================
    // 1. 创建数据库 + 带复合唯一约束的表
    // ============================================
    println!("--- 1. 创建 members 表（UNIQUE(email, deleted_at)）---\n");

    let (_pool, session) = common::db::setup_shared_sqlite_session().await?;
    println!("  ✓ 连接 SQLite 内存数据库成功 (角色: admin)");

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
    println!("  ✓ 创建表 members，约束：UNIQUE(email, deleted_at)");
    println!("    → 软删除后同 email 可重新注册\n");

    // ============================================
    // 2. 注册第一个用户 alice@example.com
    // ============================================
    println!("--- 2. 注册用户 alice@example.com ---\n");

    let alice_v1 = Model {
        id: 1,
        email: "alice@example.com".to_string(),
        name: "Alice (初版)".to_string(),
        deleted_at: None,
    };
    let am: ActiveModel = alice_v1.into();
    let inserted: Model = am
        .insert(session.connection()?)
        .await?;
    println!("  ✓ 插入记录: id={}, email={}, name={}", inserted.id, inserted.email, inserted.name);

    // ============================================
    // 3. 软删除 alice
    // ============================================
    println!("\n--- 3. 软删除 alice（DELETE SET deleted_at = now）---\n");

    let affected = Model::delete(&session, 1).await?;
    assert_eq!(affected, 1, "软删除应影响 1 行");
    println!("  ✓ 软删除 id=1，deleted_at 已设置（非物理删除）");

    // 验证 find_all 不返回软删除记录
    let active = Model::find_all(&session).await?;
    assert!(active.is_empty(), "find_all 不应返回软删除记录");
    println!("  ✓ find_all() 返回 {} 条记录（已过滤软删除）", active.len());

    // 验证 find_with_deleted 返回软删除记录
    let with_deleted = Model::find_with_deleted(&session).await?;
    assert_eq!(with_deleted.len(), 1, "find_with_deleted 应返回 1 条（含软删除）");
    assert!(with_deleted[0].deleted_at.is_some(), "软删除记录的 deleted_at 应已设置");
    println!("  ✓ find_with_deleted() 返回 {} 条记录（含软删除）", with_deleted.len());

    // ============================================
    // 4. 用同一 email 重新注册（关键演示）
    // ============================================
    println!("\n--- 4. 用同一 email 重新注册（UNIQUE(email, deleted_at) 生效）---\n");

    let alice_v2 = Model {
        id: 2,
        email: "alice@example.com".to_string(),
        name: "Alice (重新注册)".to_string(),
        deleted_at: None,
    };
    let am: ActiveModel = alice_v2.into();
    let inserted: Model = am
        .insert(session.connection()?)
        .await?;
    println!("  ✓ 重新注册成功: id={}, email={}, name={}", inserted.id, inserted.email, inserted.name);
    println!("    → 若使用 UNIQUE(email) 而非 UNIQUE(email, deleted_at)，此插入会失败！");

    // ============================================
    // 5. 验证最终状态
    // ============================================
    println!("\n--- 5. 验证最终状态 ---\n");

    // find_all 仅返回活跃记录（id=2）
    let active = Model::find_all(&session).await?;
    assert_eq!(active.len(), 1, "应有 1 条活跃记录");
    assert_eq!(active[0].id, 2, "活跃记录应为 id=2");
    assert_eq!(active[0].email, "alice@example.com");
    println!("  ✓ find_all() 返回 {} 条活跃记录:", active.len());
    for m in &active {
        println!("    - id={}, email={}, name={}, deleted_at={:?}", m.id, m.email, m.name, m.deleted_at);
    }

    // find_with_deleted 返回全部记录（id=1 软删除 + id=2 活跃）
    let all = Model::find_with_deleted(&session).await?;
    assert_eq!(all.len(), 2, "应有 2 条记录（含软删除）");
    println!("\n  ✓ find_with_deleted() 返回 {} 条记录（含软删除）:", all.len());
    for m in &all {
        let status = if m.deleted_at.is_some() { "已软删除" } else { "活跃" };
        println!("    - id={}, email={}, name={}, 状态={}", m.id, m.email, m.name, status);
    }

    // find_only_deleted 仅返回软删除记录（id=1）
    let deleted = Model::find_only_deleted(&session).await?;
    assert_eq!(deleted.len(), 1, "应有 1 条软删除记录");
    assert_eq!(deleted[0].id, 1, "软删除记录应为 id=1");
    println!("\n  ✓ find_only_deleted() 返回 {} 条软删除记录:", deleted.len());
    for m in &deleted {
        println!("    - id={}, email={}, name={}, deleted_at={:?}", m.id, m.email, m.name, m.deleted_at);
    }

    // ============================================
    // 总结
    // ============================================
    println!("\n========================================");
    println!("✨ 示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - soft_delete = true     启用软删除（自动注入 deleted_at 字段）");
    println!("  - UNIQUE(email, deleted_at)  复合唯一约束，允许同 email 重新注册");
    println!("  - find_all()             自动过滤 deleted_at IS NOT NULL 的记录");
    println!("  - find_with_deleted()    返回全部记录（含软删除）");
    println!("  - find_only_deleted()    仅返回软删除记录");
    println!("  - delete()               软删除（UPDATE SET deleted_at = now）");
    println!("  - force_delete()         物理删除（真正 DELETE）");
    println!("\n⚠️  生产环境注意:");
    println!("  SQLite/PostgreSQL 中 NULL 在唯一约束里互不相同，");
    println!("  UNIQUE(email, deleted_at) 不能阻止两条活跃记录（deleted_at=NULL）拥有相同 email。");
    println!("  应额外创建部分唯一索引：");
    println!("    CREATE UNIQUE INDEX idx_members_email_active ON members(email) WHERE deleted_at IS NULL;");

    Ok(())
}
