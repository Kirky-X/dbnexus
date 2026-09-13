// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 端到端性能基准
//!
//! 衡量真实数据库路径（sqlite 临时文件库）上的端到端开销：
//! - 池获取：`DbPool::get_session`（权限校验 + 池信号量 + 连接建立）
//! - 简单查询：`DbPool::query_rows`（含出口脱敏/RLS 旁路检查的行查询管道）
//! - 批量写：`Session::execute_raw` 循环 INSERT（逐条 e2e 写路径）
//!
//! 运行: cargo bench --bench e2e_bench --features "sqlite,runtime-tokio-rustls,sql-parser"
//! 基线数字记录于 docs/PERFORMANCE.md（本机一次性采样，CI 阈值待稳定后启用）。

#![cfg(all(
    feature = "sqlite",
    feature = "runtime-tokio-rustls",
    feature = "sql-parser"
))]

use criterion::{Criterion, criterion_group, criterion_main};
use dbnexus::DbPool;
use std::hint::black_box;

/// 建立临时文件库池并准备 `t_e2e` 表与初始行。
///
/// sqlite::memory: 的每个池化连接是独立内存库，跨连接建表不可见，
/// 故与 `query_rows` 测试口径一致使用 `sqlite:<path>?mode=rwc` 临时文件库。
async fn setup_pool(batch_rows: usize) -> (DbPool, std::path::PathBuf) {
    let db_path = std::env::temp_dir().join(format!(
        "dbnexus_t414_e2e_{}_{}.db",
        std::process::id(),
        batch_rows
    ));
    let _ = std::fs::remove_file(&db_path);
    let url = format!("sqlite:{}?mode=rwc", db_path.display());
    let pool = DbPool::new(&url).await.expect("setup pool");
    let admin = pool.get_session("admin").await.expect("admin session");
    admin
        .execute_raw_ddl("CREATE TABLE t_e2e (id INTEGER PRIMARY KEY, val INTEGER NOT NULL)")
        .await
        .expect("create table");
    (pool, db_path)
}

/// 池获取：`get_session("admin")` 全路径（权限检查 + 连接租借）
fn bench_pool_acquire(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (pool, db_path) = rt.block_on(setup_pool(0));

    let mut group = c.benchmark_group("e2e_pool");
    group.sample_size(30);
    group.bench_function("get_session_admin", |b| {
        b.iter(|| {
            black_box(rt.block_on(async { pool.get_session("admin").await.expect("get_session") }))
        })
    });
    group.finish();
    drop(pool);
    let _ = std::fs::remove_file(&db_path);
}

/// 简单查询：`query_rows` 单行 SELECT（完整行查询管道）
fn bench_simple_query(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (pool, db_path) = rt.block_on(async {
        let (pool, path) = setup_pool(0).await;
        let admin = pool.get_session("admin").await.unwrap();
        admin
            .execute_raw("INSERT INTO t_e2e (id, val) VALUES (1, 42)")
            .await
            .unwrap();
        (pool, path)
    });

    let mut group = c.benchmark_group("e2e_query");
    group.sample_size(30);
    group.bench_function("query_rows_select_single", |b| {
        b.iter(|| {
            black_box(rt.block_on(async {
                pool.query_rows("SELECT id, val FROM t_e2e WHERE id = 1", "admin")
                    .await
                    .expect("query_rows")
            }))
        })
    });
    group.finish();
    drop(pool);
    let _ = std::fs::remove_file(&db_path);
}

/// 批量写：`execute_raw` 循环 INSERT（每迭代 64 行，含逐条解析/执行）
fn bench_batch_write(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (pool, db_path) = rt.block_on(setup_pool(0));

    let mut group = c.benchmark_group("e2e_write");
    group.sample_size(20);
    group.throughput(criterion::Throughput::Elements(64));
    // 单调 ID：预热迭代也会写入，固定 ID 会在第二次迭代触发 UNIQUE 冲突
    let next_id = std::sync::atomic::AtomicU64::new(0);
    group.bench_function("execute_raw_insert_x64", |b| {
        b.iter(|| {
            rt.block_on(async {
                let admin = pool.get_session("admin").await.expect("get_session");
                for _ in 0..64u64 {
                    let i = next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    admin
                        .execute_raw(&format!(
                            "INSERT INTO t_e2e (id, val) VALUES ({}, {})",
                            i, i
                        ))
                        .await
                        .expect("insert");
                }
            })
        })
    });
    group.finish();
    drop(pool);
    let _ = std::fs::remove_file(&db_path);
}

criterion_group!(
    e2e_benches,
    bench_pool_acquire,
    bench_simple_query,
    bench_batch_write
);
criterion_main!(e2e_benches);
