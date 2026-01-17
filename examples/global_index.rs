// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 全局索引示例
//!
//! 展示如何使用 dbnexus 的全局索引功能：
//! - 创建全局索引管理器
//! - 添加索引条目
//! - 查询全局索引
//! - 处理同步事件
//! - 使用倒排索引优化删除
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example global_index --features "sqlite,global-index"
//! ```

use dbnexus::global_index::{GlobalIndex, IndexEntry, SyncEvent};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌐 DBNexus 全局索引示例\n");
    println!("========================================");

    // 1. 创建全局索引管理器
    println!("\n1️⃣ 创建全局索引管理器");
    println!("------------------------------------------");
    let global_index = GlobalIndex::new("sqlite:./global_index_example.db").await?;
    println!("✓ 全局索引管理器创建成功");

    // 2. 添加索引条目
    println!("\n2️⃣ 添加索引条目");
    println!("------------------------------------------");

    // 添加用户订单索引
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
        IndexEntry {
            table_name: "orders".to_string(),
            record_id: "order_4".to_string(),
            shard_id: 1,
            index_key: "user_id".to_string(),
            index_value: "user_123".to_string(),
        },
        IndexEntry {
            table_name: "orders".to_string(),
            record_id: "order_5".to_string(),
            shard_id: 2,
            index_key: "user_id".to_string(),
            index_value: "user_789".to_string(),
        },
    ];

    for entry in &entries {
        global_index.register_entry(entry.clone()).await?;
        println!(
            "  ✓ 添加索引: {} (user_id={}, shard={})",
            entry.record_id, entry.index_value, entry.shard_id
        );
    }

    // 3. 查询全局索引
    println!("\n3️⃣ 查询全局索引");
    println!("------------------------------------------");

    // 查询用户 user_123 的所有订单
    println!("  查询 user_123 的订单:");
    let user_123_orders = global_index.query_by_index("orders", "user_id", "user_123").await?;

    println!("  📊 找到 {} 个订单:", user_123_orders.len());
    for order in &user_123_orders {
        println!("    - {} (分片 {})", order.record_id, order.shard_id);
    }

    // 查询用户 user_456 的所有订单
    println!("\n  查询 user_456 的订单:");
    let user_456_orders = global_index.query_by_index("orders", "user_id", "user_456").await?;

    println!("  📊 找到 {} 个订单:", user_456_orders.len());
    for order in &user_456_orders {
        println!("    - {} (分片 {})", order.record_id, order.shard_id);
    }

    // 查询用户 user_789 的所有订单
    println!("\n  查询 user_789 的订单:");
    let user_789_orders = global_index.query_by_index("orders", "user_id", "user_789").await?;

    println!("  📊 找到 {} 个订单:", user_789_orders.len());
    for order in &user_789_orders {
        println!("    - {} (分片 {})", order.record_id, order.shard_id);
    }

    // 4. 查询不存在的用户
    println!("\n  查询不存在的用户 user_999:");
    let user_999_orders = global_index.query_by_index("orders", "user_id", "user_999").await?;

    println!("  📊 找到 {} 个订单", user_999_orders.len());

    // 5. 处理同步事件
    println!("\n4️⃣ 处理同步事件");
    println!("------------------------------------------");

    // 模拟插入事件
    println!("  处理插入事件:");
    let insert_event = SyncEvent::Insert {
        table_name: "orders".to_string(),
        record_id: "order_6".to_string(),
        shard_id: 2,
        index_key: "user_id".to_string(),
        index_value: "user_123".to_string(),
    };

    global_index.process_sync_event(insert_event).await?;
    println!("    ✓ 插入 order_6 (user_123)");

    // 验证插入
    let user_123_orders = global_index.query_by_index("orders", "user_id", "user_123").await?;
    println!("    📊 user_123 现在有 {} 个订单", user_123_orders.len());

    // 模拟更新事件
    println!("\n  处理更新事件:");
    let update_event = SyncEvent::Update {
        table_name: "orders".to_string(),
        record_id: "order_3".to_string(),
        shard_id: 1,
        old_index_key: "user_id".to_string(),
        old_index_value: "user_456".to_string(),
        new_index_key: "user_id".to_string(),
        new_index_value: "user_123".to_string(),
    };

    global_index.process_sync_event(update_event).await?;
    println!("    ✓ 更新 order_3 (user_456 -> user_123)");

    // 验证更新
    let user_456_orders = global_index.query_by_index("orders", "user_id", "user_456").await?;
    println!("    📊 user_456 现在有 {} 个订单", user_456_orders.len());

    let user_123_orders = global_index.query_by_index("orders", "user_id", "user_123").await?;
    println!("    📊 user_123 现在有 {} 个订单", user_123_orders.len());

    // 模拟删除事件
    println!("\n  处理删除事件:");
    let delete_event = SyncEvent::Delete {
        table_name: "orders".to_string(),
        record_id: "order_1".to_string(),
        shard_id: 0,
        index_key: "user_id".to_string(),
        index_value: "user_123".to_string(),
    };

    global_index.process_sync_event(delete_event).await?;
    println!("    ✓ 删除 order_1 (user_123)");

    // 验证删除
    let user_123_orders = global_index.query_by_index("orders", "user_id", "user_123").await?;
    println!("    📊 user_123 现在有 {} 个订单", user_123_orders.len());

    // 6. 批量添加索引
    println!("\n5️⃣ 批量添加索引");
    println!("------------------------------------------");

    let new_orders = vec![
        ("order_7", "user_456", 0),
        ("order_8", "user_456", 1),
        ("order_9", "user_789", 2),
        ("order_10", "user_123", 0),
    ];

    for (order_id, user_id, shard_id) in new_orders {
        let entry = IndexEntry {
            table_name: "orders".to_string(),
            record_id: order_id.to_string(),
            shard_id,
            index_key: "user_id".to_string(),
            index_value: user_id.to_string(),
        };
        global_index.register_entry(entry).await?;
        println!("  ✓ 批量添加: {} (user={}, shard={})", order_id, user_id, shard_id);
    }

    // 7. 查询所有用户的订单
    println!("\n6️⃣ 查询所有用户的订单");
    println!("------------------------------------------");

    let users = vec!["user_123", "user_456", "user_789"];
    for user_id in users {
        let orders = global_index.query_by_index("orders", "user_id", user_id).await?;
        println!("  📊 {}: {} 个订单", user_id, orders.len());
    }

    // 8. 演示跨分片查询
    println!("\n7️⃣ 演示跨分片查询");
    println!("------------------------------------------");

    println!("  💡 全局索引的优势:");
    println!("     1. 不需要逐个查询分片");
    println!("     2. 快速定位目标分片");
    println!("     3. 减少网络开销");
    println!("     4. 提高查询性能");

    let user_123_orders = global_index.query_by_index("orders", "user_id", "user_123").await?;

    println!("\n  📊 user_123 的订单分布:");
    let mut shard_count = std::collections::HashMap::new();
    for order in &user_123_orders {
        *shard_count.entry(order.shard_id).or_insert(0) += 1;
    }

    for (shard_id, count) in shard_count {
        println!("    - 分片 {}: {} 个订单", shard_id, count);
    }

    // 9. 演示索引缓存
    println!("\n8️⃣ 演示索引缓存");
    println!("------------------------------------------");

    println!("  💡 索引缓存机制:");
    println!("     1. 使用内存缓存加速查询");
    println!("     2. 使用倒排索引优化删除");
    println!("     3. 定期同步到数据库");

    // 10. 演示同步状态
    println!("\n9️⃣ 演示同步状态");
    println!("------------------------------------------");

    println!("  💡 同步状态类型:");
    println!("     - pending: 待同步");
    println!("     - synced: 已同步");
    println!("     - failed: 同步失败");

    // 模拟同步失败
    let failed_event = SyncEvent::Insert {
        table_name: "orders".to_string(),
        record_id: "order_11".to_string(),
        shard_id: 0,
        index_key: "user_id".to_string(),
        index_value: "user_999".to_string(),
    };

    global_index.process_sync_event(failed_event).await?;
    println!("  ✓ 模拟同步失败事件");

    // 11. 演示变更捕获（CDC）
    println!("\n🔟 演示变更捕获（CDC）");
    println!("------------------------------------------");

    println!("  💡 变更数据捕获（CDC）:");
    println!("     1. 监听分片数据变更");
    println!("     2. 自动同步到全局索引");
    println!("     3. 支持批量处理");
    println!("     4. 支持重试机制");

    println!("\n  📝 CDC 配置:");
    println!("     - batch_size: 1000");
    println!("     - poll_interval_ms: 1000");
    println!("     - max_retries: 3");
    println!("     - retry_interval_ms: 5000");

    // 12. 清理测试数据
    println!("\n1️⃣1️⃣ 清理测试数据");
    println!("------------------------------------------");

    // 删除所有索引条目
    for entry in &entries {
        let delete_event = SyncEvent::Delete {
            table_name: entry.table_name.clone(),
            record_id: entry.record_id.clone(),
            shard_id: entry.shard_id,
            index_key: entry.index_key.clone(),
            index_value: entry.index_value.clone(),
        };
        global_index.process_sync_event(delete_event).await?;
    }

    // 删除新添加的条目
    let new_order_ids = vec![
        ("order_6", "user_123", 2),
        ("order_7", "user_456", 0),
        ("order_8", "user_456", 1),
        ("order_9", "user_789", 2),
        ("order_10", "user_123", 0),
        ("order_11", "user_999", 0),
    ];
    for (order_id, user_id, shard_id) in new_order_ids {
        let delete_event = SyncEvent::Delete {
            table_name: "orders".to_string(),
            record_id: order_id.to_string(),
            shard_id,
            index_key: "user_id".to_string(),
            index_value: user_id.to_string(),
        };
        global_index.process_sync_event(delete_event).await?;
    }

    println!("  ✓ 所有测试数据已清理");

    // 验证清理
    let user_123_orders = global_index.query_by_index("orders", "user_id", "user_123").await?;
    println!("  📊 清理后 user_123 有 {} 个订单", user_123_orders.len());

    println!("\n========================================");
    println!("✨ 全局索引示例运行完成！");

    println!("\n💡 提示:");
    println!("  - 全局索引适用于跨分片唯一约束");
    println!("  - 可以用于快速定位数据所在分片");
    println!("  - 需要维护索引的一致性");
    println!("  - 考虑使用 CDC 自动同步变更");
    println!("  - 定期清理和重建索引");

    Ok(())
}
