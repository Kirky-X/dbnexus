// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! db_entity 宏 CRUD 示例
//!
//! 演示 `#[db_entity(...)]` 统一属性宏生成的完整 CRUD 方法：
//! - `insert`             插入记录
//! - `find_by_id`         按主键查询
//! - `find_all`           查询全部
//! - `find_by_condition`  条件查询（sea_orm::Condition）
//! - `update`             更新记录
//! - `delete`             按主键删除
//! - `delete_many`        批量删除
//! - `count`              统计记录数
//!
//! 同时演示：
//! - 批量插入（循环调用 insert）
//! - 分页查询（count + find_by_condition + 手动切片）
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example macros_db_crud --features "sqlite,permission,macros"
//! ```

#[path = "../common/mod.rs"]
mod common;

use dbnexus::db_entity;
use sea_orm::entity::prelude::*;

// ============================================
// 定义 Product 实体（带 db_entity 宏）
// ============================================

/// 产品实体
///
/// `#[db_entity(table_name = "products", primary_key = "id")]` 自动生成 8 个 CRUD 方法，
/// 每个方法都通过 Session 执行权限检查、指标收集和数据库操作。
#[db_entity(table_name = "products", primary_key = "id")]
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
// 主函数
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🔧 DBNexus db_crud 宏示例");
    println!("========================================\n");

    // ============================================
    // 1. 创建连接池和 Session
    // ============================================
    let (_pool, session) = common::db::setup_shared_sqlite_session().await?;
    println!("✓ 连接池和 Session 创建成功 (角色: admin)\n");

    // ============================================
    // 2. 建表
    // ============================================
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
    println!("✓ products 表创建成功\n");

    // ============================================
    // 3. 批量插入（insert）
    // ============================================
    println!("--- INSERT（批量插入） ---");
    let products = vec![
        (1, "Laptop".to_string(), 1299.99, 15),
        (2, "Mouse".to_string(), 29.99, 100),
        (3, "Keyboard".to_string(), 79.99, 50),
        (4, "Monitor".to_string(), 449.99, 20),
        (5, "Headset".to_string(), 99.99, 0),
        (6, "Webcam".to_string(), 59.99, 30),
        (7, "USB Hub".to_string(), 19.99, 75),
        (8, "Docking Station".to_string(), 199.99, 10),
        (9, "Mousepad".to_string(), 9.99, 200),
        (10, "Speaker".to_string(), 49.99, 5),
    ];

    for (id, name, price, stock) in &products {
        let model = Model {
            id: *id,
            name: name.clone(),
            price: *price,
            stock: *stock,
        };
        let inserted = Model::insert(&session, model).await?;
        println!("  ✓ 插入: id={}, name={}, price={:.2}, stock={}",
            inserted.id, inserted.name, inserted.price, inserted.stock);
    }
    println!("  共插入 {} 条记录", products.len());

    // ============================================
    // 4. count — 统计记录数
    // ============================================
    println!("\n--- COUNT ---");
    let total = Model::count(&session).await?;
    println!("  ✓ 总记录数: {}", total);

    // ============================================
    // 5. find_by_id — 按主键查询
    // ============================================
    println!("\n--- FIND_BY_ID ---");
    let product = Model::find_by_id(&session, 1).await?;
    if let Some(p) = product {
        println!("  ✓ 找到 id=1: name={}, price={:.2}", p.name, p.price);
    } else {
        println!("  ✗ 未找到 id=1");
    }

    // 查询不存在的记录
    let missing = Model::find_by_id(&session, 999).await?;
    println!("  ✓ 查询 id=999: {}", if missing.is_none() { "未找到（符合预期）" } else { "找到了" });

    // ============================================
    // 6. find_all — 查询全部
    // ============================================
    println!("\n--- FIND_ALL ---");
    let all = Model::find_all(&session).await?;
    println!("  ✓ 共 {} 条记录", all.len());
    for p in all.iter().take(3) {
        println!("    - id={}, name={}, stock={}", p.id, p.name, p.stock);
    }
    println!("    ... (省略其余)");

    // ============================================
    // 7. find_by_condition — 条件查询
    // ============================================
    println!("\n--- FIND_BY_CONDITION ---");

    // 条件 1: stock < 10（低库存）
    let low_stock_cond = sea_orm::Condition::all().add(Column::Stock.lt(10));
    let low_stock = Model::find_by_condition(&session, low_stock_cond).await?;
    println!("  [stock < 10] 共 {} 条:", low_stock.len());
    for p in &low_stock {
        println!("    - id={}, name={}, stock={}", p.id, p.name, p.stock);
    }

    // 条件 2: price > 100
    let expensive_cond = sea_orm::Condition::all().add(Column::Price.gt(100.0));
    let expensive = Model::find_by_condition(&session, expensive_cond).await?;
    println!("\n  [price > 100.0] 共 {} 条:", expensive.len());
    for p in &expensive {
        println!("    - id={}, name={}, price={:.2}", p.id, p.name, p.price);
    }

    // 条件 3: 复合条件 stock > 0 AND price < 50
    let affordable_cond = sea_orm::Condition::all()
        .add(Column::Stock.gt(0))
        .add(Column::Price.lt(50.0));
    let affordable = Model::find_by_condition(&session, affordable_cond).await?;
    println!("\n  [stock > 0 AND price < 50.0] 共 {} 条:", affordable.len());
    for p in &affordable {
        println!("    - id={}, name={}, price={:.2}, stock={}", p.id, p.name, p.price, p.stock);
    }

    // ============================================
    // 8. 分页查询（count + find_by_condition + 手动切片）
    // ============================================
    println!("\n--- 分页查询 ---");
    let page_size: u64 = 3;
    let total_count = Model::count(&session).await?;
    let total_pages = (total_count + page_size - 1) / page_size;
    println!("  总记录数: {}, 每页: {}, 总页数: {}", total_count, page_size, total_pages);

    let all_for_paging = Model::find_all(&session).await?;
    for page in 1..=total_pages as usize {
        let start = (page - 1) * page_size as usize;
        let end = std::cmp::min(start + page_size as usize, all_for_paging.len());
        let page_items = &all_for_paging[start..end];
        println!("  第 {} 页 ({} 条):", page, page_items.len());
        for p in page_items {
            println!("    - id={}, name={}", p.id, p.name);
        }
    }

    // ============================================
    // 9. update — 更新记录
    // ============================================
    println!("\n--- UPDATE ---");
    let to_update = Model::find_by_id(&session, 5).await?.unwrap();
    println!("  更新前: id={}, name={}, price={:.2}, stock={}",
        to_update.id, to_update.name, to_update.price, to_update.stock);

    let updated = Model::update(&session, Model {
        price: 89.99,
        stock: 25,
        ..to_update
    }).await?;
    println!("  更新后: id={}, name={}, price={:.2}, stock={}",
        updated.id, updated.name, updated.price, updated.stock);

    // ============================================
    // 10. delete_many — 批量删除
    // ============================================
    println!("\n--- DELETE_MANY ---");
    // 删除所有 stock == 0 的记录
    let zero_stock_cond = sea_orm::Condition::all().add(Column::Stock.eq(0));
    let before_count = Model::count(&session).await?;
    let deleted = Model::delete_many(&session, zero_stock_cond).await?;
    let after_count = Model::count(&session).await?;
    println!("  ✓ 批量删除 stock=0 的记录: 删除 {} 行", deleted);
    println!("  删除前: {} 条, 删除后: {} 条", before_count, after_count);

    // ============================================
    // 11. delete — 按主键删除
    // ============================================
    println!("\n--- DELETE ---");
    let deleted_one = Model::delete(&session, 1).await?;
    println!("  ✓ 删除 id=1: 影响 {} 行", deleted_one);

    // 验证删除
    let check = Model::find_by_id(&session, 1).await?;
    println!("  ✓ 删除后查询 id=1: {}", if check.is_none() { "未找到（符合预期）" } else { "仍存在" });

    // ============================================
    // 最终状态
    // ============================================
    println!("\n--- 最终状态 ---");
    let final_count = Model::count(&session).await?;
    println!("  ✓ 剩余记录数: {}", final_count);

    println!("\n========================================");
    println!("✨ db_entity 宏 CRUD 示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - #[db_entity(table_name=\"...\", primary_key=\"...\")]  统一属性宏自动生成 CRUD 方法");
    println!("  - Model::insert(&session, model)     插入记录");
    println!("  - Model::find_by_id(&session, id)    按主键查询");
    println!("  - Model::find_all(&session)          查询全部");
    println!("  - Model::find_by_condition(&session, cond)  条件查询");
    println!("  - Model::count(&session)             统计记录数");
    println!("  - Model::update(&session, model)     更新记录");
    println!("  - Model::delete(&session, id)        按主键删除");
    println!("  - Model::delete_many(&session, cond) 批量删除");
    println!("  - sea_orm::Condition::all().add(...) 构建查询条件");
    println!("  - Column::Xxx.lt/gt/eq(value)        列条件运算");
    println!("\n⚠️  注意: #[db_entity] 生成的所有方法都通过 Session 自动执行权限检查。");
    println!("   分页查询在 db_entity 层面通过 find_all + 手动切片实现；");
    println!("   如需 DB 层面的 offset/limit，请直接使用 sea_orm::EntityTrait。");

    Ok(())
}
