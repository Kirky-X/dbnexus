// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 基础性能基准测试
//!
//! 运行: cargo bench --bench permission_bench --features "sqlite permission"

use criterion::{Criterion, criterion_group, criterion_main};
use dbnexus::tokio::runtime::Runtime;
use dbnexus::{DbConfig, DbPool, PoolConfig};
use std::hint::black_box;

fn bench_connection_pool_creation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("pool_creation_sqlite", |b| {
        b.iter(|| {
            let _pool = rt.block_on(async { DbPool::new("sqlite::memory:").await });
        })
    });
}

fn bench_connection_pool_with_config(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 10,
            min_connections: 1,
            ..Default::default()
        },
        ..Default::default()
    };

    c.bench_function("pool_creation_with_config", |b| {
        b.iter(|| {
            let _pool = rt.block_on(async { DbPool::with_config(config.clone()).await });
        })
    });
}

fn bench_pool_status(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let pool = DbPool::new("sqlite::memory:").await.unwrap();

        c.bench_function("pool_status", |b| {
            b.iter(|| {
                black_box(pool.status());
            })
        });
    });
}

fn bench_config_building(c: &mut Criterion) {
    c.bench_function("config_builder", |b| {
        b.iter(|| {
            let config = DbConfig {
                url: "postgresql://user:pass@localhost/db".to_string(),
                pool_config: dbnexus::foundation::PoolConfig {
                    max_connections: 20,
                    min_connections: 5,
                    idle_timeout: 300,
                    acquire_timeout: 5000,
                },
                ..Default::default()
            };
            black_box(config);
        })
    });
}

fn bench_pool_config(c: &mut Criterion) {
    c.bench_function("pool_config_default", |b| {
        b.iter(|| {
            let config = PoolConfig::default();
            black_box(config);
        })
    });
}

criterion_group!(
    benches,
    bench_connection_pool_creation,
    bench_connection_pool_with_config,
    bench_pool_status,
    bench_config_building,
    bench_pool_config
);
criterion_main!(benches);
