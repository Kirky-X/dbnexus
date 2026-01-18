# API Reference

Complete API documentation for DBNexus.

## Table of Contents

- [Core Types](#core-types)
- [Connection Pool API](#connection-pool-api)
- [Session API](#session-api)
- [Configuration API](#configuration-api)
- [Permission API](#permission-api)
- [Procedural Macros](#procedural-macros)
- [Error Types](#error-types)

---

## Core Types

### `DbPool`

Main connection pool manager for database connections.

```rust
pub struct DbPool {
    // Fields are private
}
```

#### Methods

##### `new`

Creates a new connection pool with default configuration.

```rust
pub async fn new(url: &str) -> DbResult<Self>
```

**Parameters:**
- `url: &str` - Database connection URL

**Returns:**
- `DbResult<DbPool>` - Connection pool instance

**Example:**
```rust
let pool = DbPool::new("sqlite::memory:").await?;
```

##### `try_from_config`

Creates a connection pool from explicit configuration.

```rust
pub async fn try_from_config(config: DbConfig) -> DbResult<Self>
```

**Parameters:**
- `config: DbConfig` - Database configuration

**Returns:**
- `DbResult<DbPool>` - Connection pool instance

**Example:**
```rust
let config = DbConfigBuilder::new()
    .url("postgresql://localhost/db")
    .max_connections(20)
    .build()?;

let pool = DbPool::try_from_config(config).await?;
```

##### `try_from`

Synchronously creates an uninitiated connection pool.

```rust
pub fn try_from(config: &DbConfig) -> Result<Self, ConfigError>
```

##### `get_session`

Acquires a database session with role-based access control.

```rust
pub async fn get_session(&self, role: &str) -> DbResult<Session>
```

**Parameters:**
- `role: &str` - User role for permission checking

**Returns:**
- `DbResult<Session>` - Database session

**Errors:**
- `DbError::Permission` - Role not in permission config
- `DbError::ConnectionPool` - Failed to acquire connection

**Example:**
```rust
let session = pool.get_session("admin").await?;
```

##### `status`

Returns current pool status.

```rust
pub fn status(&self) -> PoolStatus
```

**Returns:**
- `PoolStatus` - Pool status information

**Example:**
```rust
let status = pool.status();
println!("Active: {}, Idle: {}", status.active, status.idle);
```

##### `clean_invalid_connections`

Manually triggers connection health check and cleanup.

```rust
pub async fn clean_invalid_connections(&self) -> u32
```

**Returns:**
- `u32` - Number of invalid connections removed

---

### `Session`

RAII-based database session for executing queries.

```rust
pub struct Session {
    // Fields are private
}
```

#### Methods

##### `execute`

Executes a SQL statement with permission checking.

```rust
pub async fn execute(&mut self, sql: &str) -> DbResult<ExecResult>
```

**Parameters:**
- `sql: &str` - SQL statement to execute

**Returns:**
- `DbResult<ExecResult>` - Execution result

**Errors:**
- `DbError::Permission` - Permission denied
- `DbError::SqlParse` - Invalid SQL syntax
- `DbError::Database` - Database error

**Example:**
```rust
let result = session.execute("SELECT * FROM users").await?;
```

##### `execute_raw`

Executes SQL without permission checking (for admin operations).

```rust
pub async fn execute_raw(&self, sql: &str) -> DbResult<ExecResult>
```

##### `begin_transaction`

Starts a database transaction.

```rust
pub async fn begin_transaction(&mut self) -> DbResult<()>
```

**Errors:**
- `DbError::Transaction` - Already in transaction

**Example:**
```rust
session.begin_transaction().await?;
User::insert(&session, user1).await?;
User::insert(&session, user2).await?;
session.commit_transaction().await?;
```

##### `commit_transaction`

Commits the current transaction.

```rust
pub async fn commit_transaction(&mut self) -> DbResult<()>
```

##### `rollback_transaction`

Rolls back the current transaction.

```rust
pub async fn rollback_transaction(&mut self) -> DbResult<()>
```

##### `is_in_transaction`

Checks if currently in a transaction.

```rust
pub fn is_in_transaction(&self) -> bool
```

##### `role`

Returns the current session's role.

```rust
pub fn role(&self) -> &str
```

---

### `PoolStatus`

Connection pool status information.

```rust
pub struct PoolStatus {
    pub total: u32,      // Total connections in pool
    pub active: u32,     // Currently active connections
    pub idle: u32,       // Idle connections (total - active)
    pub wait_count: u32,  // Number of times connections were waited for
    pub borrow_count: u64, // Total number of borrows
    pub max_active: u32, // Maximum active connections observed
}
```

---

## Configuration API

### `DbConfig`

Database configuration structure.

```rust
pub struct DbConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub idle_timeout: u64,
    pub acquire_timeout: u64,
    pub permissions_path: Option<String>,
    pub migrations_dir: Option<PathBuf>,
    pub auto_migrate: bool,
    pub migration_timeout: u64,
    pub admin_role: String,
    pub warmup_timeout: u64,
    pub warmup_retries: u32,
}
```

**Default Values:**

| Field | Default |
|-------|----------|
| `max_connections` | 20 |
| `min_connections` | 5 |
| `idle_timeout` | 300 (seconds) |
| `acquire_timeout` | 5000 (milliseconds) |
| `auto_migrate` | `false` |
| `migration_timeout` | 60 (seconds) |
| `admin_role` | `"admin"` |
| `warmup_timeout` | 30 (seconds) |
| `warmup_retries` | 3 |

### `DbConfigBuilder`

Builder for creating `DbConfig` instances.

```rust
pub struct DbConfigBuilder {
    // Internal state
}
```

#### Methods

##### `new`

Creates a new builder with defaults.

```rust
pub fn new() -> Self
```

##### `url`

Sets the database connection URL.

```rust
pub fn url(self, url: &str) -> Self
```

##### `max_connections`

Sets maximum pool size.

```rust
pub fn max_connections(self, max: u32) -> Self
```

##### `min_connections`

Sets minimum pool size.

```rust
pub fn min_connections(self, min: u32) -> Self
```

##### `idle_timeout`

Sets idle connection timeout in seconds.

```rust
pub fn idle_timeout(self, timeout: u64) -> Self
```

##### `acquire_timeout`

Sets connection acquisition timeout in milliseconds.

```rust
pub fn acquire_timeout(self, timeout: u64) -> Self
```

##### `permissions_path`

Sets the path to permissions configuration file.

```rust
pub fn permissions_path(self, path: &str) -> Self
```

##### `auto_migrate`

Enables automatic database migration.

```rust
pub fn auto_migrate(self, enabled: bool) -> Self
```

##### `admin_role`

Sets the admin role name.

```rust
pub fn admin_role(self, role: &str) -> Self
```

##### `build`

Builds the `DbConfig` instance.

```rust
pub fn build(self) -> Result<DbConfig, ConfigError>
```

**Example:**
```rust
let config = DbConfigBuilder::new()
    .url("postgresql://localhost/db")
    .max_connections(20)
    .min_connections(5)
    .idle_timeout(300)
    .acquire_timeout(5000)
    .permissions_path("/etc/dbnexus/permissions.yaml")
    .auto_migrate(true)
    .admin_role("superuser")
    .build()?;
```

### `ConfigLoader`

Loader for reading configuration from various sources.

```rust
pub struct ConfigLoader;
```

#### Methods

##### `from_env`

Loads configuration from environment variables.

```rust
pub fn from_env() -> Result<DbConfig, ConfigError>
```

**Environment Variables:**

| Variable | Type | Default | Description |
|-----------|-------|----------|-------------|
| `DATABASE_URL` | String | - | **Required**, database connection URL |
| `DB_MAX_CONNECTIONS` | u32 | 20 | Maximum pool size |
| `DB_MIN_CONNECTIONS` | u32 | 5 | Minimum pool size |
| `DB_IDLE_TIMEOUT` | u64 | 300 | Idle timeout (seconds) |
| `DB_ACQUIRE_TIMEOUT` | u64 | 5000 | Acquisition timeout (ms) |
| `DB_PERMISSIONS_PATH` | String | - | Permissions config path |
| `DB_MIGRATIONS_DIR` | String | - | Migration directory |
| `DB_AUTO_MIGRATE` | bool | false | Enable auto-migration |
| `DB_ADMIN_ROLE` | String | "admin" | Admin role name |

**Example:**
```bash
export DATABASE_URL="postgresql://localhost/db"
export DB_MAX_CONNECTIONS=20
export DB_ADMIN_ROLE=admin
```

```rust
let config = ConfigLoader::from_env()?;
```

##### `from_yaml_file`

Loads configuration from YAML file.

```rust
#[cfg(feature = "config-yaml")]
pub fn from_yaml_file(path: &str) -> Result<DbConfig, ConfigError>
```

**YAML Format:**
```yaml
url: "postgresql://localhost/db"
max_connections: 20
min_connections: 5
idle_timeout: 300
acquire_timeout: 5000
auto_migrate: true
admin_role: admin
```

##### `from_toml_file`

Loads configuration from TOML file.

```rust
#[cfg(feature = "config-toml")]
pub fn from_toml_file(path: &str) -> Result<DbConfig, ConfigError>
```

**TOML Format:**
```toml
url = "postgresql://localhost/db"
max_connections = 20
min_connections = 5
idle_timeout = 300
acquire_timeout = 5000
auto_migrate = true
admin_role = "admin"
```

##### `from_config_files`

Automatically detects and loads configuration from standard paths.

```rust
pub fn from_config_files() -> Result<DbConfig, ConfigError>
```

**Search Order:**
1. `./dbnexus.yaml`
2. `./dbnexus.toml`
3. `./config/dbnexus.yaml`
4. `./config/dbnexus.toml`
5. `~/.config/dbnexus/config.yaml`
6. `~/.dbnexus/config.toml`

---

## Permission API

### `PermissionAction`

Database operation types for permission checking.

```rust
pub enum PermissionAction {
    Select,  // SELECT queries
    Insert,  // INSERT statements
    Update,  // UPDATE statements
    Delete,  // DELETE statements
}
```

### `PermissionContext`

Context for permission checking with caching and rate limiting.

```rust
pub struct PermissionContext {
    // Private fields
}
```

#### Methods

##### `new`

Creates a new permission context.

```rust
pub fn new(role: String, cache: Arc<AsyncMutex<LruCache<String, RolePolicy>>>) -> Self
```

##### `with_rate_limiter`

Creates a context with rate limiting enabled.

```rust
pub fn with_rate_limiter(
    role: String,
    cache: Arc<AsyncMutex<LruCache<String, RolePolicy>>>,
    limiter: Arc<RateLimiter>,
) -> Self
```

##### `check_table_access`

Checks if current role can perform operation on table.

```rust
pub async fn check_table_access(&self, table: &str, action: &PermissionAction) -> bool
```

**Returns:**
- `bool` - `true` if allowed, `false` if denied

##### `load_policy`

Loads permission configuration from YAML string.

```rust
pub async fn load_policy(&self, yaml: &str) -> DbResult<()>
```

### `PermissionConfig`

Permission configuration structure.

```rust
pub struct PermissionConfig {
    pub roles: HashMap<String, RolePolicy>,
}
```

**YAML Format:**
```yaml
roles:
  admin:
    tables:
      - name: "*"
        operations:
          - select
          - insert
          - update
          - delete

  manager:
    tables:
      - name: "users"
        operations:
          - select
          - insert
          - update

  user:
    tables:
      - name: "users"
        operations:
          - select
```

#### Methods

##### `from_yaml`

Parses permission config from YAML string.

```rust
pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error>
```

##### `deny_all`

Creates a permission config that denies all access.

```rust
pub fn deny_all() -> Self
```

---

## Procedural Macros

### `#[derive(DbEntity)]`

Derive macro that marks a struct as a database entity.

**Required Attributes:**

```rust
#[derive(DbEntity)]
#[table_name = "table_name"]
struct MyEntity {
    #[primary_key]
    id: i64,
    field1: String,
    field2: i32,
}
```

**Attributes:**

| Attribute | Description | Required |
|-----------|-------------|-----------|
| `#[table_name = "..."]` | Database table name | Yes |
| `#[primary_key]` | Marks primary key field | Yes |
| `#[table_name = "..."]` | Database table name | Yes |
| `#[primary_key]` | Marks primary key field | Yes |

### `#[db_crud]`

Automatically generates CRUD methods for the entity.

**Generated Methods:**

```rust
impl MyEntity {
    // Insert a record
    pub async fn insert(session: &Session, value: MyEntity) -> DbResult<MyEntity>;

    // Find by primary key
    pub async fn find_by_id(session: &Session, id: i64) -> DbResult<Option<MyEntity>>;

    // Find all records
    pub async fn find_all(session: &Session) -> DbResult<Vec<MyEntity>>;

    // Find by condition
    pub async fn find_by_condition(
        session: &Session,
        condition: Condition
    ) -> DbResult<Vec<MyEntity>>;

    // Update a record
    pub async fn update(session: &Session, value: MyEntity) -> DbResult<MyEntity>;

    // Delete by primary key
    pub async fn delete(session: &Session, id: i64) -> DbResult<()>;

    // Delete by condition
    pub async fn delete_many(session: &Session, condition: Condition) -> DbResult<u64>;

    // Count records
    pub async fn count(session: &Session) -> DbResult<u64>;
}
```

**Example:**
```rust
#[derive(DbEntity)]
#[table_name = "users"]
#[db_crud]
struct User {
    #[primary_key]
    id: i64,
    name: String,
    email: String,
}

// Usage
let user = User {
    id: 1,
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
};

let inserted = User::insert(&session, user).await?;
let found = User::find_by_id(&session, 1).await?;
```

### `#[db_permission]`

Declares role-based access control for the entity.

**Attributes:**

```rust
#[db_permission(
    roles = ["admin", "manager"],
    operations = ["SELECT", "INSERT", "UPDATE"],
    config = "permissions.yaml"
)]
```

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|-------|-----------|-------------|
| `roles` | `Vec<&str>` | Yes | List of roles allowed to access this entity |
| `operations` | `Vec<&str>` | No | List of allowed operations (SELECT, INSERT, UPDATE, DELETE) |
| `config` | `&str` | No | Path to permissions config file for compile-time validation |

**Generated Methods:**

```rust
impl MyEntity {
    pub const ALLOWED_ROLES: &[&str] = &["admin", "manager"];
    pub const ALLOWED_OPERATIONS: &[&str] = &["SELECT", "INSERT", "UPDATE"];

    pub fn check_permission(ctx: &PermissionContext) -> DbResult<()>;
    pub fn check_operation(ctx: &PermissionContext, op: &PermissionAction) -> DbResult<()>;
}
```

**Example:**
```rust
#[derive(DbEntity)]
#[table_name = "users"]
#[db_crud]
#[db_permission(roles = ["admin", "manager"], operations = ["SELECT", "INSERT"])]
struct User {
    #[primary_key]
    id: i64,
    name: String,
}

// Usage
let session = pool.get_session("admin").await?;
User::find_all(&session).await?; // OK

let session = pool.get_session("guest").await?;
User::find_all(&session).await?; // Error: Permission denied
```

### `#[db_cache]`

Enables caching for entity queries (requires `cache` feature).

**Generated Methods:**

```rust
impl MyEntity {
    pub async fn find_cached(session: &Session, id: i64) -> DbResult<Option<MyEntity>>;
    pub async fn invalidate_cache(session: &Session, id: i64) -> DbResult<()>;
}
```

### `#[db_audit]`

Enables audit logging for entity operations (requires `audit` feature).

**Effects:**
- All CRUD operations are logged to audit trail
- Includes operation type, timestamp, user role, and result

---

## Error Types

### `DbError`

Database operation errors.

```rust
pub enum DbError {
    Connection(String),
    ConnectionPool(String),
    Permission(String),
    SqlParse(SqlParseError),
    Transaction(String),
    Migration(String),
    Validation(String),
    Database(sea_orm::DbErr),
    Internal(String),
}
```

### `ConfigError`

Configuration-related errors.

```rust
pub enum ConfigError {
    FileNotFound(PathBuf),
    InvalidFormat(String),
    MissingField(String),
    EnvVarError(String),
    IoError(io::Error),
    InvalidUrl(String),
    UnsupportedProtocol(String),
    ValidationFailed(String),
}
```

### `SqlParseError`

SQL parsing errors.

```rust
pub struct SqlParseError {
    pub message: String,
    pub sql: String,
}
```

---

## Utility Types

### `ExecResult`

Result of SQL execution.

```rust
pub struct ExecResult {
    pub rows_affected: u64,
    pub last_insert_id: Option<i64>,
}
```

---

## Type Aliases

```rust
pub type DbResult<T> = Result<T, DbError>;
pub type Operation = PermissionAction;
```

---

## Feature-Gated APIs

### Metrics (requires `metrics` feature)

```rust
use dbnexus::metrics::MetricsCollector;

let collector = MetricsCollector::new(&pool);
println!("{}", collector.export_prometheus());
```

### Audit (requires `audit` feature)

```rust
use dbnexus::audit::AuditLogger;

let logger = AuditLogger::new("/var/log/dbnexus/audit.log");
logger.log_event(audit_event).await?;
```

### Migration (requires `migration` feature)

```rust
use dbnexus::migration::MigrationExecutor;

let executor = MigrationExecutor::new(&pool);
executor.run_migrations().await?;
```

---

For more detailed documentation, see:
- [User Guide](USER_GUIDE.md)
- [Architecture](ARCHITECTURE.md)
- [Rust Docs](https://docs.rs/dbnexus)
