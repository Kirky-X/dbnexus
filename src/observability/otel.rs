// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! OTel 导出桥（`otel` feature）
//!
//! 将 dbnexus 观测数据（池饱和度/等待计数/慢查询计数等健康快照指标）导出为
//! OTLP/HTTP JSON 形态（`resourceMetrics` 信封）：
//!
//! - **传输**：[`HttpTransport`] —— 手工 HTTP/1.1 POST over TcpStream
//!   （阻塞 IO 经 `spawn_blocking` 隔离），MVP 无新增依赖
//! - **stdout fallback**：传输失败且 `stdout_fallback = true` 时逐事件输出
//!   JSON 行（自定义输出通道经 [`StdoutExporter`] 注入）
//! - **事件源**：[`metric_events_from_health_snapshot`] 直接消费
//!   `DbPool::health_snapshot()`，上层典型用法：
//!
//! ```rust,no_run
//! # async fn example(pool: std::sync::Arc<dbnexus::DbPool>) {
//! let exporter = dbnexus::observability::otel::OtelExporter::new(
//!     dbnexus::observability::otel::OtelConfig::default(),
//! );
//! let snapshot = pool.health_snapshot().await;
//! exporter.export_health_snapshot(&snapshot).await.ok();
//! # }
//! ```

use std::sync::Arc;

use async_trait::async_trait;

/// OTLP 指标事件
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OtelMetricEvent {
    /// 指标名（如 `dbnexus.pool.saturation`）
    pub name: String,
    /// 数值
    pub value: f64,
    /// 单位（None = 无量纲）
    pub unit: Option<String>,
    /// 属性键值对（进入 OTLP dataPoint attributes）
    pub attributes: Vec<(String, String)>,
    /// 时间戳（Unix 纳秒）
    pub time_unix_nano: u64,
}

/// OTLP 传输抽象
#[async_trait]
pub trait OtlpTransport: Send + Sync {
    /// 发送 OTLP 请求体（JSON 形态）到端点
    async fn send(&self, endpoint: &str, body: &serde_json::Value) -> Result<(), String>;
}

/// OTLP/HTTP 传输（手工 HTTP/1.1 POST over TcpStream，无新增依赖）
///
/// 阻塞 IO 经 `tokio::task::spawn_blocking` 隔离，读写超时
/// `timeout_ms` 兜底。
pub struct HttpTransport {
    timeout_ms: u64,
}

impl HttpTransport {
    /// 创建传输（连接/读写超时毫秒）
    pub fn new(timeout_ms: u64) -> Self {
        Self { timeout_ms }
    }
}

#[async_trait]
impl OtlpTransport for HttpTransport {
    async fn send(&self, endpoint: &str, body: &serde_json::Value) -> Result<(), String> {
        let endpoint = endpoint.to_string();
        let payload = serde_json::to_vec(body).map_err(|e| format!("serialize failed: {e}"))?;
        let timeout_ms = self.timeout_ms;
        tokio::task::spawn_blocking(move || http_post(&endpoint, &payload, timeout_ms))
            .await
            .map_err(|e| format!("otlp http task failed: {e}"))?
    }
}

/// 解析 `http://host:port/path` 并发出 JSON POST（阻塞实现）
fn http_post(endpoint: &str, body: &[u8], timeout_ms: u64) -> Result<(), String> {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    let rest = endpoint
        .strip_prefix("http://")
        .ok_or_else(|| format!("otlp endpoint must be http:// (got: {endpoint})"))?;
    let (host_port, path) = match rest.split_once('/') {
        Some((hp, p)) => (hp, format!("/{p}")),
        None => (rest, "/".to_string()),
    };

    let mut stream =
        TcpStream::connect(host_port).map_err(|e| format!("otlp connect failed: {e}"))?;
    let timeout = Duration::from_millis(timeout_ms.max(1));
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));

    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {host_port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .and_then(|_| stream.write_all(body))
        .map_err(|e| format!("otlp write failed: {e}"))?;

    let mut response = vec![0u8; 512];
    let n = stream
        .read(&mut response)
        .map_err(|e| format!("otlp read failed: {e}"))?;
    let head = String::from_utf8_lossy(&response[..n]);
    // MVP：2xx 视为成功（mock collector 与常见 OTLP collector 均返回 200）
    if head.starts_with("HTTP/1.1 2") || head.starts_with("HTTP/1.0 2") {
        Ok(())
    } else {
        Err(format!(
            "otlp collector responded non-2xx: {}",
            head.lines().next().unwrap_or("<empty>")
        ))
    }
}

/// stdout fallback 导出器
///
/// 默认输出为 `println!` JSON 行；上层/测试可注入自定义输出通道。
pub struct StdoutExporter {
    writer: Box<dyn Fn(&str) + Send + Sync>,
}

impl StdoutExporter {
    /// 默认导出器（stdout JSON 行）
    pub fn new() -> Self {
        Self {
            writer: Box::new(|line| println!("{line}")),
        }
    }

    /// 注入自定义输出通道（测试/上层 sink）
    pub fn with_writer(writer: Box<dyn Fn(&str) + Send + Sync>) -> Self {
        Self { writer }
    }

    /// 输出一行
    pub fn write_line(&self, line: &str) {
        (self.writer)(line);
    }
}

impl Default for StdoutExporter {
    fn default() -> Self {
        Self::new()
    }
}

/// OTel 导出配置
#[derive(Debug, Clone)]
pub struct OtelConfig {
    /// OTLP/HTTP 端点（如 `http://collector:4318/v1/metrics`）
    pub endpoint: String,
    /// 传输超时（毫秒）
    pub timeout_ms: u64,
    /// 传输失败时是否经 stdout fallback 输出
    pub stdout_fallback: bool,
    /// resource 属性 service.name
    pub service_name: String,
}

impl Default for OtelConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://127.0.0.1:4318/v1/metrics".to_string(),
            timeout_ms: 3000,
            stdout_fallback: true,
            service_name: "dbnexus".to_string(),
        }
    }
}

/// OTel 导出桥
pub struct OtelExporter {
    config: OtelConfig,
    transport: Arc<dyn OtlpTransport>,
    stdout: StdoutExporter,
}

impl OtelExporter {
    /// 创建导出器（真实 HTTP/1.1 传输）
    pub fn new(config: OtelConfig) -> Self {
        let transport = Arc::new(HttpTransport::new(config.timeout_ms));
        Self::with_transport(config, transport)
    }

    /// 注入自定义传输（测试 mock / gRPC 传输扩展点）
    pub fn with_transport(config: OtelConfig, transport: Arc<dyn OtlpTransport>) -> Self {
        Self {
            config,
            transport,
            stdout: StdoutExporter::new(),
        }
    }

    /// 注入自定义传输与 stdout 输出通道（测试）
    pub fn with_transport_and_stdout(
        config: OtelConfig,
        transport: Arc<dyn OtlpTransport>,
        stdout_writer: Arc<dyn Fn(&str) + Send + Sync>,
    ) -> Self {
        Self {
            config,
            transport,
            stdout: StdoutExporter::with_writer(Box::new(move |line| stdout_writer(line))),
        }
    }

    /// 导出健康快照指标（health_snapshot → OTLP metrics）
    ///
    /// 传输失败且 `stdout_fallback` 开启时逐事件输出 JSON 行并视为成功。
    pub async fn export_health_snapshot(&self, snapshot: &serde_json::Value) -> Result<(), String> {
        let events = metric_events_from_health_snapshot(snapshot);
        let body = build_otlp_metrics_request(&self.config.service_name, &events);
        match self.transport.send(&self.config.endpoint, &body).await {
            Ok(()) => Ok(()),
            Err(_) if self.config.stdout_fallback => {
                for event in &events {
                    let line = serde_json::to_string(event)
                        .unwrap_or_else(|_| format!("{{\"name\":\"{}\"}}", event.name));
                    self.stdout.write_line(&line);
                }
                Ok(())
            }
            Err(e) => Err(format!("otlp export failed: {e}")),
        }
    }
}

/// 健康快照 → 指标事件（池饱和度 / 等待计数 / 慢查询计数）
pub fn metric_events_from_health_snapshot(snapshot: &serde_json::Value) -> Vec<OtelMetricEvent> {
    let now = time::OffsetDateTime::now_utc()
        .unix_timestamp_nanos()
        .max(0) as u64;
    let pool = &snapshot["pool"];
    let mut events = Vec::with_capacity(3);
    let mut push = |name: &str, value: f64, unit: Option<&str>| {
        events.push(OtelMetricEvent {
            name: name.to_string(),
            value,
            unit: unit.map(str::to_string),
            attributes: vec![("service.name".to_string(), "dbnexus".to_string())],
            time_unix_nano: now,
        });
    };

    if let Some(saturation) = pool["saturation"].as_f64() {
        push("dbnexus.pool.saturation", saturation, None);
    }
    if let Some(wait_count) = pool["wait_count"].as_u64() {
        push(
            "dbnexus.pool.wait_count",
            wait_count as f64,
            Some("{connections}"),
        );
    }
    if let Some(slow_count) = snapshot["slow_queries"]["count"].as_u64() {
        push(
            "dbnexus.slow_queries.count",
            slow_count as f64,
            Some("{query}"),
        );
    }
    events
}

/// 构建 OTLP/HTTP JSON 请求体（简化 `resourceMetrics` 信封）
pub fn build_otlp_metrics_request(
    service_name: &str,
    events: &[OtelMetricEvent],
) -> serde_json::Value {
    let metrics: Vec<serde_json::Value> = events
        .iter()
        .map(|e| {
            let attributes: Vec<serde_json::Value> = e
                .attributes
                .iter()
                .map(|(k, v)| {
                    serde_json::json!({
                        "key": k,
                        "value": { "stringValue": v },
                    })
                })
                .collect();
            serde_json::json!({
                "name": e.name,
                "unit": e.unit,
                "gauge": {
                    "dataPoints": [{
                        "asDouble": e.value,
                        "timeUnixNano": e.time_unix_nano,
                        "attributes": attributes,
                    }],
                },
            })
        })
        .collect();

    serde_json::json!({
        "resourceMetrics": [{
            "resource": {
                "attributes": [{
                    "key": "service.name",
                    "value": { "stringValue": service_name },
                }],
            },
            "scopeMetrics": [{
                "scope": { "name": "dbnexus", "version": env!("CARGO_PKG_VERSION") },
                "metrics": metrics,
            }],
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_otlp_envelope_shape() {
        let events = vec![OtelMetricEvent {
            name: "dbnexus.pool.saturation".to_string(),
            value: 0.5,
            unit: None,
            attributes: vec![("pool".to_string(), "primary".to_string())],
            time_unix_nano: 42,
        }];
        let body = build_otlp_metrics_request("svc", &events);
        let rm = &body["resourceMetrics"][0];
        assert_eq!(rm["scopeMetrics"][0]["scope"]["name"], "dbnexus");
        let dp = &rm["scopeMetrics"][0]["metrics"][0]["gauge"]["dataPoints"][0];
        assert_eq!(dp["asDouble"], 0.5);
        assert_eq!(dp["timeUnixNano"], 42);
        assert_eq!(dp["attributes"][0]["key"], "pool");
    }

    #[test]
    fn test_metric_events_empty_snapshot() {
        let events = metric_events_from_health_snapshot(&serde_json::json!({}));
        assert!(events.is_empty(), "空快照不产生事件");
    }

    #[test]
    fn test_stdout_exporter_writes_lines() {
        let lines: Arc<std::sync::Mutex<Vec<String>>> = Arc::default();
        let sink = lines.clone();
        let exporter = StdoutExporter::with_writer(Box::new(move |l| {
            sink.lock().unwrap().push(l.to_string())
        }));
        exporter.write_line("hello");
        assert_eq!(lines.lock().unwrap().len(), 1);
    }
}
