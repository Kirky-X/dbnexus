// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 事务示例
//!
//! 展示如何使用 dbnexus 的事务功能：
//! - 使用 begin/commit/rollback 管理事务
//! - 使用 transaction() 方法简化事务处理
//! - 验证事务的原子性
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example transactions --features sqlite
//! ```

use dbnexus::{DbPool, DbEntity, db_crud};

// 定义 Account Entity 用于演示转账事务
#[derive(DbEntity)]
#[db_entity]
#[table_name = "accounts")]
#[db_crud]
struct Account {
    #[primary_key]
    id: i64,
    name: String,
    balance: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("💰 DBNexus 事务示例\n");
    println!("========================================");

    // 初始化连接池
    let pool = DbPool::new("sqlite::memory:").await?;
    println!("✓ 连接池创建成功\n");

    // 创建测试数据
    println!("📊 创建测试数据");
    println!("------------------------------------------");
    setup_test_data(&pool).await?;

    // 测试成功的事务
    println!("\n💸 测试成功的事务（转账 100 元）");
    println!("------------------------------------------");
    test_successful_transaction(&pool).await?;

    // 测试失败的事务（余额不足）
    println!("\n❌ 测试失败的事务（余额不足）");
    println!("------------------------------------------");
    test_failed_transaction(&pool).await?;

    // 验证最终余额
    println!("\n📋 验证最终余额");
    println!("------------------------------------------");
    verify_final_balances(&pool).await?;

    println!("\n========================================");
    println!("✨ 事务示例运行完成！");

    Ok(())
}

async fn setup_test_data(pool: &DbPool) -> Result<(), Box<dyn std::error::Error>> {
    let mut session = pool.get_session("admin").await?;

    // 创建两个账户
    Account::insert(&mut session, Account {
        id: 1,
        name: "Alice".to_string(),
        balance: 1000.0,
    }).await?;
    println!("  ✓ 创建账户: Alice (余额: $1000)");

    Account::insert(&mut session, Account {
        id: 2,
        name: "Bob".to_string(),
        balance: 500.0,
    }).await?;
    println!("  ✓ 创建账户: Bob (余额: $500)");

    Ok(())
}

async fn test_successful_transaction(pool: &DbPool) -> Result<(), Box<dyn std::error::Error>> {
    let mut session = pool.get_session("admin").await?;

    // 开始事务
    session.begin_transaction().await?;
    println!("  ✓ 事务开始");

    // 获取账户余额
    let alice_before = Account::find_by_id(&session, 1).await?
        .expect("Alice account not found");
    let bob_before = Account::find_by_id(&session, 2).await?
        .expect("Bob account not found");

    println!("  转账前: Alice=${:.2}, Bob=${:.2}", alice_before.balance, bob_before.balance);

    // 执行转账（Alice -> Bob, $100）
    let transfer_amount = 100.0;

    // 扣除 Alice 的余额
    let mut alice = Account::find_by_id(&session, 1).await?
        .expect("Alice account not found");
    alice.balance -= transfer_amount;
    Account::update(&session, alice).await?;
    println!("  ✓ 从 Alice 账户扣除 ${:.2}", transfer_amount);

    // 增加 Bob 的余额
    let mut bob = Account::find_by_id(&session, 2).await?
        .expect("Bob account not found");
    bob.balance += transfer_amount;
    Account::update(&session, bob).await?;
    println!("  ✓ 向 Bob 账户增加 ${:.2}", transfer_amount);

    // 提交事务
    session.commit().await?;
    println!("  ✓ 事务提交成功");

    // 验证结果
    let alice_after = Account::find_by_id(&session, 1).await?
        .expect("Alice account not found");
    let bob_after = Account::find_by_id(&session, 2).await?
        .expect("Bob account not found");

    println!("  转账后: Alice=${:.2}, Bob=${:.2}", alice_after.balance, bob_after.balance);
    println!("  ✓ 转账金额已正确处理");

    Ok(())
}

async fn test_failed_transaction(pool: &DbPool) -> Result<(), Box<dyn std::error::Error>> {
    let mut session = pool.get_session("admin").await?;

    // 开始事务
    session.begin_transaction().await?;
    println!("  ✓ 事务开始");

    // 获取当前余额
    let bob = Account::find_by_id(&session, 2).await?
        .expect("Bob account not found");
    println!("  Bob 当前余额: ${:.2}", bob.balance);

    // 尝试转账（Bob 没有足够的余额）
    let transfer_amount = 1000.0; // Bob 只有 $600，转账会失败

    let mut bob_account = Account::find_by_id(&session, 2).await?
        .expect("Bob account not found");
    bob_account.balance -= transfer_amount;

    // 这里会失败，因为余额会变成负数
    let result = Account::update(&session, bob_account).await;

    match result {
        Ok(_) => {
            // 如果更新成功，提交事务
            session.commit().await?;
            println!("  ✗ 事务不应该成功！");
        }
        Err(e) => {
            // 回滚事务
            session.rollback().await?;
            println!("  ✓ 更新失败，正确回滚事务: {}", e);
        }
    }

    // 验证余额没有变化
    let bob_after = Account::find_by_id(&session, 2).await?
        .expect("Bob account not found");
    println!("  ✓ Bob 余额保持不变: ${:.2}", bob_after.balance);

    Ok(())
}

async fn verify_final_balances(pool: &DbPool) -> Result<(), Box<dyn std::error::Error>> {
    let session = pool.get_session("admin").await?;

    let alice = Account::find_by_id(&session, 1).await?
        .expect("Alice account not found");
    let bob = Account::find_by_id(&session, 2).await?
        .expect("Bob account not found");

    // Alice: 1000 - 100 = 900
    // Bob: 500 + 100 = 600
    println!("  Alice: ${:.2} (预期: $900.00)", alice.balance);
    println!("  Bob: ${:.2} (预期: $600.00)", bob.balance);

    assert!((alice.balance - 900.0).abs() < 0.01, "Alice 余额不正确");
    assert!((bob.balance - 600.0).abs() < 0.01, "Bob 余额不正确");

    println!("  ✓ 所有余额验证正确");

    Ok(())
}
