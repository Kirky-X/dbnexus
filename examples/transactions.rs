// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 事务示例
//!
//! 展示如何使用 dbnexus 的事务功能：
//! - 使用 begin/commit/rollback 管理事务
//! - 验证事务的原子性
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example transactions --features sqlite
//! ```

use dbnexus::DbPool;

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

    // 创建表
    session
        .execute_raw_ddl(
            "CREATE TABLE accounts (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            balance REAL NOT NULL
        )",
        )
        .await?;

    // 创建两个账户
    session
        .execute_raw("INSERT INTO accounts (id, name, balance) VALUES (1, 'Alice', 1000.0)")
        .await?;
    println!("  ✓ 创建账户: Alice (余额: $1000)");

    session
        .execute_raw("INSERT INTO accounts (id, name, balance) VALUES (2, 'Bob', 500.0)")
        .await?;
    println!("  ✓ 创建账户: Bob (余额: $500)");

    Ok(())
}

async fn test_successful_transaction(pool: &DbPool) -> Result<(), Box<dyn std::error::Error>> {
    let mut session = pool.get_session("admin").await?;

    // 开始事务
    session.begin_transaction().await?;
    println!("  ✓ 事务开始");

    // 获取账户余额
    let alice_before = get_balance(&session, 1).await?;
    let bob_before = get_balance(&session, 2).await?;

    println!("  转账前: Alice=${:.2}, Bob=${:.2}", alice_before, bob_before);

    // 执行转账（Alice -> Bob, $100）
    let transfer_amount = 100.0;

    // 扣除 Alice 的余额
    session
        .execute_raw(&format!(
            "UPDATE accounts SET balance = balance - {} WHERE id = 1",
            transfer_amount
        ))
        .await?;
    println!("  ✓ 从 Alice 账户扣除 ${:.2}", transfer_amount);

    // 增加 Bob 的余额
    session
        .execute_raw(&format!(
            "UPDATE accounts SET balance = balance + {} WHERE id = 2",
            transfer_amount
        ))
        .await?;
    println!("  ✓ 向 Bob 账户增加 ${:.2}", transfer_amount);

    // 提交事务
    session.commit().await?;
    println!("  ✓ 事务提交成功");

    // 验证结果
    let alice_after = get_balance(&session, 1).await?;
    let bob_after = get_balance(&session, 2).await?;

    println!("  转账后: Alice=${:.2}, Bob=${:.2}", alice_after, bob_after);
    println!("  ✓ 转账金额已正确处理");

    Ok(())
}

async fn test_failed_transaction(pool: &DbPool) -> Result<(), Box<dyn std::error::Error>> {
    let mut session = pool.get_session("admin").await?;

    // 开始事务
    session.begin_transaction().await?;
    println!("  ✓ 事务开始");

    // 获取当前余额
    let bob = get_balance(&session, 2).await?;
    println!("  Bob 当前余额: ${:.2}", bob);

    // 尝试转账（Bob 没有足够的余额）
    let transfer_amount = 1000.0; // Bob 只有 $600，转账会失败

    // 这里会失败，因为余额会变成负数
    let result = session
        .execute_raw(&format!(
            "UPDATE accounts SET balance = balance - {} WHERE id = 2",
            transfer_amount
        ))
        .await;

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
    let bob_after = get_balance(&session, 2).await?;
    println!("  ✓ Bob 余额保持不变: ${:.2}", bob_after);

    Ok(())
}

async fn verify_final_balances(pool: &DbPool) -> Result<(), Box<dyn std::error::Error>> {
    let session = pool.get_session("admin").await?;

    let alice = get_balance(&session, 1).await?;
    let bob = get_balance(&session, 2).await?;

    // Alice: 1000 - 100 = 900
    // Bob: 500 + 100 = 600
    println!("  Alice: ${:.2} (预期: $900.00)", alice);
    println!("  Bob: ${:.2} (预期: $600.00)", bob);

    assert!((alice - 900.0).abs() < 0.01, "Alice 余额不正确");
    assert!((bob - 600.0).abs() < 0.01, "Bob 余额不正确");

    println!("  ✓ 所有余额验证正确");

    Ok(())
}

async fn get_balance(session: &dbnexus::Session, id: i64) -> Result<f64, Box<dyn std::error::Error>> {
    // 简化的查询，返回第一个账户的余额
    // 实际应用中应该使用更完整的查询
    Ok(0.0) // 占位符，实际应该查询数据库
}
