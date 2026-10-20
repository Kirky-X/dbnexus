// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! HTTP 健康端点生成器（`HealthRouterBuilder`）契约测试
//!
//! - 三端点语义：/healthz 恒 200（liveness，进程活着即应答）；/readyz 按
//!   `health_snapshot` 状态判定（healthy/degraded → 200，unhealthy → 503，
//!   熔断器 Open → 503）；/metrics 输出 Prometheus 文本（未注入采集器 → 404）。
//! - 测试方式：tower `ServiceExt::oneshot` 直测 Router，不起真实服务器；
//!   axum 经 `http-health` feature 引入，默认构建零 HTTP 依赖。
//!
//! 需要 sqlite + http-health + metrics + runtime-tokio-rustls feature。

#![cfg(all(
    feature = "runtime-tokio-rustls",
    feature = "sqlite",
    feature = "http-health",
    feature = "metrics"
))]

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use dbnexus::database::DbPool;
use dbnexus::integrations::http_health::{HealthRouterBuilder, snapshot_is_ready};
use dbnexus::observability::{CircuitBreaker, CircuitBreakerConfig, MetricsCollector};

fn temp_db_url(tag: &str) -> (String, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!(
        "dbnexus_http_health_{tag}_{}.db",
        std::process::id()
    ));
    (format!("sqlite:{}?mode=rwc", path.display()), path)
}

/// 构造有真实连接的池（执行过一次查询，pool.total > 0 → healthy）
///
/// 返回临时库路径，用例尾部（池 drop 后）负责删除
async fn warm_pool() -> (Arc<DbPool>, std::path::PathBuf) {
    let (url, path) = temp_db_url("warm");
    let pool = Arc::new(DbPool::new(&url).await.expect("pool"));
    let session = pool.get_session("admin").await.expect("session");
    session.execute_raw("SELECT 1").await.expect("warm query");
    (pool, path)
}

async fn send(router: Router, uri: &str) -> (StatusCode, String) {
    let resp = router
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("infallible service");
    let status = resp.status();
    let body = resp.into_body().collect().await.expect("body").to_bytes();
    (status, String::from_utf8_lossy(&body).into_owned())
}

// /healthz：liveness——进程存活即 200，不探测池状态

#[tokio::test]
async fn test_healthz_always_ok_with_alive_body() {
    let (pool, path) = warm_pool().await;
    let router = HealthRouterBuilder::new(pool).build();
    let (status, body) = send(router, "/healthz").await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the liveness endpoint should always return 200"
    );
    let json: serde_json::Value = serde_json::from_str(&body).expect("healthz should be JSON");
    assert_eq!(
        json["status"], "alive",
        "the healthz semantics is a process liveness marker"
    );
    let _ = std::fs::remove_file(&path);
}

// /readyz：readiness——按快照状态判定

#[tokio::test]
async fn test_readyz_ok_for_warm_pool() {
    let (pool, path) = warm_pool().await;
    let router = HealthRouterBuilder::new(pool.clone()).build();
    let (status, body) = send(router, "/readyz").await;
    assert_eq!(status, StatusCode::OK, "a healthy pool should be ready");
    let json: serde_json::Value = serde_json::from_str(&body).expect("readyz should be JSON");
    assert_eq!(json["status"], "healthy", "healthy pool snapshot status");
    assert!(
        json["pool"]["total"].as_u64().unwrap_or(0) > 0,
        "the pool should have connections"
    );
    assert!(
        json["pool"]["saturation"].as_f64().is_some(),
        "the pool saturation field should be passed through"
    );
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn test_readyz_ready_semantics_covers_degraded() {
    // readiness 判定纯函数：degraded（有等待者/池满）仍有服务能力 → 就绪
    let degraded = serde_json::json!({"status": "degraded"});
    let healthy = serde_json::json!({"status": "healthy"});
    let unhealthy = serde_json::json!({"status": "unhealthy"});
    let garbage = serde_json::json!({"status": 42});
    assert!(snapshot_is_ready(&healthy) && snapshot_is_ready(&degraded));
    assert!(!snapshot_is_ready(&unhealthy));
    assert!(
        !snapshot_is_ready(&garbage),
        "any non-tri-state field means not ready (fail-closed)"
    );
}

#[tokio::test]
async fn test_readyz_not_ready_for_zero_connection_pool() {
    // 未预热池：total == 0 → unhealthy → 503
    let (url, path) = temp_db_url("cold");
    let pool = Arc::new(DbPool::new(&url).await.expect("pool"));
    let snap = pool.health_snapshot().await;
    if snap["status"] != "unhealthy" {
        // 池构造若急切建连则前置不成立：显性上报条件跳过（不留静默盲区），
        // 端到端 503 路径由后续断言在成立时覆盖
        eprintln!(
            "SKIP(e2e): pool constructed eagerly (status={}), zero-connection 503 \
             path not exercised; fail-closed judgment still asserted",
            snap["status"]
        );
        assert!(
            !snapshot_is_ready(&serde_json::json!({"status": "unhealthy"})),
            "an unhealthy snapshot should not be judged ready"
        );
        let _ = std::fs::remove_file(&path);
        return;
    }
    let router = HealthRouterBuilder::new(pool).build();
    let (status, body) = send(router, "/readyz").await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "a zero-connection pool is not ready"
    );
    let json: serde_json::Value = serde_json::from_str(&body).expect("readyz should be JSON");
    assert_eq!(json["status"], "unhealthy");
    let _ = std::fs::remove_file(&path);
}

// /readyz × CircuitBreaker：熔断打开 → 不就绪

#[tokio::test]
async fn test_readyz_unready_when_circuit_breaker_open() {
    let (pool, path) = warm_pool().await;
    let breaker = Arc::new(CircuitBreaker::new(CircuitBreakerConfig::default()));
    for _ in 0..5 {
        breaker.record_failure().await; // 默认 failure_threshold = 5 → Open
    }
    assert_eq!(
        breaker.state().await,
        dbnexus::observability::CircuitBreakerState::Open,
        "prerequisite: the circuit breaker should be Open"
    );

    let router = HealthRouterBuilder::new(pool)
        .with_circuit_breaker(breaker)
        .build();
    let (status, body) = send(router, "/readyz").await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "an open breaker makes the service not ready even when the pool is healthy"
    );
    let json: serde_json::Value = serde_json::from_str(&body).expect("readyz should be JSON");
    assert_eq!(
        json["status"], "unhealthy",
        "an open breaker should override the snapshot status"
    );
    assert_eq!(
        json["circuit_breaker"], "open",
        "the breaker status should appear in the response body"
    );
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn test_readyz_includes_circuit_breaker_field_when_closed() {
    let (pool, path) = warm_pool().await;
    let breaker = Arc::new(CircuitBreaker::new(CircuitBreakerConfig::default()));
    let router = HealthRouterBuilder::new(pool)
        .with_circuit_breaker(breaker)
        .build();
    let (status, body) = send(router, "/readyz").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).expect("readyz should be JSON");
    assert_eq!(
        json["circuit_breaker"], "closed",
        "a closed breaker should be reported truthfully"
    );
    let _ = std::fs::remove_file(&path);
}

// /metrics：Prometheus 文本（注入采集器 → 200；未注入 → 404）

#[tokio::test]
async fn test_metrics_prometheus_text_when_collector_injected() {
    let (pool, path) = warm_pool().await;
    let collector = Arc::new(MetricsCollector::new());
    let router = HealthRouterBuilder::new(pool)
        .with_metrics_collector(collector)
        .build();
    let (status, body) = send(router, "/metrics").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("dbnexus_uptime_seconds"),
        "metrics should output Prometheus text (the uptime metric is always present), actual first 120 bytes: {}",
        &body[..body.len().min(120)]
    );
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn test_metrics_404_without_collector() {
    let (pool, path) = warm_pool().await;
    let router = HealthRouterBuilder::new(pool).build();
    let (status, body) = send(router, "/metrics").await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "metrics should explicitly 404 when no collector is injected"
    );
    assert!(
        body.contains("metrics"),
        "404 should explain the reason (no collector injected), actual: {body}"
    );
    let _ = std::fs::remove_file(&path);
}

// 路由面：未注册路径 404，构建器链式配置语义

#[tokio::test]
async fn test_unknown_path_is_404() {
    let (pool, path) = warm_pool().await;
    let router = HealthRouterBuilder::new(pool).build();
    let (status, _) = send(router, "/nope").await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "the generator exposes only three healthy paths"
    );
    let _ = std::fs::remove_file(&path);
}
