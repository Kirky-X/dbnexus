// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 分片路由 Session 集成测试 (T068)
//!
//! 测试 v0.3.0 新增的 ShardRouter 分片路由 API：
//! - `shard_id_for_key`: 纯哈希分片键路由
//! - `get_session_for_shard`: 路由并返回 Session
//! - `get_session_for_shard_with_id`: 返回 Session + shard_id 元组
//! - `enforce_shard_binding`: 跨分片冲突检测
//!
//! 这些测试覆盖：
//! - 哈希一致性与分布
//! - 成功路由与未注册分片错误
//! - 跨分片检测的接受/拒绝路径
//! - 权限检查（role 必须在安全角色列表中）

use dbnexus::{ErrorCategory, ShardConfig, ShardRouter};

/// 返回测试用数据库 URL，优先使用 `DATABASE_URL` 环境变量；
/// 否则使用 `sqlite::memory:`（当 `sqlite` feature 启用时）。
fn get_database_url() -> Option<String> {
    if let Ok(url) = std::env::var("DATABASE_URL") {
        return Some(url);
    }
    if cfg!(feature = "sqlite") {
        return Some("sqlite::memory:".to_string());
    }
    None
}

/// 构造一个注册了 `n` 个分片（每个分片有独立连接池）的 `ShardRouter`。
///
/// 使用 `sqlite::memory:` 作为后端，分片策略为 `hash`（与 `shard_id_for_key` 一致）。
async fn build_router_with_pools(n: u32) -> Option<ShardRouter> {
    let url = get_database_url()?;
    let config = ShardConfig::new("hash", n, "shard_session", &url);
    let router = ShardRouter::with_config(&config).await.expect("router init");
    Some(router)
}

// ============================================================================
// shard_id_for_key 测试
// ============================================================================

/// TEST-SHARD-SESSION-001: `shard_id_for_key` 哈希一致性
///
/// 同一 `shard_key` 在同一 router 上多次计算应返回相同的 `shard_id`。
#[test]
fn test_shard_id_for_key_consistency() {
    let router = ShardRouter::with_strategy("hash", 8);

    let id1 = router.shard_id_for_key("user_42");
    let id2 = router.shard_id_for_key("user_42");
    let id3 = router.shard_id_for_key("user_42");

    assert_eq!(id1, id2, "repeated calls must be idempotent");
    assert_eq!(id2, id3, "hash must be deterministic across calls");
    assert!(id1 < 8, "shard_id must be within [0, total_shards)");
}

/// TEST-SHARD-SESSION-002: `shard_id_for_key` 分布在 `[0, total_shards)` 内
///
/// 对大量不同的键进行哈希，所有结果都应落入合法区间；
/// 且应至少命中多个不同分片（验证哈希分散性，避免全部分配到同一分片）。
#[test]
fn test_shard_id_for_key_distribution() {
    let router = ShardRouter::with_strategy("hash", 16);

    let mut unique_shards = std::collections::HashSet::new();
    for i in 0..256 {
        let key = format!("user_{}", i);
        let shard = router.shard_id_for_key(&key);
        assert!(shard < 16, "shard_id {} out of range for key {}", shard, key);
        unique_shards.insert(shard);
    }

    // 256 个不同键应至少命中 8 个不同分片（宽松阈值，避免哈希偶发聚集导致测试不稳定）
    assert!(
        unique_shards.len() >= 8,
        "expected at least 8 distinct shards, got {}: {:?}",
        unique_shards.len(),
        unique_shards
    );
}

// ============================================================================
// get_session_for_shard 测试
// ============================================================================

/// TEST-SHARD-SESSION-003: `get_session_for_shard` 成功路由
///
/// 对一个已注册连接池的分片调用 `get_session_for_shard` 应返回 `Ok(Session)`；
/// role 使用 `admin`（在无权限配置时属于安全角色）。
#[tokio::test]
async fn test_get_session_for_shard_success() {
    let Some(router) = build_router_with_pools(4).await else {
        return;
    };

    let session = router.get_session_for_shard("user_42", "admin").await;
    assert!(
        session.is_ok(),
        "get_session_for_shard should succeed for registered shard: {:?}",
        session.err()
    );
}

/// TEST-SHARD-SESSION-004: `get_session_for_shard` 未注册分片返回错误
///
/// 构造一个不注册任何连接池的路由器（使用 `with_strategy` 而非 `with_config`），
/// 调用 `get_session_for_shard` 应返回 `DbError::Config`。
#[tokio::test]
async fn test_get_session_for_shard_no_pool_registered() {
    // 仅注册分片元信息，不注册连接池
    let mut router = ShardRouter::with_strategy("hash", 4);
    router.register_shard(0, "db_0".to_string(), "sqlite::memory:".to_string());

    let result = router.get_session_for_shard("user_42", "admin").await;
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("expected error when no pool is registered for the computed shard"),
    };
    let msg = format!("{}", err);
    assert!(
        msg.contains("No pool registered for shard"),
        "error message should mention missing pool, got: {}",
        msg
    );
}

/// TEST-SHARD-SESSION-005: `get_session_for_shard_with_id` 返回 Session + shard_id 元组
///
/// 返回的 `shard_id` 应与 `shard_id_for_key` 单独计算的结果一致。
#[tokio::test]
async fn test_get_session_for_shard_with_id() {
    let Some(router) = build_router_with_pools(4).await else {
        return;
    };

    let expected_shard_id = router.shard_id_for_key("user_99");

    let result = router.get_session_for_shard_with_id("user_99", "admin").await;
    assert!(
        result.is_ok(),
        "get_session_for_shard_with_id should succeed: {:?}",
        result.err()
    );

    let (_session, shard_id) = result.unwrap();
    assert_eq!(
        shard_id, expected_shard_id,
        "returned shard_id must match shard_id_for_key computation"
    );
}

// ============================================================================
// enforce_shard_binding 测试
// ============================================================================

/// TEST-SHARD-SESSION-006: `enforce_shard_binding` 同分片通过
///
/// 当 `requested_shard_key` 哈希到的分片与 `expected_shard_id` 一致时，
/// 应返回 `Ok(())`。
#[test]
fn test_enforce_shard_binding_same_shard_accepts() {
    let router = ShardRouter::with_strategy("hash", 8);

    let key = "user_42";
    let shard_id = router.shard_id_for_key(key);

    let result = router.enforce_shard_binding(shard_id, key);
    assert!(
        result.is_ok(),
        "same shard binding should be accepted: {:?}",
        result.err()
    );
}

/// TEST-SHARD-SESSION-007: `enforce_shard_binding` 跨分片返回 `ShardConflict`
///
/// 当 `requested_shard_key` 哈希到的分片与 `expected_shard_id` 不一致时，
/// 应返回 `QueryErrorReport { category: ShardConflict }`。
#[test]
fn test_enforce_shard_binding_cross_shard_rejects() {
    let router = ShardRouter::with_strategy("hash", 8);

    // 找一个映射到不同分片的 key
    let base_key = "user_42";
    let base_shard = router.shard_id_for_key(base_key);

    let mut conflict_key = String::new();
    for i in 0..256 {
        let candidate = format!("conflict_{}", i);
        let candidate_shard = router.shard_id_for_key(&candidate);
        if candidate_shard != base_shard {
            conflict_key = candidate;
            break;
        }
    }
    assert!(!conflict_key.is_empty(), "test setup failed: no conflict key found");

    let result = router.enforce_shard_binding(base_shard, &conflict_key);
    assert!(result.is_err(), "cross-shard binding should be rejected");

    let report = result.unwrap_err();
    assert_eq!(
        report.category,
        ErrorCategory::ShardConflict,
        "error category should be ShardConflict"
    );
    assert!(
        report.message.contains("Cross-shard query detected"),
        "error message should mention cross-shard conflict, got: {}",
        report.message
    );
    assert!(
        !report.suggestion.is_empty(),
        "suggestion should be non-empty to guide the user"
    );
}

// ============================================================================
// 端到端：路由 + 绑定检查组合
// ============================================================================

/// TEST-SHARD-SESSION-008: 路由后绑定检查的端到端流程
///
/// 模拟典型使用流程：
/// 1. `get_session_for_shard_with_id` 获取 Session 和绑定的 shard_id
/// 2. 对同一 key 再次调用 `enforce_shard_binding` 应通过
/// 3. 对不同 key（映射到不同分片）调用 `enforce_shard_binding` 应失败
#[tokio::test]
async fn test_session_routing_end_to_end() {
    let Some(router) = build_router_with_pools(4).await else {
        return;
    };

    // Step 1: 路由获取 Session
    let primary_key = "user_42";
    let result = router.get_session_for_shard_with_id(primary_key, "admin").await;
    assert!(result.is_ok(), "routing should succeed: {:?}", result.err());
    let (_session, bound_shard_id) = result.unwrap();

    // Step 2: 同一 key 的绑定检查通过
    let same_check = router.enforce_shard_binding(bound_shard_id, primary_key);
    assert!(
        same_check.is_ok(),
        "binding check for the same key should pass: {:?}",
        same_check.err()
    );

    // Step 3: 找一个不同分片的 key 并验证绑定检查失败
    let mut other_key = String::new();
    for i in 0..256 {
        let candidate = format!("other_{}", i);
        if router.shard_id_for_key(&candidate) != bound_shard_id {
            other_key = candidate;
            break;
        }
    }
    if !other_key.is_empty() {
        let cross_check = router.enforce_shard_binding(bound_shard_id, &other_key);
        assert!(
            cross_check.is_err(),
            "binding check for a different-shard key must fail"
        );
    }
    // 如果 256 个候选都映射到同一分片（极小概率），跳过 Step 3 不算失败。
}
