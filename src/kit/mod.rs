//! 基于 `trait-kit` 的统一能力管理模块
//!
//! 提供标准化的模块能力注册和发现机制，将核心模块的对外接口
//! 统一通过 `CapabilityKey` + `DbNexusKit` 门面暴露。
//!
//! # 架构
//!
//! - `keys` — `CapabilityKey` 定义，每个核心模块对应一个 key
//! - `DbNexusKit` — 类型安全的门面封装，提供 `provide_*` / `*` 方法

use std::sync::Arc;

use trait_kit::kit::{Kit, KitError};

/// 能力键（`CapabilityKey`）定义
pub mod keys;

pub use keys::*;

/// 统一能力管理门面
///
/// 基于 `Kit` 的封装，提供类型安全的模块能力注册和访问。
///
/// # 设计原则
///
/// - 每个模块的对外接口通过 `CapabilityKey` 标识
/// - 通过 `DbNexusKit` 统一注册和获取能力
/// - 支持运行时能力检查和热替换
///
/// # 示例
///
/// ```rust,ignore
/// use dbnexus::kit::{DbNexusKit, keys::*};
/// use std::sync::Arc;
///
/// let kit = DbNexusKit::new();
///
/// // 注册连接池
/// kit.provide_connection_pool(my_pool);
///
/// // 获取连接池能力
/// let pool = kit.connection_pool()?;
/// ```
pub struct DbNexusKit {
    inner: Kit,
}

impl DbNexusKit {
    /// 创建一个空的 Kit
    pub fn new() -> Self {
        Self {
            inner: Kit::new(),
        }
    }

    // ============================================================
    // Pool (foundation::pool::PoolConnector)
    // ============================================================

    /// 注册连接池连接器能力
    #[cfg(feature = "pool")]
    pub fn provide_pool(
        &self,
        pool: Arc<dyn crate::foundation::pool::PoolConnector>,
    ) -> Result<(), KitError> {
        self.inner.provide::<PoolCapKey>(pool)
    }

    /// 注册或替换连接池连接器能力
    #[cfg(feature = "pool")]
    pub fn replace_pool(&self, pool: Arc<dyn crate::foundation::pool::PoolConnector>) {
        self.inner.replace::<PoolCapKey>(pool)
    }

    /// 获取连接池连接器能力
    #[cfg(feature = "pool")]
    pub fn pool(&self) -> Result<Arc<dyn crate::foundation::pool::PoolConnector>, KitError> {
        self.inner.require::<PoolCapKey>()
    }

    /// 检查连接池连接器是否已注册
    #[cfg(feature = "pool")]
    pub fn has_pool(&self) -> bool {
        self.inner.contains::<PoolCapKey>()
    }

    // ============================================================
    // Permission (domain::permission::PermissionProvider)
    // ============================================================

    /// 注册权限提供者能力
    #[cfg(feature = "permission")]
    pub fn provide_permission(
        &self,
        provider: Arc<dyn crate::domain::permission::PermissionProvider>,
    ) -> Result<(), KitError> {
        self.inner.provide::<PermissionCapKey>(provider)
    }

    /// 注册或替换权限提供者能力
    #[cfg(feature = "permission")]
    pub fn replace_permission(&self, provider: Arc<dyn crate::domain::permission::PermissionProvider>) {
        self.inner.replace::<PermissionCapKey>(provider)
    }

    /// 获取权限提供者能力
    #[cfg(feature = "permission")]
    pub fn permission(
        &self,
    ) -> Result<Arc<dyn crate::domain::permission::PermissionProvider>, KitError> {
        self.inner.require::<PermissionCapKey>()
    }

    /// 检查权限提供者是否已注册
    #[cfg(feature = "permission")]
    pub fn has_permission(&self) -> bool {
        self.inner.contains::<PermissionCapKey>()
    }

    // ============================================================
    // ConnectionPool (database::pool::ConnectionPool)
    // ============================================================

    /// 注册数据库连接池能力
    pub fn provide_connection_pool(
        &self,
        pool: Arc<dyn crate::database::pool::ConnectionPool>,
    ) -> Result<(), KitError> {
        self.inner.provide::<ConnectionPoolCapKey>(pool)
    }

    /// 注册或替换数据库连接池能力
    pub fn replace_connection_pool(&self, pool: Arc<dyn crate::database::pool::ConnectionPool>) {
        self.inner.replace::<ConnectionPoolCapKey>(pool)
    }

    /// 获取数据库连接池能力
    pub fn connection_pool(&self) -> Result<Arc<dyn crate::database::pool::ConnectionPool>, KitError> {
        self.inner.require::<ConnectionPoolCapKey>()
    }

    /// 检查数据库连接池是否已注册
    pub fn has_connection_pool(&self) -> bool {
        self.inner.contains::<ConnectionPoolCapKey>()
    }

    // ============================================================
    // DatabaseSession (database::pool::DatabaseSession)
    // ============================================================

    /// 注册数据库会话能力
    pub fn provide_database_session(
        &self,
        session: Arc<dyn crate::database::pool::DatabaseSession>,
    ) -> Result<(), KitError> {
        self.inner.provide::<DatabaseSessionCapKey>(session)
    }

    /// 注册或替换数据库会话能力
    pub fn replace_database_session(&self, session: Arc<dyn crate::database::pool::DatabaseSession>) {
        self.inner.replace::<DatabaseSessionCapKey>(session)
    }

    /// 获取数据库会话能力
    pub fn database_session(&self) -> Result<Arc<dyn crate::database::pool::DatabaseSession>, KitError> {
        self.inner.require::<DatabaseSessionCapKey>()
    }

    /// 检查数据库会话是否已注册
    pub fn has_database_session(&self) -> bool {
        self.inner.contains::<DatabaseSessionCapKey>()
    }

    // ============================================================
    // Metrics (observability::metrics::MetricsCollectorTrait)
    // ============================================================

    /// 注册指标收集器能力
    #[cfg(feature = "metrics")]
    pub fn provide_metrics_collector(
        &self,
        collector: Arc<dyn crate::observability::metrics::MetricsCollectorTrait>,
    ) -> Result<(), KitError> {
        self.inner.provide::<MetricsCapKey>(collector)
    }

    /// 注册或替换指标收集器能力
    #[cfg(feature = "metrics")]
    pub fn replace_metrics_collector(
        &self,
        collector: Arc<dyn crate::observability::metrics::MetricsCollectorTrait>,
    ) {
        self.inner.replace::<MetricsCapKey>(collector)
    }

    /// 获取指标收集器能力
    #[cfg(feature = "metrics")]
    pub fn metrics_collector(
        &self,
    ) -> Result<Arc<dyn crate::observability::metrics::MetricsCollectorTrait>, KitError> {
        self.inner.require::<MetricsCapKey>()
    }

    /// 检查指标收集器是否已注册
    #[cfg(feature = "metrics")]
    pub fn has_metrics_collector(&self) -> bool {
        self.inner.contains::<MetricsCapKey>()
    }

    // ============================================================
    // Health (observability::health::HealthChecker)
    // ============================================================

    /// 注册健康检查器能力
    #[cfg(feature = "health-check")]
    pub fn provide_health_checker(
        &self,
        checker: Arc<crate::observability::health::HealthChecker>,
    ) -> Result<(), KitError> {
        self.inner.provide::<HealthCapKey>(checker)
    }

    /// 注册或替换健康检查器能力
    #[cfg(feature = "health-check")]
    pub fn replace_health_checker(&self, checker: Arc<crate::observability::health::HealthChecker>) {
        self.inner.replace::<HealthCapKey>(checker)
    }

    /// 获取健康检查器能力
    #[cfg(feature = "health-check")]
    pub fn health_checker(&self) -> Result<Arc<crate::observability::health::HealthChecker>, KitError> {
        self.inner.require::<HealthCapKey>()
    }

    /// 检查健康检查器是否已注册
    #[cfg(feature = "health-check")]
    pub fn has_health_checker(&self) -> bool {
        self.inner.contains::<HealthCapKey>()
    }

    // ============================================================
    // 底层 Kit 访问
    // ============================================================

    /// 访问底层的 `Kit`
    pub fn as_inner(&self) -> &Kit {
        &self.inner
    }

    /// 消费自身并返回底层的 `Kit`
    pub fn into_inner(self) -> Kit {
        self.inner
    }
}

impl Default for DbNexusKit {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for DbNexusKit {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl std::fmt::Debug for DbNexusKit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DbNexusKit").finish()
    }
}
