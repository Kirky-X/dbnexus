// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 语句级 prepared statement LRU 缓存基准
//!
//! 钉住淘汰成本不随容量线性：满容量稳态下逐条插入新键（每插必淘汰），
//! 对比两个量级容量的单次插入耗时。O(1) 淘汰（侵入式双链表）下两组
//! 基本持平；若回归为满容量线性扫描淘汰，大容量组耗时随容量成比例劣化。
//!
//! 运行: cargo bench --bench prepare_cache_bench --features prepare-cache

#![cfg(feature = "prepare-cache")]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use dbnexus::PreparedStatementCache;
use std::hint::black_box;
use std::sync::Arc;

/// 满容量稳态插入淘汰：2×capacity 键轮转，稳态后每次插入均为新键
/// （未命中）且恰好淘汰一条 LRU 尾部条目
fn bench_full_capacity_insert_eviction(c: &mut Criterion) {
    let mut group = c.benchmark_group("prepare_cache_eviction");
    for &capacity in &[128usize, 8_192] {
        group.bench_with_input(
            BenchmarkId::from_parameter(capacity),
            &capacity,
            |b, &cap| {
                let cache: PreparedStatementCache<()> = PreparedStatementCache::new(cap);
                // 2×capacity 键轮转：首轮建立稳态（缓存恰含后半键集），后续
                // 每次插入都命中淘汰路径
                let keys: Vec<Arc<str>> = (0..2 * cap)
                    .map(|i| Arc::from(format!("SELECT /* stmt{i} */ 1").as_str()))
                    .collect();
                for key in &keys {
                    cache.insert(Arc::clone(key), ());
                }
                b.iter(|| {
                    for key in &keys {
                        cache.insert(Arc::clone(black_box(key)), ());
                    }
                });
            },
        );
    }
    group.finish();
}

/// 命中路径参照组：满容量缓存重复探测同键（前移链首 + 返回产物）
fn bench_hit_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("prepare_cache_hit");
    for &capacity in &[128usize, 8_192] {
        group.bench_with_input(
            BenchmarkId::from_parameter(capacity),
            &capacity,
            |b, &cap| {
                let cache: PreparedStatementCache<()> = PreparedStatementCache::new(cap);
                for i in 0..cap {
                    cache.insert(format!("SELECT /* stmt{i} */ 1"), ());
                }
                b.iter(|| {
                    for i in 0..cap {
                        let _ = black_box(cache.get(&format!("SELECT /* stmt{i} */ 1")));
                    }
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    prepare_cache_benches,
    bench_full_capacity_insert_eviction,
    bench_hit_lookup
);
criterion_main!(prepare_cache_benches);
