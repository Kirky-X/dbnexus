// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! `OxcacheQueryCache` 查询缓存装饰器契约测试
//!
//! - key 派生：SQL + 绑定参数 + 表版本戳（SHA-256），参数不同即不同缓存项
//! - 命中/穿透：首轮全部穿透执行（from_cache=false），二轮全部命中
//!   （from_cache=true，N+1 循环场景）
//! - 写路径失效：`invalidate_table` 后仅该表查询重新执行并读到新值，
//!   未失效表的缓存项保持命中
//!
//! 测试方式：真实 `MokaMemoryBackend` + sqlite 临时文件库（`sqlite::memory:`
//! 的池化连接互不共享建表，与 http_health 同款口径用文件库）。

#![cfg(all(
    feature = "runtime-tokio-rustls",
    feature = "sqlite",
    feature = "oxcache-integration"
))]

use std::sync::Arc;

use oxcache::backend::{CacheBackend, MokaMemoryBackend};

use dbnexus::database::DbPool;
use dbnexus::integrations::oxcache_query_cache::OxcacheQueryCache;

fn make_cache() -> Arc<dyn CacheBackend + Send + Sync> {
    let backend = MokaMemoryBackend::builder().capacity(10_000).build();
    Arc::new(backend)
}

fn temp_db_url(tag: &str) -> (String, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!(
        "dbnexus_oxcache_qc_{tag}_{}.db",
        std::process::id()
    ));
    (format!("sqlite:{}?mode=rwc", path.display()), path)
}

/// 建表 users(3 行)/orders(10 行) 并返回装饰器（sqlite 临时文件库）
async fn setup(tag: &str) -> (OxcacheQueryCache, std::path::PathBuf) {
    let (url, path) = temp_db_url(tag);
    let pool = Arc::new(DbPool::new(&url).await.expect("pool"));
    let session = pool.get_session("admin").await.expect("session");
    session
        .execute_raw_ddl("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
        .await
        .expect("create users");
    session
        .execute_raw_ddl(
            "CREATE TABLE orders (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL, amount REAL)",
        )
        .await
        .expect("create orders");
    for i in 1..=3 {
        session
            .execute_with_params(
                "INSERT INTO users (id, name) VALUES (?, ?)",
                &[serde_json::json!(i), serde_json::json!(format!("user{i}"))],
            )
            .await
            .expect("insert user");
    }
    for i in 1..=10 {
        session
            .execute_with_params(
                "INSERT INTO orders (id, user_id, amount) VALUES (?, ?, ?)",
                &[
                    serde_json::json!(i),
                    serde_json::json!((i - 1) % 3 + 1),
                    serde_json::json!(i as f64 * 1.5),
                ],
            )
            .await
            .expect("insert order");
    }
    let qc = OxcacheQueryCache::new(pool, make_cache());
    (qc, path)
}

async fn cleanup(path: &std::path::Path) {
    let _ = std::fs::remove_file(path);
}

// ============================================================================
// N+1 场景：首轮穿透、二轮全命中
// ============================================================================

#[tokio::test]
async fn test_n_plus_one_first_pass_misses_second_pass_hits() {
    let (qc, path) = setup("n1").await;

    // N+1 形态：顶部一次订单列表 + 每单一次用户点查（10 单 → 10 次点查）
    let top_first = qc
        .query_cached(
            "SELECT id, user_id, amount FROM orders ORDER BY id",
            &[],
            &["orders"],
        )
        .await
        .expect("top query first pass");
    assert!(!top_first.from_cache, "首轮顶部查询应穿透");
    assert_eq!(top_first.rows.len(), 10);

    let mut first_pass_rows = Vec::new();
    for user_id in 1..=3 {
        let cq = qc
            .query_cached(
                "SELECT id, name FROM users WHERE id = ?",
                &[serde_json::json!(user_id)],
                &["users"],
            )
            .await
            .expect("point query first pass");
        assert!(!cq.from_cache, "首轮点查 user{user_id} 应穿透");
        first_pass_rows.push(cq.rows);
    }

    // 第二轮：同 SQL 同参数 → 全部命中，行内容逐字节一致
    let top_second = qc
        .query_cached(
            "SELECT id, user_id, amount FROM orders ORDER BY id",
            &[],
            &["orders"],
        )
        .await
        .expect("top query second pass");
    assert!(top_second.from_cache, "二轮顶部查询应命中");
    assert_eq!(top_second.rows, top_first.rows);

    for (idx, user_id) in (1..=3).enumerate() {
        let cq = qc
            .query_cached(
                "SELECT id, name FROM users WHERE id = ?",
                &[serde_json::json!(user_id)],
                &["users"],
            )
            .await
            .expect("point query second pass");
        assert!(cq.from_cache, "二轮点查 user{user_id} 应命中（N+1 消除）");
        assert_eq!(cq.rows, first_pass_rows[idx], "命中行内容应与穿透一致");
    }

    cleanup(&path).await;
}

// ============================================================================
// 写路径失效：invalidate_table 只触碰目标表
// ============================================================================

#[tokio::test]
async fn test_invalidate_table_touches_only_target_table() {
    let (qc, path) = setup("inval").await;

    // 预热：users 与 orders 各两轮（首轮穿透、二轮命中）
    let users_sql = "SELECT id, name FROM users WHERE id = ?";
    let orders_sql = "SELECT id, amount FROM orders ORDER BY id";
    qc.query_cached(users_sql, &[serde_json::json!(1)], &["users"])
        .await
        .expect("users warm");
    let orders_first = qc
        .query_cached(orders_sql, &[], &["orders"])
        .await
        .expect("orders warm");
    assert!(
        qc.query_cached(users_sql, &[serde_json::json!(1)], &["users"])
            .await
            .expect("users warm 2")
            .from_cache,
        "预热二轮 users 应命中"
    );
    assert!(
        qc.query_cached(orders_sql, &[], &["orders"])
            .await
            .expect("orders warm 2")
            .from_cache,
        "预热二轮 orders 应命中"
    );

    // 写路径：更新 user 1 并失效 users 表
    let pool_session = qc.pool();
    let session = pool_session.get_session("admin").await.expect("session");
    session
        .execute_with_params(
            "UPDATE users SET name = ? WHERE id = ?",
            &[serde_json::json!("renamed"), serde_json::json!(1)],
        )
        .await
        .expect("update user");
    qc.invalidate_table("users")
        .await
        .expect("invalidate users");

    // users 查询：失效后重新执行（from_cache=false）且读到新值
    let users_after = qc
        .query_cached(users_sql, &[serde_json::json!(1)], &["users"])
        .await
        .expect("users after invalidate");
    assert!(!users_after.from_cache, "失效后 users 查询应重新执行");
    assert_eq!(
        users_after.rows[0]["name"],
        serde_json::json!("renamed"),
        "失效重查应读到写路径的新值"
    );

    // orders 查询：未失效表保持命中
    let orders_after = qc
        .query_cached(orders_sql, &[], &["orders"])
        .await
        .expect("orders after users invalidate");
    assert!(orders_after.from_cache, "未失效表应保持命中");
    assert_eq!(orders_after.rows, orders_first.rows);

    cleanup(&path).await;
}

// ============================================================================
// key 派生：参数参与派生（不同参数互不串缓存）
// ============================================================================

#[tokio::test]
async fn test_different_params_are_distinct_cache_entries() {
    let (qc, path) = setup("params").await;

    let sql = "SELECT name FROM users WHERE id = ?";
    let first = qc
        .query_cached(sql, &[serde_json::json!(1)], &["users"])
        .await
        .expect("param 1 first");
    assert!(!first.from_cache);
    let second = qc
        .query_cached(sql, &[serde_json::json!(2)], &["users"])
        .await
        .expect("param 2 first");
    assert!(
        !second.from_cache,
        "同 SQL 不同参数应派生不同 key（不串缓存）"
    );
    assert_eq!(first.rows[0]["name"], serde_json::json!("user1"));
    assert_eq!(second.rows[0]["name"], serde_json::json!("user2"));

    // 再查：各自命中各自的缓存项
    let first_hit = qc
        .query_cached(sql, &[serde_json::json!(1)], &["users"])
        .await
        .expect("param 1 second");
    let second_hit = qc
        .query_cached(sql, &[serde_json::json!(2)], &["users"])
        .await
        .expect("param 2 second");
    assert!(first_hit.from_cache && second_hit.from_cache);
    assert_eq!(first_hit.rows[0]["name"], serde_json::json!("user1"));
    assert_eq!(second_hit.rows[0]["name"], serde_json::json!("user2"));

    cleanup(&path).await;
}

// ============================================================================
// 失效语义边界：未知表幂等、SQL 变化派生不同 key
// ============================================================================

#[tokio::test]
async fn test_invalidate_unknown_table_is_idempotent() {
    let (qc, path) = setup("unknown").await;
    qc.invalidate_table("never_cached_table")
        .await
        .expect("失效未知表应幂等成功");
    qc.invalidate_table("never_cached_table")
        .await
        .expect("重复失效同样成功");

    let cq = qc
        .query_cached("SELECT COUNT(*) AS n FROM users", &[], &["users"])
        .await
        .expect("query after unknown invalidate");
    assert!(!cq.from_cache);
    cleanup(&path).await;
}

#[tokio::test]
async fn test_sql_change_derives_new_entry_even_without_invalidate() {
    let (qc, path) = setup("sqlchg").await;
    let a = qc
        .query_cached("SELECT id FROM users ORDER BY id", &[], &["users"])
        .await
        .expect("sql a first");
    let b = qc
        .query_cached("SELECT id, name FROM users ORDER BY id", &[], &["users"])
        .await
        .expect("sql b first");
    assert!(!a.from_cache && !b.from_cache, "SQL 变化应派生新 key");
    let a2 = qc
        .query_cached("SELECT id FROM users ORDER BY id", &[], &["users"])
        .await
        .expect("sql a second");
    assert!(a2.from_cache);
    cleanup(&path).await;
}

// ============================================================================
// 版本戳编码单射性：连续失效不得碰撞出相同版本串（CRITICAL 回归）
// ============================================================================

#[tokio::test]
async fn test_repeated_invalidations_always_produce_miss() {
    // 同表连续失效必须每次都触发重执行——版本戳编码若非单射（如 lossy
    // UTF-8 解码多对一），相邻时间戳会碰撞出相同版本串使失效静默 no-op，
    // 命中路径返回写路径之前的旧数据。多轮循环覆盖时间戳分布
    let (qc, path) = setup("reinv").await;
    let sql = "SELECT id, name FROM users ORDER BY id";

    // 固化一个缓存条目
    let cached = qc.query_cached(sql, &[], &["users"]).await.expect("warm");
    assert!(!cached.from_cache);
    assert!(
        qc.query_cached(sql, &[], &["users"])
            .await
            .expect("warm 2")
            .from_cache
    );

    // 100 轮：双失效后必须 miss（双次失效叠加以覆盖相邻时间戳的碰撞面）
    for round in 0..100 {
        qc.invalidate_table("users")
            .await
            .expect("invalidate first of pair");
        qc.invalidate_table("users")
            .await
            .expect("invalidate second of pair");
        let cq = qc
            .query_cached(sql, &[], &["users"])
            .await
            .expect("query after double invalidate");
        assert!(
            !cq.from_cache,
            "round {round}: 双失效后必须重执行（版本戳编码必须单射）"
        );
    }
    cleanup(&path).await;
}

// ============================================================================
// 安全上下文隔离：role/namespace 参与 key 派生
// ============================================================================

#[tokio::test]
async fn test_cross_role_instances_never_share_entries() {
    // 同一后端、同一 SQL/参数/表：admin 回填的条目对受限角色不可见——
    // 行集内容依赖执行时安全上下文（RLS/脱敏/权限），命中路径无检查，
    // role 必须参与 key 派生隔离
    let (pool, path) = {
        let (url, path) = temp_db_url("roles");
        let pool = Arc::new(DbPool::new(&url).await.expect("pool"));
        let session = pool.get_session("admin").await.expect("session");
        session
            .execute_raw_ddl("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
            .await
            .expect("create");
        session
            .execute_with_params(
                "INSERT INTO users (id, name) VALUES (?, ?)",
                &[serde_json::json!(1), serde_json::json!("secret-admin-view")],
            )
            .await
            .expect("insert");
        (pool, path)
    };
    let backend = make_cache();
    let admin = OxcacheQueryCache::new(pool.clone(), backend.clone());
    let restricted = OxcacheQueryCache::new(pool, backend).with_role("analyst");

    let sql = "SELECT id, name FROM users WHERE id = ?";
    let tables = ["users"];
    // admin 穿透回填
    let admin_view = admin
        .query_cached(sql, &[serde_json::json!(1)], &tables)
        .await
        .expect("admin first");
    assert!(!admin_view.from_cache);
    assert!(
        admin
            .query_cached(sql, &[serde_json::json!(1)], &tables)
            .await
            .expect("admin second")
            .from_cache,
        "admin 二轮应命中"
    );

    // 受限角色：不得命中 admin 回填条目（安全上下文不同 → 不同 key）
    let restricted_view = restricted
        .query_cached(sql, &[serde_json::json!(1)], &tables)
        .await
        .expect("restricted first");
    assert!(
        !restricted_view.from_cache,
        "受限角色不得命中 admin 回填的缓存条目（跨安全上下文隔离）"
    );

    cleanup(&path).await;
}

#[tokio::test]
async fn test_namespace_isolates_tenants_on_shared_backend() {
    // 多租户：同一后端上不同 namespace 的实例互不共享条目
    let (pool, path) = {
        let (url, path) = temp_db_url("ns");
        let pool = Arc::new(DbPool::new(&url).await.expect("pool"));
        let session = pool.get_session("admin").await.expect("session");
        session
            .execute_raw_ddl("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT NOT NULL)")
            .await
            .expect("create");
        session
            .execute_with_params(
                "INSERT INTO t (id, v) VALUES (?, ?)",
                &[serde_json::json!(1), serde_json::json!("tenant-data")],
            )
            .await
            .expect("insert");
        (pool, path)
    };
    let backend = make_cache();
    let tenant_a = OxcacheQueryCache::new(pool.clone(), backend.clone()).with_namespace("tenant-a");
    let tenant_b = OxcacheQueryCache::new(pool, backend).with_namespace("tenant-b");

    let sql = "SELECT id, v FROM t ORDER BY id";
    let a1 = tenant_a
        .query_cached(sql, &[], &["t"])
        .await
        .expect("a first");
    assert!(!a1.from_cache);
    assert!(
        tenant_a
            .query_cached(sql, &[], &["t"])
            .await
            .expect("a second")
            .from_cache
    );
    assert!(
        !tenant_b
            .query_cached(sql, &[], &["t"])
            .await
            .expect("b first")
            .from_cache,
        "不同 namespace 不得命中对方条目（租户隔离）"
    );

    cleanup(&path).await;
}

// ============================================================================
// key 哈希输入单射性：字段值含换行不得混淆条目（注入回归）
// ============================================================================

#[tokio::test]
async fn test_hash_field_injection_cannot_confuse_entries() {
    // 旧框架用换行标签拼接哈希输入，字段边界可被字段值注入：role 与
    // namespace 的边界由 "\nns=" 标签界定，(role="r", ns="EVIL\nns=N") 与
    // (role="r\nns=EVIL", ns="N") 在同表同版本同 SQL 同参数下折叠出逐字节
    // 相同输入；无界定的纯拼接则被 (role="rE", ns="VIL\nns=N") 与
    // (role="r", ns="EVIL\nns=N") 混淆。这些安全上下文不同的实例共享后端
    // 时，一方回填的条目会被另一方命中（行集内容依赖执行时 RLS/脱敏上下
    // 文，跨上下文命中即越权读取）。长度前缀框架下字段编码单射，以下混淆
    // 对必须互不可见
    let (pool, path) = {
        let (url, path) = temp_db_url("inj");
        let pool = Arc::new(DbPool::new(&url).await.expect("pool"));
        let session = pool.get_session("admin").await.expect("session");
        session
            .execute_raw_ddl("CREATE TABLE t (id INTEGER PRIMARY KEY)")
            .await
            .expect("create");
        (pool, path)
    };
    let backend = make_cache();
    // 实例 A：受限上下文（role="r"），回填条目
    let a = OxcacheQueryCache::new(pool.clone(), backend.clone())
        .with_role("r")
        .with_namespace("EVIL\nns=N");
    // 实例 B：换行标签注入——旧标签框架下哈希输入与 A 逐字节等价
    let b = OxcacheQueryCache::new(pool.clone(), backend.clone())
        .with_role("r\nns=EVIL")
        .with_namespace("N");
    // 实例 C：字段边界漂移——无界定纯拼接下哈希输入与 A 逐字节等价
    let c = OxcacheQueryCache::new(pool, backend)
        .with_role("rE")
        .with_namespace("VIL\nns=N");

    let sql = "SELECT id FROM t ORDER BY id";
    let first = a.query_cached(sql, &[], &["t"]).await.expect("a first");
    assert!(!first.from_cache);
    assert!(
        a.query_cached(sql, &[], &["t"])
            .await
            .expect("a second")
            .from_cache,
        "A 自身的正常命中语义不受影响"
    );

    // B/C 查询相同语句：不得命中 A 回填的条目（若命中则 from_cache 为
    // true 且不会走到数据库执行，即构成跨上下文越权读取）
    for (name, ctx) in [("标签注入", &b), ("边界漂移", &c)] {
        let outcome = ctx.query_cached(sql, &[], &["t"]).await;
        if let Ok(cq) = outcome {
            assert!(
                !cq.from_cache,
                "{name} 上下文不得命中他上下文回填的条目（哈希输入必须单射）"
            );
        }
    }

    cleanup(&path).await;
}

// ============================================================================
// fail-closed：空 tables 与非法表名拒绝
// ============================================================================

#[tokio::test]
async fn test_empty_tables_and_invalid_table_names_are_rejected() {
    let (qc, path) = setup("failclosed").await;
    let err = qc
        .query_cached("SELECT id FROM users", &[], &[])
        .await
        .expect_err("空 tables 必须拒绝（漏传即永不受失效影响）");
    assert!(
        format!("{err}").contains("table"),
        "错误应说明表集非法: {err}"
    );

    for bad in ["", "users;drop", "us ers", "用户"] {
        assert!(
            qc.query_cached("SELECT id FROM users", &[], &[bad])
                .await
                .is_err(),
            "非法表名 {bad:?} 应拒绝（版本键命名空间注入面）"
        );
    }
    cleanup(&path).await;
}

// ============================================================================
// 多表查询：任一表失效即重执行
// ============================================================================

#[tokio::test]
async fn test_multi_table_query_invalidated_by_any_mentioned_table() {
    let (qc, path) = setup("multitable").await;
    // 多表声明语义：tables 由调用方声明 SQL 依赖的全部表（dbnexus sqlite
    // 查询通道是单表 SELECT MVP，JOIN 不可执行，但装饰器的失效语义与
    // SQL 通道能力正交——单表 SQL + 双表声明即可验证"任一提及表失效
    // 都重新执行"）
    let sql = "SELECT id, user_id FROM orders WHERE user_id = ?";
    let tables = ["orders", "users"];
    let first = qc
        .query_cached(sql, &[serde_json::json!(1)], &tables)
        .await
        .expect("multi-table first");
    assert!(!first.from_cache);
    let second = qc
        .query_cached(sql, &[serde_json::json!(1)], &tables)
        .await
        .expect("multi-table second");
    assert!(second.from_cache, "二轮多表声明查询应命中");

    qc.invalidate_table("users")
        .await
        .expect("invalidate users");
    let third = qc
        .query_cached(sql, &[serde_json::json!(1)], &tables)
        .await
        .expect("multi-table after users invalidate");
    assert!(
        !third.from_cache,
        "任一提及表失效都应重新执行（users 失效触及 orders 声明查询）"
    );

    // 只失效 orders 后同样重执行；orders 与 users 都失效后仍只重执行一次
    qc.invalidate_table("orders")
        .await
        .expect("invalidate orders");
    let fourth = qc
        .query_cached(sql, &[serde_json::json!(1)], &tables)
        .await
        .expect("multi-table after orders invalidate");
    assert!(!fourth.from_cache);
    let fifth = qc
        .query_cached(sql, &[serde_json::json!(1)], &tables)
        .await
        .expect("multi-table second pass after both invalidations");
    assert!(fifth.from_cache, "失效后的首轮穿透即回填，再查命中");
    cleanup(&path).await;
}

// ============================================================================
// 策略换装失效：set_data_protection 运行时换策略后旧 key 命中失效
// ============================================================================

#[cfg(feature = "data-protection")]
use dbnexus::access::data_protection::{DataProtection, MaskStrategy, MaskingEngine};

#[cfg(feature = "data-protection")]
#[tokio::test]
async fn test_policy_change_invalidates_cached_entries() {
    let (url, path) = temp_db_url("dpepoch");
    let pool = Arc::new(DbPool::new(&url).await.expect("pool"));
    {
        let session = pool.get_session("admin").await.expect("session");
        session
            .execute_raw_ddl("CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT NOT NULL)")
            .await
            .expect("create users");
        session
            .execute_with_params(
                "INSERT INTO users (id, email) VALUES (?, ?)",
                &[serde_json::json!(1), serde_json::json!("alice@example.com")],
            )
            .await
            .expect("insert user");
    }
    let qc = OxcacheQueryCache::new(pool.clone(), make_cache());
    let sql = "SELECT id, email FROM users ORDER BY id";

    // 策略 v1：email 哈希脱敏——穿透回填的行集是脱敏出口
    pool.set_data_protection(DataProtection {
        masking: Some(Arc::new(
            MaskingEngine::new().rule("email", MaskStrategy::Hash),
        )),
        rls: None,
    })
    .await;
    let v1 = qc
        .query_cached(sql, &[], &["users"])
        .await
        .expect("v1 first");
    assert!(!v1.from_cache);
    assert_eq!(
        v1.rows[0]["email"].as_str().unwrap().len(),
        64,
        "v1 出口为哈希脱敏"
    );
    assert!(
        qc.query_cached(sql, &[], &["users"])
            .await
            .expect("v1 second")
            .from_cache,
        "同策略内正常命中语义不受影响"
    );

    // 策略 v2：撤销脱敏——key 派生纳入策略世代，旧策略脱敏行集必须失效，
    // 重执行返回明文（否则缓存把已撤销的脱敏永久钉死，策略收紧同理放行
    // 旧宽策略行集）
    pool.set_data_protection(DataProtection::default()).await;
    let v2 = qc
        .query_cached(sql, &[], &["users"])
        .await
        .expect("v2 first");
    assert!(
        !v2.from_cache,
        "set_data_protection 换装后旧 key 必须失效重执行"
    );
    assert_eq!(
        v2.rows[0]["email"], "alice@example.com",
        "v2 出口为明文（撤销脱敏即时生效）"
    );
    assert!(
        qc.query_cached(sql, &[], &["users"])
            .await
            .expect("v2 second")
            .from_cache,
        "新策略世代下重新建立命中"
    );

    cleanup(&path).await;
}
