// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 语句级 prepared statement LRU 缓存测试
//!
//! 覆盖池级缓存启用后 `execute_cached` 的命中断言、LRU 容量淘汰，
//! 以及未启用时的直通等价行为。

#![cfg(all(
    feature = "prepare-cache",
    feature = "sqlite",
    feature = "sql-parser",
    feature = "runtime-tokio-rustls"
))]

use dbnexus::{DbPool, DbPoolBuilder, PrepareCacheStats, PreparedStatementCache};

async fn temp_pool(tag: &str) -> (DbPool, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("dbnexus_t420_{}_{}.db", tag, std::process::id()));
    let _ = std::fs::remove_file(&path);
    let url = format!("sqlite:{}?mode=rwc", path.display());
    let pool = DbPool::new(&url).await.expect("pool");
    let admin = pool.get_session("admin").await.expect("admin");
    admin
        .execute_raw_ddl("CREATE TABLE t420 (id INTEGER PRIMARY KEY, val TEXT)")
        .await
        .expect("create table");
    (pool, path)
}

/// 池级缓存：同一语句两次 execute_cached → 1 未命中 + 1 命中
#[tokio::test]
async fn test_execute_cached_records_hits() {
    let (pool, path) = temp_pool("hits").await;
    pool.enable_prepare_cache(16);

    let admin = pool.get_session("admin").await.expect("admin");
    // 同一语句幂等重复执行（SELECT 无副作用），命中断言不受数据影响
    let sql = "SELECT id, val FROM t420";
    admin.execute_cached(sql).await.expect("first execute");
    admin.execute_cached(sql).await.expect("second execute");

    let stats = pool.prepare_cache_stats().expect("cache enabled");
    let PrepareCacheStats { hits, misses, .. } = stats;
    assert_eq!((hits, misses), (1, 1), "同语句重复执行应命中: {stats:?}");

    let _ = std::fs::remove_file(&path);
}

/// builder 启用 + 不同语句各自未命中、容量淘汰由 LRU 记账
#[tokio::test]
async fn test_builder_enabled_cache_and_eviction_accounting() {
    let path = std::env::temp_dir().join(format!("dbnexus_t420_b_{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let url = format!("sqlite:{}?mode=rwc", path.display());
    let pool = DbPoolBuilder::new()
        .url(&url)
        .prepare_cache(1) // 容量 1：第二条语句必淘汰第一条
        .build()
        .await
        .expect("pool");
    let admin = pool.get_session("admin").await.expect("admin");
    admin
        .execute_raw_ddl("CREATE TABLE t420 (id INTEGER PRIMARY KEY, val TEXT)")
        .await
        .expect("create table");

    admin
        .execute_cached("INSERT INTO t420 VALUES (1, 'a')")
        .await
        .expect("s1");
    admin
        .execute_cached("INSERT INTO t420 VALUES (2, 'b')")
        .await
        .expect("s2");

    let stats = pool.prepare_cache_stats().expect("cache enabled");
    assert_eq!(stats.misses, 2, "两条不同语句均未命中");
    assert_eq!(stats.evictions, 1, "容量 1 下第二条应淘汰第一条");
    assert_eq!(stats.size, 1);

    let _ = std::fs::remove_file(&path);
}

/// 未启用缓存：execute_cached 等价 execute_raw（直通），stats 为 None
#[tokio::test]
async fn test_disabled_cache_is_passthrough() {
    let (pool, path) = temp_pool("off").await;
    assert!(pool.prepare_cache_stats().is_none(), "未启用应返回 None");

    let admin = pool.get_session("admin").await.expect("admin");
    admin
        .execute_cached("INSERT INTO t420 (id, val) VALUES (1, 'a')")
        .await
        .expect("passthrough execute");
    let rows = pool
        .query_rows("SELECT val FROM t420", "admin")
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);

    let _ = std::fs::remove_file(&path);
}

/// 独立缓存实例：泛型 V 可承载调用方产物（驱动句柄包装的接入口径）
#[tokio::test]
async fn test_standalone_generic_cache() {
    let cache: PreparedStatementCache<String> = PreparedStatementCache::new(4);
    let (v1, hit1) = cache.get_or_prepare("SELECT 1", |sql| format!("handle:{sql}"));
    assert!(!hit1);
    let (v2, hit2) = cache.get_or_prepare("SELECT 1", |sql| format!("handle:{sql}"));
    assert!(hit2);
    assert_eq!(&*v1, &*v2, "命中应复用同一产物");
}
