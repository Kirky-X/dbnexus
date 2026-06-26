use trait_kit::core::capability::CapabilityKey;

/// Capability key for the domain permission provider (`domain::permission::PermissionProvider`).
#[cfg(feature = "permission")]
pub struct PermissionCapKey;
#[cfg(feature = "permission")]
impl CapabilityKey for PermissionCapKey {
    type Capability = dyn crate::domain::permission::PermissionProvider;
    const NAME: &'static str = "dbnexus::permission";
}

/// Capability key for the database connection pool (`database::pool::ConnectionPool`).
pub struct ConnectionPoolCapKey;
impl CapabilityKey for ConnectionPoolCapKey {
    type Capability = dyn crate::database::pool::ConnectionPool;
    const NAME: &'static str = "dbnexus::connection_pool";
}

/// Capability key for the database session (`database::pool::DatabaseSession`).
pub struct DatabaseSessionCapKey;
impl CapabilityKey for DatabaseSessionCapKey {
    type Capability = dyn crate::database::pool::DatabaseSession;
    const NAME: &'static str = "dbnexus::database_session";
}

/// Capability key for the metrics collector (`observability::metrics::MetricsCollectorTrait`).
#[cfg(feature = "metrics")]
pub struct MetricsCapKey;
#[cfg(feature = "metrics")]
impl CapabilityKey for MetricsCapKey {
    type Capability = dyn crate::observability::metrics::MetricsCollectorTrait;
    const NAME: &'static str = "dbnexus::metrics";
}

/// Capability key for the health checker (`observability::health::HealthChecker`).
#[cfg(feature = "health-check")]
pub struct HealthCapKey;
#[cfg(feature = "health-check")]
impl CapabilityKey for HealthCapKey {
    type Capability = crate::observability::health::HealthChecker;
    const NAME: &'static str = "dbnexus::health";
}
