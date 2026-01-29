// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 正确的 dbnexus 使用方式示例
//!
//! 本示例展示如何正确使用 dbnexus 的 `#[db_crud]` 属性宏：
//! - 正确的宏组合（DeriveEntityModel + DeriveModel + DeriveActiveModel）
//! - 使用 #[db_crud] 属性宏生成 CRUD 方法
//! - 通过 Entity API 进行数据库操作
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example correct_usage --features sqlite,permission,metrics,audit
//! ```

use dbnexus::{DbConfigBuilder, DbEntity, DbPool, db_crud};
use sea_orm::entity::prelude::*;

// ============================================
// 正确的实体定义方式
// ============================================

/// 用户实体
///
/// ✅ 正确的宏组合：
/// - `DeriveEntityModel` - 生成 Entity、Model、ActiveModel
/// - `DeriveModel` - 从 Model 生成 ActiveModel
/// - `DeriveActiveModel` - 支持 ActiveModel 操作
/// - `#[db_crud]` - 自动生成带权限控制的 CRUD 方法
///
/// ✅ 正确的属性：
/// - `#[sea_orm(table_name = "...")]` - 指定表名
/// - `#[sea_orm(primary_key)]` - 标记主键字段
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, DeriveModel, DeriveActiveModel)]
#[sea_orm(table_name = "users")]
#[db_crud] // ← 关键：使用属性宏生成 CRUD 方法
pub struct User {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub email: String,
    pub age: i32,
    pub status: String,
}

/// 日志实体
///
/// 展示不同的字段类型
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, DeriveModel, DeriveActiveModel)]
#[sea_orm(table_name = "logs")]
#[db_crud]
pub struct Log {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub level: String,
    pub message: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// ============================================
// 主函数
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🗄️  dbnexus 正确使用方式示例\n");

    // 初始化连接池
    let config = DbConfigBuilder::new()
        .url("sqlite:file::memory:?cache=shared")
        .permissions_path("src/permissions.yaml")
        .admin_role("admin")
        .build()
        .expect("Failed to build config");
    let pool = DbPool::with_config(config).await?;
    println!("✓ 连接池创建成功");

    // 获取管理员 Session
    let session = pool.get_session("admin").await?;
    println!("✓ Session 获取成功 (角色: admin)");

    // 创建表
    session
        .execute_raw_ddl(
            "CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                email TEXT NOT NULL,
                age INTEGER,
                status TEXT DEFAULT 'active'
            )",
        )
        .await?;
    session
        .execute_raw_ddl(
            "CREATE TABLE logs (
                id INTEGER PRIMARY KEY,
                level TEXT NOT NULL,
                message TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
        )
        .await?;
    println!("✓ 表创建成功");

    // ============================================
    // CRUD 操作演示
    // ============================================
    println!("\n📋 CRUD 操作演示:\n");

    // ------------------------------------------------
    // 1. 插入数据 (CREATE)
    // ------------------------------------------------
    println!("【1】插入数据 (CREATE)");

    let user = User {
        id: 0, // 主键由数据库自动生成
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
        age: 30,
        status: "active".to_string(),
    };
    // ✅ 使用 Entity::insert() - 宏生成的方法
    let inserted_user = Entity::insert(&session, user).await?;
    println!("   ✓ 插入用户: {} (id={})", inserted_user.name, inserted_user.id);

    let log = Log {
        id: 0,
        level: "INFO".to_string(),
        message: "User created successfully".to_string(),
        created_at: chrono::Utc::now(),
    };
    // ✅ 使用 LogEntity::insert()
    let inserted_log = LogEntity::insert(&session, log).await?;
    println!("   ✓ 插入日志: [{}] {}", inserted_log.level, inserted_log.message);

    // ------------------------------------------------
    // 2. 查询数据 (READ)
    // ------------------------------------------------
    println!("\n【2】查询数据 (READ)");

    // 2.1 按 ID 查询
    // ✅ 使用 Entity::find_by_id()
    let user_by_id = Entity::find_by_id(&session, inserted_user.id).await?;
    println!("   ✓ 按 ID 查询: 用户 {} (email={})", user_by_id.name, user_by_id.email);

    // 2.2 查询所有
    // ✅ 使用 Entity::find_all()
    let all_users = Entity::find_all(&session).await?;
    println!("   ✓ 查询所有: {} 个用户", all_users.len());

    // 2.3 条件查询
    // ✅ 使用 Entity::find_by_condition()
    let condition = Condition::all().add(Column::Name.eq("Alice")).add(Column::Age.gte(25));
    let adult_alices = Entity::find_by_condition(&session, condition).await?;
    println!("   ✓ 条件查询: {} 个 25 岁以上的 Alice", adult_alices.len());

    // 2.4 统计数量
    // ✅ 使用 Entity::count()
    let user_count = Entity::count(&session).await?;
    println!("   ✓ 统计数量: {} 个用户", user_count);

    // ------------------------------------------------
    // 3. 更新数据 (UPDATE)
    // ------------------------------------------------
    println!("\n【3】更新数据 (UPDATE)");

    // ✅ 使用 Entity::update()
    let updated_user = Entity::update(&session, user_by_id).await?;
    println!("   ✓ 更新用户: {} (age 字段可更新)", updated_user.name);

    // ------------------------------------------------
    // 4. 删除数据 (DELETE)
    // ------------------------------------------------
    println!("\n【4】删除数据 (DELETE)");

    // 4.1 按 ID 删除
    // ✅ 使用 Entity::delete()
    let deleted_count = Entity::delete(&session, inserted_user.id).await?;
    println!("   ✓ 按 ID 删除: 影响 {} 行", deleted_count);

    // 4.2 批量删除
    // ✅ 使用 Entity::delete_many()
    let delete_condition = Condition::all().add(Column::Status.eq("inactive"));
    let batch_deleted = Entity::delete_many(&session, delete_condition).await?;
    println!("   ✓ 批量删除: 影响 {} 行", batch_deleted);

    // ============================================
    // 权限控制演示
    // ============================================
    println!("\n📋 权限控制演示:\n");

    // 获取管理员 Session - 可以访问
    let admin_session = pool.get_session("admin").await?;
    let admin_users = Entity::find_all(&admin_session).await?;
    println!("✓ Admin 角色: 可以查询用户 (找到 {} 个)", admin_users.len());

    // 获取普通用户 Session - 会被拒绝访问（如果配置了权限）
    // let user_session = pool.get_session("guest").await?;
    // let guest_users = Entity::find_all(&user_session).await?; // 会报错！
    // println!("✗ Guest 角色: 被拒绝访问");

    // ============================================
    // 指标收集演示（需要启用 metrics 特性）
    // ============================================
    println!("\n📋 指标收集演示:");
    println!("   (如果启用了 metrics 特性，以下操作会被记录)");
    println!("   - insert: 记录到 'insert' 指标");
    println!("   - select: 记录到 'select' 指标");
    println!("   - update: 记录到 'update' 指标");
    println!("   - delete: 记录到 'delete' 指标");

    // ============================================
    // 审计日志演示（需要启用 audit 特性）
    // ============================================
    println!("\n📋 审计日志演示:");
    println!("   (如果启用了 audit 特性，以下操作会被审计)");
    println!("   - 所有 CRUD 操作会被记录到审计日志");
    println!("   - 包括操作类型、用户、时间戳等信息");

    // ============================================
    // 清理
    // ============================================
    session.execute_raw_ddl("DROP TABLE users").await?;
    session.execute_raw_ddl("DROP TABLE logs").await?;
    println!("\n✓ 清理完成");

    // 获取连接池状态
    let status = pool.status();
    println!(
        "\n📊 连接池状态: total={}, active={}, idle={}",
        status.total, status.active, status.idle
    );

    println!("\n✨ 示例运行完成！");
    println!("\n📚 总结:");
    println!("  1. 使用 #[db_crud] 属性宏自动生成 CRUD 方法");
    println!("  2. CRUD 方法自动包含权限检查和指标收集");
    println!("  3. 通过 Entity API 进行所有数据库操作");
    println!("  4. Session 封装底层连接，保证安全性");

    Ok(())
}
