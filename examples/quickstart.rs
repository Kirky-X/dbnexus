//! 快速开始示例
//!
//! 展示 dbnexus 的基本使用方法，包括：
//! - 定义 Entity 并自动生成 CRUD 方法
//! - 创建数据库连接池
//! - 获取 Session 执行数据库操作
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example quickstart --features sqlite
//! ```

use dbnexus::{DbPool, DbEntity, db_crud};

// 定义 User Entity
//
// #[derive(DbEntity)] 自动将结构体映射为 Sea-ORM Entity
// #[db_entity] 标记为 dbnexus Entity
// #[table_name = "users")] 指定数据库表名
// #[db_crud] 自动生成 CRUD 方法
#[derive(DbEntity)]
#[db_entity]
#[table_name = "users")]
#[db_crud]
struct User {
    /// 主键字段，使用 #[primary_key] 标记
    #[primary_key]
    id: i64,
    name: String,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化连接池（使用 SQLite 内存模式）
    // 在生产环境中，请使用实际的数据库连接字符串
    let pool = DbPool::new("sqlite::memory:").await?;
    println!("✓ 连接池创建成功");

    // 获取管理员 Session
    // Session 自动从连接池获取连接，并在 drop 时自动归还
    let session = pool.get_session("admin").await?;
    println!("✓ Session 获取成功 (角色: admin)");

    // 插入用户
    // User::insert 是由 #[db_crud] 自动生成的方法
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    let inserted = User::insert(&session, user).await?;
    println!("✓ 用户插入成功: {} <{}>", inserted.name, inserted.email);

    // 查询用户
    // User::find_by_id 根据主键查找记录
    let found = User::find_by_id(&session, 1).await?;
    if let Some(user) = found {
        println!("✓ 用户查询成功: {} <{}>", user.name, user.email);
    }

    // 更新用户
    // User::update 更新记录
    let mut user = found.unwrap();
    user.email = "alice_new@example.com".to_string();
    User::update(&session, user).await?;
    println!("✓ 用户更新成功");

    // 删除用户
    // User::delete 根据主键删除记录
    User::delete(&session, 1).await?;
    println!("✓ 用户删除成功");

    // 获取连接池状态
    let status = pool.status();
    println!(
        "\n📊 连接池状态: total={}, active={}, idle={}",
        status.total, status.active, status.idle
    );

    println!("\n✨ 示例运行完成！");

    Ok(())
}
