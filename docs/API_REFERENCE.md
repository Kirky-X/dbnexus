# DBNexus API Reference

<div align="center">

**Complete API documentation for DBNexus**

</div>

## Table of Contents

1. [Core Types](#core-types)
2. [DbPool](#dbpool)
3. [Session](#session)
4. [Configuration](#configuration-1)
5. [Permission Engine](#permission-engine)
6. [Audit System](#audit-system)
7. [Metrics](#metrics-1)
8. [Sharding](#sharding-1)
9. [Migration](#migration)
10. [Error Types](#error-types)

---

## Core Types

### DbError

```rust
use dbnexus::DbError;
```

The main error type for DBNexus.

```rust
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Connection error: {0}")]
    Connection(#[from] sea_orm::DbErr),

    #[error("Permission denied: {0}")]
    Permission(String),

    #[error("Migration error: {0}")]
    Migration(String),

    #[error("Internal error: {0}")]
    Internal(String),
}
```

### DbResult

```rust
pub type DbResult<T> = Result<T, DbError>;
```

Generic result type for DBNexus operations.

---

## DbPool

```rust
use dbnexus::DbPool;
```

Main entry point for database operations.

### Creation

```rust
impl DbPool {
    /// Create pool with default config (10 connections)
    pub async fn new(url: &str, max_connections: u32) -> DbResult<Self>;

    /// Create pool with custom config
    pub async fn with_config(config: DbConfig) -> DbResult<Self>;
}
```

### Session Management

```rust
impl DbPool {
    /// Get a session with the specified role
    pub async fn get_session(&self, role: &str) -> DbResult<Session>;

    /// Get pool status
    pub fn status(&self) -> PoolStatus;
}
```

### Pool Status

```rust
#[derive(Debug, Clone)]
pub struct PoolStatus {
    pub total: u32,   // Total connections
    pub active: u32,  // Active connections
    pub idle: u32,    // Idle connections
}
```

### Example

```rust
let pool = DbPool::new("sqlite://example.db", 10).await?;
let session = pool.get_session("admin").await?;
let status = pool.status();
```

---

## Session

```rust
use dbnexus::Session;
```

Wrapper for database connections with permission context.

### Query Execution

```rust
impl Session {
    /// Execute raw SQL (no permission check)
    pub async fn execute_raw(&self, sql: &str) -> DbResult<sea_orm::ExecResult>;

    /// Execute with permission checks
    pub async fn execute(&mut self, sql: &str) -> DbResult<sea_orm::ExecResult>;
}
```

### Transaction Management

```rust
impl Session {
    /// Begin a transaction
    pub async fn begin(&mut self) -> DbResult<sea_orm::DatabaseTransaction>;

    /// Commit transaction
    pub async fn commit(&mut self) -> DbResult<()>;

    /// Rollback transaction
    pub async fn rollback(&mut self);
}
```

### Session Role

```rust
impl Session {
    /// Get the role for this session
    pub fn role(&self) -> &str;
}
```

### Example

```rust
let mut session = pool.get_session("admin").await?;

// Execute query
let result = session.execute_raw("SELECT * FROM users").await?;

// Execute with permission check
let result = session.execute("SELECT * FROM users").await?;

// Transaction
let mut tx = session.begin().await?;
tx.execute("INSERT INTO users (name) VALUES ('John')").await?;
tx.commit().await?;
```

---

## Configuration

### DbConfig

```rust
use dbnexus::DbConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub idle_timeout: u64,
    pub acquire_timeout: u64,
    pub permissions_path: Option<PathBuf>,
    pub migrations_dir: Option<PathBuf>,
    pub auto_migrate: bool,
    pub migration_timeout: u64,
    pub admin_role: String,
}
```

### Default Implementation

```rust
impl Default for DbConfig {
    fn default() -> Self {
        Self {
            url: "sqlite://:memory:".to_string(),
            max_connections: 10,
            min_connections: 2,
            idle_timeout: 300,
            acquire_timeout: 3000,
            permissions_path: None,
            migrations_dir: None,
            auto_migrate: false,
            migration_timeout: 60,
            admin_role: "admin".to_string(),
        }
    }
}
```

### Configuration Loading

```rust
impl DbConfig {
    /// Load from environment variables
    pub fn from_env() -> Result<Self, ConfigError>;

    /// Load from TOML file
    pub fn from_toml(path: impl AsRef<Path>) -> Result<Self, ConfigError>;
}
```

### Configuration Correction

```rust
impl DbConfig {
    /// Get actual config with corrections applied
    pub fn get_actual_config(&self) -> DbConfig;

    /// Auto-correct configuration
    pub fn auto_correct(self) -> DbConfig;
}
```

---

## Permission Engine

### PermissionAction

```rust
use dbnexus::PermissionAction;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PermissionAction {
    Select,
    Insert,
    Update,
    Delete,
}
```

### TablePermission

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TablePermission {
    pub name: String,                    // Table name (supports wildcard *)
    pub operations: Vec<PermissionAction>,  // Allowed operations
}
```

### RolePolicy

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RolePolicy {
    pub tables: Vec<TablePermission>,
}

impl RolePolicy {
    /// Check if role allows operation on table
    pub fn allows(&self, table: &str, action: &PermissionAction) -> bool;
}
```

### PermissionConfig

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PermissionConfig {
    pub roles: HashMap<String, RolePolicy>,
}

impl PermissionConfig {
    /// Load from YAML file
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, String>;

    /// Get policy for a role
    pub fn get_role_policy(&self, role: &str) -> Option<&RolePolicy>;

    /// Check access permission
    pub fn check_access(&self, role: &str, table: &str, action: PermissionAction) -> bool;
}
```

### PermissionContext

```rust
use dbnexus::PermissionContext;

#[derive(Debug, Clone)]
pub struct PermissionContext {
    role: String,
    policy_cache: Arc<AsyncMutex<LruCache<String, RolePolicy>>>,
    rate_limiter: Option<Arc<RateLimiter>>,
}

impl PermissionContext {
    pub fn new(role: String, policy_cache: Arc<AsyncMutex<LruCache<String, RolePolicy>>>) -> Self;

    pub fn role(&self) -> &str;

    pub async fn check_table_access(&self, table: &str, operation: &PermissionAction) -> bool;

    pub async fn remaining(&self, key: &str) -> u32;

    pub async fn reset(&self, key: &str);
}
```

### RateLimiter

```rust
use dbnexus::RateLimiter;

#[derive(Debug, Clone)]
pub struct RateLimiter {
    max_requests: u32,
    window_duration: Duration,
    requests: Arc<RwLock<HashMap<String, Vec<Instant>>>>,
}

impl RateLimiter {
    pub fn new(max_requests: u32, window_duration: Duration) -> Self;

    /// Check if request is allowed
    pub async fn check(&self, key: &str) -> bool;

    /// Get remaining requests
    pub async fn remaining(&self, key: &str) -> u32;

    /// Reset rate limit for key
    pub async fn reset(&self, key: &str);
}
```

---

## Audit System

### AuditOperation

```rust
use dbnexus::audit::AuditOperation;

#[derive(Debug, Clone, PartialEq)]
pub enum AuditOperation {
    Create,
    Read,
    Update,
    Delete,
}
```

### AuditEvent

```rust
use dbnexus::audit::AuditEvent;

#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub operation: AuditOperation,
    pub entity_type: String,
    pub entity_id: String,
    pub user_id: String,
    pub before_value: Option<String>,
    pub after_value: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub severity: AuditSeverity,
    pub extra: Option<String>,
}

impl AuditEvent {
    pub fn create(entity_type: &str, entity_id: &str, user_id: &str) -> Self;

    pub fn read(entity_type: &str, entity_id: &str, user_id: &str) -> Self;

    pub fn update(
        entity_type: &str,
        entity_id: &str,
        user_id: &str,
        before: Option<String>,
        after: Option<String>,
    ) -> Self;

    pub fn delete(entity_type: &str, entity_id: &str, user_id: &str, before: Option<String>) -> Self;
}
```

### AuditConfig

```rust
use dbnexus::audit::AuditConfig;

#[derive(Debug, Clone, Default)]
pub struct AuditConfig {
    pub alert_operations: Vec<AuditOperation>,
    pub alert_severity: AuditSeverity,
}
```

### AuditLogger

```rust
use dbnexus::audit::{AuditLogger, AuditConfig, MemoryAuditStorage};

#[derive(Clone)]
pub struct AuditLogger {
    config: AuditConfig,
    storage: Arc<dyn AuditStorage>,
}

impl AuditLogger {
    pub fn new(config: AuditConfig, storage: Arc<dyn AuditStorage>) -> Self;

    pub async fn log(&self, event: AuditEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    pub async fn query(&self, filters: &AuditQueryFilters) -> Result<Vec<AuditEvent>, Box<dyn std::error::Error + Send + Sync>>;
}
```

### AuditStorage

```rust
use dbnexus::audit::AuditStorage;

#[async_trait::async_trait]
pub trait AuditStorage: Send + Sync {
    async fn store(&self, event: &AuditEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    async fn query(&self, filters: &AuditQueryFilters) -> Result<Vec<AuditEvent>, Box<dyn std::error::Error + Send + Sync>>;
}
```

### MemoryAuditStorage

```rust
use dbnexus::audit::MemoryAuditStorage;

#[derive(Debug)]
pub struct MemoryAuditStorage {
    max_events: usize,
}

impl MemoryAuditStorage {
    pub fn new(max_events: usize) -> Self;
}
```

---

## Metrics

### PoolMetrics

```rust
use dbnexus::metrics::PoolMetrics;

#[derive(Debug, Clone, Default)]
pub struct PoolMetrics {
    pub total_connections: u64,
    pub active_connections: u64,
    pub idle_connections: u64,
    pub queries_total: u64,
    pub queries_per_second: f64,
    pub avg_latency_ns: u64,
    pub p50_latency_ns: u64,
    pub p95_latency_ns: u64,
    pub p99_latency_ns: u64,
}
```

### LatencyPercentiles

```rust
use dbnexus::metrics::LatencyPercentiles;

#[derive(Debug, Clone, Default)]
pub struct LatencyPercentiles {
    pub p50: u64,
    pub p75: u64,
    pub p90: u64,
    pub p95: u64,
    pub p99: u64,
    pub p999: u64,
}
```

### MetricsCollector

```rust
use dbnexus::metrics::MetricsCollector;

#[derive(Debug, Clone)]
pub struct MetricsCollector {
    // ... implementation details
}

impl MetricsCollector {
    pub fn new() -> Arc<Self>;

    pub async fn get_pool_metrics(&self) -> PoolMetrics;

    pub async fn record_query(&self, latency_ns: u64, success: bool);

    pub async fn record_connection_acquired(&self);

    pub async fn record_connection_released(&self);
}
```

---

## Sharding

### ShardConfig

```rust
use dbnexus::sharding::ShardConfig;

#[derive(Debug, Clone)]
pub struct ShardConfig {
    pub shard_id: String,
    pub url: String,
}
```

### ShardRouter

```rust
use dbnexus::sharding::ShardRouter;

#[derive(Debug, Clone)]
pub struct ShardRouter {
    // ... implementation details
}

impl ShardRouter {
    pub fn with_configs<F>(configs: Vec<ShardConfig>, key_extractor: F) -> Self
    where
        F: Fn(&str) -> u32 + Send + Sync + 'static;

    pub async fn get_shard(&self, key: &str) -> DbResult<DbPool>;

    pub async fn broadcast_query(&self, sql: &str) -> Result<Vec<DbResult<sea_orm::ExecResult>>, Box<dyn std::error::Error>>;
}
```

---

## Migration

### MigrationExecutor

```rust
use dbnexus::migration::{MigrationExecutor, MigrationConfig};

#[derive(Debug)]
pub struct MigrationExecutor {
    // ... implementation details
}

impl MigrationExecutor {
    pub fn new(pool: DbPool, migrations_dir: PathBuf) -> Self;

    pub async fn ensure_migration_table_exists(&self) -> DbResult<()>;

    pub async fn is_migration_applied(&self, version: u32) -> DbResult<bool>;

    pub async fn run_pending_migrations(&self) -> DbResult<Vec<MigrationResult>>;

    pub async fn rollback_last_migration(&mut self) -> DbResult<()>;
}
```

### MigrationConfig

```rust
#[derive(Debug, Clone)]
pub struct MigrationConfig {
    pub direction: MigrationDirection,
    pub version: Option<u32>,
    pub dry_run: bool,
}
```

### MigrationResult

```rust
#[derive(Debug)]
pub struct MigrationResult {
    pub version: u32,
    pub description: String,
    pub success: bool,
    pub duration: Duration,
    pub error: Option<String>,
}
```

---

## Error Types

### ConfigError

```rust
use dbnexus::config::ConfigError;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid configuration: {0}")]
    Invalid(String),

    #[error("File not found: {0}")]
    NotFound(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Validation error: {0}")]
    ValidationError(Vec<String>),
}
```

### PermissionError

```rust
use dbnexus::permission::PermissionError;

#[derive(Debug, thiserror::Error)]
pub enum PermissionError {
    #[error("Role not found: {0}")]
    RoleNotFound(String),

    #[error("Permission denied: {0}")]
    Denied(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}
```

---

## Version

0.1.0

## Authors

DBNexus Team
