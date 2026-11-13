// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 外部连接记账：池外物理连接的 RAII 登记（`track_external_connection`）
//!
//! 背景：外层池的每个池单元由 `sea_orm::Database::connect(url)` 创建，本身
//! 是一个内层 sqlx 池。外部调用方（如 crawlrs 的 PG advisory lock 守卫）经
//! `Session.connection()` 拿到 SeaORM 连接后自行 `begin()`，会从内层池另取
//! 一条物理连接承载事务——这条连接不经外层 `acquire_connection`，外层
//! `active/total` 计数不可见。本模块提供 RAII 记账把它纳入 `PoolStatus.external`。
//!
//! 语义（消费方必读）：
//! 1. 记账不占信号量许可，不改变 `max_connections` 配额；池外物理连接的
//!    总上限由数据库侧（如 PG `max_connections`）兜底。
//! 2. `external` 不并入 `total`/`active`，`idle = total - active` 与饱和度
//!    口径不变；真实物理占用 = 外层指标 + external（仍非 PG 会话总数——
//!    内层 sqlx 池自持的空闲连接同样不计入）。
//! 3. 守卫 Drop 即注销；无手动释放路径，计数漂移在构造上不可能。

use std::sync::Arc;

use super::DbPoolInner;

/// 外部连接记账守卫：持有时计数为 +1，Drop 时 -1
///
/// 不派生 `Clone`/`Copy`，不暴露内部字段，无手动 `release()`——注销唯一
/// 路径是 Drop（含 panic 展开与异步任务取消展开），漏配对不可能。
/// 持有 `Arc<DbPoolInner>` 而非借用：守卫可跨 `.await` 点与任意结构体持有，
/// 不被钉死在 `&DbPool` 生存期上。
pub struct ExternalConnectionGuard {
    inner: Arc<DbPoolInner>,
}

impl Drop for ExternalConnectionGuard {
    fn drop(&mut self) {
        // 防御性下穿保护：正常路径（track 后持守卫）计数必 >0，checked_sub
        // 不会失败；若因未来代码缺陷在 0 值上注销，no-op 优于 u32 回绕。
        // Err 仅在闭包返回 None 时出现，此处为预期的下穿拦截，显式忽略。
        let _ = self.inner.external_count.fetch_update(
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
            |v| v.checked_sub(1),
        );
    }
}

impl super::DbPool {
    /// 登记一条池外物理连接（外部连接记账）
    ///
    /// 返回 RAII 守卫：持有时 `status().external` 为 +1，Drop 时 -1。
    /// 仅记账，不占信号量许可、不影响 `acquire_connection` 配额；
    /// `external` 不并入 `total`/`active`，饱和度口径不变。
    ///
    /// 典型场景：调用方经 `Session.connection()` 拿到 SeaORM 连接后自行
    /// `begin()` 事务长期持锁（如 advisory lock），该物理连接在外层池
    /// 计数之外——登记后监控可读出真实物理占用。
    pub fn track_external_connection(&self) -> ExternalConnectionGuard {
        self.inner
            .external_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        ExternalConnectionGuard {
            inner: Arc::clone(&self.inner),
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "sqlite")]
    use super::super::{DbConfig, DbPool};

    /// 守卫生命周期：track +1 / Drop -1，多条并存按现值递减
    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_track_external_connection_guard_lifecycle() {
        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            pool_config: crate::foundation::PoolConfig {
                max_connections: 4,
                min_connections: 0,
                ..Default::default()
            },
            ..Default::default()
        };
        let pool = DbPool::with_config(config).await.expect("pool");

        assert_eq!(pool.status().external, 0, "initial external count is zero");

        let guard = pool.track_external_connection();
        assert_eq!(pool.status().external, 1, "track increments by one");

        drop(guard);
        assert_eq!(pool.status().external, 0, "guard Drop decrements by one");

        let g1 = pool.track_external_connection();
        let g2 = pool.track_external_connection();
        assert_eq!(pool.status().external, 2, "two concurrent tracks");

        drop(g1);
        assert_eq!(pool.status().external, 1, "one guard still held");

        drop(g2);
        assert_eq!(pool.status().external, 0, "all guards released");
    }
}
