// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 大文件拆分：自 db_pool.rs 按职责纯移动的 impl 块（行为不变）。

use super::*;

impl DbPool {
    /// 记录连接获取延迟指标（metrics feature 启用时有效，否则编译消除）
    #[cfg(feature = "metrics")]
    #[inline]
    pub(super) fn record_acquire_duration(&self, start: Instant) {
        if let Some(collector) = self.inner.metrics_collector.read().expect("metrics_collector lock").clone() {
            collector.record_connection_acquire_duration(start.elapsed());
        }
    }

    #[cfg(not(feature = "metrics"))]
    #[inline]
    pub(super) fn record_acquire_duration(&self, _start: Instant) {}

    /// 记录连接获取超时指标
    #[cfg(feature = "metrics")]
    #[inline]
    pub(super) fn record_acquire_timeout(&self, start: Instant) {
        let elapsed_ms = start.elapsed().as_millis() as u64;
        if let Some(collector) = self.inner.metrics_collector.read().expect("metrics_collector lock").clone() {
            collector.record_connection_timeout_level(elapsed_ms);
        }
    }

    #[cfg(not(feature = "metrics"))]
    #[inline]
    pub(super) fn record_acquire_timeout(&self, _start: Instant) {}

    /// 更新最大等待者计数（使用 CAS 避免竞态条件）
    pub(super) fn update_max_waiters(&self, current_waiters: u32) {
        let mut current = self.inner.max_waiters.load(Ordering::Acquire);
        while current_waiters > current {
            match self.inner.max_waiters.compare_exchange(
                current,
                current_waiters,
                Ordering::SeqCst,
                Ordering::Acquire,
            ) {
                Ok(_) => return,
                Err(observed) => {
                    current = observed;
                }
            }
        }
    }

    /// 归还连接到池中
    ///
    /// 将使用完毕的连接归还到空闲连接队列。
    /// 如果空闲队列已满（达到最大连接数），则丢弃该连接。
    /// 归还后会通知一个等待的请求者有新连接可用。
    ///
    /// # Arguments
    ///
    /// * `conn` - 要归还的数据库连接
    ///
    /// # Note
    ///
    /// 此方法会归还信号量许可，确保连接池可以继续接受新的连接请求。
    /// 使用 tokio::spawn 在后台执行异步操作，避免阻塞调用者。
    /// 与迁移路径（auto-migrate））
    #[cfg(any(feature = "auto-migrate", feature = "postgres"))]
    pub(crate) fn release_connection(&self, conn: DbConnection) {
        DbPoolInner::release_connection(&self.inner, conn);
    }

    /// 获取连接池状态
    ///
    /// 返回当前连接池的统计信息，包括总连接数、活跃连接数和空闲连接数。
    ///
    /// # Returns
    ///
    /// 连接池状态信息
    ///
    /// # Example
    ///
    /// ```rust
    /// use dbnexus::DbPool;
    ///
    /// # async fn example(pool: &DbPool) {
    /// let status = pool.status();
    /// println!("Total: {}, Active: {}, Idle: {}",
    ///     status.total, status.active, status.idle);
    /// # }
    /// ```
    pub fn status(&self) -> PoolStatus {
        let total = self.inner.total_count.load(Ordering::SeqCst);
        let active = self.inner.active_count.load(Ordering::SeqCst);
        let wait_count = self.inner.wait_count.load(Ordering::SeqCst);
        let max_waiters = self.inner.max_waiters.load(Ordering::SeqCst);
        let borrow_count = self.inner.borrow_count.load(Ordering::SeqCst);
        let max_active = self.inner.max_active.load(Ordering::SeqCst);

        PoolStatus {
            total,
            active,
            idle: total.saturating_sub(active),
            wait_count,
            max_waiters,
            borrow_count,
            max_active,
        }
    }

    /// 获取连接池告警指标
    ///
    /// 从 metrics_collector（如果启用）获取告警相关指标，包括：
    /// - `slow_acquires`：获取时长超过 1s 的次数
    /// - `timeout_errors`：获取超时总次数（warn + error + critical 之和）
    /// - `critical_timeouts`：严重超时（≥10s）次数
    /// - `wait_count`：当前正在等待获取连接的协程数
    /// - `max_waiters`：历史最大并发等待者峰值
    ///
    /// # Returns
    ///
    /// 连接池告警指标（始终返回有效值，metrics 未启用时所有计数为 0）
    #[cfg(feature = "metrics")]
    pub fn pool_metrics(&self) -> PoolMetrics {
        let wait_count = self.inner.wait_count.load(Ordering::SeqCst);
        let max_waiters = self.inner.max_waiters.load(Ordering::SeqCst);
        let collector = self
            .inner
            .metrics_collector
            .read()
            .expect("metrics_collector lock")
            .clone();
        if let Some(collector) = collector {
            let stats = collector.connection_acquire_stats();
            PoolMetrics {
                slow_acquires: stats.slow_acquires,
                timeout_errors: stats.timeout_warn + stats.timeout_error + stats.timeout_critical,
                critical_timeouts: stats.timeout_critical,
                wait_count,
                max_waiters,
            }
        } else {
            PoolMetrics {
                slow_acquires: 0,
                timeout_errors: 0,
                critical_timeouts: 0,
                wait_count,
                max_waiters,
            }
        }
    }

    /// 获取配置
    ///
    /// 返回连接池的配置引用。
    ///
    /// # Returns
    ///
    /// 连接池配置的引用
    ///
    /// # Example
    ///
    /// ```rust
    /// use dbnexus::DbPool;
    ///
    /// # async fn example(pool: &DbPool) {
    /// let config = pool.config();
    /// println!("Max connections: {}", config.pool_config.max_connections);
    /// # }
    /// ```
    pub fn config(&self) -> &DbConfig {
        &self.inner.config
    }

    /// 运行自动迁移
    ///
    /// 如果配置中启用了 `auto_migrate`，此方法会在连接池创建后自动执行迁移。
    /// 也可以手动调用此方法来执行迁移。
    ///
    /// # Returns
    ///
    /// 成功应用的迁移数量
    #[cfg(feature = "auto-migrate")]
    pub async fn run_auto_migrate(&self) -> Result<u32, DbError> {
        if let Some(ref migrations_dir) = self.inner.config.migrations_dir {
            self.run_migrations(migrations_dir).await
        } else {
            Ok(0)
        }
    }

    /// 手动运行迁移
    ///
    /// # Arguments
    ///
    /// * `migrations_dir` - 迁移文件目录路径
    ///
    /// # Returns
    ///
    /// 成功应用的迁移数量
    #[cfg(feature = "auto-migrate")]
    pub async fn run_migrations(&self, migrations_dir: &std::path::Path) -> Result<u32, DbError> {
        use crate::database::MigrationExecutor;

        let db_type = self
            .inner
            .config
            .database_type()
            .map_err(|e| DbError::Config(e.to_string()))?;

        // 获取一个连接来执行迁移
        let connection = self.acquire_connection().await?;

        // 从 DbConnection 提取 SeaORM 连接用于迁移执行器
        let connection_for_migration = connection.as_sea_orm()?.clone();

        let mut executor = MigrationExecutor::new(connection_for_migration, db_type);

        let applied = executor.run_migrations(migrations_dir).await?;

        // 归还连接到池中
        self.release_connection(connection);

        Ok(applied)
    }

}
