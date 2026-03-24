// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 宏使用完整示例
//!
//! 展示 DBNexus 所有 5 个派生宏和属性宏的组合使用：
//! - DbEntity        → 生成 table_name()、primary_key_column() 等元方法
//! - DeriveEntityModel → SeaORM 生成的 Model/Entity/Column 类型
//! - db_crud         → 生成 insert/update/delete/find_by_id/find_all
//! - db_permission   → 生成权限检查常量和 check_operation()
//! - db_cache        → 生成缓存常量和 cache_key()/cache_config()
//! - db_audit        → 生成审计相关常量
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example macro_usage --features macros,sqlite,permission,cache,audit
//! ```

use dbnexus::{DbConfig, DbEntity, DbPool, db_audit, db_cache, db_crud, db_permission};
use sea_orm::entity::prelude::*;

// ============================================
// 定义 Product 实体 - 展示所有宏的组合
// 注意: DbEntity 必须在 DeriveEntityModel 之前
// ============================================

#[derive(Clone, Debug, PartialEq, DbEntity, DeriveEntityModel)]
#[sea_orm(table_name = "products")]
#[db_crud(table_name = "products")]
#[db_permission(roles = ["admin", "manager", "user"], operations = ["create", "read", "update", "delete"])]
#[db_cache(ttl = 3600, max_capacity = 1000)]
#[db_audit(table_name = "products_audit")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub price: f64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

/// 运行宏使用示例
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    run().await
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("📦 DBNexus 宏使用完整示例");
    println!("========================================\n");

    // ============================================
    // 1. 展示 DbEntity 生成的元方法
    // ============================================
    println!("--- DbEntity 元方法 ---");
    println!("  table_name()        = \"{}\"", Model::table_name());
    println!("  primary_key_column() = \"{}\"\n", Model::primary_key_column());

    // ============================================
    // 2. 展示 db_crud 生成的常量
    // ============================================
    println!("--- db_crud 常量 ---");
    println!("  CRUD_TABLE_NAME = \"{}\"", Model::CRUD_TABLE_NAME);
    println!("  ALLOWED_OPERATIONS = {:?}\n", Model::ALLOWED_OPERATIONS);

    // ============================================
    // 3. 展示 db_permission 生成的常量和权限检查
    // ============================================
    println!("--- db_permission ---");
    println!("  ALLOWED_ROLES = {:?}", Model::ALLOWED_ROLES);
    println!("  (PermissionContext::new 需要 policy_cache 参数，跳过运行时演示)\n");

    // ============================================
    // 4. 展示 db_cache 生成的缓存配置
    // ============================================
    println!("--- db_cache ---");
    println!("  CACHE_MAX_CAPACITY = {}", Model::CACHE_MAX_CAPACITY);
    println!("  CACHE_TTL          = {} 秒", Model::CACHE_TTL);
    println!("  cache_key(&1)       = \"{}\"", Model::cache_key(&1));
    println!("  cache_config()       = {:?}\n", Model::cache_config());

    // ============================================
    // 5. 展示 db_audit 生成的审计配置
    // ============================================
    println!("--- db_audit ---");
    println!("  AUDIT_TABLE_NAME = \"{}\"", Model::AUDIT_TABLE_NAME);
    println!("  AUDIT_ROLES      = {:?}\n", Model::AUDIT_ROLES);

    // ============================================
    // 6. CRUD 实际操作演示
    // ============================================
    println!("--- CRUD 操作演示 ---");

    let config = DbConfig {
        url: "sqlite:file::memory:?cache=shared".to_string(),
        admin_role: "admin".to_string(),
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await?;
    let session = pool.get_session("admin").await?;

    // 创建表
    session
        .execute_raw_ddl(
            "CREATE TABLE products (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                price REAL NOT NULL
            )",
        )
        .await?;

    // INSERT
    let product = Model {
        id: 0,
        name: "笔记本".to_string(),
        price: 5999.0,
    };
    let inserted = Model::insert(&session, product).await?;
    println!("  ✓ 插入: id={}, name=笔记本, price=5999", inserted.id);

    // FIND_BY_ID
    let found = Model::find_by_id(&session, inserted.id).await?.expect("产品不存在");
    println!("  ✓ 查询: id={}, name={}, price={}", found.id, found.name, found.price);

    // UPDATE
    let updated = Model::update(&session, Model { price: 4999.0, ..found }).await?;
    println!("  ✓ 更新: id={}, 新价格={}", updated.id, updated.price);

    // DELETE
    let affected = Model::delete(&session, inserted.id).await?;
    println!("  ✓ 删除: id={}, 影响 {} 行", inserted.id, affected);

    // 清理
    session.execute_raw_ddl("DROP TABLE products").await?;

    println!("\n========================================");
    println!("✨ 宏使用完整示例完成！");
    println!("========================================");
    println!("\n📚 本示例展示了 6 个宏的组合使用:");
    println!("  - DbEntity         → table_name() / primary_key_column()");
    println!("  - DeriveEntityModel → SeaORM Model / Entity / Column");
    println!("  - db_crud          → insert / update / delete / find_by_id / find_all");
    println!("  - db_permission    → ALLOWED_ROLES / check_operation()");
    println!("  - db_cache         → cache_key() / cache_config()");
    println!("  - db_audit         → AUDIT_TABLE_NAME / AUDIT_ROLES");

    Ok(())
}
