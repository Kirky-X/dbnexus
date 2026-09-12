// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! OTel 导出桥（`otel` feature）测试
//!
//! 慢查询/池指标事件导出 OTLP（MVP：手工 HTTP/1.1 otlp-http 客户端 +
//! stdout fallback）；mock collector（内存传输 + 本地 socket mock server）单测。

#![cfg(all(
    feature = "runtime-tokio-rustls",
    feature = "sqlite",
    feature = "health-check",
    feature = "otel"
))]

use std::sync::Arc;

use dbnexus::observability::otel::{
    metric_events_from_health_snapshot, OtelConfig, OtelExporter, OtlpTransport,
};
use dbnexus::DbPool;

fn temp_db_url(tag: &str) -> (String, std::path::PathBuf) {
    let path =
        std::env::temp_dir().join(format!("dbnexus_t412_{}_{}.db", tag, std::process::id()));
    (format!("sqlite:{}?mode=rwc", path.display()), path)
}

async fn health_snapshot_json() -> (Arc<DbPool>, serde_json::Value, std::path::PathBuf) {
    let (url, path) = temp_db_url("otel");
    let pool = Arc::new(DbPool::new(&url).await.unwrap());
    let snap = pool.health_snapshot().await;
    (pool, snap, path)
}

// ============================================================================
// 指标事件映射
// ============================================================================

#[tokio::test]
async fn test_otel_events_from_health_snapshot() {
    let (_pool, snap, path) = health_snapshot_json().await;
    let events = metric_events_from_health_snapshot(&snap);
    assert!(!events.is_empty(), "健康快照应映射出指标事件");
    let names: Vec<&str> = events.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"dbnexus.pool.saturation"), "池饱和度事件缺失: {names:?}");
    assert!(names.contains(&"dbnexus.pool.wait_count"));
    assert!(names.contains(&"dbnexus.slow_queries.count"));
    let saturation = events.iter().find(|e| e.name == "dbnexus.pool.saturation").unwrap();
    assert!((0.0..=1.0).contains(&saturation.value));
    let _ = std::fs::remove_file(&path);
}

// ============================================================================
// mock collector（内存传输）
// ============================================================================

/// 内存传输：记录请求体与端点（mock collector）
struct InMemoryCollector {
    requests: std::sync::Mutex<Vec<(String, serde_json::Value)>>,
}

impl InMemoryCollector {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            requests: std::sync::Mutex::new(Vec::new()),
        })
    }
    fn last_body(&self) -> serde_json::Value {
        self.requests.lock().unwrap().last().unwrap().1.clone()
    }
}

#[async_trait::async_trait]
impl OtlpTransport for InMemoryCollector {
    async fn send(&self, endpoint: &str, body: &serde_json::Value) -> Result<(), String> {
        self.requests
            .lock()
            .unwrap()
            .push((endpoint.to_string(), body.clone()));
        Ok(())
    }
}

#[tokio::test]
async fn test_otel_export_to_mock_collector() {
    let (_pool, snap, path) = health_snapshot_json().await;
    let collector = InMemoryCollector::new();
    let exporter = OtelExporter::with_transport(
        OtelConfig {
            endpoint: "http://mock:4318/v1/metrics".to_string(),
            timeout_ms: 1000,
            stdout_fallback: false,
            service_name: "dbnexus-test".to_string(),
        },
        collector.clone(),
    );

    exporter.export_health_snapshot(&snap).await.unwrap();
    let body = collector.last_body();
    assert_eq!(
        collector.requests.lock().unwrap()[0].0,
        "http://mock:4318/v1/metrics"
    );

    // OTLP 请求体形态：resourceMetrics → resource(service.name) → scopeMetrics → metrics[]
    let rm = &body["resourceMetrics"][0];
    assert_eq!(
        rm["resource"]["attributes"][0]["key"],
        "service.name",
        "OTLP 信封应含 resource 属性"
    );
    let metrics = rm["scopeMetrics"][0]["metrics"].as_array().unwrap();
    assert!(metrics.len() >= 3, "至少导出饱和度/等待/慢查询指标");
    let names: Vec<&str> = metrics
        .iter()
        .map(|m| m["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"dbnexus.pool.saturation"));
    // gauge dataPoint 含值与时间戳
    let dp = &metrics[0]["gauge"]["dataPoints"][0];
    assert!(dp["asDouble"].is_number());
    assert!(dp["timeUnixNano"].is_u64());

    let _ = std::fs::remove_file(&path);
}

// ============================================================================
// stdout fallback（传输失败时落本地输出，注入收集器断言）
// ============================================================================

struct FailingTransport;

#[async_trait::async_trait]
impl OtlpTransport for FailingTransport {
    async fn send(&self, _endpoint: &str, _body: &serde_json::Value) -> Result<(), String> {
        Err("collector down".to_string())
    }
}

#[tokio::test]
async fn test_otel_stdout_fallback_on_transport_failure() {
    let (_pool, snap, path) = health_snapshot_json().await;

    let fallback_lines: Arc<std::sync::Mutex<Vec<String>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = fallback_lines.clone();

    let exporter = OtelExporter::with_transport_and_stdout(
        OtelConfig {
            endpoint: "http://mock:4318/v1/metrics".to_string(),
            timeout_ms: 1000,
            stdout_fallback: true,
            service_name: "dbnexus-test".to_string(),
        },
        Arc::new(FailingTransport),
        Arc::new(move |line: &str| sink.lock().unwrap().push(line.to_string())),
    );

    // 传输失败但 stdout fallback 兜底 → 导出仍成功
    exporter.export_health_snapshot(&snap).await.unwrap();
    let lines = fallback_lines.lock().unwrap();
    assert!(!lines.is_empty(), "传输失败时应输出 stdout fallback 行");
    let parsed: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    assert!(
        parsed["name"].is_string(),
        "fallback 行应为指标事件 JSON"
    );

    let _ = std::fs::remove_file(&path);
}

// ============================================================================
// 本地 socket mock collector（真实 HTTP/1.1 传输层）
// ============================================================================

#[tokio::test]
async fn test_otel_http_transport_against_mock_collector_socket() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    // mock collector：accept 一个连接，读取完整请求，回 200
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = vec![0u8; 8192];
        let n = stream.read(&mut buf).unwrap();
        let request = String::from_utf8_lossy(&buf[..n]).to_string();
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
        request
    });

    let body = serde_json::json!({
        "resourceMetrics": [{
            "scopeMetrics": [{
                "metrics": [{"name": "dbnexus.pool.saturation", "gauge": {"dataPoints": [{"asDouble": 0.4}]}}]
            }]
        }]
    });

    let transport = dbnexus::observability::otel::HttpTransport::new(2000);
    transport
        .send(&format!("http://{addr}/v1/metrics"), &body)
        .await
        .unwrap();

    let request = server.join().unwrap();
    assert!(request.starts_with("POST /v1/metrics HTTP/1.1\r\n"), "应发出 OTLP metrics POST: {request}");
    assert!(request.contains("Content-Type: application/json"));
    // 请求体应包含指标名
    assert!(request.contains("dbnexus.pool.saturation"));
}
