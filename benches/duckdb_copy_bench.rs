// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! DuckDB COPY 批量写入验收基准（`duckdb::memory:` 嵌入库）
//!
//! 对比同一批 500 行数据的两条写入路径：
//! - 逐行 INSERT：单行语句循环 `execute_duckdb_raw`（inklog 参数化时代的等价形态）
//! - COPY 批量导入：`DbPool::copy_in`（CSV 载荷 + 临时文件 `COPY FROM`）
//!
//! `duckdb::memory:` 的池连接在 idle 队列空时会重新 open 出全新空库，
//! 基准按"建表后归还连接、逐段串行复用"的口径组织（与 duckdb_copy_tests
//! 同款，`duckdb:file:` 多连接会触发文件锁冲突故不可用）。
//!
//! 运行: cargo bench --bench duckdb_copy_bench --features "duckdb,copy,sql-parser,runtime-tokio-rustls"
//! 基线数字记录于 docs/PERFORMANCE.md（本机一次性采样）。

#![cfg(all(
    feature = "duckdb",
    feature = "copy",
    feature = "runtime-tokio-rustls",
    feature = "sql-parser"
))]

use criterion::{Criterion, criterion_group, criterion_main};
use dbnexus::DbPool;
use dbnexus::database::copy::CopyStatement;

const ROWS: usize = 500;

/// 建立 `duckdb::memory:` 池并准备 `t_bench_copy` 表（建表后归还连接）。
async fn setup_pool() -> DbPool {
    let pool = DbPool::new("duckdb::memory:").await.expect("setup pool");
    let admin = pool.get_session("admin").await.expect("admin session");
    admin
        .execute_duckdb_raw("CREATE TABLE t_bench_copy (id INTEGER, val VARCHAR)")
        .await
        .expect("create table");
    drop(admin);
    pool
}

fn bench_duckdb_copy_vs_row_by_row(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let pool = rt.block_on(setup_pool());

    let rows: Vec<Vec<serde_json::Value>> = (0..ROWS)
        .map(|i| vec![serde_json::json!(i), serde_json::json!(format!("v{i}"))])
        .collect();
    let stmt = CopyStatement::new("t_bench_copy", &["id".to_string(), "val".to_string()])
        .expect("合法标识符应通过");
    let single = "INSERT INTO t_bench_copy (id, val) VALUES (?, ?)";

    let mut group = c.benchmark_group("copy_duckdb");
    group.sample_size(30);
    group.throughput(criterion::Throughput::Elements(ROWS as u64));

    group.bench_function("row_by_row_500", |b| {
        b.iter(|| {
            rt.block_on(async {
                let admin = pool.get_session("admin").await.expect("get_session");
                // 两条路径同款清表：criterion 多轮迭代间表不膨胀，成本对称
                admin
                    .execute_duckdb_raw("DELETE FROM t_bench_copy")
                    .await
                    .expect("清表");
                for row in &rows {
                    admin
                        .execute_duckdb_raw_with_params(
                            single,
                            vec![
                                dbnexus::database::DuckValue::BigInt(row[0].as_i64().unwrap_or(0)),
                                dbnexus::database::DuckValue::Text(
                                    row[1].as_str().unwrap_or_default().to_string(),
                                ),
                            ],
                        )
                        .await
                        .expect("逐行插入");
                }
                drop(admin);
            })
        })
    });

    group.bench_function("copy_in_500", |b| {
        b.iter(|| {
            rt.block_on(async {
                let admin = pool.get_session("admin").await.expect("get_session");
                admin
                    .execute_duckdb_raw("DELETE FROM t_bench_copy")
                    .await
                    .expect("清表");
                drop(admin);
                pool.copy_in(&stmt, &rows).await.expect("COPY 导入");
            })
        })
    });

    group.finish();
    drop(pool);
}

criterion_group!(benches, bench_duckdb_copy_vs_row_by_row);
criterion_main!(benches);
