// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 结构化健康导出（`health-check` feature）
//!
//! `DbPool::health_snapshot()` 汇聚既有健康数据为单个 JSON 文档，供
//! HTTP `/healthz`、trait-kit 健康聚合等上层直接消费：
//!
//! - **池饱和度**：复用 `DbPool::status()`（total/active/idle/wait/借用计数）
//!   并派生 `saturation`（活跃/总连接，零连接池取 1.0 满载语义）
//! - **副本状态**：经 `set_replica_health_provider` 注入的提供者输出
//!   （副本路由可接入；未注入时为空数组）
//! - **慢查询计数**：metrics collector 注入后来自 `MetricsCollector`
//!   慢查询环（未注入时计数为 0）
//!
//! ```rust,no_run
//! # async fn example(pool: &dbnexus::DbPool) {
//! let snapshot = pool.health_snapshot().await;
//! // {"status":"healthy","pool":{...,"saturation":0.4},"slow_queries":{...},"replicas":[...]}
//! println!("{}", serde_json::to_string(&snapshot).unwrap());
//! # }
//! ```

use std::sync::Arc;

use crate::database::pool::DbPool;

/// 副本健康状态提供者
///
/// 返回的 JSON 对象数组将原样进入 `health_snapshot().replicas`；
/// 副本负载均衡路由可注入其真实副本状态。
pub type ReplicaHealthProvider = Arc<dyn Fn() -> Vec<serde_json::Value> + Send + Sync>;

impl DbPool {
    /// 注册/清除副本健康状态提供者（运行时可重设，供副本路由接入）
    ///
    /// `None` 清除既有提供者（快照回到空 `replicas`）。
    #[cfg(feature = "health-check")]
    pub async fn set_replica_health_provider(&self, provider: Option<ReplicaHealthProvider>) {
        *self
            .inner
            .replica_health_provider
            .write()
            .expect("replica_health_provider lock") = provider;
    }

    /// 结构化健康快照：池饱和度 + 副本状态 + 慢查询计数
    ///
    /// 返回的 JSON 供 HTTP 健康端点 / kit 健康聚合直接输出；
    /// `status` 语义与 `AsyncHealthCheck for DbNexusModule` 对齐：
    /// 零连接 → `unhealthy`，有等待者或池满 → `degraded`，其余 → `healthy`。
    #[cfg(feature = "health-check")]
    pub async fn health_snapshot(&self) -> serde_json::Value {
        let st = self.status();

        // 池饱和度：活跃/总连接；零连接池无服务能力，取 1.0（满载语义）
        let saturation = if st.total == 0 {
            1.0
        } else {
            (st.active as f64 / st.total as f64).clamp(0.0, 1.0)
        };

        // 状态判定（与 kit 模块健康映射同口径）
        let status = if st.total == 0 {
            "unhealthy"
        } else if st.wait_count > 0 || st.active >= st.total {
            "degraded"
        } else {
            "healthy"
        };

        // 慢查询计数（metrics collector 注入路径；未注入为 0）
        #[cfg(feature = "metrics")]
        let slow_queries = {
            let collector = self
                .inner
                .metrics_collector
                .read()
                .expect("metrics_collector lock")
                .clone();
            if let Some(collector) = collector {
                let cfg = collector.slow_query_config_snapshot();
                serde_json::json!({
                    "count": collector.slow_queries().len(),
                    "threshold_ms": cfg.threshold_ms,
                    "enabled": cfg.enabled,
                })
            } else {
                serde_json::json!({ "count": 0 })
            }
        };
        #[cfg(not(feature = "metrics"))]
        let slow_queries = serde_json::json!({ "count": 0 });

        // 副本状态（提供者注入；短临界区，锁内无 await）
        let replicas = {
            let provider = self
                .inner
                .replica_health_provider
                .read()
                .expect("replica_health_provider lock");
            match provider.as_ref() {
                Some(p) => serde_json::Value::Array(p()),
                None => serde_json::Value::Array(Vec::new()),
            }
        };

        serde_json::json!({
            "status": status,
            "pool": {
                "total": st.total,
                "active": st.active,
                "idle": st.idle,
                "wait_count": st.wait_count,
                "max_waiters": st.max_waiters,
                "borrow_count": st.borrow_count,
                "max_active": st.max_active,
                "max_connections": self.inner.config.pool_config.max_connections,
                "saturation": saturation,
            },
            "slow_queries": slow_queries,
            "replicas": replicas,
        })
    }

    /// 注入/清除 metrics collector（池查询指标与慢查询记录的数据源）
    ///
    /// 既有 `Session` 执行路径已从 `pool_inner.metrics_collector` 读取指标采集器，
    /// 此 setter 打通运行时注入（与 `set_permission_config` 同款模式），使
    /// `health_snapshot` 的慢查询计数反映真实数据。
    #[cfg(all(feature = "health-check", feature = "metrics"))]
    pub async fn set_metrics_collector(&self, collector: Option<Arc<crate::observability::MetricsCollector>>) {
        *self.inner.metrics_collector.write().expect("metrics_collector lock") = collector;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // saturation 语义：零连接池满载、活跃占比 clamp
    #[test]
    fn test_saturation_semantics_reference() {
        // 与 health_snapshot 内联计算保持一致的两条语义
        let total = 0u32;
        let saturation = if total == 0 { 1.0 } else { 0.0 };
        assert_eq!(saturation, 1.0);

        let (total, active) = (5u32, 2u32);
        let saturation = (active as f64 / total as f64).clamp(0.0, 1.0);
        assert!((saturation - 0.4).abs() < f64::EPSILON);
    }
}
