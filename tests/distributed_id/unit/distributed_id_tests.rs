// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 分布式 ID 生成器单元测试

use std::collections::HashSet;
use std::sync::Arc;
use std::thread;

use dbnexus::{DistributedIdGenerator, IdComponents, SnowflakeIdGenerator};

// ============================================================================
// SnowflakeIdGenerator 基础测试
// ============================================================================

#[test]
fn test_new_generator_valid_machine_id() {
    let id_gen = SnowflakeIdGenerator::new(0, 1_700_000_000_000);
    assert!(id_gen.is_ok());
}

#[test]
fn test_new_generator_max_machine_id() {
    let id_gen = SnowflakeIdGenerator::new(1023, 1_700_000_000_000);
    assert!(id_gen.is_ok());
}

#[test]
fn test_new_generator_invalid_machine_id() {
    let result = SnowflakeIdGenerator::new(1024, 1_700_000_000_000);
    assert!(result.is_err());
    let err = result.err().unwrap();
    assert!(err.contains("1024"));
}

#[test]
fn test_generated_id_is_nonzero() {
    let id_gen = SnowflakeIdGenerator::new(1, 1_700_000_000_000).unwrap();
    let id = id_gen.next_id().expect("ID generation should succeed");
    assert_ne!(id, 0, "Generated ID should not be 0");
}

#[test]
fn test_ids_are_monotonically_increasing() {
    let id_gen = SnowflakeIdGenerator::new(5, 1_700_000_000_000).unwrap();
    let mut prev_id = 0u64;
    for _ in 0..10_000 {
        let id = id_gen.next_id().expect("ID generation should succeed");
        assert!(id > prev_id, "IDs must be monotonically increasing: {id} <= {prev_id}");
        prev_id = id;
    }
}

#[test]
fn test_ids_are_unique() {
    let id_gen = SnowflakeIdGenerator::new(10, 1_700_000_000_000).unwrap();
    let mut ids = HashSet::new();
    for _ in 0..10_000 {
        let id = id_gen.next_id().expect("ID generation should succeed");
        assert!(ids.insert(id), "Duplicate ID generated: {id}");
    }
}

// ============================================================================
// parse_id 测试
// ============================================================================

#[test]
fn test_parse_id_extracts_machine_id() {
    let machine_id = 42;
    let id_gen = SnowflakeIdGenerator::new(machine_id, 1_700_000_000_000).unwrap();
    let id = id_gen.next_id().expect("ID generation should succeed");
    let components = id_gen.parse_id(id);
    assert_eq!(components.machine_id, machine_id);
}

#[test]
fn test_parse_id_timestamp_is_positive() {
    let id_gen = SnowflakeIdGenerator::new(1, 1_700_000_000_000).unwrap();
    let id = id_gen.next_id().expect("ID generation should succeed");
    let components = id_gen.parse_id(id);
    assert!(components.timestamp_ms > 0, "Timestamp should be positive");
}

#[test]
fn test_parse_id_sequence_starts_at_zero() {
    let id_gen = SnowflakeIdGenerator::new(1, 1_700_000_000_000).unwrap();
    let id = id_gen.next_id().expect("ID generation should succeed");
    let components = id_gen.parse_id(id);
    // 第一个 ID 的 sequence 应为 0（新毫秒重置）
    assert_eq!(components.sequence, 0);
}

#[test]
fn test_parse_id_roundtrip() {
    let machine_id = 100;
    let id_gen = SnowflakeIdGenerator::new(machine_id, 1_700_000_000_000).unwrap();
    let id = id_gen.next_id().expect("ID generation should succeed");
    let components = id_gen.parse_id(id);

    // 重新组装 ID 应等于原始 ID
    let recomposed =
        (components.timestamp_ms << 22) | ((components.machine_id as u64) << 12) | (components.sequence as u64);
    assert_eq!(recomposed, id);
}

// ============================================================================
// 并发安全测试
// ============================================================================

#[test]
fn test_concurrent_id_generation_no_duplicates() {
    let id_gen = Arc::new(SnowflakeIdGenerator::new(7, 1_700_000_000_000).unwrap());
    let num_threads = 10;
    let ids_per_thread = 1000;
    let mut handles = Vec::new();

    for _ in 0..num_threads {
        let id_gen_clone = Arc::clone(&id_gen);
        handles.push(thread::spawn(move || {
            let mut ids = Vec::with_capacity(ids_per_thread);
            for _ in 0..ids_per_thread {
                ids.push(id_gen_clone.next_id().expect("ID generation should succeed"));
            }
            ids
        }));
    }

    let mut all_ids = HashSet::new();
    for handle in handles {
        let ids = handle.join().unwrap();
        for id in ids {
            assert!(all_ids.insert(id), "Duplicate ID in concurrent test: {id}");
        }
    }

    assert_eq!(all_ids.len(), num_threads * ids_per_thread);
}

#[test]
fn test_concurrent_ids_monotonically_increasing_per_thread() {
    let id_gen = Arc::new(SnowflakeIdGenerator::new(3, 1_700_000_000_000).unwrap());
    let num_threads = 4;
    let ids_per_thread = 500;
    let mut handles = Vec::new();

    for _ in 0..num_threads {
        let id_gen_clone = Arc::clone(&id_gen);
        handles.push(thread::spawn(move || {
            let mut ids = Vec::with_capacity(ids_per_thread);
            for _ in 0..ids_per_thread {
                ids.push(id_gen_clone.next_id().expect("ID generation should succeed"));
            }
            ids
        }));
    }

    for handle in handles {
        let ids = handle.join().unwrap();
        for window in ids.windows(2) {
            assert!(
                window[1] > window[0],
                "IDs within a thread should be monotonically increasing"
            );
        }
    }
}

// ============================================================================
// IdComponents 结构测试
// ============================================================================

#[test]
fn test_id_components_equality() {
    let a = IdComponents {
        timestamp_ms: 1000,
        machine_id: 5,
        sequence: 0,
    };
    let b = IdComponents {
        timestamp_ms: 1000,
        machine_id: 5,
        sequence: 0,
    };
    assert_eq!(a, b);
}

#[test]
fn test_id_components_inequality() {
    let a = IdComponents {
        timestamp_ms: 1000,
        machine_id: 5,
        sequence: 0,
    };
    let b = IdComponents {
        timestamp_ms: 1001,
        machine_id: 5,
        sequence: 0,
    };
    assert_ne!(a, b);
}
