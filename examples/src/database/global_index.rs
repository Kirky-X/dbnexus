// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 全局索引示例
//!
//! 演示如何使用 [`GlobalIndex`] 管理跨分片的全局索引：
//! - 创建 `GlobalIndex`（自动建表）
//! - 批量同步 `IndexEntry` 到全局索引表
//! - 通过索引键查询跨分片数据
//! - 展示 `SyncResult` 和 `SyncEvent` 类型
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example global_index --features "sqlite,global-index"
//! ```

use dbnexus::{
    DbPool, GlobalIndex, IndexEntry, SYNC_STATUS_FAILED, SYNC_STATUS_PENDING, SYNC_STATUS_SYNCED,
    SyncEvent,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🌐 DBNexus global index example");
    println!("========================================\n");

    // 创建全局索引管理器
    // GlobalIndex::new 接受 Arc<DbPool>，通过连接池统一管理连接生命周期
    let pool = DbPool::new("sqlite::memory:").await?;
    let global_index = GlobalIndex::new(Arc::new(pool)).await?;
    println!("✓ GlobalIndex manager created (global_index table created automatically)");

    // 准备索引条目（模拟跨分片数据）
    // 模拟 3 个分片的订单数据
    let entries = vec![
        IndexEntry {
            table_name: "orders".to_string(),
            record_id: "order_001".to_string(),
            shard_id: 0,
            index_key: "user_id".to_string(),
            index_value: "user_100".to_string(),
        },
        IndexEntry {
            table_name: "orders".to_string(),
            record_id: "order_002".to_string(),
            shard_id: 1,
            index_key: "user_id".to_string(),
            index_value: "user_100".to_string(), // 同一用户在不同分片的订单
        },
        IndexEntry {
            table_name: "orders".to_string(),
            record_id: "order_003".to_string(),
            shard_id: 2,
            index_key: "user_id".to_string(),
            index_value: "user_200".to_string(),
        },
        IndexEntry {
            table_name: "orders".to_string(),
            record_id: "order_004".to_string(),
            shard_id: 0,
            index_key: "user_id".to_string(),
            index_value: "user_200".to_string(), // user_200 也在分片0有订单
        },
        IndexEntry {
            table_name: "orders".to_string(),
            record_id: "order_005".to_string(),
            shard_id: 3,
            index_key: "user_id".to_string(),
            index_value: "user_300".to_string(),
        },
    ];

    println!("\n📋 Preparing to sync {} index entries:", entries.len());
    for entry in &entries {
        println!(
            "  - {} (record: {}, shard: {}, {}: {})",
            entry.table_name, entry.record_id, entry.shard_id, entry.index_key, entry.index_value
        );
    }

    // 批量同步索引条目
    println!("\nSyncing the index into the global index table...");
    let sync_result = global_index.batch_sync(entries).await?;

    println!("\n📊 Sync result (SyncResult):");
    println!("  - succeeded: {}", sync_result.success);
    println!("  - synced count: {}", sync_result.synced_count);
    println!("  - failed count: {}", sync_result.failed_count);
    if !sync_result.errors.is_empty() {
        println!("  - error message:");
        for err in &sync_result.errors {
            println!("    • {}", err);
        }
    }

    // 通过索引查询跨分片数据
    println!("\n─── Cross-shard index query ───\n");

    // 查询 user_100 的所有订单（分布在分片 0 和 1）
    println!("Query orders where user_id = user_100:");
    let results = global_index
        .query_by_index("orders", "user_id", "user_100")
        .await?;
    println!("  ✓ found {} records", results.len());
    for entry in &results {
        println!(
            "    → record: {}, shard: {}",
            entry.record_id, entry.shard_id
        );
    }

    // 查询 user_200 的订单（分布在分片 0 和 2）
    println!("\nQuery orders where user_id = user_200:");
    let results = global_index
        .query_by_index("orders", "user_id", "user_200")
        .await?;
    println!("  ✓ found {} records", results.len());
    for entry in &results {
        println!(
            "    → record: {}, shard: {}",
            entry.record_id, entry.shard_id
        );
    }

    // 查询不存在的用户
    println!("\nQuery orders where user_id = user_999 (does not exist):");
    let results = global_index
        .query_by_index("orders", "user_id", "user_999")
        .await?;
    println!("  ✓ found {} records", results.len());

    // 展示同步状态常量和 SyncEvent
    println!("\n─── Sync status constants ───\n");
    println!("  SYNC_STATUS_PENDING = \"{}\"", SYNC_STATUS_PENDING);
    println!("  SYNC_STATUS_SYNCED  = \"{}\"", SYNC_STATUS_SYNCED);
    println!("  SYNC_STATUS_FAILED  = \"{}\"", SYNC_STATUS_FAILED);

    println!("\n─── SyncEvent event types ───\n");
    // SyncEvent 用于表示数据变更事件（binlog/CDC 风格）
    let sample_entry = IndexEntry {
        table_name: "orders".to_string(),
        record_id: "order_006".to_string(),
        shard_id: 1,
        index_key: "user_id".to_string(),
        index_value: "user_400".to_string(),
    };

    let insert_event = SyncEvent::Insert(sample_entry.clone());
    let update_event = SyncEvent::Update(sample_entry.clone());
    let delete_event = SyncEvent::Delete(sample_entry);

    println!("  SyncEvent::Insert(entry) - data insertion event");
    println!("  SyncEvent::Update(entry) - data update event");
    println!("  SyncEvent::Delete(entry) - data deletion event");
    println!("\n  event examples:");
    match &insert_event {
        SyncEvent::Insert(e) => println!(
            "    Insert: {}@{} (shard {})",
            e.table_name, e.record_id, e.shard_id
        ),
        _ => unreachable!(),
    }
    match &update_event {
        SyncEvent::Update(e) => println!(
            "    Update: {}@{} (shard {})",
            e.table_name, e.record_id, e.shard_id
        ),
        _ => unreachable!(),
    }
    match &delete_event {
        SyncEvent::Delete(e) => println!(
            "    Delete: {}@{} (shard {})",
            e.table_name, e.record_id, e.shard_id
        ),
        _ => unreachable!(),
    }

    // 再次同步（验证 upsert 语义）
    println!("\n─── Verify upsert semantics (sync the same entries twice) ───\n");
    let duplicate_entries = vec![IndexEntry {
        table_name: "orders".to_string(),
        record_id: "order_001".to_string(),
        shard_id: 0,
        index_key: "user_id".to_string(),
        index_value: "user_100_updated".to_string(),
    }];
    let result = global_index.batch_sync(duplicate_entries).await?;
    println!(
        "  ✓ duplicate sync completed: synced={}, failed={}",
        result.synced_count, result.failed_count
    );

    let results = global_index
        .query_by_index("orders", "user_id", "user_100_updated")
        .await?;
    println!(
        "  ✓ queried the updated value: found {} records",
        results.len()
    );
    for entry in &results {
        println!(
            "    → record: {}, shard: {}, value: {}",
            entry.record_id, entry.shard_id, entry.index_value
        );
    }

    println!("\n========================================");
    println!("✨ Global index example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - GlobalIndex::new(Arc<DbPool>) creates the manager and the table automatically");
    println!("  - IndexEntry struct: table_name, record_id, shard_id, index_key, index_value");
    println!("  - batch_sync(entries) batch sync (upsert; batched inserts avoid parameter limits)");
    println!("  - query_by_index(table, key, value) cross-shard query");
    println!("  - SyncEvent enum: Insert/Update/Delete, for CDC-style change capture");
    println!("  - sync status: SYNC_STATUS_PENDING/SYNCED/FAILED");
    println!("  - batch_sync implements upsert semantics via OnConflict");

    Ok(())
}
