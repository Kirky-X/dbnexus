// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 分片路由性能基准测试（T092）
//!
//! 衡量 `ShardRouter` 的核心路由开销：
//! - 分片路由计算（`shard_id_for_key` 纯哈希计算）
//! - 跨分片查询拒绝（`enforce_shard_binding` 冲突检测路径）
//!
//! 运行: cargo bench --bench sharding_bench --features "sharding"

#![cfg(feature = "sharding")]

use criterion::{Criterion, criterion_group, criterion_main};
use dbnexus::ShardRouter;
use std::hint::black_box;

// ============================================================================
// 基准测试
// ============================================================================

/// 分片路由计算：测量 `shard_id_for_key` 的吞吐量
///
/// 使用 8 个分片的 hash 策略，对 100 个不同 key 计算分片 ID。
/// 纯哈希计算，无 I/O 开销。
fn bench_shard_id_for_key(c: &mut Criterion) {
    let router = ShardRouter::with_strategy("hash", 8);

    c.bench_function("shard_id_for_key", |b| {
        b.iter(|| {
            for i in 0..100 {
                let key = format!("user_{i}");
                let _ = black_box(router.shard_id_for_key(&key));
            }
        })
    });
}

/// 跨分片查询拒绝：测量 `enforce_shard_binding` 在冲突路径上的开销
///
/// 预计算一个 key 的 shard_id，然后用不同的 key 触发冲突，
/// 测量错误构造 + 返回的开销。
fn bench_enforce_shard_binding_conflict(c: &mut Criterion) {
    let router = ShardRouter::with_strategy("hash", 8);

    // 找到一个会冲突的 key 对：expected_shard_id 来自 key_a，但请求 key_b
    // 这里直接用 key_a 的 shard_id 作为 expected，然后用 key_b 触发冲突
    let key_a = "user_42";
    let key_b = "user_99";
    let shard_a = router.shard_id_for_key(key_a);
    let shard_b = router.shard_id_for_key(key_b);

    // 确保两个 key 确实映射到不同分片（否则切换更大的 key 空间）
    assert_ne!(
        shard_a, shard_b,
        "test keys must map to different shards for conflict benchmark"
    );

    c.bench_function("enforce_shard_binding_conflict", |b| {
        b.iter(|| {
            // 每次都触发冲突（返回 Err）
            for _ in 0..100 {
                let result = router.enforce_shard_binding(shard_a, key_b);
                let _ = black_box(result);
            }
        })
    });
}

criterion_group!(benches, bench_shard_id_for_key, bench_enforce_shard_binding_conflict);
criterion_main!(benches);
