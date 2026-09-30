// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 大文件拆分：自 session.rs 按职责纯移动的 impl 块（行为不变）。

use super::*;

impl Session {
    /// 记录查询指标
    #[cfg(all(feature = "metrics", feature = "permission"))]
    pub(super) fn record_query_metrics(&self, query_type: &str, duration: Duration, success: bool) {
        if let Some(metrics) = &self.metrics_collector {
            metrics.record_query(query_type, duration, success, None);
        }
    }

    /// 记录查询指标（无 metrics 特性）
    #[cfg(all(not(feature = "metrics"), feature = "permission"))]
    pub(super) fn record_query_metrics(
        &self,
        _query_type: &str,
        _duration: Duration,
        _success: bool,
    ) {
        // No-op when metrics feature is disabled
    }

    /// 记录 `execute_raw` 查询指标（含慢查询检测）。
    ///
    /// 将查询耗时经 `MetricsCollector::record_query` 录入指标收集器，
    /// 内部自动比对 `SlowQueryConfig` 阈值并记录慢查询事件。
    /// `metrics` feature 未启用时此方法不存在（零开销）。
    #[cfg(all(feature = "metrics", feature = "sql-parser"))]
    pub(super) fn record_execute_metrics(&self, start: std::time::Instant, success: bool) {
        if let Some(metrics) = &self.metrics_collector {
            metrics.record_query("execute_raw", start.elapsed(), success, None);
        }
    }

    /// 查询缓存——检查 `cache_provider` 是否有缓存的查询结果。
    ///
    /// 返回 `Some(bytes)` 表示缓存命中，`None` 表示未命中。
    /// 仅在缓存能力可用（`cache`/`oxcache-integration`）且已注入 `cache_provider` 时有效。
    #[cfg(feature = "cache-available")]
    pub async fn query_cache_get(&self, key: &str) -> Option<Vec<u8>> {
        // ArcSwapOption::load() 返回 Guard，clone 内部 Arc 后立即释放 Guard，
        // 避免跨 .await 持有 Guard。
        let provider = self.pool_inner.cache_provider.load().clone()?;
        provider.get(key).await.ok().flatten()
    }

    /// 查询缓存——将查询结果存入 `cache_provider`。
    ///
    /// TTL 取自 `CacheConfig.default_ttl`。
    #[cfg(feature = "cache-available")]
    pub async fn query_cache_set(&self, key: &str, value: Vec<u8>) {
        let provider = match self.pool_inner.cache_provider.load().clone() {
            Some(p) => p,
            None => return,
        };
        let ttl = std::time::Duration::from_secs(self.pool_inner.config.cache_config.default_ttl);
        let _ = provider.set(key, value, Some(ttl)).await;
    }

    /// 记录查询指标并标记写操作
    ///
    /// 统一 execute 流程中 metrics 记录与 mark_write 逻辑，
    /// 避免在多个 cfg 分支中重复实现。
    #[cfg(feature = "permission")]
    pub(super) async fn record_metrics_and_mark_write(
        &self,
        action: &PermissionAction,
        start: Instant,
    ) {
        let duration = start.elapsed();
        self.record_query_metrics(&format!("{:?}", action), duration, true);
        if is_write_action(action) {
            self.mark_write().await;
        }
    }

    /// 检查表级权限
    ///
    /// 此方法为 ORM 操作提供权限检查，确保所有实体操作都经过权限验证
    pub async fn check_table_permission(
        &self,
        _table_name: &str,
        _operation: &str,
    ) -> DbResult<()> {
        #[cfg(feature = "permission")]
        {
            let action = match _operation {
                "INSERT" => PermissionAction::Insert,
                "SELECT" => PermissionAction::Select,
                "UPDATE" => PermissionAction::Update,
                "DELETE" => PermissionAction::Delete,
                _ => {
                    return Err(DbError::Permission(i18n::t(
                        "session-unknown-operation",
                        &[("operation", _operation.to_string())],
                    )));
                }
            };

            // Admin 角色绕过权限检查
            // vuln-0001 修复：admin bypass 仍记录审计事件（进程级审计环）以保留审计链
            if self.is_admin {
                audit_admin_bypass(&self.role, _table_name, &action);
            } else {
                super::check_table_or_error(&self.permission_ctx, _table_name, &action).await?;
            }
        }
        Ok(())
    }

    /// 记录指标
    #[cfg(feature = "metrics")]
    pub fn record_metric(&self, operation: &str, table_name: &str, success: bool) {
        if let Some(metrics) = &self.metrics_collector {
            // 使用表名的哈希值作为 bytes 参数
            let bytes = Some(table_name.len() as u64);
            metrics.record_query(
                operation,
                std::time::Duration::from_millis(0),
                success,
                bytes,
            );
        }
    }
}
