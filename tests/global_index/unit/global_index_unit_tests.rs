// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 全局索引单元测试
//!
//! 测试 IndexEntry、SyncEvent、SyncResult 等基础数据结构

use dbnexus::{IndexEntry, SYNC_STATUS_FAILED, SYNC_STATUS_PENDING, SYNC_STATUS_SYNCED, SyncEvent, SyncResult};

// ============================================================================
// IndexEntry 测试
// ============================================================================

/// TEST-GIDX-001: IndexEntry 创建测试
#[test]
fn test_index_entry_creation() {
    let entry = IndexEntry {
        table_name: "orders".to_string(),
        record_id: "order_123".to_string(),
        shard_id: 0,
        index_key: "user_id".to_string(),
        index_value: "user_456".to_string(),
    };

    assert_eq!(entry.table_name, "orders");
    assert_eq!(entry.record_id, "order_123");
    assert_eq!(entry.shard_id, 0);
    assert_eq!(entry.index_key, "user_id");
    assert_eq!(entry.index_value, "user_456");
}

/// TEST-GIDX-002: IndexEntry 克隆测试
#[test]
fn test_index_entry_clone() {
    let entry = IndexEntry {
        table_name: "products".to_string(),
        record_id: "prod_789".to_string(),
        shard_id: 5,
        index_key: "category".to_string(),
        index_value: "electronics".to_string(),
    };

    let cloned = entry.clone();

    assert_eq!(entry.table_name, cloned.table_name);
    assert_eq!(entry.record_id, cloned.record_id);
    assert_eq!(entry.shard_id, cloned.shard_id);
    assert_eq!(entry.index_key, cloned.index_key);
    assert_eq!(entry.index_value, cloned.index_value);
}

/// TEST-GIDX-003: IndexEntry Debug 格式化测试
#[test]
fn test_index_entry_debug() {
    let entry = IndexEntry {
        table_name: "users".to_string(),
        record_id: "user_1".to_string(),
        shard_id: 1,
        index_key: "email".to_string(),
        index_value: "test@example.com".to_string(),
    };

    let debug_str = format!("{:?}", entry);

    assert!(debug_str.contains("IndexEntry"));
    assert!(debug_str.contains("users"));
    assert!(debug_str.contains("user_1"));
    assert!(debug_str.contains("email"));
    assert!(debug_str.contains("test@example.com"));
}

/// TEST-GIDX-004: IndexEntry 边界值测试 - 空字符串
#[test]
fn test_index_entry_empty_strings() {
    let entry = IndexEntry {
        table_name: "".to_string(),
        record_id: "".to_string(),
        shard_id: 0,
        index_key: "".to_string(),
        index_value: "".to_string(),
    };

    assert_eq!(entry.table_name, "");
    assert_eq!(entry.record_id, "");
    assert_eq!(entry.index_key, "");
    assert_eq!(entry.index_value, "");
}

/// TEST-GIDX-005: IndexEntry 边界值测试 - 最大 shard_id
#[test]
fn test_index_entry_max_shard_id() {
    let entry = IndexEntry {
        table_name: "test_table".to_string(),
        record_id: "rec_1".to_string(),
        shard_id: u32::MAX,
        index_key: "key".to_string(),
        index_value: "value".to_string(),
    };

    assert_eq!(entry.shard_id, u32::MAX);
}

/// TEST-GIDX-006: IndexEntry 特殊字符测试
#[test]
fn test_index_entry_special_characters() {
    let entry = IndexEntry {
        table_name: "table-with_special.chars".to_string(),
        record_id: "rec:123:456".to_string(),
        shard_id: 0,
        index_key: "key:subkey".to_string(),
        index_value: "value with spaces and 'quotes'".to_string(),
    };

    assert!(entry.table_name.contains('-'));
    assert!(entry.table_name.contains('_'));
    assert!(entry.table_name.contains('.'));
    assert!(entry.index_value.contains(' '));
    assert!(entry.index_value.contains('\''));
}

// ============================================================================
// SyncEvent 测试
// ============================================================================

/// TEST-GIDX-007: SyncEvent Insert 变体测试
#[test]
fn test_sync_event_insert() {
    let entry = IndexEntry {
        table_name: "orders".to_string(),
        record_id: "order_1".to_string(),
        shard_id: 0,
        index_key: "user_id".to_string(),
        index_value: "user_123".to_string(),
    };

    let event = SyncEvent::Insert(entry.clone());

    if let SyncEvent::Insert(e) = event {
        assert_eq!(e.table_name, entry.table_name);
        assert_eq!(e.record_id, entry.record_id);
    } else {
        panic!("Expected SyncEvent::Insert");
    }
}

/// TEST-GIDX-008: SyncEvent Update 变体测试
#[test]
fn test_sync_event_update() {
    let entry = IndexEntry {
        table_name: "products".to_string(),
        record_id: "prod_1".to_string(),
        shard_id: 2,
        index_key: "sku".to_string(),
        index_value: "SKU-001".to_string(),
    };

    let event = SyncEvent::Update(entry.clone());

    if let SyncEvent::Update(e) = event {
        assert_eq!(e.table_name, entry.table_name);
        assert_eq!(e.shard_id, 2);
    } else {
        panic!("Expected SyncEvent::Update");
    }
}

/// TEST-GIDX-009: SyncEvent Delete 变体测试
#[test]
fn test_sync_event_delete() {
    let entry = IndexEntry {
        table_name: "users".to_string(),
        record_id: "user_1".to_string(),
        shard_id: 0,
        index_key: "id".to_string(),
        index_value: "12345".to_string(),
    };

    let event = SyncEvent::Delete(entry.clone());

    if let SyncEvent::Delete(e) = event {
        assert_eq!(e.record_id, entry.record_id);
    } else {
        panic!("Expected SyncEvent::Delete");
    }
}

/// TEST-GIDX-010: SyncEvent Debug 格式化测试
#[test]
fn test_sync_event_debug() {
    let entry = IndexEntry {
        table_name: "test".to_string(),
        record_id: "rec".to_string(),
        shard_id: 0,
        index_key: "key".to_string(),
        index_value: "val".to_string(),
    };

    let insert_event = SyncEvent::Insert(entry.clone());
    let update_event = SyncEvent::Update(entry.clone());
    let delete_event = SyncEvent::Delete(entry);

    let insert_debug = format!("{:?}", insert_event);
    let update_debug = format!("{:?}", update_event);
    let delete_debug = format!("{:?}", delete_event);

    assert!(insert_debug.contains("Insert"));
    assert!(update_debug.contains("Update"));
    assert!(delete_debug.contains("Delete"));
}

/// TEST-GIDX-011: SyncEvent Clone 测试
#[test]
fn test_sync_event_clone() {
    let entry = IndexEntry {
        table_name: "orders".to_string(),
        record_id: "order_1".to_string(),
        shard_id: 1,
        index_key: "user_id".to_string(),
        index_value: "user_123".to_string(),
    };

    let event = SyncEvent::Insert(entry);
    let cloned = event.clone();

    if let (SyncEvent::Insert(e1), SyncEvent::Insert(e2)) = (event, cloned) {
        assert_eq!(e1.table_name, e2.table_name);
        assert_eq!(e1.record_id, e2.record_id);
    } else {
        panic!("Clone failed");
    }
}

// ============================================================================
// SyncResult 测试
// ============================================================================

/// TEST-GIDX-012: SyncResult 成功状态测试
#[test]
fn test_sync_result_success() {
    let result = SyncResult {
        success: true,
        synced_count: 10,
        failed_count: 0,
        errors: vec![],
    };

    assert!(result.success);
    assert_eq!(result.synced_count, 10);
    assert_eq!(result.failed_count, 0);
    assert!(result.errors.is_empty());
}

/// TEST-GIDX-013: SyncResult 部分失败状态测试
#[test]
fn test_sync_result_partial_failure() {
    let result = SyncResult {
        success: false,
        synced_count: 8,
        failed_count: 2,
        errors: vec![
            "Failed to sync entry: connection error".to_string(),
            "Failed to sync entry: timeout".to_string(),
        ],
    };

    assert!(!result.success);
    assert_eq!(result.synced_count, 8);
    assert_eq!(result.failed_count, 2);
    assert_eq!(result.errors.len(), 2);
}

/// TEST-GIDX-014: SyncResult Debug 格式化测试
#[test]
fn test_sync_result_debug() {
    let result = SyncResult {
        success: true,
        synced_count: 5,
        failed_count: 0,
        errors: vec![],
    };

    let debug_str = format!("{:?}", result);

    assert!(debug_str.contains("SyncResult"));
    assert!(debug_str.contains("success"));
    assert!(debug_str.contains("synced_count"));
}

/// TEST-GIDX-015: SyncResult 完全失败状态测试
#[test]
fn test_sync_result_complete_failure() {
    let result = SyncResult {
        success: false,
        synced_count: 0,
        failed_count: 5,
        errors: vec![
            "Error 1".to_string(),
            "Error 2".to_string(),
            "Error 3".to_string(),
            "Error 4".to_string(),
            "Error 5".to_string(),
        ],
    };

    assert!(!result.success);
    assert_eq!(result.synced_count, 0);
    assert_eq!(result.failed_count, 5);
    assert_eq!(result.errors.len(), 5);
}

// ============================================================================
// 同步状态常量测试
// ============================================================================

/// TEST-GIDX-016: 同步状态常量值测试
#[test]
fn test_sync_status_constants() {
    assert_eq!(SYNC_STATUS_PENDING, "pending");
    assert_eq!(SYNC_STATUS_SYNCED, "synced");
    assert_eq!(SYNC_STATUS_FAILED, "failed");
}

/// TEST-GIDX-017: 同步状态常量唯一性测试
#[test]
fn test_sync_status_constants_uniqueness() {
    let statuses = [SYNC_STATUS_PENDING, SYNC_STATUS_SYNCED, SYNC_STATUS_FAILED];
    let unique_count = statuses.iter().collect::<std::collections::HashSet<_>>().len();

    assert_eq!(unique_count, 3, "All sync status constants should be unique");
}

// ============================================================================
// IndexEntry 集合操作测试
// ============================================================================

/// TEST-GIDX-018: IndexEntry 向量操作测试
#[test]
fn test_index_entry_vector_operations() {
    let entries = [
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
            index_value: "user_456".to_string(),
        },
    ];

    assert_eq!(entries.len(), 3);

    // 按索引值分组
    let grouped: std::collections::HashMap<String, Vec<&IndexEntry>> =
        entries.iter().fold(std::collections::HashMap::new(), |mut acc, entry| {
            acc.entry(entry.index_value.clone()).or_default().push(entry);
            acc
        });

    assert_eq!(grouped.get("user_123").unwrap().len(), 2);
    assert_eq!(grouped.get("user_456").unwrap().len(), 1);
}

/// TEST-GIDX-019: IndexEntry 过滤测试
#[test]
fn test_index_entry_filtering() {
    let entries = [
        IndexEntry {
            table_name: "orders".to_string(),
            record_id: "order_1".to_string(),
            shard_id: 0,
            index_key: "user_id".to_string(),
            index_value: "user_123".to_string(),
        },
        IndexEntry {
            table_name: "products".to_string(),
            record_id: "prod_1".to_string(),
            shard_id: 0,
            index_key: "category".to_string(),
            index_value: "electronics".to_string(),
        },
        IndexEntry {
            table_name: "orders".to_string(),
            record_id: "order_2".to_string(),
            shard_id: 1,
            index_key: "user_id".to_string(),
            index_value: "user_456".to_string(),
        },
    ];

    let orders: Vec<&IndexEntry> = entries.iter().filter(|e| e.table_name == "orders").collect();

    assert_eq!(orders.len(), 2);

    let shard_0: Vec<&IndexEntry> = entries.iter().filter(|e| e.shard_id == 0).collect();

    assert_eq!(shard_0.len(), 2);
}

/// TEST-GIDX-020: IndexEntry 排序测试
#[test]
fn test_index_entry_sorting() {
    let mut entries = [
        IndexEntry {
            table_name: "orders".to_string(),
            record_id: "order_3".to_string(),
            shard_id: 2,
            index_key: "user_id".to_string(),
            index_value: "user_789".to_string(),
        },
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
            index_value: "user_456".to_string(),
        },
    ];

    // 按 shard_id 排序
    entries.sort_by_key(|e| e.shard_id);

    assert_eq!(entries[0].shard_id, 0);
    assert_eq!(entries[1].shard_id, 1);
    assert_eq!(entries[2].shard_id, 2);
}

// ============================================================================
// SyncEvent 向量操作测试
// ============================================================================

/// TEST-GIDX-021: SyncEvent 向量操作测试
#[test]
fn test_sync_event_vector_operations() {
    let events = [
        SyncEvent::Insert(IndexEntry {
            table_name: "orders".to_string(),
            record_id: "order_1".to_string(),
            shard_id: 0,
            index_key: "user_id".to_string(),
            index_value: "user_123".to_string(),
        }),
        SyncEvent::Update(IndexEntry {
            table_name: "orders".to_string(),
            record_id: "order_1".to_string(),
            shard_id: 0,
            index_key: "user_id".to_string(),
            index_value: "user_123_updated".to_string(),
        }),
        SyncEvent::Delete(IndexEntry {
            table_name: "orders".to_string(),
            record_id: "order_2".to_string(),
            shard_id: 1,
            index_key: "user_id".to_string(),
            index_value: "user_456".to_string(),
        }),
    ];

    assert_eq!(events.len(), 3);

    // 统计各类型事件数量
    let insert_count = events.iter().filter(|e| matches!(e, SyncEvent::Insert(_))).count();
    let update_count = events.iter().filter(|e| matches!(e, SyncEvent::Update(_))).count();
    let delete_count = events.iter().filter(|e| matches!(e, SyncEvent::Delete(_))).count();

    assert_eq!(insert_count, 1);
    assert_eq!(update_count, 1);
    assert_eq!(delete_count, 1);
}

// ============================================================================
// 边界条件测试
// ============================================================================

/// TEST-GIDX-022: IndexEntry 长字符串测试
#[test]
fn test_index_entry_long_strings() {
    let long_string = "a".repeat(10000);

    let entry = IndexEntry {
        table_name: long_string.clone(),
        record_id: long_string.clone(),
        shard_id: 0,
        index_key: long_string.clone(),
        index_value: long_string.clone(),
    };

    assert_eq!(entry.table_name.len(), 10000);
    assert_eq!(entry.record_id.len(), 10000);
    assert_eq!(entry.index_key.len(), 10000);
    assert_eq!(entry.index_value.len(), 10000);
}

/// TEST-GIDX-023: IndexEntry Unicode 字符测试
#[test]
fn test_index_entry_unicode_characters() {
    let entry = IndexEntry {
        table_name: "用户表".to_string(),
        record_id: "记录_123".to_string(),
        shard_id: 0,
        index_key: "邮箱".to_string(),
        index_value: "用户@例子.测试".to_string(),
    };

    assert_eq!(entry.table_name, "用户表");
    assert_eq!(entry.record_id, "记录_123");
    assert_eq!(entry.index_key, "邮箱");
    assert_eq!(entry.index_value, "用户@例子.测试");
}

/// TEST-GIDX-024: IndexEntry JSON 字符串测试
#[test]
fn test_index_entry_json_value() {
    let json_value = r#"{"name":"test","value":123,"nested":{"key":"value"}}"#;

    let entry = IndexEntry {
        table_name: "configs".to_string(),
        record_id: "config_1".to_string(),
        shard_id: 0,
        index_key: "settings".to_string(),
        index_value: json_value.to_string(),
    };

    assert!(entry.index_value.starts_with('{'));
    assert!(entry.index_value.ends_with('}'));
    assert!(entry.index_value.contains("nested"));
}

/// TEST-GIDX-025: SyncResult 大量错误测试
#[test]
fn test_sync_result_many_errors() {
    let errors: Vec<String> = (0..1000).map(|i| format!("Error {}", i)).collect();

    let result = SyncResult {
        success: false,
        synced_count: 0,
        failed_count: 1000,
        errors: errors.clone(),
    };

    assert_eq!(result.errors.len(), 1000);
    assert_eq!(result.errors[0], "Error 0");
    assert_eq!(result.errors[999], "Error 999");
}
