// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! PermissionCache TTL + SWR 单元测试（oxcache 承载）
//!
//! 覆盖：
//! - 默认配置、TTL/refresh_interval/SWR 链式构造
//! - 未过期命中、过期 SWR 返回旧值（itch stale 窗口）
//! - 后台刷新更新值、provider 缺失时降级、刷新后新鲜窗内不重复回源
//! - 失效与清空、并发访问
//!
//! 迁移说明：读写面为异步方法（oxcache 异步面承载）；`len`/`is_empty`/
//! `is_expired` 已随吸收移除（条目存在性经 `get` 返回值判定）；
//! `refresh` 对 Fresh 条目为空操作（时间维度由 stale 窗口承担）。

#![cfg(feature = "permission")]

use std::sync::Arc;
use std::time::Duration;

use dbnexus::access::{
    MemoryPermissionProvider, PermissionAction, PermissionCache, RolePolicy, TablePermission,
};

// 辅助函数

fn sample_policy(table: &str, op: PermissionAction) -> RolePolicy {
    RolePolicy {
        tables: vec![TablePermission {
            name: table.to_string(),
            operations: vec![op],
        }],
    }
}

async fn make_provider_with_role(role: &str, policy: RolePolicy) -> Arc<MemoryPermissionProvider> {
    let provider = MemoryPermissionProvider::new();
    provider.add_role(role, policy).await;
    Arc::new(provider)
}

// 配置测试

/// 默认配置应启用 SWR 且 TTL=300s
#[tokio::test]
async fn test_permission_cache_default_config() {
    let cache = PermissionCache::new();
    let config = cache.config();
    assert_eq!(config.ttl, Duration::from_secs(300));
    assert_eq!(config.refresh_interval, Duration::from_secs(60));
    assert!(config.stale_while_revalidate);
}

/// 链式构造应正确应用 TTL/refresh_interval/SWR 设置
#[tokio::test]
async fn test_permission_cache_builder_chaining() {
    let cache = PermissionCache::new()
        .with_ttl(Duration::from_secs(30))
        .with_refresh_interval(Duration::from_secs(5))
        .with_stale_while_revalidate(false);
    let config = cache.config();
    assert_eq!(config.ttl, Duration::from_secs(30));
    assert_eq!(config.refresh_interval, Duration::from_secs(5));
    assert!(!config.stale_while_revalidate);
}

// TTL 行为测试

/// 未过期条目应命中返回值
#[tokio::test]
async fn test_permission_cache_get_unexpired() {
    let cache = PermissionCache::new().with_ttl(Duration::from_secs(60));
    cache
        .insert("admin", sample_policy("users", PermissionAction::Select))
        .await;
    let got = cache
        .get("admin")
        .await
        .expect("unexpired entry should hit");
    assert_eq!(got.tables.len(), 1);
    assert_eq!(got.tables[0].name, "users");
}

/// 过期条目在 SWR 禁用时应返回 None
#[tokio::test]
async fn test_permission_cache_get_expired_no_swr_returns_none() {
    let cache = PermissionCache::new()
        .with_ttl(Duration::from_millis(1))
        .with_stale_while_revalidate(false);
    cache
        .insert("admin", sample_policy("users", PermissionAction::Select))
        .await;
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(
        cache.get("admin").await.is_none(),
        "expired entry without SWR should return None"
    );
}

/// 过期条目在 SWR 启用时应返回旧值
#[tokio::test]
async fn test_permission_cache_get_expired_with_swr_returns_stale() {
    let cache = PermissionCache::new()
        .with_ttl(Duration::from_millis(1))
        .with_stale_while_revalidate(true);
    cache
        .insert("admin", sample_policy("users", PermissionAction::Select))
        .await;
    tokio::time::sleep(Duration::from_millis(20)).await;
    let got = cache
        .get("admin")
        .await
        .expect("SWR should return stale value for expired entry");
    assert_eq!(got.tables[0].name, "users");
}

// 后台刷新测试

/// refresh 应从 provider 更新缓存值
#[tokio::test]
async fn test_permission_cache_refresh_updates_value() {
    let provider =
        make_provider_with_role("admin", sample_policy("orders", PermissionAction::Delete)).await;
    // ttl 50ms（快速转入 stale）+ stale 窗 60s（回源写回前持续可服务旧值）
    let cache = PermissionCache::new()
        .with_ttl(Duration::from_millis(50))
        .with_refresh_interval(Duration::from_secs(60))
        .with_provider(provider);

    // 初始插入一个不同的值
    cache
        .insert("admin", sample_policy("users", PermissionAction::Select))
        .await;
    // 等条目转入 stale 窗口（ttl 50ms）后刷新：后台回源写回 provider 值
    tokio::time::sleep(Duration::from_millis(60)).await;
    cache.refresh("admin").await;
    // 轮询等待后台回源落库（refresh 为触发式，写回异步完成）
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let got = loop {
        if let Some(policy) = cache.get("admin").await
            && policy.tables[0].name == "orders"
        {
            break policy;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for the refresh to land from the source"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    assert_eq!(got.tables[0].name, "orders");
    assert_eq!(got.tables[0].operations[0], PermissionAction::Delete);
}

/// refresh 在 provider 缺失时应保留旧值（降级而非 panic）
#[tokio::test]
async fn test_permission_cache_refresh_without_provider_keeps_stale() {
    // ttl 50ms + stale 窗 60s：条目转入 stale 后触发回源路径
    let cache = PermissionCache::new()
        .with_ttl(Duration::from_millis(50))
        .with_refresh_interval(Duration::from_secs(60));
    // 不附加 provider
    cache
        .insert("admin", sample_policy("users", PermissionAction::Select))
        .await;
    // 等条目转入 stale 窗口后 refresh：无 provider 时不写缓存，旧值保留
    tokio::time::sleep(Duration::from_millis(60)).await;
    cache.refresh("admin").await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    let got = cache
        .get("admin")
        .await
        .expect("stale value should be kept after failed refresh");
    assert_eq!(got.tables[0].name, "users");
}

/// 刷新成功后条目回到 Fresh：新鲜窗内的再次 refresh 不重复回源
/// （时间维度由 stale 窗口承担，替代原 refresh_interval 节流）
#[tokio::test]
async fn test_permission_cache_refresh_skipped_within_fresh_window() {
    let provider =
        make_provider_with_role("admin", sample_policy("v2", PermissionAction::Select)).await;
    let cache = PermissionCache::new()
        .with_ttl(Duration::from_millis(50))
        .with_refresh_interval(Duration::from_secs(60))
        .with_provider(provider.clone());

    // 手动插入旧值，等条目转入 stale 窗口
    cache
        .insert("admin", sample_policy("v1", PermissionAction::Select))
        .await;
    tokio::time::sleep(Duration::from_millis(60)).await;
    // 第一次 refresh 触发回源
    cache.refresh("admin").await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(policy) = cache.get("admin").await
            && policy.tables[0].name == "v2"
        {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for the first refresh to land from the source"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // 变更 provider 值到 v3：条目刚回源为 Fresh（ttl 50ms），立即 refresh
    // 不应再回源
    provider
        .add_role("admin", sample_policy("v3", PermissionAction::Select))
        .await;
    cache.refresh("admin").await;

    // 验证：值仍为 v2（新鲜窗内未回源；读取不等待后台任务）
    let after_second = cache
        .get("admin")
        .await
        .expect("entry should be kept after fresh-window refresh");
    assert_eq!(after_second.tables[0].name, "v2");
}

// 失效与并发测试

/// invalidate 应使单个条目不可达（其余条目保持可达）
#[tokio::test]
async fn test_permission_cache_invalidate() {
    let cache = PermissionCache::new();
    cache
        .insert("a", sample_policy("t1", PermissionAction::Select))
        .await;
    cache
        .insert("b", sample_policy("t2", PermissionAction::Insert))
        .await;
    cache.invalidate("a").await;
    assert!(cache.get("a").await.is_none());
    assert!(cache.get("b").await.is_some());
}

/// clear 应清空所有条目
#[tokio::test]
async fn test_permission_cache_clear() {
    let cache = PermissionCache::new();
    cache
        .insert("a", sample_policy("t1", PermissionAction::Select))
        .await;
    cache
        .insert("b", sample_policy("t2", PermissionAction::Insert))
        .await;
    cache.clear().await;
    assert!(
        cache.get("a").await.is_none(),
        "entries should be unreachable after clear"
    );
    assert!(
        cache.get("b").await.is_none(),
        "entries should be unreachable after clear"
    );
}

/// 并发访问应线程安全（多任务读写）
#[tokio::test(flavor = "multi_thread")]
async fn test_permission_cache_concurrent_access() {
    let cache = PermissionCache::new().with_ttl(Duration::from_secs(60));
    cache
        .insert(
            "shared",
            sample_policy("shared_table", PermissionAction::Select),
        )
        .await;

    let cache_clone = cache.clone();
    let writer = tokio::spawn(async move {
        for i in 0..10 {
            cache_clone
                .insert(
                    &format!("key_{i}"),
                    sample_policy(&format!("t{i}"), PermissionAction::Select),
                )
                .await;
        }
    });

    let cache_clone2 = cache.clone();
    let reader = tokio::spawn(async move {
        for _ in 0..10 {
            let _ = cache_clone2.get("shared").await;
        }
    });

    writer.await.expect("writer task panicked");
    reader.await.expect("reader task panicked");

    assert!(cache.get("shared").await.is_some());
    assert!(cache.get("key_5").await.is_some());
}
