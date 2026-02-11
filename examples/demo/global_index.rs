// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 全局索引示例
//!
//! 展示如何使用 dbnexus 的全局索引功能：
//! - 创建全局索引管理器
//! - 批量同步索引条目
//! - 通过索引查询记录
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example global_index --features "sqlite,global-index,macros"
//! ```

use dbnexus::global_index::{GlobalIndex, IndexEntry};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌐 DBNexus 全局索引示例\n");
    println!("========================================");

    // 1. 创建全局索引管理器
    println!("\n1️⃣ 创建全局索引管理器");
    println!("------------------------------------------");
    let global_index = GlobalIndex::new("sqlite::memory:").await?;
    println!("✓ 全局索引管理器创建成功");

    // 2. 准备索引条目
    println!("\n2️⃣ 准备索引条目");
    println!("------------------------------------------");

    let entries = vec![
        IndexEntry {
            table_name: "orders".to_string(),
            record_id: "order_1".to_string(),
            shard_id: 0,
            index_key: "user_id".to_string(),
            index_value: "user_123".to_string(),
        },
        IndexEntry {
            table_name: "orders".to_string(),
            record_id: "order_2".to_string(),
            shard_id: 0,
            index_key: "user_id".to_string(),
            index_value: "user_123".to_string(),
        },
        IndexEntry {
            table_name: "orders".to_string(),
            record_id: "order_3".to_string(),
            shard_id: 1,
            index_key: "user_id".to_string(),
            index_value: "user_456".to_string(),
        },
    ];

    println!("✓ 准备了 {} 个索引条目", entries.len());

    // 3. 批量同步索引
    println!("\n3️⃣ 批量同步索引");
    println!("------------------------------------------");
    let sync_result = global_index.batch_sync(entries).await?;
    println!("  ✓ 成功同步: {} 条", sync_result.synced_count);
    if sync_result.failed_count > 0 {
        println!("  ✗ 同步失败: {} 条", sync_result.failed_count);
        for error in &sync_result.errors {
            println!("    - {}", error);
        }
    }

    // 4. 通过索引查询
    println!("\n4️⃣ 通过索引查询");
    println!("------------------------------------------");
    let results = global_index.query_by_index("orders", "user_id", "user_123").await?;
    println!("  ✓ 查询 user_123 的订单: 找到 {} 条", results.len());
    for entry in &results {
        println!("    - {} (shard={})", entry.record_id, entry.shard_id);
    }

    let results = global_index.query_by_index("orders", "user_id", "user_456").await?;
    println!("  ✓ 查询 user_456 的订单: 找到 {} 条", results.len());

    println!("\n========================================");
    println!("✅ 全局索引示例运行完成！");

    Ok(())
}
