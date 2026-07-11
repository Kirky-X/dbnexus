// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 权限引擎性能基准测试（T091）
//!
//! 衡量 `PermissionCache` 的核心操作开销：
//! - 角色策略加载（`insert`）
//! - 缓存命中（`get` 命中未过期条目）
//! - 缓存未命中（`get` 查找不存在的 key）
//!
//! 运行: cargo bench --bench permission_engine_bench --features "permission"

#![cfg(feature = "permission")]

use criterion::{Criterion, criterion_group, criterion_main};
use dbnexus::access::permission::{PermissionAction, PermissionCache, RolePolicy, TablePermission};
use std::hint::black_box;
use std::time::Duration;

// ============================================================================
// 辅助函数
// ============================================================================

/// 构造一个包含单表权限的 `RolePolicy`
fn sample_policy(table: &str, action: PermissionAction) -> RolePolicy {
    RolePolicy {
        tables: vec![TablePermission {
            name: table.to_string(),
            operations: vec![action],
        }],
    }
}

/// 预填充缓存：插入 `n` 个角色策略（key = "role_{i}"）
fn populate_cache(cache: &PermissionCache, n: usize) {
    for i in 0..n {
        let action = match i % 4 {
            0 => PermissionAction::Select,
            1 => PermissionAction::Insert,
            2 => PermissionAction::Update,
            _ => PermissionAction::Delete,
        };
        cache.insert(&format!("role_{i}"), sample_policy(&format!("t{i}"), action));
    }
}

// ============================================================================
// 基准测试
// ============================================================================

/// 角色策略加载：测量 `PermissionCache::insert` 的吞吐量
fn bench_role_policy_load(c: &mut Criterion) {
    c.bench_function("permission_cache_insert", |b| {
        b.iter_with_setup(
            || PermissionCache::new().with_ttl(Duration::from_secs(300)),
            |cache| {
                for i in 0..100 {
                    cache.insert(
                        &format!("role_{i}"),
                        sample_policy(&format!("t{i}"), PermissionAction::Select),
                    );
                }
                black_box(&cache);
                cache
            },
        )
    });
}

/// 缓存命中：预填充 100 个条目后，反复 `get` 已存在的 key
fn bench_permission_cache_hit(c: &mut Criterion) {
    let cache = PermissionCache::new().with_ttl(Duration::from_secs(300));
    populate_cache(&cache, 100);

    c.bench_function("permission_cache_hit", |b| {
        b.iter(|| {
            // 交替访问不同 key，避免编译器过度优化
            for i in 0..100 {
                let key = format!("role_{i}");
                let _ = black_box(cache.get(&key));
            }
        })
    });
}

/// 缓存未命中：预填充 100 个条目后，`get` 不存在的 key
fn bench_permission_cache_miss(c: &mut Criterion) {
    let cache = PermissionCache::new().with_ttl(Duration::from_secs(300));
    populate_cache(&cache, 100);

    c.bench_function("permission_cache_miss", |b| {
        b.iter(|| {
            for i in 0..100 {
                // key 不存在于缓存中
                let key = format!("missing_{i}");
                let _ = black_box(cache.get(&key));
            }
        })
    });
}

criterion_group!(
    benches,
    bench_role_policy_load,
    bench_permission_cache_hit,
    bench_permission_cache_miss
);
criterion_main!(benches);
