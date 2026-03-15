// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 全局索引集成测试
//!
//! 测试 GlobalIndex 的数据库操作，包括：
//! - 全局索引创建测试
//! - 全局索引查询测试
//! - 全局索引更新测试
//! - 全局索引删除测试
//! - 跨分片索引同步测试
//! - 索引一致性检查测试

use dbnexus::global_index::{GlobalIndex, IndexEntry};

/// 获取测试数据库 URL
fn get_database_url() -> String {
    if let Ok(url) = std::env::var("DATABASE_URL") {
        return url;
    }

    // 默认使用 SQLite 内存数据库
    "sqlite::memory:".to_string()
}

// ============================================================================
// 全局索引创建测试
// ============================================================================

/// TEST-GIDX-INT-001: GlobalIndex 创建测试
#[tokio::test]
async fn test_global_index_creation() {
    let url = get_database_url();
    let result = GlobalIndex::new(&url).await;

    assert!(result.is_ok(), "GlobalIndex should be created successfully");

    let global_index = result.unwrap();
    // 验证全局索引管理器已创建
    let _ = global_index;
}

/// TEST-GIDX-INT-002: GlobalIndex 无效连接字符串测试
#[tokio::test]
async fn test_global_index_invalid_connection() {
    // 使用无效的连接字符串
    let result = GlobalIndex::new("invalid://connection:string").await;

    // 应该返回错误
    assert!(result.is_err(), "Invalid connection string should return error");
}

/// TEST-GIDX-INT-003: GlobalIndex SQLite 内存数据库创建测试
#[tokio::test]
async fn test_global_index_sqlite_memory() {
    let result = GlobalIndex::new("sqlite::memory:").await;

    assert!(result.is_ok(), "SQLite memory database should work");

    let global_index = result.unwrap();
    let _ = global_index;
}

// ============================================================================
// 全局索引查询测试
// ============================================================================

/// TEST-GIDX-INT-004: 空索引查询测试
#[tokio::test]
async fn test_query_empty_index() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    // 查询不存在的索引
    let result = global_index.query_by_index("orders", "user_id", "nonexistent").await;

    assert!(result.is_ok(), "Query should succeed even on empty index");

    let entries = result.unwrap();
    assert!(entries.is_empty(), "Empty index should return empty result");
}

/// TEST-GIDX-INT-005: 单条索引查询测试
#[tokio::test]
async fn test_query_single_index_entry() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    // 准备索引条目
    let entry = IndexEntry {
        table_name: "orders".to_string(),
        record_id: "order_1".to_string(),
        shard_id: 0,
        index_key: "user_id".to_string(),
        index_value: "user_123".to_string(),
    };

    // 同步索引
    let sync_result = global_index.batch_sync(vec![entry]).await;
    assert!(sync_result.is_ok(), "Sync should succeed");

    // 查询索引
    let result = global_index.query_by_index("orders", "user_id", "user_123").await;

    assert!(result.is_ok(), "Query should succeed");

    let entries = result.unwrap();
    assert_eq!(entries.len(), 1, "Should find exactly one entry");
    assert_eq!(entries[0].record_id, "order_1");
    assert_eq!(entries[0].shard_id, 0);
}

/// TEST-GIDX-INT-006: 多条索引查询测试
#[tokio::test]
async fn test_query_multiple_index_entries() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    // 准备多条索引条目，相同的索引值
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
            shard_id: 1,
            index_key: "user_id".to_string(),
            index_value: "user_123".to_string(),
        },
        IndexEntry {
            table_name: "orders".to_string(),
            record_id: "order_3".to_string(),
            shard_id: 2,
            index_key: "user_id".to_string(),
            index_value: "user_123".to_string(),
        },
    ];

    // 同步索引
    let sync_result = global_index.batch_sync(entries).await;
    assert!(sync_result.is_ok());

    // 查询索引
    let result = global_index.query_by_index("orders", "user_id", "user_123").await;

    assert!(result.is_ok());

    let entries = result.unwrap();
    assert_eq!(entries.len(), 3, "Should find all three entries");
}

/// TEST-GIDX-INT-007: 不同表名查询测试
#[tokio::test]
async fn test_query_different_tables() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    // 准备不同表的索引条目
    let entries = vec![
        IndexEntry {
            table_name: "orders".to_string(),
            record_id: "order_1".to_string(),
            shard_id: 0,
            index_key: "user_id".to_string(),
            index_value: "user_123".to_string(),
        },
        IndexEntry {
            table_name: "products".to_string(),
            record_id: "product_1".to_string(),
            shard_id: 0,
            index_key: "user_id".to_string(),
            index_value: "user_123".to_string(),
        },
    ];

    global_index.batch_sync(entries).await.unwrap();

    // 查询 orders 表
    let orders = global_index.query_by_index("orders", "user_id", "user_123").await.unwrap();
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].table_name, "orders");

    // 查询 products 表
    let products = global_index.query_by_index("products", "user_id", "user_123").await.unwrap();
    assert_eq!(products.len(), 1);
    assert_eq!(products[0].table_name, "products");
}

/// TEST-GIDX-INT-008: 不同索引键查询测试
#[tokio::test]
async fn test_query_different_index_keys() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    let entries = vec![
        IndexEntry {
            table_name: "users".to_string(),
            record_id: "user_1".to_string(),
            shard_id: 0,
            index_key: "email".to_string(),
            index_value: "test@example.com".to_string(),
        },
        IndexEntry {
            table_name: "users".to_string(),
            record_id: "user_1".to_string(),
            shard_id: 0,
            index_key: "phone".to_string(),
            index_value: "1234567890".to_string(),
        },
    ];

    global_index.batch_sync(entries).await.unwrap();

    // 按 email 查询
    let by_email = global_index.query_by_index("users", "email", "test@example.com").await.unwrap();
    assert_eq!(by_email.len(), 1);

    // 按 phone 查询
    let by_phone = global_index.query_by_index("users", "phone", "1234567890").await.unwrap();
    assert_eq!(by_phone.len(), 1);
}

// ============================================================================
// 全局索引更新测试
// ============================================================================

/// TEST-GIDX-INT-009: 索引更新测试
#[tokio::test]
async fn test_index_update() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    // 初始条目
    let entry = IndexEntry {
        table_name: "orders".to_string(),
        record_id: "order_1".to_string(),
        shard_id: 0,
        index_key: "user_id".to_string(),
        index_value: "user_123".to_string(),
    };

    global_index.batch_sync(vec![entry]).await.unwrap();

    // 更新条目（相同的 index_value，不同的 shard_id）
    let updated_entry = IndexEntry {
        table_name: "orders".to_string(),
        record_id: "order_1".to_string(),
        shard_id: 5,
        index_key: "user_id".to_string(),
        index_value: "user_123".to_string(),
    };

    global_index.batch_sync(vec![updated_entry]).await.unwrap();

    // 查询验证更新
    let result = global_index.query_by_index("orders", "user_id", "user_123").await.unwrap();

    // 由于使用 upsert，应该只有一条记录
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].shard_id, 5, "Shard ID should be updated");
}

/// TEST-GIDX-INT-010: 批量更新测试
#[tokio::test]
async fn test_batch_update() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    // 初始批量条目
    let entries: Vec<IndexEntry> = (0..10)
        .map(|i| IndexEntry {
            table_name: "orders".to_string(),
            record_id: format!("order_{}", i),
            shard_id: 0,
            index_key: "user_id".to_string(),
            index_value: format!("user_{}", i),
        })
        .collect();

    let result = global_index.batch_sync(entries).await.unwrap();

    assert!(result.success, "Batch sync should succeed");
    assert_eq!(result.synced_count, 10);
    assert_eq!(result.failed_count, 0);
}

/// TEST-GIDX-INT-011: 更新后查询一致性测试
#[tokio::test]
async fn test_update_query_consistency() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    // 创建初始条目
    let entry = IndexEntry {
        table_name: "products".to_string(),
        record_id: "prod_1".to_string(),
        shard_id: 0,
        index_key: "sku".to_string(),
        index_value: "SKU-001".to_string(),
    };

    global_index.batch_sync(vec![entry.clone()]).await.unwrap();

    // 第一次查询
    let result1 = global_index.query_by_index("products", "sku", "SKU-001").await.unwrap();
    assert_eq!(result1.len(), 1);

    // 更新
    let updated_entry = IndexEntry {
        table_name: "products".to_string(),
        record_id: "prod_1".to_string(),
        shard_id: 10,
        index_key: "sku".to_string(),
        index_value: "SKU-001".to_string(),
    };

    global_index.batch_sync(vec![updated_entry]).await.unwrap();

    // 第二次查询
    let result2 = global_index.query_by_index("products", "sku", "SKU-001").await.unwrap();
    assert_eq!(result2.len(), 1);
    assert_eq!(result2[0].shard_id, 10);
}

// ============================================================================
// 全局索引删除测试
// ============================================================================

/// TEST-GIDX-INT-012: 索引不存在查询测试（模拟删除场景）
#[tokio::test]
async fn test_index_nonexistent_query() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    // 创建索引
    let entry = IndexEntry {
        table_name: "orders".to_string(),
        record_id: "order_1".to_string(),
        shard_id: 0,
        index_key: "user_id".to_string(),
        index_value: "user_123".to_string(),
    };

    global_index.batch_sync(vec![entry]).await.unwrap();

    // 查询不存在的索引值
    let result = global_index.query_by_index("orders", "user_id", "nonexistent").await.unwrap();

    assert!(result.is_empty(), "Should return empty for nonexistent index value");
}

/// TEST-GIDX-INT-013: 索引不存在表名查询测试
#[tokio::test]
async fn test_index_nonexistent_table_query() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    // 查询不存在的表
    let result = global_index.query_by_index("nonexistent_table", "some_key", "some_value").await;

    assert!(result.is_ok(), "Query should succeed for nonexistent table");
    assert!(result.unwrap().is_empty(), "Should return empty for nonexistent table");
}

// ============================================================================
// 跨分片索引同步测试
// ============================================================================

/// TEST-GIDX-INT-014: 多分片索引同步测试
#[tokio::test]
async fn test_multi_shard_sync() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    // 准备来自不同分片的索引条目
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
            shard_id: 1,
            index_key: "user_id".to_string(),
            index_value: "user_123".to_string(),
        },
        IndexEntry {
            table_name: "orders".to_string(),
            record_id: "order_3".to_string(),
            shard_id: 2,
            index_key: "user_id".to_string(),
            index_value: "user_123".to_string(),
        },
        IndexEntry {
            table_name: "orders".to_string(),
            record_id: "order_4".to_string(),
            shard_id: 3,
            index_key: "user_id".to_string(),
            index_value: "user_456".to_string(),
        },
    ];

    let result = global_index.batch_sync(entries).await.unwrap();

    assert!(result.success);
    assert_eq!(result.synced_count, 4);

    // 验证跨分片查询
    let user_123_orders = global_index.query_by_index("orders", "user_id", "user_123").await.unwrap();
    assert_eq!(user_123_orders.len(), 3);

    let user_456_orders = global_index.query_by_index("orders", "user_id", "user_456").await.unwrap();
    assert_eq!(user_456_orders.len(), 1);
}

/// TEST-GIDX-INT-015: 分片 ID 范围测试
#[tokio::test]
async fn test_shard_id_range() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    // 测试不同的分片 ID
    let entries: Vec<IndexEntry> = (0..100)
        .map(|i| IndexEntry {
            table_name: "logs".to_string(),
            record_id: format!("log_{}", i),
            shard_id: i,
            index_key: "timestamp".to_string(),
            index_value: format!("2024-01-{}", i % 30 + 1),
        })
        .collect();

    let result = global_index.batch_sync(entries).await.unwrap();

    assert!(result.success);
    assert_eq!(result.synced_count, 100);
}

/// TEST-GIDX-INT-016: 大量分片同步性能测试
#[tokio::test]
async fn test_large_shard_sync() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    // 准备大量索引条目
    let entries: Vec<IndexEntry> = (0..1000)
        .map(|i| IndexEntry {
            table_name: "events".to_string(),
            record_id: format!("event_{}", i),
            shard_id: i % 10,
            index_key: "event_type".to_string(),
            index_value: format!("type_{}", i % 5),
        })
        .collect();

    let result = global_index.batch_sync(entries).await.unwrap();

    assert!(result.success);
    assert_eq!(result.synced_count, 1000);
    assert_eq!(result.failed_count, 0);
}

// ============================================================================
// 索引一致性检查测试
// ============================================================================

/// TEST-GIDX-INT-017: 索引一致性基本测试
#[tokio::test]
async fn test_index_consistency_basic() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    // 创建索引
    let entry = IndexEntry {
        table_name: "orders".to_string(),
        record_id: "order_1".to_string(),
        shard_id: 0,
        index_key: "user_id".to_string(),
        index_value: "user_123".to_string(),
    };

    global_index.batch_sync(vec![entry.clone()]).await.unwrap();

    // 多次查询应该返回一致结果
    for _ in 0..5 {
        let result = global_index.query_by_index("orders", "user_id", "user_123").await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].record_id, "order_1");
    }
}

/// TEST-GIDX-INT-018: 并发查询一致性测试
#[tokio::test]
async fn test_concurrent_query_consistency() {
    use std::sync::Arc;

    let global_index = Arc::new(GlobalIndex::new("sqlite::memory:").await.unwrap());

    // 创建索引
    let entries: Vec<IndexEntry> = (0..10)
        .map(|i| IndexEntry {
            table_name: "orders".to_string(),
            record_id: format!("order_{}", i),
            shard_id: i % 3,
            index_key: "user_id".to_string(),
            index_value: format!("user_{}", i % 5),
        })
        .collect();

    global_index.batch_sync(entries).await.unwrap();

    // 并发查询
    let mut handles = vec![];

    for i in 0..10 {
        let gi = Arc::clone(&global_index);
        let handle = tokio::spawn(async move {
            let user_id = format!("user_{}", i % 5);
            gi.query_by_index("orders", "user_id", &user_id).await
        });
        handles.push(handle);
    }

    // 等待所有查询完成
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok(), "Concurrent query should succeed");
    }
}

/// TEST-GIDX-INT-019: 并发同步一致性测试
#[tokio::test]
async fn test_concurrent_sync_consistency() {
    use std::sync::Arc;

    let global_index = Arc::new(GlobalIndex::new("sqlite::memory:").await.unwrap());

    let mut handles = vec![];

    // 并发同步不同的索引条目
    for i in 0..5 {
        let gi = Arc::clone(&global_index);
        let handle = tokio::spawn(async move {
            let entry = IndexEntry {
                table_name: "orders".to_string(),
                record_id: format!("order_{}", i),
                shard_id: i,
                index_key: "batch".to_string(),
                index_value: format!("batch_{}", i),
            };
            gi.batch_sync(vec![entry]).await
        });
        handles.push(handle);
    }

    // 等待所有同步完成
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok(), "Concurrent sync should succeed");
    }

    // 验证所有条目都已同步
    for i in 0..5 {
        let result = global_index
            .query_by_index("orders", "batch", &format!("batch_{}", i))
            .await
            .unwrap();
        assert_eq!(result.len(), 1, "Each batch should have exactly one entry");
    }
}

/// TEST-GIDX-INT-020: 索引数据完整性测试
#[tokio::test]
async fn test_index_data_integrity() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    // 创建包含特殊字符的索引
    let entry = IndexEntry {
        table_name: "special_table".to_string(),
        record_id: "rec-with-special_chars.123".to_string(),
        shard_id: 42,
        index_key: "complex_key".to_string(),
        index_value: "value with spaces and 'quotes'".to_string(),
    };

    global_index.batch_sync(vec![entry.clone()]).await.unwrap();

    // 查询并验证数据完整性
    let result = global_index
        .query_by_index("special_table", "complex_key", "value with spaces and 'quotes'")
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].record_id, "rec-with-special_chars.123");
    assert_eq!(result[0].shard_id, 42);
    assert_eq!(result[0].index_value, "value with spaces and 'quotes'");
}

// ============================================================================
// 边界条件测试
// ============================================================================

/// TEST-GIDX-INT-021: 空条目批量同步测试
#[tokio::test]
async fn test_empty_batch_sync() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    // 同步空列表
    let result = global_index.batch_sync(vec![]).await.unwrap();

    assert!(result.success);
    assert_eq!(result.synced_count, 0);
    assert_eq!(result.failed_count, 0);
}

/// TEST-GIDX-INT-022: 大批量同步测试
#[tokio::test]
async fn test_large_batch_sync() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    // 准备大量索引条目
    let entries: Vec<IndexEntry> = (0..5000)
        .map(|i| IndexEntry {
            table_name: "large_table".to_string(),
            record_id: format!("rec_{}", i),
            shard_id: i % 100,
            index_key: "idx".to_string(),
            index_value: format!("val_{}", i),
        })
        .collect();

    let result = global_index.batch_sync(entries).await.unwrap();

    assert!(result.success);
    assert_eq!(result.synced_count, 5000);
}

/// TEST-GIDX-INT-023: Unicode 索引值测试
#[tokio::test]
async fn test_unicode_index_values() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    let entry = IndexEntry {
        table_name: "用户表".to_string(),
        record_id: "记录_001".to_string(),
        shard_id: 0,
        index_key: "邮箱".to_string(),
        index_value: "用户@例子.测试".to_string(),
    };

    global_index.batch_sync(vec![entry.clone()]).await.unwrap();

    let result = global_index.query_by_index("用户表", "邮箱", "用户@例子.测试").await.unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].record_id, "记录_001");
}

/// TEST-GIDX-INT-024: JSON 索引值测试
#[tokio::test]
async fn test_json_index_value() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    let json_value = r#"{"type":"order","status":"pending","amount":100.50}"#;

    let entry = IndexEntry {
        table_name: "configs".to_string(),
        record_id: "config_1".to_string(),
        shard_id: 0,
        index_key: "settings".to_string(),
        index_value: json_value.to_string(),
    };

    global_index.batch_sync(vec![entry]).await.unwrap();

    let result = global_index.query_by_index("configs", "settings", json_value).await.unwrap();

    assert_eq!(result.len(), 1);
}

/// TEST-GIDX-INT-025: 最大分片 ID 测试
#[tokio::test]
async fn test_max_shard_id() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    let entry = IndexEntry {
        table_name: "max_shard_test".to_string(),
        record_id: "rec_1".to_string(),
        shard_id: u32::MAX,
        index_key: "test".to_string(),
        index_value: "max_shard".to_string(),
    };

    global_index.batch_sync(vec![entry.clone()]).await.unwrap();

    let result = global_index.query_by_index("max_shard_test", "test", "max_shard").await.unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].shard_id, u32::MAX);
}

// ============================================================================
// 错误处理测试
// ============================================================================

/// TEST-GIDX-INT-026: 查询参数验证测试
#[tokio::test]
async fn test_query_parameter_validation() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    // 空参数查询
    let result = global_index.query_by_index("", "", "").await;
    assert!(result.is_ok(), "Query with empty parameters should succeed");

    let entries = result.unwrap();
    assert!(entries.is_empty(), "Empty parameters should return empty result");
}

/// TEST-GIDX-INT-027: 重复同步测试
#[tokio::test]
async fn test_duplicate_sync() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    let entry = IndexEntry {
        table_name: "duplicates".to_string(),
        record_id: "rec_1".to_string(),
        shard_id: 0,
        index_key: "key".to_string(),
        index_value: "value".to_string(),
    };

    // 多次同步相同的条目
    for _ in 0..5 {
        let result = global_index.batch_sync(vec![entry.clone()]).await.unwrap();
        assert!(result.success);
    }

    // 应该只有一条记录（upsert 行为）
    let result = global_index.query_by_index("duplicates", "key", "value").await.unwrap();
    assert_eq!(result.len(), 1, "Duplicate syncs should result in single entry");
}

/// TEST-GIDX-INT-028: 混合操作测试
#[tokio::test]
async fn test_mixed_operations() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    // 创建多个条目
    let entries: Vec<IndexEntry> = (0..5)
        .map(|i| IndexEntry {
            table_name: "mixed".to_string(),
            record_id: format!("rec_{}", i),
            shard_id: i,
            index_key: "key".to_string(),
            index_value: format!("value_{}", i),
        })
        .collect();

    global_index.batch_sync(entries).await.unwrap();

    // 查询
    for i in 0..5 {
        let result = global_index
            .query_by_index("mixed", "key", &format!("value_{}", i))
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
    }

    // 更新部分条目
    let updated = IndexEntry {
        table_name: "mixed".to_string(),
        record_id: "rec_0".to_string(),
        shard_id: 100,
        index_key: "key".to_string(),
        index_value: "value_0".to_string(),
    };

    global_index.batch_sync(vec![updated]).await.unwrap();

    // 验证更新
    let result = global_index.query_by_index("mixed", "key", "value_0").await.unwrap();
    assert_eq!(result[0].shard_id, 100);
}

// ============================================================================
// 性能相关测试
// ============================================================================

/// TEST-GIDX-INT-029: 查询性能测试
#[tokio::test]
async fn test_query_performance() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    // 创建大量索引
    let entries: Vec<IndexEntry> = (0..10000)
        .map(|i| IndexEntry {
            table_name: "perf_test".to_string(),
            record_id: format!("rec_{}", i),
            shard_id: i % 100,
            index_key: "category".to_string(),
            index_value: format!("cat_{}", i % 100),
        })
        .collect();

    global_index.batch_sync(entries).await.unwrap();

    // 执行查询
    let start = std::time::Instant::now();

    for _ in 0..100 {
        let _ = global_index.query_by_index("perf_test", "category", "cat_50").await.unwrap();
    }

    let duration = start.elapsed();

    // 查询应该在合理时间内完成
    assert!(duration.as_millis() < 5000, "Queries should complete in reasonable time");
}

/// TEST-GIDX-INT-030: 连接复用测试
#[tokio::test]
async fn test_connection_reuse() {
    let global_index = GlobalIndex::new("sqlite::memory:").await.unwrap();

    // 多次操作应该复用同一个连接
    for i in 0..100 {
        let entry = IndexEntry {
            table_name: "reuse_test".to_string(),
            record_id: format!("rec_{}", i),
            shard_id: 0,
            index_key: "batch".to_string(),
            index_value: format!("batch_{}", i),
        };

        let result = global_index.batch_sync(vec![entry]).await.unwrap();
        assert!(result.success);
    }

    // 验证所有条目
    for i in 0..100 {
        let result = global_index
            .query_by_index("reuse_test", "batch", &format!("batch_{}", i))
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
    }
}
