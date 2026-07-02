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
        Self { inner: Kit::new() }
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
    pub fn permission(&self) -> Result<Arc<dyn crate::domain::permission::PermissionProvider>, KitError> {
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
    pub fn replace_metrics_collector(&self, collector: Arc<dyn crate::observability::metrics::MetricsCollectorTrait>) {
        self.inner.replace::<MetricsCapKey>(collector)
    }

    /// 获取指标收集器能力
    #[cfg(feature = "metrics")]
    pub fn metrics_collector(&self) -> Result<Arc<dyn crate::observability::metrics::MetricsCollectorTrait>, KitError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Mock 实现：ConnectionPool =====

    struct MockConnectionPool {
        status_value: crate::database::pool::PoolStatus,
        config_value: crate::foundation::config::DbConfig,
    }

    #[async_trait::async_trait]
    impl crate::database::pool::ConnectionPool for MockConnectionPool {
        async fn get_session(&self, _role: &str) -> crate::foundation::error::DbResult<crate::database::pool::Session> {
            Err(crate::foundation::error::DbError::new(sea_orm::DbErr::Custom(
                "mock not implemented".to_string(),
            )))
        }

        fn status(&self) -> crate::database::pool::PoolStatus {
            self.status_value.clone()
        }

        fn config(&self) -> &crate::foundation::config::DbConfig {
            &self.config_value
        }
    }

    // ===== Mock 实现：DatabaseSession =====

    struct MockDatabaseSession {
        role_value: String,
    }

    #[async_trait::async_trait]
    impl crate::database::pool::DatabaseSession for MockDatabaseSession {
        async fn execute(&self, _sql: &str) -> crate::foundation::error::DbResult<sea_orm::ExecResult> {
            Err(crate::foundation::error::DbError::new(sea_orm::DbErr::Custom(
                "mock not implemented".to_string(),
            )))
        }

        async fn execute_raw(&self, _sql: &str) -> crate::foundation::error::DbResult<sea_orm::ExecResult> {
            Err(crate::foundation::error::DbError::new(sea_orm::DbErr::Custom(
                "mock not implemented".to_string(),
            )))
        }

        async fn execute_raw_ddl(&self, _sql: &str) -> crate::foundation::error::DbResult<sea_orm::ExecResult> {
            Err(crate::foundation::error::DbError::new(sea_orm::DbErr::Custom(
                "mock not implemented".to_string(),
            )))
        }

        async fn begin_transaction(&self) -> crate::foundation::error::DbResult<()> {
            Ok(())
        }

        async fn commit(&self) -> crate::foundation::error::DbResult<()> {
            Ok(())
        }

        async fn rollback(&self) -> crate::foundation::error::DbResult<()> {
            Ok(())
        }

        fn role(&self) -> &str {
            &self.role_value
        }

        async fn is_in_transaction(&self) -> bool {
            false
        }
    }

    fn make_mock_pool() -> MockConnectionPool {
        MockConnectionPool {
            status_value: crate::database::pool::PoolStatus {
                total: 10,
                active: 5,
                idle: 5,
                wait_count: 0,
                max_waiters: 0,
                borrow_count: 0,
                max_active: 10,
            },
            config_value: crate::foundation::config::DbConfig::default(),
        }
    }

    // ===== DbNexusKit 基础测试 =====

    #[test]
    fn test_kit_new_creates_empty_kit() {
        let kit = DbNexusKit::new();
        assert!(!kit.has_connection_pool());
        assert!(!kit.has_database_session());
    }

    #[test]
    fn test_kit_default_equals_new() {
        let kit1 = DbNexusKit::new();
        let kit2 = DbNexusKit::default();
        assert!(!kit1.has_connection_pool());
        assert!(!kit2.has_connection_pool());
    }

    #[test]
    fn test_kit_clone_preserves_registrations() {
        let kit = DbNexusKit::new();
        let mock = make_mock_pool();
        kit.provide_connection_pool(Arc::new(mock)).unwrap();

        let cloned = kit.clone();
        assert!(cloned.has_connection_pool());
        assert!(kit.has_connection_pool());
    }

    #[test]
    fn test_kit_debug_format() {
        let kit = DbNexusKit::new();
        let debug_str = format!("{:?}", kit);
        assert!(debug_str.contains("DbNexusKit"));
    }

    // ===== ConnectionPool 能力测试 =====

    #[test]
    fn test_provide_connection_pool() {
        let kit = DbNexusKit::new();
        let mock = make_mock_pool();
        assert!(!kit.has_connection_pool());

        kit.provide_connection_pool(Arc::new(mock)).unwrap();
        assert!(kit.has_connection_pool());
    }

    #[test]
    fn test_connection_pool_get() {
        let kit = DbNexusKit::new();
        let mock = make_mock_pool();
        kit.provide_connection_pool(Arc::new(mock)).unwrap();

        let pool = kit.connection_pool();
        assert!(pool.is_ok());
        let pool = pool.unwrap();
        assert_eq!(pool.status().total, 10);
        assert_eq!(pool.status().active, 5);
    }

    #[test]
    fn test_connection_pool_get_when_not_registered() {
        let kit = DbNexusKit::new();
        let result = kit.connection_pool();
        assert!(result.is_err());
    }

    #[test]
    fn test_replace_connection_pool() {
        let kit = DbNexusKit::new();
        let mock1 = MockConnectionPool {
            status_value: crate::database::pool::PoolStatus {
                total: 5,
                active: 2,
                idle: 3,
                wait_count: 0,
                max_waiters: 0,
                borrow_count: 0,
                max_active: 5,
            },
            config_value: crate::foundation::config::DbConfig::default(),
        };
        let mock2 = MockConnectionPool {
            status_value: crate::database::pool::PoolStatus {
                total: 20,
                active: 10,
                idle: 10,
                wait_count: 0,
                max_waiters: 0,
                borrow_count: 0,
                max_active: 20,
            },
            config_value: crate::foundation::config::DbConfig::default(),
        };

        kit.provide_connection_pool(Arc::new(mock1)).unwrap();
        assert_eq!(kit.connection_pool().unwrap().status().total, 5);

        kit.replace_connection_pool(Arc::new(mock2));
        assert_eq!(kit.connection_pool().unwrap().status().total, 20);
    }

    #[test]
    fn test_provide_connection_pool_duplicate_fails() {
        let kit = DbNexusKit::new();
        let mock1 = make_mock_pool();
        let mock2 = make_mock_pool();

        kit.provide_connection_pool(Arc::new(mock1)).unwrap();
        let result = kit.provide_connection_pool(Arc::new(mock2));
        assert!(result.is_err());
    }

    // ===== DatabaseSession 能力测试 =====

    #[test]
    fn test_provide_database_session() {
        let kit = DbNexusKit::new();
        let mock = MockDatabaseSession {
            role_value: "admin".to_string(),
        };
        assert!(!kit.has_database_session());

        kit.provide_database_session(Arc::new(mock)).unwrap();
        assert!(kit.has_database_session());
    }

    #[test]
    fn test_database_session_get() {
        let kit = DbNexusKit::new();
        let mock = MockDatabaseSession {
            role_value: "user".to_string(),
        };
        kit.provide_database_session(Arc::new(mock)).unwrap();

        let session = kit.database_session();
        assert!(session.is_ok());
        assert_eq!(session.unwrap().role(), "user");
    }

    #[test]
    fn test_database_session_get_when_not_registered() {
        let kit = DbNexusKit::new();
        let result = kit.database_session();
        assert!(result.is_err());
    }

    #[test]
    fn test_replace_database_session() {
        let kit = DbNexusKit::new();
        let mock1 = MockDatabaseSession {
            role_value: "user1".to_string(),
        };
        let mock2 = MockDatabaseSession {
            role_value: "user2".to_string(),
        };

        kit.provide_database_session(Arc::new(mock1)).unwrap();
        assert_eq!(kit.database_session().unwrap().role(), "user1");

        kit.replace_database_session(Arc::new(mock2));
        assert_eq!(kit.database_session().unwrap().role(), "user2");
    }

    // ===== as_inner / into_inner 测试 =====

    #[test]
    fn test_as_inner_returns_reference() {
        let kit = DbNexusKit::new();
        let _inner: &Kit = kit.as_inner();
    }

    #[test]
    fn test_into_inner_consumes_kit() {
        let kit = DbNexusKit::new();
        let mock = make_mock_pool();
        kit.provide_connection_pool(Arc::new(mock)).unwrap();

        let inner = kit.into_inner();
        assert!(inner.contains::<ConnectionPoolCapKey>());
    }

    // ===== Permission 能力测试（feature-gated） =====

    #[cfg(feature = "permission")]
    #[test]
    fn test_permission_capability_lifecycle() {
        use crate::domain::permission::{PermissionAction, PermissionError, PermissionProvider, RolePolicy};

        struct MockPermissionProvider;

        #[async_trait::async_trait]
        impl crate::domain::permission::PermissionChecker for MockPermissionProvider {
            async fn check(
                &self,
                _role: &str,
                _table: &str,
                _action: PermissionAction,
            ) -> Result<bool, PermissionError> {
                Ok(true)
            }
        }

        #[async_trait::async_trait]
        impl crate::domain::permission::PolicyManager for MockPermissionProvider {
            async fn get_policy(&self, _role: &str) -> Result<Option<RolePolicy>, PermissionError> {
                Ok(None)
            }

            async fn refresh(&self) -> Result<(), PermissionError> {
                Ok(())
            }
        }

        #[async_trait::async_trait]
        impl crate::domain::permission::PermissionLifecycle for MockPermissionProvider {
            async fn health_check(&self) -> anyhow::Result<()> {
                Ok(())
            }

            async fn shutdown(&self) {}
        }

        impl PermissionProvider for MockPermissionProvider {}

        let kit = DbNexusKit::new();
        assert!(!kit.has_permission());

        kit.provide_permission(Arc::new(MockPermissionProvider)).unwrap();
        assert!(kit.has_permission());

        let provider = kit.permission();
        assert!(provider.is_ok());

        kit.replace_permission(Arc::new(MockPermissionProvider));
        assert!(kit.has_permission());
    }

    #[cfg(feature = "permission")]
    #[test]
    fn test_permission_get_when_not_registered_returns_error() {
        let kit = DbNexusKit::new();
        let result = kit.permission();
        assert!(result.is_err());
    }

    // ===== Metrics 能力测试（feature-gated） =====

    #[cfg(feature = "metrics")]
    #[test]
    fn test_metrics_capability_lifecycle() {
        use crate::observability::metrics::MockMetrics;

        let kit = DbNexusKit::new();
        assert!(!kit.has_metrics_collector());

        kit.provide_metrics_collector(Arc::new(MockMetrics::new())).unwrap();
        assert!(kit.has_metrics_collector());

        let collector = kit.metrics_collector();
        assert!(collector.is_ok());
        // 验证 trait object 可调用
        let collector = collector.unwrap();
        collector.record_query(std::time::Duration::from_millis(10));
        assert_eq!(collector.query_stats().count, 0); // MockMetrics 是 no-op

        kit.replace_metrics_collector(Arc::new(MockMetrics::new()));
        assert!(kit.has_metrics_collector());
    }

    #[cfg(feature = "metrics")]
    #[test]
    fn test_metrics_get_when_not_registered_returns_error() {
        let kit = DbNexusKit::new();
        assert!(!kit.has_metrics_collector());
        let result = kit.metrics_collector();
        assert!(result.is_err());
    }

    #[cfg(feature = "metrics")]
    #[test]
    fn test_metrics_provide_duplicate_fails() {
        use crate::observability::metrics::MockMetrics;

        let kit = DbNexusKit::new();
        kit.provide_metrics_collector(Arc::new(MockMetrics::new())).unwrap();
        let result = kit.provide_metrics_collector(Arc::new(MockMetrics::new()));
        assert!(result.is_err());
    }

    // ===== HealthChecker 能力测试（feature-gated） =====

    #[cfg(feature = "health-check")]
    #[test]
    fn test_health_checker_capability_lifecycle() {
        use crate::observability::health::HealthChecker;

        let kit = DbNexusKit::new();
        assert!(!kit.has_health_checker());

        let checker = Arc::new(HealthChecker::new(1000));
        kit.provide_health_checker(checker).unwrap();
        assert!(kit.has_health_checker());

        let checker = kit.health_checker();
        assert!(checker.is_ok());
        let _checker = checker.unwrap();

        kit.replace_health_checker(Arc::new(HealthChecker::new(2000)));
        assert!(kit.has_health_checker());
    }

    #[cfg(feature = "health-check")]
    #[test]
    fn test_health_checker_get_when_not_registered_returns_error() {
        let kit = DbNexusKit::new();
        assert!(!kit.has_health_checker());
        let result = kit.health_checker();
        assert!(result.is_err());
    }

    #[cfg(feature = "health-check")]
    #[test]
    fn test_health_checker_provide_duplicate_fails() {
        use crate::observability::health::HealthChecker;

        let kit = DbNexusKit::new();
        kit.provide_health_checker(Arc::new(HealthChecker::new(1000))).unwrap();
        let result = kit.provide_health_checker(Arc::new(HealthChecker::new(2000)));
        assert!(result.is_err());
    }
}
