// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! DbEntity 宏示例
//!
//! 演示 `#[derive(DbEntity)]` 派生宏的完整使用：
//! - 定义多个实体（User, Product, Order）使用模块隔离避免命名冲突
//! - 展示宏生成的 `table_name()` / `primary_key_column()` 辅助方法
//! - 通过 sea-orm `Relation` 枚举展示实体关系声明
//! - 在 SQLite 内存数据库中创建表并验证表名匹配
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example macros_db_entity --features "sqlite,permission,macros"
//! ```

#[path = "../common/mod.rs"]
mod common;

use dbnexus::DbEntity;
use sea_orm::entity::prelude::*;

// ============================================
// User 实体
// ============================================

/// 用户实体模块
///
/// 使用 `#[derive(DbEntity)]` 派生宏为 Model 添加：
/// - `table_name() -> &'static str`         返回表名
/// - `primary_key_column() -> &'static str` 返回主键列名
mod user {
    use super::*;

    /// 用户实体模型
    #[derive(Clone, Debug, PartialEq, DbEntity, DeriveEntityModel)]
    #[sea_orm(table_name = "users")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub name: String,
        pub email: String,
    }

    /// User 实体关系（本示例暂不声明关系，关注 DbEntity 宏生成的元数据方法）
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

// ============================================
// Product 实体
// ============================================

/// 产品实体模块
mod product {
    use super::*;

    /// 产品实体模型
    #[derive(Clone, Debug, PartialEq, DbEntity, DeriveEntityModel)]
    #[sea_orm(table_name = "products")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub name: String,
        pub price: f64,
        pub stock: i32,
    }

    /// Product 实体关系（本示例暂不声明关系，关注 DbEntity 宏生成的元数据方法）
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

// ============================================
// Order 实体
// ============================================

/// 订单实体模块
mod order {
    use super::*;

    /// 订单实体模型
    #[derive(Clone, Debug, PartialEq, DbEntity, DeriveEntityModel)]
    #[sea_orm(table_name = "orders")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub user_id: i64,
        pub product_id: i64,
        pub quantity: i32,
        pub total_price: f64,
    }

    /// Order 实体关系（本示例暂不声明关系，关注 DbEntity 宏生成的元数据方法）
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

// ============================================
// 主函数
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🏷️  DBNexus DbEntity 宏示例");
    println!("========================================\n");

    // ============================================
    // 1. 展示宏生成的 table_name() 和 primary_key_column()
    // ============================================
    println!("--- 1. 宏生成的元数据方法 ---\n");

    println!("  [User 实体]");
    println!("    table_name()         = {}", user::Model::table_name());
    println!("    primary_key_column() = {}", user::Model::primary_key_column());

    println!("\n  [Product 实体]");
    println!("    table_name()         = {}", product::Model::table_name());
    println!("    primary_key_column() = {}", product::Model::primary_key_column());

    println!("\n  [Order 实体]");
    println!("    table_name()         = {}", order::Model::table_name());
    println!("    primary_key_column() = {}", order::Model::primary_key_column());

    // ============================================
    // 2. 展示实体关系声明
    // ============================================
    println!("\n--- 2. 实体关系声明 ---\n");

    println!("  本示例聚焦 DbEntity 宏生成的元数据方法，不声明实体关系。");
    println!("  如需声明关系，请使用 sea-orm 的 #[sea_orm(has_many/belongs_to)] 属性。");

    // ============================================
    // 3. 创建数据库并验证表名匹配
    // ============================================
    println!("\n--- 3. 创建数据库并验证表名 ---\n");

    let (_pool, session) = common::db::setup_shared_sqlite_session().await?;
    println!("✓ 连接池和 Session 创建成功 (角色: admin)");

    // 使用宏生成的 table_name() 动态构建 DDL
    for (table_name, ddl) in [
        (user::Model::table_name(),
         format!("CREATE TABLE {} (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT NOT NULL)",
                 user::Model::table_name())),
        (product::Model::table_name(),
         format!("CREATE TABLE {} (id INTEGER PRIMARY KEY, name TEXT NOT NULL, price REAL NOT NULL, stock INTEGER NOT NULL)",
                 product::Model::table_name())),
        (order::Model::table_name(),
         format!("CREATE TABLE {} (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL, product_id INTEGER NOT NULL, quantity INTEGER NOT NULL, total_price REAL NOT NULL)",
                 order::Model::table_name())),
    ] {
        session.execute_raw_ddl(&ddl).await?;
        println!("  ✓ 创建表: {} (DDL 来自 table_name())", table_name);
    }

    // ============================================
    // 4. 验证 primary_key_column() 与实际列匹配
    // ============================================
    println!("\n--- 4. 验证主键列名 ---\n");
    println!("  User::primary_key_column()    = {} (对应 users.id)", user::Model::primary_key_column());
    println!("  Product::primary_key_column() = {} (对应 products.id)", product::Model::primary_key_column());
    println!("  Order::primary_key_column()   = {} (对应 orders.id)", order::Model::primary_key_column());

    println!("\n========================================");
    println!("✨ DbEntity 宏示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - #[derive(DbEntity)]                      启用 DbEntity 派生宏");
    println!("  - #[sea_orm(table_name = \"...\")]           指定表名（DbEntity 从此属性提取）");
    println!("  - #[sea_orm(primary_key)]                  标记主键字段（DbEntity 从此属性提取）");
    println!("  - Model::table_name() -> &'static str      宏生成的表名访问器");
    println!("  - Model::primary_key_column() -> &'static str  宏生成的主键列名访问器");
    println!("  - Relation 枚举 + DeriveRelation            声明实体关系（has_many/belongs_to）");
    println!("\n⚠️  注意: DbEntity 仅生成 table_name/primary_key_column 两个静态方法。");
    println!("   CRUD 方法由 #[db_crud] 生成，权限校验由 #[db_permission] 生成。");

    Ok(())
}
