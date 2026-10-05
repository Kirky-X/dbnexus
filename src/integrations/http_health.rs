// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! HTTP 健康端点生成器（`http-health` feature）
//!
//! [`HealthRouterBuilder`] 把既有健康数据源装配为挂载三个标准端点的
//! axum [`Router`]，供 K8s/反代健康探测与 Prometheus 抓取直接消费：
//!
//! | 端点 | 语义 | 数据源 | 成功态 |
//! |------|------|--------|--------|
//! | `GET /healthz` | **liveness**：进程存活标记 | 无（恒 200） | 200 |
//! | `GET /readyz` | **readiness**：能否承接流量 | `DbPool::health_snapshot`（池饱和度/副本/慢查询）+ 可选熔断器 | healthy/degraded → 200，否则 503 |
//! | `GET /metrics` | **Prometheus 抓取** | 可选 `MetricsCollector::export_prometheus` | 已注入 → 200，未注入 → 404 |
//!
//! # 契约
//!
//! - **生成而非服务**：`build()` 只产出 [`Router`]，监听端口/优雅停机由
//!   消费方用 `axum::serve` 自行编排——生成器不占用运行时配置
//! - **readiness fail-closed**：快照 `status` 非三态值（healthy/degraded/
//!   unhealthy）一律判不就绪，未知字段形态不产生误放行；熔断器 `Open`
//!   覆盖快照状态为 `unhealthy`（池健康但下游调用被熔断拒绝时不应接流量）
//! - **默认零 HTTP 依赖**：本 feature 未启用时库不引入 axum；启用后也仅
//!   生成 Router，无服务器循环
//! - **可选数据源跟随自身 feature 门控**：熔断器随 `health-check`（本
//!   feature 依赖它，恒可用）；metrics 采集器随 `metrics`——该 feature
//!   未启用时 `with_metrics_collector` 不存在，`/metrics` 恒为 404 说明
//!
//! # 暴露面与部署契约
//!
//! 三端点生成后**无鉴权、无速率限制**——K8s/反代健康探测免鉴权是行业
//! 惯例，但 `/readyz` 返回完整池快照（连接数/饱和度/慢查询统计/replicas
//! 数组）、`/metrics` 返回全量指标文档，属内部拓扑信息。部署时必须置于
//! 内网或反代鉴权之后，监听地址用 loopback/内网接口而非 `0.0.0.0`；
//! 需要与业务端点隔离时把 Router `merge`/`nest` 进带鉴权的 admin Router
//! （如 `app.nest("/internal/health", health_router)`）。
//!
//! ```rust,no_run
//! # async fn example(pool: std::sync::Arc<dbnexus::DbPool>) -> std::io::Result<()> {
//! let app = dbnexus::integrations::http_health::HealthRouterBuilder::new(pool).build();
//! let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await?;
//! axum::serve(listener, app).await
//! # }
//! ```

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};

use crate::database::pool::DbPool;
use crate::i18n;

/// 三端点共享的应用状态
#[derive(Clone)]
struct HealthState {
    pool: Arc<DbPool>,
    circuit_breaker: Option<Arc<crate::observability::CircuitBreaker>>,
    #[cfg(feature = "metrics")]
    metrics_collector: Option<Arc<crate::observability::MetricsCollector>>,
}

/// HTTP 健康端点生成器
///
/// 链式注入数据源后 `build()` 产出挂载 `/healthz` `/readyz` `/metrics`
/// 的 [`Router`]。池为必选数据源；熔断器与 metrics 采集器可选（未注入时
/// `/readyz` 不含熔断字段、`/metrics` 返回 404 说明）。注入方法跟随数据
/// 源类型的 feature 门控：熔断器随 `health-check`（本 feature 依赖），
/// 采集器随 `metrics`（未启用时 `/metrics` 恒 404）。
#[derive(Clone)]
pub struct HealthRouterBuilder {
    pool: Arc<DbPool>,
    circuit_breaker: Option<Arc<crate::observability::CircuitBreaker>>,
    #[cfg(feature = "metrics")]
    metrics_collector: Option<Arc<crate::observability::MetricsCollector>>,
}

impl HealthRouterBuilder {
    /// 以池为必选健康数据源开始构建
    ///
    /// 池快照（`health_snapshot`）是 readiness 判定与响应体的基础。
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self {
            pool,
            circuit_breaker: None,
            #[cfg(feature = "metrics")]
            metrics_collector: None,
        }
    }

    /// 注入熔断器：`Open` 态使 `/readyz` 判不就绪（覆盖池快照状态）
    pub fn with_circuit_breaker(
        mut self,
        breaker: Arc<crate::observability::CircuitBreaker>,
    ) -> Self {
        self.circuit_breaker = Some(breaker);
        self
    }

    /// 注入 metrics 采集器：`/metrics` 输出其 Prometheus 文本
    ///
    /// 未注入时 `/metrics` 显性 404（契约：不返回伪造的空指标文档）。
    /// 本方法在 `metrics` feature 下才存在（采集器类型的门控一致）。
    #[cfg(feature = "metrics")]
    pub fn with_metrics_collector(
        mut self,
        collector: Arc<crate::observability::MetricsCollector>,
    ) -> Self {
        self.metrics_collector = Some(collector);
        self
    }

    /// 产出挂载 `/healthz` `/readyz` `/metrics` 的 Router
    ///
    /// 监听与服务生命周期由消费方编排（`axum::serve(listener, router)`）。
    pub fn build(self) -> Router {
        let state = HealthState {
            pool: self.pool,
            circuit_breaker: self.circuit_breaker,
            #[cfg(feature = "metrics")]
            metrics_collector: self.metrics_collector,
        };
        Router::new()
            .route("/healthz", get(healthz))
            .route("/readyz", get(readyz))
            .route("/metrics", get(metrics))
            .with_state(state)
    }
}

/// `GET /healthz`：liveness——进程存活即 200，不探测池/下游
async fn healthz() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "alive" }))
}

/// `GET /readyz`：readiness——快照状态 + 熔断器共同判定
///
/// - 快照 `status` 为 healthy/degraded → 200（degraded = 有等待者/池满，
///   仍有服务能力，不应摘除实例）
/// - unhealthy 或未知状态 → 503（fail-closed）
/// - 熔断器 `Open` → 覆盖为 unhealthy → 503；其余熔断态如实入响应体
async fn readyz(State(state): State<HealthState>) -> impl IntoResponse {
    let mut snapshot = state.pool.health_snapshot().await;

    // 熔断态并入快照：Open 覆盖状态（下游拒绝 → 不接流量），closed/
    // half-open 只上报不干预判定
    if let Some(breaker) = &state.circuit_breaker {
        let breaker_state = breaker.state().await;
        snapshot["circuit_breaker"] = serde_json::Value::String(breaker_state.to_string());
        if breaker_state == crate::observability::CircuitBreakerState::Open {
            snapshot["status"] = serde_json::Value::String("unhealthy".to_string());
        }
    }

    if snapshot_is_ready(&snapshot) {
        (StatusCode::OK, Json(snapshot)).into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(snapshot)).into_response()
    }
}

/// `GET /metrics`：Prometheus 文本（未注入采集器 → 404 说明）
///
/// 全量导出是同步 CPU 密集操作（遍历全部标签 + 百分位排序 + 直方图
/// 统计），经 `spawn_blocking` 下放避免压在执行器线程上。
#[cfg(feature = "metrics")]
async fn metrics(State(state): State<HealthState>) -> impl IntoResponse {
    let Some(collector) = state.metrics_collector.clone() else {
        return (
            StatusCode::NOT_FOUND,
            i18n::t_simple("metrics-collector-not-configured"),
        )
            .into_response();
    };
    // join 失败（执行器关闭等极端场景）返回 503：Prometheus 将标记
    // scrape failed 触发既有告警管道，与"导出成功但无指标"可区分——
    // 若降级为 200 注释体，抓取端视为成功、序列经 staleness 窗口才消失，
    // 构成监控盲区；body 仍为注释行兜底（text exposition 解析不破）
    match tokio::task::spawn_blocking(move || collector.export_prometheus()).await {
        Ok(body) => (
            StatusCode::OK,
            [(
                axum::http::header::CONTENT_TYPE,
                "text/plain; version=0.0.4",
            )],
            body,
        )
            .into_response(),
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            format!(
                "# {}\n",
                i18n::t("metrics-export-join-failed", &[("error", e.to_string())])
            ),
        )
            .into_response(),
    }
}

/// `GET /metrics`（`metrics` feature 未启用）：端点恒 404 说明
///
/// 采集器类型在该门控下不存在，无状态可导出——独立的无参 handler
/// 避免借用不存在的字段。
#[cfg(not(feature = "metrics"))]
async fn metrics() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        i18n::t_simple("metrics-feature-not-enabled"),
    )
        .into_response()
}

/// readiness 判定（fail-closed）：仅 healthy/degraded 就绪
///
/// 快照缺失 `status` 或状态不在三态枚举内（含未来新增态）一律不就绪——
/// 判定面向"确认可用"，而非"未见异常"。
pub fn snapshot_is_ready(snapshot: &serde_json::Value) -> bool {
    matches!(
        snapshot.get("status").and_then(|s| s.as_str()),
        Some("healthy") | Some("degraded")
    )
}
