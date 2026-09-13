// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 数据 API 网关测试
//!
//! 覆盖列白名单投影、过滤（eq/contains）、排序、分页与总数、
//! 未暴露列/未知端点的拒绝语义，以及 manifest 导出。

#![cfg(all(
    feature = "data-api",
    feature = "sqlite",
    feature = "sql-parser",
    feature = "runtime-tokio-rustls"
))]

use dbnexus::{DataApiGateway, Filter, ListRequest, OrderDirection, TableEndpoint};
use std::sync::Arc;

async fn setup_gateway(tag: &str) -> (DataApiGateway, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("dbnexus_t419_{}_{}.db", tag, std::process::id()));
    let _ = std::fs::remove_file(&path);
    let url = format!("sqlite:{}?mode=rwc", path.display());
    let pool = Arc::new(dbnexus::DbPool::new(&url).await.expect("pool"));
    let admin = pool.get_session("admin").await.expect("admin session");
    // secret 列故意不暴露到白名单
    admin
        .execute_raw_ddl(
            "CREATE TABLE t419_users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER, secret TEXT)",
        )
        .await
        .expect("create table");
    for i in 1..=7i64 {
        admin
            .execute_raw(&format!(
                "INSERT INTO t419_users (id, name, age, secret) VALUES ({}, 'user_{i}', {}, 's{i}')",
                i, 20 + i
            ))
            .await
            .expect("insert");
    }

    let gateway = DataApiGateway::new(pool)
        .register(
            "users",
            TableEndpoint::new("t419_users", &["id", "name", "age"])
                .expect("endpoint")
                .with_orderable(&["id", "age"])
                .expect("orderable")
                .with_page_limits(50, 3),
        )
        .expect("register");
    (gateway, path)
}

/// 白名单投影 + 分页 + 默认页大小钳制 + total
#[tokio::test]
async fn test_list_whitelist_projection_and_pagination() {
    let (gateway, path) = setup_gateway("list").await;

    let resp = gateway
        .list(
            "users",
            &ListRequest {
                page: 1,
                ..Default::default()
            },
        )
        .await
        .expect("list");
    // default_page_size=3 生效（page_size=0 → 默认）
    assert_eq!(resp.page_size, 3);
    assert_eq!(resp.items.len(), 3);
    assert_eq!(resp.total, 7);

    // 白名单投影：secret 列不出现
    let first = &resp.items[0];
    assert!(first.get("name").is_some(), "白名单列应返回");
    assert!(first.get("secret").is_none(), "未暴露列不应出现");

    // 第 2 页
    let page2 = gateway
        .list(
            "users",
            &ListRequest {
                page: 2,
                ..Default::default()
            },
        )
        .await
        .expect("page 2");
    assert_eq!(page2.items.len(), 3);
    assert_ne!(page2.items[0]["id"], resp.items[0]["id"], "翻页应移动窗口");

    let _ = std::fs::remove_file(&path);
}

/// 过滤：eq + contains 组合（AND）
#[tokio::test]
async fn test_list_filters() {
    let (gateway, path) = setup_gateway("filter").await;

    let resp = gateway
        .list(
            "users",
            &ListRequest {
                page: 1,
                page_size: 50,
                filters: vec![Filter::eq("age", 23)],
                ..Default::default()
            },
        )
        .await
        .expect("filter eq");
    assert_eq!(resp.total, 1);
    assert_eq!(resp.items[0]["name"], "user_3");

    let resp = gateway
        .list(
            "users",
            &ListRequest {
                page: 1,
                page_size: 50,
                filters: vec![Filter::contains("name", "user_"), Filter::eq("age", 25)],
                ..Default::default()
            },
        )
        .await
        .expect("filter combo");
    assert_eq!(resp.total, 1);
    assert_eq!(resp.items[0]["name"], "user_5");

    let _ = std::fs::remove_file(&path);
}

/// 排序（白名单内）+ 页大小硬上限钳制
#[tokio::test]
async fn test_list_order_and_page_size_clamp() {
    let (gateway, path) = setup_gateway("order").await;

    let resp = gateway
        .list(
            "users",
            &ListRequest {
                page: 1,
                page_size: 10, // 超过 max_page_size=50 内但 >7，取全量
                order: Some(("age".to_string(), OrderDirection::Desc)),
                ..Default::default()
            },
        )
        .await
        .expect("list desc");
    assert_eq!(resp.items.len(), 7);
    assert_eq!(resp.items[0]["age"], 27, "降序首行应为最大 age");

    // page_size 超上限 → 钳制到 max_page_size
    let resp = gateway
        .list(
            "users",
            &ListRequest {
                page: 1,
                page_size: 999,
                ..Default::default()
            },
        )
        .await
        .expect("clamp");
    assert_eq!(resp.page_size, 50);

    let _ = std::fs::remove_file(&path);
}

/// 拒绝语义：未暴露列过滤 → Permission；不可排序列 → Permission；未知端点 → Config
#[tokio::test]
async fn test_rejects_unexposed_columns_and_unknown_endpoint() {
    let (gateway, path) = setup_gateway("reject").await;

    // secret 列不在白名单 → Permission（可映射 403）
    let err = gateway
        .list(
            "users",
            &ListRequest {
                page: 1,
                filters: vec![Filter::eq("secret", "s1")],
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
    assert!(
        format!("{err}").contains("not exposed"),
        "未暴露列应被拒绝，实际: {err}"
    );

    // name 未加入可排序列白名单 → Permission
    let err = gateway
        .list(
            "users",
            &ListRequest {
                page: 1,
                order: Some(("name".to_string(), OrderDirection::Asc)),
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
    assert!(
        format!("{err}").contains("not orderable"),
        "不可排序列应被拒绝，实际: {err}"
    );

    // 未知端点 → Config
    let err = gateway
        .list("nope", &ListRequest::default())
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("unknown data-api endpoint"));

    // page=0 → Config
    let err = gateway
        .list(
            "users",
            &ListRequest {
                page: 0,
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("page starts at 1"));

    let _ = std::fs::remove_file(&path);
}

/// get 单行查询 + manifest 导出
#[tokio::test]
async fn test_get_and_manifest() {
    let (gateway, path) = setup_gateway("get").await;

    let row = gateway.get("users", 3).await.expect("get").expect("row");
    assert_eq!(row["name"], "user_3");
    assert!(row.get("secret").is_none());

    let missing = gateway.get("users", 99_999).await.expect("get missing");
    assert!(missing.is_none());

    let manifest = gateway.manifest();
    let endpoints = manifest["endpoints"].as_array().expect("endpoints");
    assert_eq!(endpoints.len(), 1);
    assert_eq!(endpoints[0]["name"], "users");
    assert_eq!(endpoints[0]["columns"].as_array().unwrap().len(), 3);

    let _ = std::fs::remove_file(&path);
}
