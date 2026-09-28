// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! batch_insert 多值 INSERT 验收基准（sqlite 临时文件库）
//!
//! 对比同一批 500 行数据的两条写入路径：
//! - 逐行 INSERT：单行语句循环 `execute_with_params`（inklog 时代的写法）
//! - 多值 batch_insert：`BatchInsertStatement::chunk_rows` 500 行分块后
//!   逐块 `execute_with_params`
//!
//! 运行: cargo bench --bench batch_insert_bench --features "sqlite,copy,sql-parser,runtime-tokio-rustls"
//! 基线数字记录于 docs/PERFORMANCE.md（本机一次性采样）。

#![cfg(all(
    feature = "sqlite",
    feature = "copy",
    feature = "runtime-tokio-rustls",
    feature = "sql-parser"
))]

use criterion::{Criterion, criterion_group, criterion_main};
use dbnexus::DbPool;
use dbnexus::database::copy::{BatchInsertStatement, PlaceholderStyle};

const ROWS: usize = 500;

/// 建立临时文件库池并准备 `t_bench_batch` 表。
///
/// `sqlite::memory:` 的每个池化连接是独立内存库，跨连接建表不可见，
/// 与 e2e_bench 口径一致使用 `sqlite:<path>?mode=rwc` 临时文件库。
async fn setup_pool() -> (DbPool, std::path::PathBuf) {
    let db_path = std::env::temp_dir().join(format!(
        "dbnexus_bench_batch_insert_{}.db",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&db_path);
    let url = format!("sqlite:{}?mode=rwc", db_path.display());
    let pool = DbPool::new(&url).await.expect("setup pool");
    let admin = pool.get_session("admin").await.expect("admin session");
    admin
        .execute_raw_ddl("CREATE TABLE t_bench_batch (id INTEGER PRIMARY KEY, val TEXT NOT NULL)")
        .await
        .expect("create table");
    (pool, db_path)
}

fn bench_batch_insert_vs_row_by_row(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (pool, db_path) = rt.block_on(setup_pool());

    let rows: Vec<Vec<serde_json::Value>> = (0..ROWS)
        .map(|i| vec![serde_json::json!(i), serde_json::json!(format!("v{i}"))])
        .collect();
    let stmt = BatchInsertStatement::new("t_bench_batch", &["id".to_string(), "val".to_string()])
        .expect("合法标识符应通过");
    let chunks = stmt
        .chunk_rows(&rows, PlaceholderStyle::QMark)
        .expect("分块应成功");

    let mut group = c.benchmark_group("batch_insert_sqlite");
    group.sample_size(30);
    group.throughput(criterion::Throughput::Elements(ROWS as u64));

    group.bench_function("row_by_row_500", |b| {
        b.iter(|| {
            rt.block_on(async {
                let admin = pool.get_session("admin").await.expect("get_session");
                // 两条路径同款清表：criterion 多轮迭代间数据不残留，成本对称
                admin
                    .execute_raw("DELETE FROM t_bench_batch")
                    .await
                    .expect("清表");
                let single = stmt.build(1, PlaceholderStyle::QMark).expect("单行语句");
                for row in &rows {
                    admin
                        .execute_with_params(&single, row)
                        .await
                        .expect("逐行插入");
                }
            })
        })
    });

    group.bench_function("multi_value_chunked_500", |b| {
        b.iter(|| {
            rt.block_on(async {
                let admin = pool.get_session("admin").await.expect("get_session");
                admin
                    .execute_raw("DELETE FROM t_bench_batch")
                    .await
                    .expect("清表");
                for (sql, params) in &chunks {
                    admin
                        .execute_with_params(sql, params)
                        .await
                        .expect("多值插入");
                }
            })
        })
    });

    group.finish();
    drop(pool);
    let _ = std::fs::remove_file(&db_path);
}

criterion_group!(benches, bench_batch_insert_vs_row_by_row);
criterion_main!(benches);
