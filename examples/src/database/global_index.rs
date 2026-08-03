// Copyright (c) 2026 Kirky.X
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
    DbPool, GlobalIndex, IndexEntry, SyncEvent, SYNC_STATUS_FAILED, SYNC_STATUS_PENDING, SYNC_STATUS_SYNCED,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🌐 DBNexus 全局索引示例");
    println!("========================================\n");

    // ============================================
    // 1. 创建全局索引管理器
    // ============================================
    // GlobalIndex::new 接受 Arc<DbPool>，通过连接池统一管理连接生命周期
    let pool = DbPool::new("sqlite::memory:").await?;
    let global_index = GlobalIndex::new(Arc::new(pool)).await?;
    println!("✓ 全局索引管理器创建成功（global_index 表已自动创建）");

    // ============================================
    // 2. 准备索引条目（模拟跨分片数据）
    // ============================================
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

    println!("\n📋 准备同步 {} 条索引条目:", entries.len());
    for entry in &entries {
        println!(
            "  - {} (record: {}, shard: {}, {}: {})",
            entry.table_name, entry.record_id, entry.shard_id, entry.index_key, entry.index_value
        );
    }

    // ============================================
    // 3. 批量同步索引条目
    // ============================================
    println!("\n同步索引到全局索引表...");
    let sync_result = global_index.batch_sync(entries).await?;

    println!("\n📊 同步结果 (SyncResult):");
    println!("  - 成功: {}", sync_result.success);
    println!("  - 同步数量: {}", sync_result.synced_count);
    println!("  - 失败数量: {}", sync_result.failed_count);
    if !sync_result.errors.is_empty() {
        println!("  - 错误信息:");
        for err in &sync_result.errors {
            println!("    • {}", err);
        }
    }

    // ============================================
    // 4. 通过索引查询跨分片数据
    // ============================================
    println!("\n─── 跨分片索引查询 ───\n");

    // 查询 user_100 的所有订单（分布在分片 0 和 1）
    println!("查询 user_id = user_100 的订单:");
    let results = global_index.query_by_index("orders", "user_id", "user_100").await?;
    println!("  ✓ 找到 {} 条记录", results.len());
    for entry in &results {
        println!("    → record: {}, shard: {}", entry.record_id, entry.shard_id);
    }

    // 查询 user_200 的订单（分布在分片 0 和 2）
    println!("\n查询 user_id = user_200 的订单:");
    let results = global_index.query_by_index("orders", "user_id", "user_200").await?;
    println!("  ✓ 找到 {} 条记录", results.len());
    for entry in &results {
        println!("    → record: {}, shard: {}", entry.record_id, entry.shard_id);
    }

    // 查询不存在的用户
    println!("\n查询 user_id = user_999（不存在）的订单:");
    let results = global_index.query_by_index("orders", "user_id", "user_999").await?;
    println!("  ✓ 找到 {} 条记录", results.len());

    // ============================================
    // 5. 展示同步状态常量和 SyncEvent
    // ============================================
    println!("\n─── 同步状态常量 ───\n");
    println!("  SYNC_STATUS_PENDING = \"{}\"", SYNC_STATUS_PENDING);
    println!("  SYNC_STATUS_SYNCED  = \"{}\"", SYNC_STATUS_SYNCED);
    println!("  SYNC_STATUS_FAILED  = \"{}\"", SYNC_STATUS_FAILED);

    println!("\n─── SyncEvent 事件类型 ───\n");
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

    println!("  SyncEvent::Insert(entry) - 数据插入事件");
    println!("  SyncEvent::Update(entry) - 数据更新事件");
    println!("  SyncEvent::Delete(entry) - 数据删除事件");
    println!("\n  事件示例:");
    match &insert_event {
        SyncEvent::Insert(e) => println!("    Insert: {}@{} (shard {})", e.table_name, e.record_id, e.shard_id),
        _ => unreachable!(),
    }
    match &update_event {
        SyncEvent::Update(e) => println!("    Update: {}@{} (shard {})", e.table_name, e.record_id, e.shard_id),
        _ => unreachable!(),
    }
    match &delete_event {
        SyncEvent::Delete(e) => println!("    Delete: {}@{} (shard {})", e.table_name, e.record_id, e.shard_id),
        _ => unreachable!(),
    }

    // ============================================
    // 6. 再次同步（验证 upsert 语义）
    // ============================================
    println!("\n─── 验证 upsert 语义（重复同步相同条目） ───\n");
    let duplicate_entries = vec![IndexEntry {
        table_name: "orders".to_string(),
        record_id: "order_001".to_string(),
        shard_id: 0,
        index_key: "user_id".to_string(),
        index_value: "user_100_updated".to_string(), // 更新值
    }];
    let result = global_index.batch_sync(duplicate_entries).await?;
    println!(
        "  ✓ 重复同步完成: synced={}, failed={}",
        result.synced_count, result.failed_count
    );

    // 验证更新后的值
    let results = global_index
        .query_by_index("orders", "user_id", "user_100_updated")
        .await?;
    println!("  ✓ 查询更新后的值: 找到 {} 条记录", results.len());
    for entry in &results {
        println!(
            "    → record: {}, shard: {}, value: {}",
            entry.record_id, entry.shard_id, entry.index_value
        );
    }

    println!("\n========================================");
    println!("✨ 全局索引示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - GlobalIndex::new(Arc<DbPool>) 创建管理器并自动建表");
    println!("  - IndexEntry 结构: table_name, record_id, shard_id, index_key, index_value");
    println!("  - batch_sync(entries) 批量同步（支持 upsert，分批插入避免参数限制）");
    println!("  - query_by_index(table, key, value) 跨分片查询");
    println!("  - SyncEvent 枚举: Insert/Update/Delete，用于 CDC 风格变更捕获");
    println!("  - 同步状态: SYNC_STATUS_PENDING/SYNCED/FAILED");
    println!("  - batch_sync 使用 OnConflict 实现 upsert 语义");

    Ok(())
}
