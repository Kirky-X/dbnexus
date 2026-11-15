# 📘 Dbnexus API Reference

The public API documentation of DBNexus. All signatures defer to the latest documentation generated at [docs.rs/dbnexus](https://docs.rs/dbnexus); for a usage introduction, see the [📖 User Guide](USER_GUIDE.md).

## 📋 Table of Contents

<details open>
<summary>📑 Table of Contents</summary>

- [🧩 Core Types](#-core-types)
  - [Connection Pool DbPool](#connection-pool-dbpool)
  - [Session](#session)
  - [Pool Status PoolStatus](#pool-status-poolstatus)
- [⚙️ Configuration API](#️-configuration-api)
- [🔐 Permission API](#-permission-api)
- [🏷️ Procedural Macros](#️-procedural-macros)
- [⚠️ Error Types](#️-error-types)
- [🧰 Type Aliases](#-type-aliases)
- [🚪 Feature-Gated API Overview](#-feature-gated-api-overview)
- [📊 Observability API](#-observability-api)
- [🗄️ DuckDB Database](#️-duckdb-database)
- [🔑 Authentication System](#-authentication-system)
- [🌐 Graph Databases](#-graph-databases)
- [🗺️ Sharding and Distributed Capabilities](#️-sharding-and-distributed-capabilities)
- [🔎 SQL Parsing and Security Tools](#-sql-parsing-and-security-tools)
- [🧩 Kit and Cache Integration](#-kit-and-cache-integration)
- [🌍 Internationalization](#-internationalization)
- [📚 Related Documents](#-related-documents)

</details>

---

## 🧩 Core Types

### Connection Pool DbPool

The primary connection pool manager for database connections.

```rust
pub struct DbPool { /* fields private */ }
```

#### Method Overview

| Method | Signature essentials | Description |
|------|----------|------|
| `new` | `async fn new(url: &str) -> DbResult<Self>` | create a pool with default configuration |
| `with_config` | `async fn with_config(config: DbConfig) -> DbResult<Self>` | create from explicit configuration (with auto-correction) |
| `try_from_config` | `async fn try_from_config(config: DbConfig) -> DbResult<Self>` | create from explicit configuration (strict mode) |
| `try_from` | `fn try_from(config: &DbConfig) -> Result<Self, ConfigError>` | synchronously create an uninitialized pool |
| `get_session` | `async fn get_session(&self, role: &str) -> DbResult<Session>` | acquire a session with role-based permission checks |
| `query_rows` | `async fn query_rows(&self, sql: &str, role: &str) -> DbResult<Vec<serde_json::Value>>` | unified row query: parse → permission → execute → JSON exit (0.6.0-rc.4) |
| `status` | `fn status(&self) -> PoolStatus` | return a snapshot of the current pool status |
| `config` | `fn config(&self) -> &DbConfig` | return the effective configuration |
| `clean_invalid_connections` | `async fn clean_invalid_connections(&self) -> u32` | manually trigger a connection health check and cleanup (`pool-health-check`) |
| `health_snapshot` | `async fn health_snapshot(&self) -> serde_json::Value` | pool saturation/replica status/slow-query counts summarized as a single JSON (`health-check`) |
| `set_cache_provider` | `fn set_cache_provider(&mut self, provider: Arc<dyn DbCacheProvider + Send + Sync>)` | inject a cache implementation (`cache` / `oxcache-integration`) |
| `set_ddl_guard` | `fn set_ddl_guard(&self, guard: Arc<dyn DdlGuardPolicy>)` | inject the unified DDL guard policy (`sql-parser`) |
| `enable_prepare_cache` | `fn enable_prepare_cache(&self, capacity: usize)` | enable the statement-level prepared statement LRU cache (`prepare-cache`) |
| `prepare_cache_stats` | `fn prepare_cache_stats(&self) -> Option<PrepareCacheStats>` | statement cache hit-rate metrics (`prepare-cache`) |
| `set_permission_config` | `async fn set_permission_config(&self, config: PermissionConfig) -> DbResult<()>` | replace the permission configuration at runtime, passed by value (the role policy cache swaps atomically, `permission`) |

#### Errors

`get_session` may return:

| Error | Scenario |
|------|------|
| `DbError::Permission` | the role is not in the permission configuration |
| `DbError::Connection` | acquiring a connection failed (`DbError` has no `ConnectionPool` variant; pool acquisition failures pass through the sea-orm `DbErr` as the `Connection` variant) |

**Example**:

```rust
use dbnexus::DbPool;

let pool = DbPool::new("sqlite::memory:").await?;
let session = pool.get_session("admin").await?;

// Unified row query (returns real data rows)
let rows = pool.query_rows("SELECT id, name FROM users WHERE id = 1", "admin").await?;
```

### Session

A RAII-based database session; the connection returns to the pool automatically when dropped.

```rust
pub struct Session { /* fields private */ }
```

#### Method Overview

| Method | Signature essentials | Description |
|------|----------|------|
| `role` | `fn role(&self) -> &str` | return the current session role |
| `execute` | `async fn execute(&self, sql: &str) -> DbResult<ExecResult>` | execute SQL with permission checks |
| `execute_raw` | `async fn execute_raw(&self, sql: &str) -> DbResult<ExecResult>` | safe-pipeline execution (parse → per-table permission → slow-query timing, wired in 0.6.0-rc.4) |
| `query_rows` | `async fn query_rows(&self, sql: &str) -> DbResult<Vec<serde_json::Value>>` | row query, returning JSON data rows |
| `execute_raw_ddl` | `async fn execute_raw_ddl(&self, sql: &str) -> DbResult<ExecResult>` | execute DDL, admin role only, validated by `DdlGuard` |
| `execute_cached` | `async fn execute_cached(&self, sql: &str) -> DbResult<ExecResult>` | statement-cache-aware execution (`prepare-cache`) |
| `batch_execute` | `async fn batch_execute(&self, sqls: Vec<&str>) -> DbResult<Vec<DbResult<ExecResult>>>` | execute multiple statements in a batch |
| `begin_transaction` | `async fn begin_transaction(&self) -> Result<(), DbError>` | begin a transaction (returns `DbError::Transaction` if already in one) |
| `commit` | `async fn commit(&self) -> Result<(), DbError>` | commit the current transaction |
| `rollback` | `async fn rollback(&self) -> Result<(), DbError>` | roll back the current transaction |
| `is_in_transaction` | `async fn is_in_transaction(&self) -> bool` | check whether a transaction is active |
| `query_cache_get` / `query_cache_set` | `async fn query_cache_get(&self, key: &str) -> Option<Vec<u8>>` etc. | read-only query result cache read/write (`cache`, 0.6.0-rc.4) |

Errors `execute` may return (parse failure is not silently allowed through: statements that fail to parse or are unsupported fall into the `None` branch of `execute` and are handed to the `execute_raw` pipeline; non-admin roles get `DbError::Permission` on parse failure (fail-closed), and only admin is allowed through; statements parsed as DDL/DCL/Transaction return `DbError::Permission` for all roles):

| Error | Scenario |
|------|------|
| `DbError::Permission` | permission denied (table-level grants, DDL guard interception) |
| `DbError::RateLimited` | the permission check triggered rate limiting (429 semantics, carrying `retry_after_secs`) |
| `DbError::Connection` | database execution/connection error (sea-orm `DbErr` passes through via `#[from]`, including connection acquisition timeouts) |

**Example**:

```rust
let result = session.execute("SELECT * FROM users").await?;
```

### Pool Status PoolStatus

A snapshot of the connection pool status.

```rust
pub struct PoolStatus {
    pub total: u32,        // total connections in the pool
    pub active: u32,       // currently active connections
    pub idle: u32,         // idle connections (total - active)
    pub wait_count: u32,   // requests currently waiting for a connection
    pub max_waiters: u32,  // maximum wait count (historical peak)
    pub borrow_count: u64, // total borrow operations
    pub max_active: u32,   // observed peak of active connections (historical peak)
}
```

> The pool's internal counters are maintained with atomics; the hot path is lock-free. `status()` returns the snapshot values at one instant.

---

## ⚙️ Configuration API

The configuration types are implemented as direct `serde` deserialization; for the multiple configuration sources (environment variables / YAML / JSON / TOML / programmatic), see the [User Guide](USER_GUIDE.md#-configuration).

### `DbConfig`

```rust
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct DbConfig {
    pub url: String,

    /// Connection pool configuration (flattened via `#[serde(flatten)]` to keep serialization backward-compatible)
    #[serde(flatten)]
    pub pool_config: PoolConfig,

    #[serde(default)]
    pub permissions_path: Option<String>,

    #[serde(default)]
    pub migrations_dir: Option<PathBuf>,

    #[serde(default)]
    pub auto_migrate: bool,

    #[serde(default = "default_migration_timeout")]
    pub migration_timeout: u64,

    #[serde(default = "default_admin_role")]
    pub admin_role: String,

    #[serde(default = "default_warmup_timeout")]
    pub warmup_timeout: u64,

    #[serde(default = "default_warmup_retries")]
    pub warmup_retries: u32,

    /// Cache configuration (the `cache`-related capacity sub-structure; falls back to `CacheConfig::default()` when missing)
    #[serde(default)]
    pub cache_config: CacheConfig,

    /// Runtime retry policy (available when the `retry` feature is enabled)
    #[cfg(feature = "retry")]
    #[serde(default)]
    pub retry_policy: Option<crate::reliability::RetryPolicy>,

    /// Connection failover configuration (available when the `failover` feature is enabled)
    #[cfg(feature = "failover")]
    #[serde(default)]
    pub failover_config: Option<FailoverConfig>,

    /// Replica routing configuration (available when the `replica-routing` feature is enabled)
    #[cfg(feature = "replica-routing")]
    #[serde(default)]
    pub replica_config: Option<ReplicaConfig>,

    /// SQLite connection-level PRAGMA configuration (available when the `sqlite` feature is enabled)
    #[cfg(feature = "sqlite")]
    #[serde(default)]
    pub sqlite_pragmas: Option<SqlitePragmas>,
}
```

### `PoolConfig`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    #[serde(default = "default_min_connections")]
    pub min_connections: u32,

    #[serde(default = "default_idle_timeout")]
    pub idle_timeout: u64,

    #[serde(default = "default_acquire_timeout")]
    pub acquire_timeout: u64,
}
```

**Default values and environment variables**:

| Field | Default | Environment variable (`from_env`) |
|------|--------|------------------------|
| `url` | required | `DATABASE_URL` |
| `max_connections` | 20 | `DB_MAX_CONNECTIONS` |
| `min_connections` | 5 | `DB_MIN_CONNECTIONS` |
| `idle_timeout` | 300 (seconds) | `DB_IDLE_TIMEOUT` |
| `acquire_timeout` | 5000 (milliseconds) | `DB_ACQUIRE_TIMEOUT` |
| `permissions_path` | none | `DB_PERMISSIONS_PATH` |
| `migrations_dir` | none | `DB_MIGRATIONS_DIR` |
| `auto_migrate` | `false` | `DB_AUTO_MIGRATE` |
| `migration_timeout` | 60 (seconds) | `DB_MIGRATION_TIMEOUT` |
| `admin_role` | `"admin"` | `DB_ADMIN_ROLE` |
| `warmup_timeout` | 30 (seconds) | `DB_WARMUP_TIMEOUT` |
| `warmup_retries` | 3 | `DB_WARMUP_RETRIES` |
| `cache_config` | `CacheConfig::default()` | none (config sources / programmatic only) |
| `retry_policy` | `None` (`retry`) | none (config sources / programmatic only) |
| `failover_config` | `None` (`failover`) | none (config sources / programmatic only) |
| `replica_config` | `None` (`replica-routing`) | none (config sources / programmatic only) |
| `sqlite_pragmas` | `None` (`sqlite`) | none (config sources / programmatic only) |

> `from_env` does not read environment variables for these five fields (fixed `CacheConfig::default()` and `None`); the last four fields are `#[cfg]` optional fields under their features — when not enabled they neither exist in the struct nor participate in serialization. None of the configuration types mark `deny_unknown_fields`, so writing sub-nodes like `failover_config:` into a configuration source while the feature is disabled is **silently ignored** by serde (no error); rebuild with the corresponding feature for them to take effect.

### `SqlitePragmas` (`sqlite` feature, the `sqlite_pragmas:` sub-node)

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SqlitePragmas {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub busy_timeout_ms: Option<u64>,   // driver default 5000 (built into sqlx)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub journal_mode: Option<String>,   // e.g. "WAL"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synchronous: Option<String>,    // e.g. "NORMAL"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreign_keys: Option<bool>,
}
```

Takes effect on the sqlite backend only: when `sqlite_pragmas = Some`, every connection in the pool executes the corresponding `PRAGMA` statements in order after being established (all connections of this pool are created through the same connection-establishment point); any PRAGMA failure fails connection establishment. `journal_mode` / `synchronous` are case-insensitive enum allowlists (journal_mode ∈ {DELETE,TRUNCATE,PERSIST,MEMORY,WAL,OFF}; synchronous ∈ {OFF,NORMAL,FULL,EXTRA}); values outside the allowlist error at connection establishment — SQLite silently ignores semantically invalid values, and the application-level allowlist guarantees "configured means effective". When the whole sub-config is absent (`None`), connection behavior is exactly the same as before this configuration was introduced; when unconfigured, sqlx's default busy_timeout is 5 seconds. Connection-string query parameters do not carry PRAGMA configuration — sqlx 0.9 errors outright on URL parameters outside its allowlist (`mode`/`cache`/`immutable`/`vfs`).

### `CacheConfig` (the `cache_config:` sub-node)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub policy_cache_capacity: u64,    // permission policy cache capacity, default 4096
    pub sql_parse_cache_capacity: u64, // SQL parse cache capacity, default 1000
    pub query_cache_capacity: u64,     // query result cache capacity, default 10000
    pub default_ttl: u64,              // default TTL (seconds), default 300
}
```

`Default` is a hand-written `impl` (taking the above default values field by field). `validate()` requires all three capacities to be > 0, otherwise it returns `ConfigError::InvalidCacheCapacity` (with the field name in the message).

### `FailoverConfig` (`failover` feature, the `failover_config:` sub-node)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverConfig {
    pub urls: Vec<String>,                      // ordered chain [primary, replica1, replica2, ...], first item is the primary; no serde default — if you write this sub-node you must provide it
    pub health_check_query: Option<String>,     // custom health check SQL; `None` = `SELECT 1`
    pub failover_threshold: u32,                // N consecutive failures trigger the switch, default 3
}
```

Hand-written `impl Default`: `urls` is an empty list, `health_check_query` is `None`, `failover_threshold` is 3.

### `ReplicaConfig` (`replica-routing` feature, the `replica_config:` sub-node)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaConfig {
    pub replica_urls: Vec<String>,        // replica URL list (no serde default)
    pub max_lag_seconds: f64,             // maximum allowed replication lag (seconds); beyond it, fall back to the primary; default 5.0
    pub lag_check_interval_secs: u64,     // lag check interval (seconds), default 10
}
```

Hand-written `impl Default`: `replica_urls` is an empty list, `max_lag_seconds` is 5.0, `lag_check_interval_secs` is 10.

YAML example (each of the three sub-nodes requires its feature enabled):

```yaml
url: "postgresql://localhost/mydb"
cache_config:
  query_cache_capacity: 20000
  default_ttl: 60
failover_config:              # requires the `failover` feature
  urls:
    - "postgresql://primary/mydb"
    - "postgresql://standby/mydb"
  failover_threshold: 3
```

### Configuration Loading Methods

| Method | Signature | Feature requirement |
|------|------|----------|
| `DbConfig::from_env` | `fn from_env() -> Result<DbConfig, ConfigError>` | `config-env` |
| `DbConfig::from_yaml_str` | `fn from_yaml_str(yaml: &str) -> Result<DbConfig, serde_saphyr::Error>` | `yaml` |
| `DbConfig::from_json_str` | `fn from_json_str(json: &str) -> Result<DbConfig, serde_json::Error>` | none |

**Example**:

```rust
let config = DbConfig::from_yaml_str(
    r#"
url: "sqlite::memory:"
max_connections: 20
"#,
)?;
let pool = DbPool::with_config(config).await?;
```

---

## 🔐 Permission API

### `PermissionAction`

The database operation types used for permission checks:

```rust
pub enum PermissionAction {
    Select,    // SELECT query
    Insert,    // INSERT statement
    Update,    // UPDATE statement
    Delete,    // DELETE statement
    // The following variants are available only when a graph database feature is enabled:
    Traverse,  // graph traversal (ladybug / neo4j)
    Match,     // graph match (ladybug / neo4j)
}
```

### `PermissionContext`

A permission-check context with caching and rate limiting.

| Method | Signature essentials | Description |
|------|----------|------|
| `new` | `fn new(role: String, policy_cache: Arc<Cache<String, RolePolicy>>) -> Self` | create with a shared policy cache |
| `with_cache_size_and_rate_limit` | `async fn with_cache_size_and_rate_limit(role: String, cache_capacity: usize, max_requests: u32, window_secs: u64) -> Result<Self, PermissionError>` | custom cache size and token-bucket rate limiting |
| `with_config_and_rate_limit` | `async fn with_config_and_rate_limit(role: String, config: &DbConfig, max_requests: u32, window_secs: u64) -> Result<Self, PermissionError>` | create from `DbConfig` with rate limiting attached |
| `check_table_access` | `async fn check_table_access(&self, table: &str, operation: &PermissionAction) -> bool` | check whether the current role may perform the operation on the table (both rate-limit and policy denials return `false`) |
| `check_table_access_decision` | `async fn check_table_access_decision(&self, table: &str, operation: &PermissionAction) -> TableAccessDecision` | decision-style check: distinguishes `Allowed` / `Denied` (403) / `RateLimited{retry_after}` (429) |
| `with_cache_size_and_backend` | `async fn with_cache_size_and_backend(role: String, cache_capacity: usize, backend: RateLimitBackend) -> Result<Self, PermissionError>` | dual-backend switch: `TokenBucket{max_requests, window_secs}` (default) or `External(Arc<dyn Limiter>)` to inject external rate limiting (limiteron etc.) |
| `set_audit_logger` | `fn set_audit_logger(&mut self, logger: Arc<AuditLogger>)` (`audit` feature) | attach an auditor; rate-limit denials produce the `rate_limit_exceeded` audit event |
| `load_policy` | `async fn load_policy(&self, config: &PermissionConfig) -> Result<(), String>` | load the permission configuration |

> The permission cache has a TTL and singleflight request coalescing (protection against cache stampede), and supports background hot reload.
> Rate-limiting unification: the `Limiter` port (the `dbnexus-limiter-port` crate) extracts the common contract of the token bucket and external backends; for the 429/403 semantics and the session-layer mapping, see `docs/en/RATE_LIMITING.md`.

### `PermissionConfig`

```rust
pub struct PermissionConfig {
    pub roles: HashMap<String, RolePolicy>,
}
```

| Method | Signature | Feature requirement |
|------|------|----------|
| `from_yaml_str` | `fn from_yaml_str(yaml: &str) -> Result<Self, serde_saphyr::Error>` | `yaml` |
| `deny_all` | `fn deny_all() -> Self` | create a configuration that denies all access |

For the YAML format, see [User Guide · Permission Control](USER_GUIDE.md#-permission-control).

### Permission Engine `PolicyDecisionPoint` (the `permission-engine` feature)

The advanced policy decision point (PDP), supporting RBAC + ABAC, with built-in caching and rate limiting.

```rust
use dbnexus::{PolicyDecisionPoint, RbacPermissionProvider};
use std::sync::Arc;

// Default: cache TTL 5 minutes, rate limit 100 requests/minute
let provider = Arc::new(RbacPermissionProvider::new());
let pdp = PolicyDecisionPoint::new(provider);

// Or customize via the builder
let pdp = PolicyDecisionPoint::builder()
    .provider(provider)
    .cache_ttl_seconds(600)
    .rate_limit(200, 60)
    .build();
```

**Exported types**: `PolicyDecisionPoint`, `PermissionRule`, `PermissionDecision`, `PermissionSubject`, `PermissionResource`, `RbacPermissionProvider`, `Role`

---

## 🏷️ Procedural Macros

### `#[db_entity]`

The unified attribute macro that marks a struct as a database entity, generating Sea-ORM entity glue, permission-checked CRUD methods, and cache/audit configuration. This macro consolidates the separate `db_crud` / `db_cache` / `db_audit` macros of earlier versions (`#[db_entity]` is gated by the `macros` feature and imported via `dbnexus::db_entity`).

**Required parameters**:

| Parameter | Description |
|------|------|
| `table_name = "..."` | database table name |
| `primary_key = "..."` | primary key field name |

**Optional parameters**:

| Parameter | Description |
|------|------|
| `timestamps = true` | automatically manage `created_at` / `updated_at` (via `ActiveModelBehavior::before_save`) |
| `soft_delete = true` | automatically inject a `deleted_at` field and rewrite the semantics of `find*` / `delete`, additionally generating `force_delete` |
| `validate` | integrate the `validator` crate (the `validation` feature) |
| `permissions(...)` | entity-level permission allowlist: `permissions(roles = [...], operations = [...])`; generates the `ALLOWED_ROLES` / `ALLOWED_OPERATIONS` constants and the `allowed_roles()` / `allowed_operations()` / `check_permission()` / `check_operation()` methods at compile time (example: `permission_macro`) |
| `cache(...)` | generate cache configuration (the `cache` feature) |
| `audit(...)` | generate audit configuration (the `audit` feature) |
| `hooks(...)` | 6 lifecycle hooks: `before_insert` / `after_insert` / `before_update` / `after_update` / `before_delete` / `after_delete`; the orchestration order is validate → timestamps → user hooks |

> The macro's `permissions(...)` only generates entity-level allowlist constants and check methods, which must be called explicitly; permission control on the SQL execution chain is still enforced by `Session` at runtime according to the permission configuration (YAML/JSON).

**Generated methods**:

| Method | Signature essentials |
|------|----------|
| `insert` | `async fn insert(session: &Session, value: Self) -> DbResult<Self>` |
| `find_by_id` | `async fn find_by_id<PK>(session: &Session, pk: PK) -> DbResult<Option<Self>>` (generic primary key, supporting i64 / Uuid / String etc.) |
| `find_by_ids` | `async fn find_by_ids<PK>(session: &Session, pks: Vec<PK>) -> DbResult<Vec<Self>>` (maps to sea-orm `is_in`) |
| `find_all` | `async fn find_all(session: &Session) -> DbResult<Vec<Self>>` |
| `find_by_condition` | `async fn find_by_condition(session: &Session, condition: Condition) -> DbResult<Vec<Self>>` |
| `count` | `async fn count(session: &Session) -> DbResult<u64>` |
| `exists` | `async fn exists(session: &Session, pk) -> DbResult<bool>` |
| `update` | `async fn update(session: &Session, value: Self) -> DbResult<Self>` |
| `delete` | `async fn delete<PK>(session: &Session, pk: PK) -> DbResult<()>` (constraints vary with the `soft_delete` parameter) |
| `delete_many` | `async fn delete_many(session: &Session, condition: Condition) -> DbResult<u64>` |
| `insert_many` | `async fn insert_many(session: &Session, models: Vec<Self>) -> DbResult<InsertResult>` |
| `update_many` | `async fn update_many(session: &Session, filter: Condition, updates: Self) -> DbResult<u64>` |
| `paginate` | `async fn paginate(session: &Session, page_size: u64) -> DbResult<Paginator>` |
| `query` | `async fn query(session: &Session) -> DbResult<Select<Self>>` (returns the native Sea-ORM query builder) |
| `find_with_deleted` / `find_only_deleted` / `force_delete` | generated only for entities with `soft_delete = true` |
| `table_name()` / `primary_key_column()` | constant accessors for the table name and primary key column name |
| `schema(backend)` | generates the `Table` structure for migrations (works with the `migration` feature) |

**Example**:

```rust
use dbnexus::db_entity;
use dbnexus::sea_orm::entity::prelude::*;

#[db_entity(table_name = "users", primary_key = "id")]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub email: String,
}

// Usage
let user = Model { id: 1, name: "Alice".to_string(), email: "alice@example.com".to_string() };
let inserted = Model::insert(&session, user).await?;
let found = Model::find_by_id(&session, 1).await?;
```

### `#[db_repository]`

The generic repository macro (the `repository` feature, added in 0.6.0-rc.4): generates the `Repository<T>` CRUD port implementation for an entity, working with the `JsonRepository` reference implementation (identifier allowlist + SQL escaping, reusing the `query_rows` pipeline) and the `impl_json_repository!` implementation macro. The execution role must be declared explicitly (`JsonRepository::with_role` / the macro's `role = "..."`): when the role is not declared, queries fail closed with an error rather than falling back to admin (admin would bypass both table-level RBAC and row-level RLS).

---

## ⚠️ Error Types

### `DbError`

Database operation errors.

```rust
pub enum DbError {
    /// Database connection error
    Connection(#[from] sea_orm::DbErr),

    /// Configuration error
    Config(String),

    /// Permission error
    Permission(String),

    /// Rate-limit denial (HTTP 429 semantics; returned when the permission check triggers rate limiting, distinct from a policy denial)
    RateLimited {
        /// Suggested wait time (seconds); `None` = not provided by the rate-limiting backend
        retry_after_secs: Option<u64>,
    },

    /// Transaction error
    Transaction(String),

    /// Migration error
    Migration(String),

    /// Cache operation error
    Cache(String),

    /// Query error
    Query(String),

    /// Data validation error (the validation feature)
    #[cfg(feature = "validation")]
    Validation(String),

    /// The current operation is not supported by this backend/feature (explicit rejection, avoiding silent degradation)
    Unsupported(String),

    /// Optimistic-lock version conflict (returned when the `update_if_version` condition misses)
    VersionConflict {
        /// The conflicting table name
        table: String,
        /// The conflicting row ID
        id: i64,
    },
}
```

### `ConfigError`

Configuration-related errors.

```rust
pub enum ConfigError {
    /// Missing required field
    MissingField(String),
    /// Missing URL
    MissingUrl,
    /// Invalid cache capacity
    InvalidCacheCapacity(String),
    /// Invalid value
    InvalidValue { key: String, message: String },
    /// Invalid format
    InvalidFormat(String),
    /// File not found
    FileNotFound(String),
    /// IO error
    IoError(String),
    /// Invalid URL
    InvalidUrl(String),
    /// Unsupported protocol
    UnsupportedProtocol(String),
    /// Parse error
    ParseError(String),
    /// Validation error
    ValidationError(String),
}
```

### `SqlParseError`

SQL parsing errors.

```rust
pub enum SqlParseError {
    /// SQL failed to parse (syntax error or invalid structure)
    ParseError(String),
    /// Unsupported SQL statement type
    UnsupportedStatement(String),
    /// Empty SQL statement
    EmptyStatement,
    /// Multiple SQL statements detected (only a single statement is allowed)
    MultipleStatements,
    /// The SQL statement contains variables (possible dynamic SQL injection)
    ContainsVariables(String),
}
```

### Unified Error Report (0.6.0-rc.4)

| Type | Description |
|------|------|
| `ErrorCode` | the unified error code table (segmented numeric codes + machine-readable names) |
| `UnifiedDbError` | the top-level unified error structure, with bidirectional `From` compatibility with `DbError` / `DbNexusError` |
| `QueryErrorReport` | structured error report: category + message + fix suggestion |
| `ErrorCategory` | error categories (permission denied / injection risk / syntax error / shard conflict, etc.) |

```rust
use dbnexus::{ErrorCategory, QueryErrorReport};

let report = QueryErrorReport::new(
    ErrorCategory::PermissionDenied,
    "SELECT on users denied for role guest",
    "Please grant the guest role SELECT permission on the users table",
);
```

---

## 🧰 Type Aliases

```rust
pub type DbResult<T> = Result<T, DbError>;
pub type Operation = PermissionAction;
```

`ExecResult` is re-exported directly from `sea_orm`, carrying information such as the number of rows affected after execution.

---

## 🚪 Feature-Gated API Overview

| Feature | Main exported types / methods |
|------|---------------------|
| `metrics` | `MetricsCollector` (Prometheus export, slow-query detection) |
| `health-check` | `HealthChecker`, `CircuitBreaker`, `health_snapshot()` |
| `otel` | health snapshot metrics OTLP/HTTP export bridge |
| `audit` | `AuditLogger`, `AuditEvent`, `AuditStorage` |
| `permission` | `PermissionContext`, `PermissionConfig`, `PermissionAction` |
| `permission-engine` | `PolicyDecisionPoint`, `RbacPermissionProvider` |
| `permission-facade` | `PermissionFacade` (the RBAC + masking + RLS unified facade; tenant predicates are injected only into single-table SELECTs, the admin channel skips them) |
| `data-protection` | field masking and row-level security predicate injection (the read exit `query_rows` + raw write exits UPDATE/DELETE; predicate column names are allowlist-validated, predicate values are escaped and written as literals) |
| `authentication` | `AuthenticationManager`, `JwtManager`, `PasswordHasher` |
| `sql-parser` | `SqlParser`, `DdlGuard`, `is_ddl_operation`, `contains_sql_injection` (the `InjectionEngine` type itself is exported unconditionally, see [Injection Detection](#injection-detection-injectionengine-type-exported-unconditionally-some-methods-gated-on-sql-parser)) |
| `sharding` | `ShardRouter`, `ShardConfig`, `create_strategy` |
| `global-index` | `GlobalIndex`, `IndexEntry`, `SyncResult` |
| `retry` | `RetryPolicy`, `RetryExecutor`, `RetryError`, `is_idempotent_operation` |
| `failover` | `FailoverConfig`, `CircuitBreaker` coordination |
| `replica-routing` | `ReplicaConfig` (read/write splitting routing) |
| `scatter-gather` | cross-shard Scatter-Gather query executor |
| `saga` | Saga distributed transaction orchestration (persistent recovery) |
| `distributed-id` | Snowflake distributed ID generator |
| `duckdb` | `DuckDbConnection`, `DuckDbRow`, `DuckDbExecResult` |
| `ladybug` / `neo4j` | `LadybugConnection` / `Neo4jConnection`, `GraphConnection` |
| `cache` / `oxcache-integration` | `DbCacheProvider`, `OxcacheDbCacheAdapter` |
| `prepare-cache` | `Session::execute_cached`, `PrepareCacheStats` |
| `query-dsl` | the `q!` macro, `QueryFragment`, `DslCondition`, `DslOp` |
| `repository` | `Repository<T>`, `JsonRepository`, `impl_json_repository!` |
| `data-api` | `DataApiGateway` (entity-to-JSON query endpoints) |
| `entity-events` | `EntityEvent`, `EntityEventBus`, `DbOutboxStore`, `OutboxDispatcher` |
| `copy` | batch write: COPY wrappers (`CopyStatement`/`encode_copy_rows`/`encode_duckdb_copy_rows`) + multi-row INSERT building (`BatchInsertStatement`) |
| `kit` | `DbNexusModule` and satellite modules (see [Kit and Cache Integration](#-kit-and-cache-integration)) |
| `validation` | the `DbError::Validation` variant + validator integration |
| `pool-health-check` | `DbPool::clean_invalid_connections`, `validate_and_recreate_connections` |

---

## 📊 Observability API

### Metrics Collection `MetricsCollector` (the `metrics` feature)

```rust
use dbnexus::MetricsCollector;

let collector = MetricsCollector::new();

let pool_metrics = collector.pool_status();          // PoolMetrics { total, active, idle }
if let Some(stats) = collector.get_query_stats("SELECT") {
    println!("P99 latency: {}ns", stats.latency_percentiles.p99_ns);
}

println!("{}", collector.export_prometheus());        // Prometheus text format
```

Slow-query detection: `Session::execute_raw` times every return path; when the `SlowQueryConfig` threshold is exceeded, it is recorded to `MetricsCollector` automatically.

### Health Check and Circuit Breaker (the `health-check` feature)

```rust
use dbnexus::{CircuitBreaker, CircuitBreakerConfig, HealthChecker};

let checker = HealthChecker::new(1000); // check_timeout_ms (milliseconds)
let status = checker.check().await;

// Circuit breaker: opens automatically after consecutive failures reach the threshold, half-open recovery
let breaker = CircuitBreaker::new(CircuitBreakerConfig::default());
```

`HealthCheckResult` carries a `HealthStatus` (`Healthy` / `Unhealthy(String)` / `Degraded(String)`), the check duration, and details; this type is **not exported from the crate root** — type annotations must reference it by the full path `dbnexus::observability::health::HealthCheckResult`.

### Audit Logging `AuditLogger` (the `audit` feature)

```rust
use dbnexus::AuditLogger;

let logger = AuditLogger::new(); // default in-memory storage (capacity 10000)
// Records operations with user context; admin permission-bypass operations are recorded as well
```

Custom storage and configuration are injected via `AuditLogger::with_config(config: AuditConfig, storage: Arc<dyn AuditStorage>)`; 0.6.0-rc.4 adds the `DbAuditStorage` database storage implementation (gated on `audit` + `sql-parser`) and the `PermissionAuditChain` HMAC chain-signed audit chain.

---

## 📦 Batch Write (the `copy` feature)

Two batch-write paths; pick one according to backend capability. Shared contract: identifier allowlist validation closes the injection surface at build time; data values travel via encoding (COPY) or bound parameters (batch_insert), never concatenated into SQL text; errors are explicit (empty sets / row-column mismatch / illegal identifiers error at build time).

### Multi-Row INSERT Building `BatchInsertStatement`

Parameterized batch writes for backends without a COPY protocol (sqlite/mysql/duckdb): the row set is chunked by `min(500, bind_param_limit(style)/column count)` to build `INSERT ... VALUES (...), (...)` (wide tables shrink automatically; the placeholder count per statement never exceeds the backend bind limit; if the column count itself exceeds the limit, it errors at build time), with placeholders bound. Typical consumers are bulk log persistence (inklog etc.): `chunk_rows` produces `(sql, params)` sequences, fed one by one into `Session::execute_with_params` (sqlite/mysql/postgres) or, after conversion via `json_to_duck_value`, through the DuckDB parameterized channel.

```rust
use dbnexus::database::copy::{BatchInsertStatement, PlaceholderStyle, BATCH_INSERT_CHUNK_SIZE};

let stmt = BatchInsertStatement::new("t_logs", &["ts".into(), "msg".into()])?;
// fail-fast full validation: returning Ok guarantees every statement can be executed safely (no partial-write risk)
for (sql, params) in stmt.chunk_rows(&rows, PlaceholderStyle::QMark)? {
    session.execute_with_params(&sql, &params).await?;   // sqlite/mysql/postgres
}
// postgres dialect: PlaceholderStyle::Dollar ($N numbering continues across rows)
```

When the row set is owned and not reused, `chunk_rows_owned(rows, style)` (consumed by value) moves parameters via `Vec::append` at the pointer level, avoiding the per-value deep copies of the borrowing `chunk_rows` (the biggest gains are for JSON object/array columns); the validation and chunking contracts match the borrowing version.

### COPY Wrappers `CopyStatement` and `DbPool::copy_in`

- **postgres**: `build()` produces `COPY ... FROM STDIN`, and `copy_in` streams over the protocol (`encode_copy_rows` provides PG text row encoding).
- **duckdb**: `build_from_file(path)` produces `COPY ... FROM 'path'` (the options string pairs one-to-one with the CSV encoding of `encode_duckdb_copy_rows`: unquoted empty field = NULL, quoted empty field `""` = empty string); `copy_in` imports via a temp file and returns the imported row count, cleaning up the payload file on success or failure.
- **Other backends**: `copy_in` errors explicitly and never silently degrades to row-by-row INSERT — use `BatchInsertStatement` instead.

```rust
use dbnexus::database::copy::CopyStatement;

let stmt = CopyStatement::new("t_logs", &["ts".into(), "msg".into()])?;
let inserted: u64 = pool.copy_in(&stmt, &rows).await?;
```

**Note**: pool connections of `duckdb::memory:` do not share state with each other (when the idle queue is empty, a brand-new empty database is opened); before copy_in, return the table-creating connection first (serial reuse of a single connection), or use a file database + `with_existing_duckdb_connection` to share a handle.

---

## 🌐 HTTP Health Endpoint Generator (the `http-health` feature)

`HealthRouterBuilder` (`dbnexus::integrations::http_health`) assembles the existing health data sources into an axum Router mounting three standard endpoints. **Generates, does not serve**: `build()` only produces the Router; listening and graceful shutdown are orchestrated by the consumer with `axum::serve`; when the feature is not enabled, the library pulls in no HTTP dependencies.

| Endpoint | Semantics | Data source | Success state |
|------|------|--------|--------|
| `GET /healthz` | liveness: process-alive marker | none | always 200 `{"status":"alive"}` |
| `GET /readyz` | readiness: can it take traffic | `DbPool::health_snapshot` (pool saturation/replicas/slow queries) + optional circuit breaker | healthy/degraded → 200, otherwise 503 |
| `GET /metrics` | Prometheus scrape | optional `MetricsCollector::export_prometheus` | injected → 200 (`text/plain; version=0.0.4`), not injected → 404 |

Contract: readiness is **fail-closed** — a snapshot `status` that is not healthy/degraded (including unknown values/missing) always yields 503; a circuit breaker in `Open` overrides the snapshot status to `unhealthy` (no traffic is accepted when the pool is healthy but the downstream is rejected by the breaker), and closed/half-open are reported truthfully in the response body's `circuit_breaker` field; when no collector is injected, `/metrics` returns an explicit 404 with the reason, never a fabricated empty document.

```rust
use std::sync::Arc;
use dbnexus::DbPool;
use dbnexus::integrations::http_health::HealthRouterBuilder;

let pool = Arc::new(DbPool::new("sqlite:data.db?mode=rwc").await?);
let app = HealthRouterBuilder::new(pool)          // the pool snapshot is the required data source
    .with_circuit_breaker(breaker)                // optional: Open → /readyz 503
    .with_metrics_collector(collector)            // optional: /metrics serves Prometheus
    .build();

let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await?;
axum::serve(listener, app).await?;
```

**Deployment warning**: once generated, the three endpoints have no authentication and no rate limiting; `/readyz` returns the full pool snapshot (connection count/saturation/slow-query statistics/the replicas array) and `/metrics` returns the full metrics document — listen on a loopback/internal interface, or place the Router behind an authenticating reverse proxy; to isolate it from business endpoints, `nest` the Router into an authenticated admin Router (e.g. `app.nest("/internal/health", health_router)`). The replicas array's content comes from the provider injected by the consumer (it must be cheap, purely synchronous, and free of blocking I/O — every `/readyz` probe calls it inside the snapshot critical section; consumers should cache re-probe results in the background themselves).

The optional data sources follow their own feature gates: `with_circuit_breaker` comes with `health-check` (a dependency of this feature, always available); `with_metrics_collector` comes with `metrics` — when that feature is not enabled, the method does not exist, and `/metrics` always returns 404 with a note that metrics must be enabled and rebuilt.

---

## 🗄️ DuckDB Database

Requires the `duckdb` feature. The embedded analytical database is integrated as an analytical read-only bypass (bypassing sea-orm); since 0.3.0 it uses a connection pool to support true parallel queries.

```rust
use dbnexus::DuckDbConnection;

// Create standalone (not through DbPool, because DuckDB bypasses sea-orm)
let conn = DuckDbConnection::new("duckdb::memory:")?;
// Or specify the pool size
let conn = DuckDbConnection::with_pool_size("duckdb://path/to/analytics.db", 8)?;

// Async execution (internally bridged to the synchronous duckdb crate via spawn_blocking)
let result: DuckDbExecResult = conn.execute("INSERT INTO events VALUES (1)").await?;
let rows: Vec<DuckDbRow> = conn.query("SELECT COUNT(*) FROM events").await?;

// Health check and pool size query
conn.health_check().await?;
let size = conn.pool_size();
```

**URL formats**: `:memory:` / `duckdb::memory:` (in-memory), `duckdb:path/to/file.db`, `duckdb://path/to/file.db` (file)

**Exported types**: `DuckDbConnection`, `DuckDbRow`, `DuckDbExecResult`

---

## 🔑 Authentication System

Requires the `authentication` feature. JWT authentication + password strength validation, based on `jsonwebtoken` + `bcrypt`.

| Method | Signature essentials | Description |
|------|----------|------|
| `AuthenticationManager::new` | `fn new(jwt_secret: &[u8]) -> AuthResult<Self>` | create the manager (the key is at least 32 bytes) |
| `with_config` | `fn with_config(jwt_secret: &[u8], access_exp_secs: u64, refresh_exp_secs: u64) -> AuthResult<Self>` | custom expiry times |
| `register_user` | `async fn register_user(username: &str, password: &str, role: &str) -> AuthResult<()>` | the full flow of strength validation → hashing → persistence |
| `authenticate` | `async fn authenticate(credentials: AuthCredentials) -> AuthResult<String>` | verify credentials and issue an access token |
| `verify_token` | `fn verify_token(token: &str) -> AuthResult<JwtClaims>` | validate a JWT (synchronous) |
| `refresh_token` | `async fn refresh_token(refresh_token: &str) -> AuthResult<String>` | refresh the access token |

`JwtManager` additionally provides `verify_access_token` / `verify_refresh_token` (validating `token_type` distinctly, preventing a refresh token from being reused as an access token) and an optional distributed revocation cache (the `oxcache-integration` feature; the revocation entry TTL equals the token's remaining lifetime).

**Exported types**: `AuthenticationManager`, `JwtManager`, `PasswordHasher`, `AuthCredentials`, `JwtClaims`, `TokenType`, `User`, `AuthError`, `AuthResult`

---

## 🌐 Graph Databases

Requires the `ladybug` or `neo4j` feature. The embedded graph database (Ladybug) and the server-side graph database (Neo4j) are unified through the `GraphConnection` trait and can be mixed with relational databases.

```rust
use dbnexus::{GraphConnection, LadybugConnection};

// Ladybug embedded graph database (the second parameter is the concurrent query count pool_size, minimum 1)
let conn = LadybugConnection::new("ladybug:path/to/graph.db", 4)?;
let result = conn.execute_cypher("MATCH (n) RETURN n").await?;

// Neo4j server-side graph database
use dbnexus::Neo4jConnection;
let conn = Neo4jConnection::new("neo4j://localhost:7687", "user", "pass").await?;
```

**Session graph transaction support**:

```rust
// Session serializes Cypher execution through the graph-operation mutex, supporting begin/commit/rollback;
// the only public entry is execute_cypher_with_params (the parameterized channel, with injection protection)
let mut params = std::collections::HashMap::new();
params.insert("name".to_string(), serde_json::json!("Alice"));
session.execute_cypher_with_params("CREATE (n:User {name: $name})", params).await?;
```

> Raw Cypher execution is deprecated: use the `execute_cypher_with_params` parameterized channel instead (the default implementation refuses silent fallback).

**Permission control**: the `Traverse` and `Match` variants of `PermissionAction` are used for graph operation permission control.

**Exported types**: `LadybugConnection`, `Neo4jConnection`, `GraphConnection`, `GraphExecResult`, `GraphNode`, `GraphQueryResult`, `GraphRel`, `GraphRow`, `GraphTransaction`, `GraphValue`

---

## 🗺️ Sharding and Distributed Capabilities

### Shard Routing `ShardRouter` (the `sharding` feature)

| Method | Signature essentials | Description |
|------|----------|------|
| `new` | `fn new<S: ShardingStrategy + 'static>(strategy: S, total_shards: u32) -> Self` | create with a strategy instance |
| `with_strategy` | `fn with_strategy(strategy: &str, total_shards: u32) -> Self` | create by strategy name (synchronous) |
| `with_config` | `async fn with_config(config: &ShardConfig) -> Result<Self, DbError>` | asynchronously initialize all shard pools in parallel |
| `shard_id_for_key` | `fn shard_id_for_key(shard_key: &str) -> u32` | compute the shard ID for a key |
| `calculate_shard` | `fn calculate_shard(timestamp: DateTime<Utc>, key: &str) -> u32` | compute the shard by time + key |
| `get_session_for_shard` | `async fn get_session_for_shard(shard_key, role) -> Result<Session, DbError>` | acquire the Session for the shard |
| `get_session_for_shard_with_id` | `async fn get_session_for_shard_with_id(shard_key, role) -> Result<(Session, u32), DbError>` | also return the shard ID |

The `create_strategy(name)` factory supports `"yearly"`, `"monthly"`, `"daily"`, `"hash"`, `"consistent-hash"`; unknown names fall back to the default `YearlyStrategy`.

**Exported types**: `ShardRouter`, `ShardConfig`, `ShardingStrategy`, `create_strategy`

### Global Index `GlobalIndex` (the `global-index` feature)

Cross-shard query index support, persisted to the database via sea-orm.

```rust
use dbnexus::{DbPool, GlobalIndex};
use std::sync::Arc;

let pool = DbPool::new("sqlite::memory:").await?;
let index = GlobalIndex::new(Arc::new(pool)).await?;

// Query by index key
let entries = index.query_by_index("users", "user_id", "user_123").await?;

// Batch sync
let result = index.batch_sync(entries).await?;
```

**Exported types**: `GlobalIndex`, `IndexEntry`, `SyncEvent`, `SyncResult`

### Runtime Retry (the `retry` feature)

Only idempotent queries (SELECT / SHOW / EXPLAIN) retry automatically; non-idempotent operations execute directly without retrying.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum retry count (not counting the first execution), default 3
    pub max_retries: u32,
    /// Initial backoff interval (milliseconds), default 100
    pub initial_backoff_ms: u64,
    /// Upper bound of the maximum backoff interval (milliseconds), default 5000
    pub max_backoff_ms: u64,
    /// Backoff growth multiplier, default 2.0
    pub multiplier: f64,
    /// Whether to add random jitter (to avoid the thundering herd), default true
    pub jitter: bool,
    /// Overall wall-clock timeout (milliseconds); `None` means no timeout limit, default `None`
    pub overall_timeout_ms: Option<u64>,
    /// Non-BUSY write errors never retry; BUSY write errors retry only for out-of-transaction statements when the switch is on, default false
    #[serde(default)]
    pub retry_busy_writes: bool,
}
```

Backoff strategy: the wait time of the Nth retry = `min(initial_backoff * multiplier^N, max_backoff)`; with `jitter = true`, ±25% random jitter is added.

**BUSY retry on the write path**: with `retry_busy_writes` (default `false`) enabled, a non-idempotent statement enters the existing backoff retry only when "out of transaction + the error is BUSY/locked (`database is locked` / `SQLITE_BUSY` / `SQLITE_LOCKED` etc., identified via `is_busy_error`)" — BUSY means the statement did not take effect, so a retry repeats without side effects; a BUSY inside a transaction may be `SQLITE_BUSY_SNAPSHOT` under WAL, which must roll back the transaction, so it never retries at the statement level. Once enabled, the upper bound of write latency ≈ `(1 + max_retries) × busy_timeout + Σ backoff`: busy_timeout is the per-attempt wait upper bound inside the driver (sqlx default 5 seconds, adjustable via `sqlite_pragmas.busy_timeout_ms`) and is the dominant term; Σ backoff is the sum of the retry backoffs.

```rust
pub struct RetryExecutor;

impl RetryExecutor {
    /// Associated function (not an instance method). Automatically retries only when the sql is judged an idempotent operation;
    /// with `retry_busy_writes` enabled, out-of-transaction BUSY write errors are retryable as well.
    pub async fn execute_with_retry<F, Fut, T>(
        policy: &RetryPolicy,
        operation: F,
        sql: &str,
    ) -> Result<T, RetryError>
    where
        F: Fn() -> Fut + Send + Sync,
        Fut: Future<Output = Result<T, DbError>> + Send,
        T: Send;
}
```

```rust
pub enum RetryError {
    /// Retries exhausted, containing the last error
    Exhausted { attempts: u32, last_error: DbError },
    /// A non-idempotent operation was refused a retry
    NonRetryable(DbError),
    /// Overall timeout
    Timeout { timeout_ms: u64, last_error: DbError },
}
```

```rust
use dbnexus::is_idempotent_operation;

assert!(is_idempotent_operation("SELECT * FROM users"));
assert!(!is_idempotent_operation("INSERT INTO users VALUES (1)"));
```

**Exported types**: `RetryPolicy`, `RetryExecutor`, `RetryError`, `is_idempotent_operation`, `is_busy_error`

> For the entry types of the other distributed capabilities (`failover` / `replica-routing` / `scatter-gather` / `shard-migration` / `saga` / `distributed-id`), see the [Feature-Gated API Overview](#-feature-gated-api-overview); for end-to-end usage, see the same-named examples under `examples/`.

---

## 🔎 SQL Parsing and Security Tools

### `SqlParser` (the `sql-parser` feature)

```rust
use dbnexus::{SqlParser, SqlOperationType, contains_sql_injection, is_ddl_operation};

let parser = SqlParser::new().await;

// Returns Result<Option<(table name, PermissionAction)>, SqlParseError>
// Only DML operations return Some; DDL / DCL / transaction / others return None
let op = parser.parse_operation_async("SELECT * FROM users").await?;
let op_sync = parser.parse_operation("SELECT * FROM users"); // synchronous version

assert!(is_ddl_operation("CREATE TABLE foo (id INT)"));
assert!(contains_sql_injection("'; DROP TABLE--"));
```

`SqlOperationType` variants: `Select`, `Insert`, `Update`, `Delete`, `Ddl` (CREATE / ALTER / DROP / TRUNCATE), `Dcl` (GRANT / REVOKE), `Transaction` (BEGIN / COMMIT / ROLLBACK), `Other`.

**Exported types**: `SqlParser`, `SqlOperationType`, `SqlParseError`, `is_ddl_operation`, `contains_sql_injection`

### Injection Detection `InjectionEngine` (type exported unconditionally; some methods gated on `sql-parser`)

The unified injection detection engine: the relational / DDL / graph rule tables were merged into a single registry (0.6.0-rc.4), scanning by category (`RuleCategory`). `InjectionEngine` / `RuleCategory` can be used without the `sql-parser` feature — `global()` / `scan()` / `scan_ddl()` / `scan_graph()` have no feature gate; only relational SQL parsing-related methods such as `scan_relational` are gated on `sql-parser`.

```rust
use dbnexus::{InjectionEngine, RuleCategory};

let engine = InjectionEngine::global();
let hits = engine.scan_ddl("DROP DATABASE production");
```

### DDL Guard `DdlGuard` (the `sql-parser` feature)

AST-based DDL validation, working with the unified injection detection engine to block dangerous patterns. For the full description, the three-state result, the allowlist, and examples, see [User Guide · DDL Security Guard](USER_GUIDE.md#ddl-security-guard).

The guard policy is replaceable: the `DdlGuardPolicy` port + `AuditingDdlGuard` (audit decorator) / `DryRunDdlGuard` (dry run), injected via `DbPoolBuilder::ddl_guard` / `DbPool::set_ddl_guard`.

### Sensitive Data Masking `SensitiveMasker` (always available)

```rust
use dbnexus::{MaskType, SensitiveMasker};

let masked = SensitiveMasker::mask("alice@example.com", MaskType::Email)?;
// => "a***@example.com"
```

`MaskType` variants: `Phone`, `Email`, `IdCard`, `BankCard`, `Name`, `Address` (all with Unicode-safe handling).

**Exported types**: `SensitiveMasker`, `MaskType`, `SensitiveError`

---

## 🧩 Kit and Cache Integration

### trait-kit Integration (the `kit` feature)

Unified capability management based on trait-kit. The `kit` feature implies the full pool / cache / audit / health capability closure; after registration, all capabilities can be `require`d:

```rust
use dbnexus::DbNexusModule;

let module = DbNexusModule::new();
```

**Exported types**: `DbNexusModule`, `DbNexusCacheModule`, `DbNexusAuditModule`, `DbNexusHealthModule`, `DbHealthCapability`, `DbNexusBuildObserver` (the satellite modules were added in 0.6.0-rc.4; all four capabilities can be registered independently)

### Cache Provider (the `cache` / `oxcache-integration` features)

The `DbCacheProvider` trait is the cache abstraction layer: `Session` drives read-only query result caching through the injected provider (`query_cache_get` / `query_cache_set`), and `cache_provider` is read lock-free via `ArcSwapOption`.

```rust
use dbnexus::{DbCacheProvider, DbPoolBuilder};

let pool = DbPoolBuilder::new()
    .config(config)
    .cache_provider(custom_cache) // Arc<dyn DbCacheProvider + Send + Sync>
    .build()
    .await?;
```

**Exported types**: `DbCacheProvider`, `OxcacheDbCacheAdapter` (the `oxcache-integration` feature, adapting oxcache)

### Query Cache Decorator (the `oxcache-integration` feature)

`OxcacheQueryCache` assembles the oxcache backend into the `DbPool` parameterized query channel: `query_cached` derives the cache key (SHA-256) from the security context + data-protection policy epoch + SQL + bound parameters + table version stamps; a hit returns the cached row set (`from_cache = true`, zero database round-trips), a miss executes through and backfills; `invalidate_table` bumps the per-table version stamp (nanosecond timestamp, avoiding read-modify-write races, idempotent) after the write path, so cache entries mentioning that table naturally expire.

```rust
use std::sync::Arc;
use dbnexus::integrations::oxcache_query_cache::OxcacheQueryCache;

let qc = OxcacheQueryCache::new(pool, cache_backend).with_default_ttl(std::time::Duration::from_secs(60));
// N+1 point queries: from the second round everything hits (from_cache = true)
let cq = qc.query_cached(
    "SELECT id, name FROM users WHERE id = ?",
    &[serde_json::json!(user_id)],
    &["users"],                                    // tables the SQL depends on (declared explicitly)
).await?;
// Invalidate the table after the write path: subsequent query_cached executions re-run and read the new value
qc.invalidate_table("users").await?;
```

Contract: **table names are declared explicitly** (the `tables` parameter, not a blank allowlist, empty sets rejected) rather than parsed from SQL — invalidation correctness takes priority; a missed extraction of aliases/subqueries by SQL parsing would directly become a dirty read; the key is derived from `SHA-256(security context + data-protection policy epoch + table version raw bytes + SQL + parameter JSON)`, and **role/namespace participate in the key** — row-set contents depend on the execution-time RLS/masking/permission context, so multiple instances sharing the same backend must distinguish security contexts with `with_role`/`with_namespace` (full rows backfilled by admin are invisible to restricted roles, and the same applies across tenants), otherwise a hit is an unauthorized read; under the `data-protection` feature, **the policy epoch participates in the key** — after `set_data_protection` swaps the policy at runtime, row sets backfilled under the old policy can no longer be hit (old RLS predicates/masking exits do not produce dirty reads; tightening/loosening takes effect immediately); any change to SQL/parameters/any table invalidation/security context/policy swap creates a new entry (old entries are left to the backend's TTL/LRU eviction); table versions enter the hash as raw bytes + invalidation writes a decimal string (encoding is injective, repeated invalidations do not collide back to the old key); version read failures error explicitly (otherwise a missed invalidation would silently produce dirty reads); when a miss executes successfully but the cache backfill fails, `DbError::Cache` is returned (the query is idempotent and safe to retry). **Exported types**: `OxcacheQueryCache`, `CachedQuery`.

---

## 🌍 Internationalization

ICU4X-based locale-aware formatting (a core feature, always available).

```rust
use dbnexus::DbI18nFormatter;

let formatter = DbI18nFormatter::new("zh-CN")?;
let formatted = formatter.format_number(1234567.89)?;
```

**Exported types**: `DbI18nFormatter`, `I18nError`

---

## 📚 Related Documents

| Document | Description |
|------|------|
| [📖 User Guide](USER_GUIDE.md) | The complete tutorial from installation to advanced usage |
| [🏗️ Architecture](ARCHITECTURE.md) | Design philosophy, module breakdown, data flow, and security/performance design |
| [📦 Online API docs](https://docs.rs/dbnexus) | Latest documentation auto-generated by docs.rs |
