// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! oxcache 查询缓存装饰器验收基准（sqlite 临时文件库 + Moka 后端）
//!
//! 对比同一批 N+1 形态查询的两条执行路径，验证「命中收益 > 装饰器
//! 开销」的核心假设：
//! - 穿透（cache-miss）：每查询走 key 派生 + 后端 get miss + SQL 执行 + 回填
//! - 命中（cache-hit）：每查询走 key 派生 + 后端 get hit，数据库零往返
//!
//! 运行: cargo bench --bench oxcache_query_cache_bench --features "sqlite,oxcache-integration,sql-parser,runtime-tokio-rustls"
//! 基线数字记录于 docs/PERFORMANCE.md（本机多轮采样）。

#![cfg(all(
    feature = "sqlite",
    feature = "oxcache-integration",
    feature = "sql-parser",
    feature = "runtime-tokio-rustls"
))]

use std::sync::Arc;

use criterion::{Criterion, criterion_group, criterion_main};
use oxcache::backend::{CacheBackend, MokaMemoryBackend};

use dbnexus::database::DbPool;
use dbnexus::integrations::oxcache_query_cache::OxcacheQueryCache;

const USERS: i64 = 20;
const ORDERS: i64 = 200;

fn make_cache() -> Arc<dyn CacheBackend + Send + Sync> {
    Arc::new(MokaMemoryBackend::builder().capacity(100_000).build())
}

/// 建库预热：users/ORDERS 行 + 一次全查询填充（形成命中态）
async fn setup() -> (OxcacheQueryCache, std::path::PathBuf) {
    let db_path = std::env::temp_dir().join(format!(
        "dbnexus_bench_oxcache_qc_{}.db",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&db_path);
    let url = format!("sqlite:{}?mode=rwc", db_path.display());
    let pool = Arc::new(DbPool::new(&url).await.expect("pool"));
    let session = pool.get_session("admin").await.expect("session");
    session
        .execute_raw_ddl("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
        .await
        .expect("create users");
    session
        .execute_raw_ddl("CREATE TABLE orders (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL)")
        .await
        .expect("create orders");
    for i in 1..=USERS {
        session
            .execute_with_params(
                "INSERT INTO users (id, name) VALUES (?, ?)",
                &[serde_json::json!(i), serde_json::json!(format!("user{i}"))],
            )
            .await
            .expect("insert user");
    }
    for i in 1..=ORDERS {
        session
            .execute_with_params(
                "INSERT INTO orders (id, user_id) VALUES (?, ?)",
                &[serde_json::json!(i), serde_json::json!((i - 1) % USERS + 1)],
            )
            .await
            .expect("insert order");
    }
    (OxcacheQueryCache::new(pool, make_cache()), db_path)
}

fn bench_query_cache_hit_vs_miss(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (qc, db_path) = rt.block_on(setup());

    let point_sql = "SELECT id, name FROM users WHERE id = ?";
    let mut group = c.benchmark_group("oxcache_query_cache");
    group.sample_size(30);
    group.throughput(criterion::Throughput::Elements(ORDERS as u64));

    let run_200 = |qc: &OxcacheQueryCache, expect_hit: bool| {
        rt.block_on(async {
            for uid in 1..=ORDERS {
                let cq = qc
                    .query_cached(point_sql, &[serde_json::json!(uid)], &["users"])
                    .await
                    .expect("query");
                assert_eq!(
                    cq.from_cache, expect_hit,
                    "uid {uid}: expect_hit={expect_hit}"
                );
            }
        })
    };

    // 穿透：每轮失效一次使 200 个点查（uid 1..=200，含空结果回填）全走
    // key 派生 + miss + 执行 + 回填全链（1 次失效成本均摊到 200 查询）
    group.bench_function("n_plus_one_200_always_miss", |b| {
        b.iter(|| {
            rt.block_on(async {
                qc.invalidate_table("users").await.expect("invalidate");
            });
            run_200(&qc, false);
        })
    });

    // 命中：预热回填后每轮全命中（key 派生 + 后端 get hit，数据库零往返）
    // 预热回填（miss 组刚跑完缓存已满，先失效重建命中态）
    rt.block_on(async {
        qc.invalidate_table("users")
            .await
            .expect("prewarm invalidate")
    });
    run_200(&qc, false);
    run_200(&qc, true);
    group.bench_function("n_plus_one_200_all_hit", |b| b.iter(|| run_200(&qc, true)));

    group.finish();
    drop(qc);
    let _ = std::fs::remove_file(&db_path);
}

/// 命中全路径（key 派生 + 版本批量读 + 缓存 get + 反序列化）——派生
/// 纯成本的上界口径（反序列化随行集规模混入）
fn bench_derive_key_cost(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (qc, db_path) = rt.block_on(setup());
    let mut group = c.benchmark_group("oxcache_query_cache");
    group.sample_size(30);

    for (tables_n, tables) in &[(1usize, vec!["users"]), (2usize, vec!["users", "orders"])] {
        let sql = "SELECT id FROM users WHERE id = ?";
        // 预热回填（命中全路径 = key 派生 + 版本读 + 缓存 get + 反序列化）
        rt.block_on(qc.query_cached(sql, &[serde_json::json!(1)], tables))
            .expect("prewarm");
        let plural = if *tables_n > 1 { "s" } else { "" };
        group.bench_function(format!("hit_path_{tables_n}_table{plural}"), |b| {
            b.iter(|| {
                rt.block_on(async {
                    let cq = qc
                        .query_cached(sql, &[serde_json::json!(1)], tables)
                        .await
                        .expect("derive key bench");
                    assert!(cq.from_cache, "预热后应为命中路径");
                })
            })
        });
    }
    group.finish();
    drop(qc);
    let _ = std::fs::remove_file(&db_path);
}

criterion_group!(
    benches,
    bench_query_cache_hit_vs_miss,
    bench_derive_key_cost
);
criterion_main!(benches);
