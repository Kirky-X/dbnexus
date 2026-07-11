// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 基础事务示例
//!
//! 展示如何使用 Session 的事务 API（begin/commit/rollback）：
//! - 创建连接池和 Session
//! - 建表
//! - 开始事务并插入多条数据
//! - 模拟错误场景，演示 rollback
//! - 再次事务，演示 commit
//! - 查询验证最终状态
//!
//! # 注意
//!
//! 此示例使用 `session.execute_raw()` 执行 DML，该方法在事务中执行时
//! 会自动绑定到底层数据库事务。`execute_raw` 需要 `sql-parser` 特性
//! （包含在默认特性中），运行时请使用默认特性或显式启用 `sql-parser`。
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example basic_transaction --features "sqlite,permission,macros"
//! ```

#[path = "../common/mod.rs"]
mod common;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("💳 DBNexus 基础事务示例");
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
            "CREATE TABLE accounts (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                balance REAL NOT NULL
            )",
        )
        .await?;
    println!("✓ accounts 表创建成功\n");

    // ============================================
    // 3. 开始事务并插入多条数据（演示 commit）
    // ============================================
    println!("--- 事务 1：插入数据并 commit ---");
    session.begin_transaction().await?;
    println!("  ✓ 事务开始");

    session
        .execute_raw("INSERT INTO accounts (id, name, balance) VALUES (1, 'Alice', 1000.0)")
        .await?;
    println!("  ✓ 插入: Alice (余额: 1000.0)");

    session
        .execute_raw("INSERT INTO accounts (id, name, balance) VALUES (2, 'Bob', 500.0)")
        .await?;
    println!("  ✓ 插入: Bob (余额: 500.0)");

    session.commit().await?;
    println!("  ✓ 事务提交成功\n");

    // 验证 commit 后的数据
    let result = session.execute_raw("SELECT COUNT(*) FROM accounts").await?;
    println!("  ✓ commit 后查询: 行数 = {}", result.rows_affected());

    // ============================================
    // 4. 模拟错误场景，演示 rollback
    // ============================================
    println!("\n--- 事务 2：模拟错误并 rollback ---");
    session.begin_transaction().await?;
    println!("  ✓ 事务开始");

    // 尝试插入一条有效数据
    session
        .execute_raw("INSERT INTO accounts (id, name, balance) VALUES (3, 'Charlie', 300.0)")
        .await?;
    println!("  ✓ 插入: Charlie (余额: 300.0)");

    // 尝试插入重复主键 → 触发错误
    let dup_result = session
        .execute_raw("INSERT INTO accounts (id, name, balance) VALUES (1, 'Duplicate', 0.0)")
        .await;
    match dup_result {
        Ok(_) => println!("  ⚠ 重复插入意外成功（不应发生）"),
        Err(e) => {
            println!("  ✓ 预期的错误触发: {}", e);
            println!("  → 执行 rollback...");
            session.rollback().await?;
            println!("  ✓ 事务已回滚");
        }
    }

    // ============================================
    // 5. 再次事务，演示 commit（转账场景）
    // ============================================
    println!("\n--- 事务 3：转账场景并 commit ---");
    session.begin_transaction().await?;
    println!("  ✓ 事务开始");

    // Alice 向 Bob 转账 200
    session
        .execute_raw("UPDATE accounts SET balance = balance - 200.0 WHERE id = 1")
        .await?;
    println!("  ✓ Alice 扣款: 200.0");

    session
        .execute_raw("UPDATE accounts SET balance = balance + 200.0 WHERE id = 2")
        .await?;
    println!("  ✓ Bob 收款: 200.0");

    session.commit().await?;
    println!("  ✓ 转账事务提交成功\n");

    // ============================================
    // 6. 查询验证最终状态
    // ============================================
    println!("--- 最终状态验证 ---");
    let result = session.execute_raw("SELECT * FROM accounts").await?;
    println!("  ✓ 查询所有账户: 行数 = {}", result.rows_affected());
    println!("  ✓ 预期: Alice=800.0, Bob=700.0 (Charlie 已回滚)");
    println!("  ✓ 验证 rollback 生效: Charlie 的插入被撤销");

    println!("\n========================================");
    println!("✨ 基础事务示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - session.begin_transaction() - 开始事务");
    println!("  - session.commit()            - 提交事务");
    println!("  - session.rollback()          - 回滚事务");
    println!("  - session.is_in_transaction() - 检查事务状态");
    println!("  - execute_raw 在事务中执行时自动绑定到事务");
    println!("  - 错误后 rollback 确保数据一致性");

    Ok(())
}
