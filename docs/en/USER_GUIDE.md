# 📖 Dbnexus User Guide

The complete guide to using DBNexus in your application, covering installation, configuration, entity definition, CRUD, permission control, transactions, and advanced features.

> Related documents: [📘 API Reference](API_REFERENCE.md) ｜ [🏗️ Architecture](ARCHITECTURE.md) ｜ [🔒 Security](SECURITY.md) ｜ [📊 Performance Baseline](PERFORMANCE.md). For the full feature table, see the [Feature Flags Reference](#feature-flags-reference) below.

## 📋 Table of Contents

<details open>
<summary>📑 Table of Contents</summary>

- [🚀 Quick Start](#-quick-start)
  - [Prerequisites](#prerequisites)
  - [Installation](#installation)
  - [Choosing a Database Driver](#choosing-a-database-driver)
  - [Verifying the Installation](#verifying-the-installation)
- [🔧 Configuration](#-configuration)
  - [Environment Variables](#environment-variables)
  - [YAML Configuration File](#yaml-configuration-file)
  - [JSON Configuration](#json-configuration)
  - [TOML Configuration](#toml-configuration)
  - [Programmatic Configuration](#programmatic-configuration)
  - [Configuration Parameters](#configuration-parameters)
- [🧩 Defining Entities](#-defining-entities)
- [📦 Connection Pool and Sessions](#-connection-pool-and-sessions)
- [📝 CRUD Operations](#-crud-operations)
- [🔐 Permission Control](#-permission-control)
- [🔄 Transactions](#-transactions)
- [🧱 Advanced Features](#-advanced-features)
- [✅ Best Practices](#-best-practices)
- [🧯 Troubleshooting](#-troubleshooting)
- [📦 Complete Example](#-complete-example)
- [📚 Related Documents](#-related-documents)

</details>

---

## 🚀 Quick Start

This section walks you through the minimal closed loop of "connect to the database → define an entity → run CRUD" from scratch.

### Prerequisites

- Rust **1.97.1** or later (pinned by `rust-toolchain.toml`, edition 2024)
- Basic knowledge of Rust and SQL
- One of the databases: PostgreSQL, MySQL, SQLite, DuckDB (graph databases Ladybug / Neo4j optional)

### Installation

Add the dependencies to `Cargo.toml`:

```toml
[dependencies]
dbnexus = { version = "0.6.1", features = ["runtime-tokio-rustls", "sqlite", "permission", "macros"] }
tokio = { version = "1.54", features = ["rt-multi-thread", "macros"] }
```

> DBNexus's `default` feature set is empty: the runtime, database driver, and functional features must all be enabled explicitly.
> `permission` force-enables `sql-parser` and `cache` (preventing injection from bypassing permission checks; `cache` is the explicit dependency of the permission policy cache).

### Choosing a Database Driver

| Driver feature | Database | Type | Constraints |
|----------|--------|------|------|
| `sqlite` | SQLite | Embedded relational | mutually exclusive with server-side drivers (`postgres`/`mysql`); can coexist with `duckdb` |
| `postgres` | PostgreSQL | Server relational | mutually exclusive with embedded drivers (`sqlite`/`duckdb`); mutually exclusive with `mysql` |
| `mysql` | MySQL | Server relational | mutually exclusive with embedded drivers (`sqlite`/`duckdb`); mutually exclusive with `postgres` |
| `duckdb` | DuckDB | Embedded analytical | mutually exclusive with server-side drivers (`postgres`/`mysql`); mutually exclusive with `ladybug`; can coexist with `sqlite` |
| `ladybug` | Ladybug | Embedded graph database | mutually exclusive with `duckdb` (duplicate-symbol mbedtls link conflict); can coexist with relational drivers |
| `neo4j` | Neo4j | Graph database server | can coexist with relational drivers |

```toml
# Minimal configuration for embedded devices
dbnexus = { version = "0.6.1", default-features = false, features = ["runtime-tokio-rustls", "sqlite", "config-env"] }

# PostgreSQL with enterprise features
dbnexus = { version = "0.6.1", features = ["runtime-tokio-rustls", "postgres", "permission", "metrics", "audit"] }

# SQLite with basic features
dbnexus = { version = "0.6.1", features = ["runtime-tokio-rustls", "sqlite", "permission"] }
```

**Important**: driver mutual exclusion is enforced per group, with three compile-time guards (`compile_error!`) in total: `postgres` and `mysql` are mutually exclusive; embedded (`sqlite`/`duckdb`) and server-side (`postgres`/`mysql`) are mutually exclusive; `duckdb` and `ladybug` are mutually exclusive. Same-side combinations can coexist (e.g. `sqlite` + `duckdb`); conflicting combinations fail the build at compile time.

### Verifying the Installation

```rust
use dbnexus::DbPool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("sqlite::memory:").await?;
    println!("DBNexus is ready");
    Ok(())
}
```

The overall path from installation to the first query is shown below:

```mermaid
flowchart TD
    A["Install dbnexus"] --> B["Choose a runtime and database driver"]
    B --> C["Configure the connection pool<br/>environment variables / YAML / JSON"]
    C --> D["Define entities<br/>the db_entity macro"]
    D --> E["Acquire a session<br/>get_session with a role"]
    E --> F["CRUD and transactions<br/>every statement goes through permission checks"]
    F --> G{Need enterprise capabilities?}
    G -->|yes| H["Enable features on demand<br/>metrics / audit / sharding etc."]
    G -->|no| I["Go live"]
    H --> I
```

---

## 🔧 Configuration

### Environment Variables

The fastest configuration method is environment variables (requires the `config-env` feature):

```bash
export DATABASE_URL="postgresql://user:password@localhost/mydb"
export DB_MAX_CONNECTIONS=20
export DB_MIN_CONNECTIONS=5
export DB_ADMIN_ROLE=admin
```

```rust
use dbnexus::{DbConfig, DbPool};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = DbConfig::from_env()?;
    let pool = DbPool::with_config(config).await?;
    println!("Connected");
    Ok(())
}
```

### YAML Configuration File

Create `dbnexus.yaml` (requires the `yaml` feature):

```yaml
url: "postgresql://localhost/mydb"
max_connections: 20
min_connections: 5
idle_timeout: 300
acquire_timeout: 5000
auto_migrate: true
admin_role: admin
```

```rust
use dbnexus::DbConfig;

let yaml_content = std::fs::read_to_string("dbnexus.yaml")?;
let config = DbConfig::from_yaml_str(&yaml_content)?;
let pool = DbPool::with_config(config).await?;
```

### JSON Configuration

`DbConfig` supports direct serde JSON deserialization (no extra feature required):

```rust
use dbnexus::DbConfig;

let json_content = std::fs::read_to_string("dbnexus.json")?;
let config = DbConfig::from_json_str(&json_content)?;
let pool = DbPool::with_config(config).await?;
```

### TOML Configuration

DBNexus does not bundle a `toml` parser; the application brings in the `toml` crate itself for deserialization (`DbConfig` implements `serde::Deserialize`):

```toml
[dependencies]
toml = "0.9"
```

```rust
use dbnexus::DbConfig;

let toml_content = std::fs::read_to_string("dbnexus.toml")?;
let config: DbConfig = toml::from_str(&toml_content)?;
let pool = DbPool::with_config(config).await?;
```

### Programmatic Configuration

Construct the `DbConfig` struct directly:

```rust
use dbnexus::{DbConfig, DbPool, PoolConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = DbConfig {
        url: "postgresql://localhost/mydb".to_string(),
        pool_config: PoolConfig {
            max_connections: 20,
            min_connections: 5,
            idle_timeout: 300,
            acquire_timeout: 5000,
        },
        admin_role: "admin".to_string(),
        auto_migrate: true,
        ..Default::default()
    };

    let pool = DbPool::with_config(config).await?;
    Ok(())
}
```

### Configuration Parameters

For the types, default values, and corresponding environment variables of all configuration fields, see [API Reference · Configuration API](API_REFERENCE.md#️-configuration-api).

> The fields of `PoolConfig` are flattened via `#[serde(flatten)]`, so keys such as `max_connections` can be written directly in YAML/JSON; in Rust code they are accessed via `config.pool_config.max_connections`.

> Besides the pool parameters, `DbConfig` carries five sub-configs: `cache_config` (cache-related capacity and default TTL), `retry_policy` (`retry`), `failover_config` (`failover`), `replica_config` (`replica-routing`), and `sqlite_pragmas` (`sqlite`, connection-level SQLite PRAGMAs: busy_timeout/journal_mode/synchronous/foreign_keys). The last four are `#[cfg]`-gated fields: when the corresponding feature is not enabled, they do not exist in the struct and do not participate in serialization (writing these sub-nodes to a config source is silently ignored by serde); none of the five fields **read environment variables** — they can only be set via YAML/JSON/TOML or programmatically. For the field table and the YAML shape, see [API Reference · Configuration API](API_REFERENCE.md#️-configuration-api).

---

## 🧩 Defining Entities

### Basic Entity Definition

Use the `#[db_entity]` macro to map a struct to a database table and get the macro-generated permission-checked CRUD methods:

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
```

`table_name` and `primary_key` are required parameters; for the optional parameters `timestamps` / `soft_delete` / `validate` / `cache(...)` / `audit(...)` / `hooks(...)` and their feature requirements, see the `#[db_entity]` parameter table in [API Reference · Procedural Macros](API_REFERENCE.md#️-procedural-macros).

> When using field types such as `chrono::DateTime` / `uuid::Uuid`, enable the corresponding `with-chrono` / `with-uuid` type-bridge features.

### Cache and Audit Sub-parameters

The `cache(...)` sub-parameters:

| Parameter | Description | Default |
|------|------|--------|
| `ttl` | cache time-to-live (seconds) | 300 |
| `strategy` | cache strategy | `"lru"` |
| `max_capacity` | maximum cache capacity | 10000 |

The `audit(...)` sub-parameters:

| Parameter | Description | Default |
|------|------|--------|
| `table_name` | audit log table name | empty string (`""`; the generated `AUDIT_TABLE_NAME` is an empty string when unconfigured — specify it yourself for it to be meaningful) |
| `operations` | list of audited operations | `["INSERT", "UPDATE", "DELETE"]` |
| `roles` | list of roles allowed to audit | `["admin"]` |
| `log_values` | whether to record field values | `true` |

### Complex Entity Example

```rust
use dbnexus::db_entity;
use dbnexus::sea_orm::entity::prelude::*;

#[db_entity(
    table_name = "orders",
    primary_key = "id",
    cache(ttl = 60, strategy = "lru", max_capacity = 5000),
    audit(table_name = "audit_log", operations = ["INSERT", "UPDATE", "DELETE"], log_values = true)
)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "orders")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub user_id: i64,
    pub amount: f64,
    pub status: String,
}
```

---

## 📦 Connection Pool and Sessions

### Acquiring a Session

Acquire a database session with a specific role via `get_session()`:

```rust
use dbnexus::DbPool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("sqlite::memory:").await?;

    // Acquire a session with a role
    let session = pool.get_session("admin").await?;
    println!("Role: {}", session.role());

    // The session automatically returns the connection when dropped
    Ok(())
}
```

### Session Lifetime (RAII)

Sessions are managed by RAII; the connection returns when the scope ends, with no manual release required:

```rust
{
    let session = pool.get_session("admin").await?;
    // The connection is active here
    Model::find_all(&session).await?;
    Model::insert(&session, new_user).await?;
} // The connection is automatically returned to the pool here
```

### Checking Pool Status

Monitor connection pool health:

```rust
let status = pool.status();

println!("Total connections: {}", status.total);
println!("Active connections: {}", status.active);
println!("Idle connections: {}", status.idle);
println!("Wait count: {}", status.wait_count);
println!("Observed peak active: {}", status.max_active);
```

### Manual Health Check

Trigger a connection health check and cleanup (requires the `pool-health-check` feature):

```rust
let invalid_count = pool.clean_invalid_connections().await;
println!("Removed {} invalid connections", invalid_count);
```

---

## 📝 CRUD Operations

The following examples assume a `Model` entity has been defined and a `session` acquired. For the full method signatures, see [API Reference · Procedural Macros](API_REFERENCE.md#️-procedural-macros).

### Create (Insert)

```rust
let user = Model {
    id: 1,
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
};

let inserted = Model::insert(&session, user).await?;
println!("Inserted user: {}", inserted.name);
```

### Read (Query)

Find by primary key:

```rust
let user = Model::find_by_id(&session, 1).await?;
if let Some(user) = user {
    println!("Found user: {}", user.name);
}
```

Find all records:

```rust
let users = Model::find_all(&session).await?;
println!("Found {} users", users.len());
```

Find by condition:

```rust
use dbnexus::sea_orm::entity::prelude::*;

let condition = Condition::all()
    .add(Column::Name.like("%Alice%"))
    .add(Column::Id.gte(1));

let users = Model::find_by_condition(&session, condition).await?;
```

Count records:

```rust
let count = Model::count(&session).await?;
println!("Total users: {}", count);
```

### Update

```rust
let mut user = Model::find_by_id(&session, 1).await?.unwrap();
user.email = "alice_new@example.com".to_string();

let updated = Model::update(&session, user).await?;
println!("Updated user: {}", updated.email);
```

### Delete

Delete by primary key:

```rust
Model::delete(&session, 1).await?;
println!("Deleted the user with ID 1");
```

Delete many by condition:

```rust
use dbnexus::sea_orm::entity::prelude::*;

let condition = Condition::all().add(Column::Id.lt(100));
let deleted_count = Model::delete_many(&session, condition).await?;
println!("Deleted {} records", deleted_count);
```

---

## 🔐 Permission Control

Permission control is enforced by `Session` at runtime according to the permission configuration (YAML/JSON); every statement goes through the "parse → per-table permission check" pipeline before execution. Target tables on JOIN / subquery paths are checked as well. For the full design, see the [Security document](SECURITY.md#-security-design-overview).

### Defining a Permission Policy

Create `permissions.yaml`:

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
      - name: "orders"
        operations:
          - select

  user:
    tables:
      - name: "users"
        operations:
          - select
```

### Using Permissions

```rust
// admin has all operations on all tables
let admin_session = pool.get_session("admin").await?;
Model::insert(&admin_session, user).await?;
Model::delete(&admin_session, 1).await?;

// manager can only select/insert/update on users
let manager_session = pool.get_session("manager").await?;
Model::find_all(&manager_session).await?;      // OK
Model::insert(&manager_session, user).await?;  // OK
Model::delete(&manager_session, 1).await?;     // Error: permission denied

// user can only select
let user_session = pool.get_session("user").await?;
Model::find_all(&user_session).await?;         // OK
Model::insert(&user_session, user).await?;     // Error: permission denied
```

### Structured Denial Reason Codes

Permission denials return `DbError::PermissionDenied { reason }`; `reason.reason_code()` yields a stable machine-readable code.
HTTP / MCP layers can read the `reason_code` field of `UnifiedDbError::to_json()`; the legacy `DbError::Permission(String)` does not carry this field.

| Reason Code | Meaning |
| --- | --- |
| `DDL_FORBIDDEN` | DDL statements are rejected by this API |
| `UNSUPPORTED_STATEMENT` | Statement type is not supported for permission checking |
| `TABLE_EXTRACT_FAILED` | No valid table name could be extracted |
| `SQL_PARSE_FAILED` | SQL statement could not be parsed |
| `TABLE_NOT_IN_POLICY` | The role policy does not authorize the table |
| `ROLE_NOT_DEFINED` | The role has no loaded permission policy |
| `RLS_PARSE_FAILED` | Parsing failed before row-level security injection |
| `RLS_MULTI_TABLE_UNSUPPORTED` | Row-level security supports single-table statements only |
| `RLS_PREDICATE_REJECTED` | The row-level security predicate could not be applied |

Denial reasons never carry raw SQL; table and role names are sanitized before entering errors and audit records.

### Wildcard Table

The `"*"` of the admin role in the permission policy example above is the wildcard table: it matches all tables and grants the role every operation listed for it.

### Action-Level Control

Restrict specific operations:

```yaml
roles:
  readonly:
    tables:
      - name: "reports"
        operations:
          - select  # only SELECT is allowed
```

---

## 🔄 Transactions

### Basic Transaction

```rust
use dbnexus::DbPool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("postgresql://localhost/mydb").await?;
    let session = pool.get_session("admin").await?;

    // Begin the transaction
    session.begin_transaction().await?;

    // Run the operations
    Model::insert(&session, user1).await?;
    Model::insert(&session, user2).await?;

    // Commit the transaction
    session.commit().await?;

    Ok(())
}
```

### Transaction with Error Handling

```rust
session.begin_transaction().await?;

match perform_operations(&session).await {
    Ok(_) => {
        session.commit().await?;
    }
    Err(e) => {
        eprintln!("Error: {}", e);
        session.rollback().await?;
    }
}
```

### RAII Transaction Guard

Wrap the "rollback on drop" semantics in a guard struct (`Drop` cannot `await`; the rollback must run explicitly in an async context or be evaluated immediately with `now_or_never`):

```rust
use dbnexus::{DbResult, Session};
use futures::FutureExt;

struct TransactionGuard<'a> {
    session: &'a Session,
    active: bool,
}

impl<'a> TransactionGuard<'a> {
    pub async fn begin(session: &'a Session) -> DbResult<Self> {
        session.begin_transaction().await?;
        Ok(Self { session, active: true })
    }

    pub async fn commit(mut self) -> DbResult<()> {
        self.active = false;
        self.session.commit().await
    }
}

impl Drop for TransactionGuard<'_> {
    fn drop(&mut self) {
        if self.active {
            // Roll back immediately on drop (evaluated in a synchronous context)
            let _ = self.session.rollback().now_or_never();
        }
    }
}

// Usage
let tx = TransactionGuard::begin(&session).await?;
Model::insert(&session, user).await?;
tx.commit().await?; // explicit commit; if an error occurred earlier and the guard was dropped, it rolls back automatically
```

---

## 🧱 Advanced Features

### Query Result Cache

After enabling the `cache` feature, a cache backend can be injected via `DbCacheProvider` to cache driver read-only query results (the write path invalidates automatically):

```toml
[dependencies.dbnexus]
version = "0.6.1"
features = ["cache"]
```

Implement the `DbCacheProvider` trait (see `examples/cache_standalone` for a complete custom-cache example) or use the built-in adapter (the `oxcache-integration` feature provides `OxcacheDbCacheAdapter`, adapting oxcache), and inject it via `DbPoolBuilder::cache_provider`; for the injection code and the Provider abstraction, see [API Reference · Kit and Cache Integration](API_REFERENCE.md#-kit-and-cache-integration).

The `#[db_entity(... cache(...))]` macro parameters generate cache configuration constants (`CACHE_TTL` / `CACHE_STRATEGY` / `CACHE_MAX_CAPACITY`) and a `cache_key()` helper for the entity, for use by the caching layer:

```rust
#[db_entity(
    table_name = "products",
    primary_key = "id",
    cache(ttl = 60, strategy = "lru", max_capacity = 5000)
)]
// ... entity definition

// Macro-generated cache configuration
assert_eq!(Product::CACHE_TTL, 60);
assert_eq!(Product::CACHE_STRATEGY, "lru");
assert_eq!(Product::CACHE_MAX_CAPACITY, 5000);
assert_eq!(Product::cache_key(1), "products:1");
```

### Metrics

Enable Prometheus metrics:

```toml
[dependencies.dbnexus]
version = "0.6.1"
features = ["metrics"]
```

`MetricsCollector` collects pool metrics and query statistics (including P99 latency percentiles) and exports Prometheus format; for the full code, see [API Reference · Observability API](API_REFERENCE.md#-observability-api):

```rust
let collector = MetricsCollector::new();
println!("{}", collector.export_prometheus());
```

### Audit Logging

```toml
[dependencies.dbnexus]
version = "0.6.1"
features = ["audit"]
```

Declare audit configuration for an entity with `#[db_entity(... audit(...))]`, and operations are recorded automatically; admin permission-bypass operations are audited as well:

```rust
#[db_entity(
    table_name = "sensitive_data",
    primary_key = "id",
    audit(table_name = "audit_log", operations = ["INSERT", "UPDATE", "DELETE"], log_values = true)
)]
// ... entity definition

// All operations are recorded in the audit log automatically
SensitiveData::insert(&session, data).await?;
SensitiveData::find_by_id(&session, 1).await?;
```

### DuckDB Embedded Database

DuckDB is an embedded analytical database suited to OLAP scenarios, integrated as an analytical read-only bypass (bypassing sea-orm). Since 0.3.0, true parallel queries are supported through the connection-pool mode.

```toml
[dependencies.dbnexus]
version = "0.6.1"
features = ["duckdb"]
```

For the URL format and the full API of `DuckDbConnection`, see [API Reference · DuckDB Database](API_REFERENCE.md#️-duckdb-database). `DuckDbRow` fetches values by column name: `row.get("column_name") -> Option<&DuckValue>`.

```rust
use dbnexus::DuckDbConnection;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a DuckDB connection (default pool size 4)
    let conn = DuckDbConnection::new(":memory:")?;

    // Create a table and insert data
    conn.execute("CREATE TABLE sales (id INTEGER, amount DOUBLE)").await?;
    conn.execute("INSERT INTO sales VALUES (1, 100.0), (2, 200.0)").await?;

    // Query
    let rows = conn.query("SELECT id, amount FROM sales").await?;
    for row in &rows {
        // Fetch values by column name
        if let (Some(id), Some(amount)) = (row.get("id"), row.get("amount")) {
            println!("id={:?}, amount={:?}", id, amount);
        }
    }

    // Health check
    conn.health_check().await?;
    Ok(())
}
```

### JWT Authentication

Enable the JWT-based user authentication system:

```toml
[dependencies.dbnexus]
version = "0.6.1"
features = ["authentication"]
```

For method signatures and exported types, see [API Reference · Authentication System](API_REFERENCE.md#-authentication-system). Related types: `AuthCredentials` (fields: `username`, `password`), `JwtClaims` (fields: `sub`, `username`, `role`, `exp`, `iat`, `token_type`).

```rust
use dbnexus::{AuthenticationManager, AuthCredentials};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // It is recommended to read the JWT secret from an environment variable, at least 32 bytes
    let secret = b"my-secret-key-32-bytes-long-xxxxx";
    let manager = AuthenticationManager::new(secret)?;

    // Register a user (the password must pass the strength check: at least 12 characters, with uppercase, lowercase, digits, and special characters)
    manager.register_user("alice", "Str0ng!Passw0rd", "admin").await?;

    // Authenticate
    let credentials = AuthCredentials {
        username: "alice".to_string(),
        password: "Str0ng!Passw0rd".to_string(),
    };
    let token = manager.authenticate(credentials).await?;
    println!("Token: {}", token);

    // Verify the token (synchronous method, no await needed)
    let claims = manager.verify_token(&token)?;
    println!("User: {}, Role: {}", claims.sub, claims.role);

    Ok(())
}
```

### DDL Security Guard

`DdlGuard` validates DDL statements based on AST (abstract syntax tree) analysis, working with the unified injection detection engine to block dangerous patterns. Requires the `sql-parser` feature.

```toml
[dependencies.dbnexus]
version = "0.6.1"
features = ["sql-parser"]
```

API notes:

- `DdlGuard::new() -> Self` — create a guard instance
- `guard.validate(sql: &str) -> Result<DdlValidationResult, String>` — validate SQL, returning a `Result` (`Err` when parsing fails)

The three variants of `DdlValidationResult`:

| Variant | Meaning |
|------|------|
| `Allowed` | validation passed |
| `Forbidden(String)` | the SQL was blocked, with a reason |
| `ParseError(String)` | the SQL failed to parse |

Statement types allowed by the allowlist: `CreateTable`, `AlterTable`, `DropTable`, `CreateIndex`, `DropIndex`, `CreateView`, `DropView`, `Truncate`, `Query` (SELECT), and the DML (`Insert` / `Update` / `Delete`) required by migration transactions. Dangerous patterns such as `DROP DATABASE` are hard-blocked by the unified injection detection engine.

Note: `Session::execute_raw_ddl` and the DuckDB raw execution channel already call `DdlGuard` automatically internally; only the admin role can execute DDL.

```rust
use dbnexus::{DdlGuard, DdlValidationResult};

fn main() {
    let guard = DdlGuard::new();

    // Allowed DDL (CREATE TABLE is in the allowlist)
    assert!(matches!(
        guard.validate("CREATE TABLE users (id INT)").unwrap(),
        DdlValidationResult::Allowed
    ));

    // Rejected dangerous pattern (DROP DATABASE hits the injection detection engine)
    match guard.validate("DROP DATABASE production").unwrap() {
        DdlValidationResult::Forbidden(reason) => {
            println!("Rejected: {}", reason);
        }
        _ => panic!("DROP DATABASE should be rejected"),
    }
}
```

### Shard Routing

Enable data sharding for horizontal scaling:

```toml
[dependencies.dbnexus]
version = "0.6.1"
features = ["sharding"]
```

For the `ShardRouter` method signatures and the `create_strategy` strategy factory (including the fallback behavior for unknown strategy names), see [API Reference · Sharding and Distributed Capabilities](API_REFERENCE.md#️-sharding-and-distributed-capabilities).

```rust
use dbnexus::ShardRouter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a 4-shard router with the hash strategy (synchronous method, does not create a pool)
    let router = ShardRouter::with_strategy("hash", 4);

    // Compute the shard ID for a key
    let shard_id = router.shard_id_for_key("user_123");
    println!("Shard: {}", shard_id);

    Ok(())
}
```

### Data Validation

After enabling the `validation` feature, `DbError` gains a `Validation(String)` variant, and the [`validator`](https://docs.rs/validator) crate is integrated to provide declarative validation:

```toml
[dependencies.dbnexus]
version = "0.6.1"
features = ["validation"]
```

```rust
use dbnexus::DbError;
use validator::Validate;

#[derive(Validate)]
struct UserInput {
    #[validate(email)]
    email: String,
    #[validate(length(min = 8))]
    password: String,
}

fn process_input(input: &UserInput) -> Result<(), DbError> {
    // Run the validator crate's validation, converting errors into DbError::Validation
    input.validate().map_err(|e| DbError::Validation(e.to_string()))?;
    // Process the input...
    Ok(())
}
```

### Graph Databases (requires the `ladybug` or `neo4j` feature)

DBNexus unifies graph databases through the `GraphConnection` trait; graph databases and relational databases are not mutually exclusive and can be mixed. For the constructors of `LadybugConnection` / `Neo4jConnection`, the `execute_cypher` method signatures, and exported types, see [API Reference · Graph Databases](API_REFERENCE.md#-graph-databases); raw Cypher execution is deprecated — use the `execute_cypher_with_params` parameterized channel (with injection protection) instead.

```toml
[dependencies.dbnexus]
version = "0.6.1"
features = ["ladybug"]  # or "neo4j"
```

```rust
use dbnexus::{GraphConnection, GraphExecResult, LadybugConnection};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The second parameter is the number of concurrent queries (Semaphore permits, minimum 1)
    let conn = LadybugConnection::new("ladybug::memory:", 4)?;

    // Create a node
    conn.execute_cypher("CREATE (n:User {name: 'Alice'})").await?;

    // Query: execute_cypher returns the GraphExecResult enum; reads are Query(GraphQueryResult)
    let result = conn.execute_cypher("MATCH (n:User) RETURN n").await?;
    match result {
        GraphExecResult::Query(q) => println!("Node count: {}", q.rows.len()),
        GraphExecResult::Write { rows_affected } => println!("Rows affected: {}", rows_affected),
    }

    Ok(())
}
```

### Internationalized Formatting (core feature, always available)

ICU4X-based locale-aware number/date/plural/collation formatting. Internationalization is a core system component and requires no extra feature; for the construction and usage of `DbI18nFormatter`, see [API Reference · Internationalization](API_REFERENCE.md#-internationalization).

---

## ✅ Best Practices

### 1. Use Environment Variables for Sensitive Data

Never hard-code credentials:

```rust
// Bad
let url = "postgresql://user:password@localhost/db";

// Good
let url = std::env::var("DATABASE_URL")?;
```

### 2. Always Use Transactions for Multi-Step Operations

```rust
// Bad: if user2 fails, user1 has already been inserted
Model::insert(&session, user1).await?;
Model::insert(&session, user2).await?;

// Good: either both succeed or both fail
session.begin_transaction().await?;
Model::insert(&session, user1).await?;
Model::insert(&session, user2).await?;
session.commit().await?;
```

### 3. Rely on RAII to Manage Sessions

The connection returns to the pool automatically when a session is dropped; manual intervention in connection release is unnecessary and should be avoided:

```rust
// Good: the connection returns when the scope ends
{
    let session = pool.get_session("admin").await?;
    Model::find_all(&session).await?;
} // The connection is returned automatically
```

### 4. Configure Appropriate Pool Sizes

Low-traffic applications use small pools (`min_connections: 1`, `max_connections: 5`); high-traffic applications scale by the concurrency watermark (`min_connections: 10`, `max_connections: 100`), and verify the capacity matches using the wait count from `pool.status()`.

### 5. Use Permission Control

Even for internal tools, converge permissions by role:

```rust
// Bad: everyday business uses admin, full access
let session = pool.get_session("admin").await?;

// Good: limited access, explicit permissions
let session = pool.get_session("read_only").await?;
```

### 6. Monitor Pool Status

Check pool health regularly:

```rust
let status = pool.status();
if status.wait_count > 1000 {
    eprintln!("Warning: high connection wait count");
}
```

### 7. Handle Errors Gracefully

```rust
// Bad: panics on error
let user = Model::find_by_id(&session, 1).await?.unwrap();

// Good: branch handling
match Model::find_by_id(&session, 1).await {
    Ok(Some(user)) => { /* use the user */ }
    Ok(None) => { /* handle not-found */ }
    Err(e) => { /* handle the error */ }
}
```

### 8. Use Type-Safe Operations

```rust
// Bad: raw SQL, no type safety, error-prone
session.execute("DELETE FROM users WHERE id = 1").await?;

// Good: type-safe, permission-checked automatically
Model::delete(&session, 1).await?;
```

---

## 🧯 Troubleshooting

### Connection Pool Exhaustion

**Symptom**: `DbError::Connection(...)` — `DbError` has no `ConnectionPool` variant; pool exhaustion surfaces through the sea-orm connection acquisition error as the `Connection` variant (an empty duckdb pool surfaces as `DbErr::Custom` carrying pool-exhaustion text).

**Solutions**:

1. Increase the pool size (raise `PoolConfig::max_connections`);
2. Check for connection leaks:

   ```rust
   let status = pool.status();
   println!("Active: {}, Total: {}", status.active, status.total);
   ```

3. Make sure sessions are dropped within scope following the RAII pattern.

### Permission Denied Errors

**Symptom**: `DbError::Permission("Permission denied...")`

**Solutions**:

1. Check whether the role is in the permission configuration:

   ```yaml
   roles:
     my_role:  # must match exactly
   ```

2. Verify the operation is allowed:

   ```yaml
   roles:
     my_role:
       tables:
         - name: "users"
           operations:
             - select  # make sure the operation is listed
   ```

3. Check the role's letter case (must match the configuration exactly):

   ```rust
   let session = pool.get_session("My_Role").await?; // Error: case mismatch
   let session = pool.get_session("my_role").await?; // Correct
   ```

### Database Connection Errors

**Symptom**: `DbError::Connection("Failed to connect...")`

**Solutions**:

1. Verify the URL format:

   ```text
   # SQLite
   sqlite::memory:
   sqlite:///path/to/db

   # PostgreSQL
   postgresql://user:password@host:port/database

   # MySQL
   mysql://user:password@host:port/database
   ```

2. Check network connectivity:

   ```bash
   ping postgres-server
   telnet postgres-server 5432
   ```

3. Verify credentials and permissions:

   ```bash
   psql -U username -h postgres-server -d database
   ```

### Slow Query Performance

**Symptom**: queries take too long

**Solutions**:

1. Enable and inspect metrics:

   ```rust
   let collector = MetricsCollector::new();
   if let Some(stats) = collector.get_query_stats("SELECT") {
       println!("P99 latency: {}ns", stats.latency_percentiles.p99_ns);
   }
   ```

2. Use indexes:

   ```sql
   CREATE INDEX idx_users_email ON users(email);
   ```

3. Filter in the database instead of fetching everything and filtering in memory:

   ```rust
   // Bad: fetch all, then filter in memory
   let users = Model::find_all(&session).await?;

   // Good: filter in the database
   let condition = Condition::all().add(Column::Name.like("%Alice%"));
   let users = Model::find_by_condition(&session, condition).await?;
   ```

### High Memory Usage

**Symptom**: the application uses too much memory

**Solutions**:

1. Reduce the pool size (lower `PoolConfig::max_connections`);
2. Enable connection idle timeout (lower `PoolConfig::idle_timeout`);
3. Check cache capacity: lower the entity `cache(max_capacity = ...)` parameter or the cache provider capacity.

---

## 📦 Complete Example

Adapted from [examples/src/basic/basic_crud.rs](../../examples/src/basic/basic_crud.rs) (52 runnable examples in total, see [examples/README.md](../../examples/README.md)):

```rust
use dbnexus::{db_entity, DbPool};
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the connection pool (embedded SQLite, no external database needed)
    let pool = DbPool::new("sqlite::memory:").await?;

    // Admin session (RAII: the connection returns automatically on drop)
    let session = pool.get_session("admin").await?;

    // Create the table (DDL is admin-only, validated by DdlGuard)
    session
        .execute_raw_ddl("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, email TEXT)")
        .await?;

    // Insert a user
    let user = Model {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    Model::insert(&session, user).await?;

    // Query
    let users = Model::find_all(&session).await?;
    println!("Found {} users", users.len());

    // Pool status
    let status = pool.status();
    println!("Pool status: total {}, active {}, idle {}", status.total, status.active, status.idle);

    Ok(())
}
```

---

## 📚 Related Documents

| Document | Description |
|------|------|
| [📘 API Reference](API_REFERENCE.md) | Detailed description of all public APIs |
| [🏗️ Architecture](ARCHITECTURE.md) | Design philosophy, module breakdown, data flow, and security/performance design |
| [🔒 Security](SECURITY.md) | Defense-in-depth design, best practices, and the vulnerability reporting process |
| [📊 Performance Baseline](PERFORMANCE.md) | End-to-end benchmark data and reproduction commands |
| [🧪 Test Scenario Baseline](TEST_SCENARIOS.md) | Test pyramid baseline, driver-group matrix, and E2E scenario definitions |
| [📦 Online API docs](https://docs.rs/dbnexus) | Latest documentation auto-generated by docs.rs |

---

## 🏢 Enterprise Features Complete List (opt-in)

| Feature | Description |
|------|------|
| `metrics` | Prometheus-format metrics export with slow-query detection |
| `audit` | Audit logging; admin bypass operations are recorded as well |
| `migration` / `auto-migrate` | Database migrations and automatic migration execution |
| `sharding` | Data sharding: consistent-hash strategies and session-level shard routing |
| `global-index` | Cross-shard global index |
| `cache` | oxcache cache (moka L1 backend), lock-free `ArcSwap` reads |
| `permission-engine` | Advanced permission engine: policy decision point, role inheritance, caching and rate limiting |
| `authentication` | JWT authentication (access/refresh token distinction) + bcrypt password policy |
| `data-protection` 🆕 | Field-level masking (mask/hash/truncate) and row-level security predicate injection (row-read exit + raw write exit UPDATE/DELETE; single-table statements; multi-table/set operations are rejected fail-closed) |
### RLS JOIN degradation (`data-protection`)

By default `RlsPolicy` rejects multi-table statements (JOIN/subquery), fail-closed. A policy can explicitly declare `on_join_failure: ApplyToPrimaryTableOnly` (serde key `apply_to_primary_table_only`) to inject the tenant predicate into the primary table (FROM first table) only. Other tables rely on per-table RBAC checks; row-level isolation is **not** applied to them.

- `JoinFailurePolicy::Reject` (default): multi-table statements are denied with `RLS_MULTI_TABLE_UNSUPPORTED`; existing configurations are unchanged.
- Top-level set operations (UNION/INTERSECT/EXCEPT) are always rejected, regardless of the join policy.
- INNER predicates in subqueries are not injected; their tables are covered by per-table RBAC only.
- Each degraded execution emits an `rls_degraded` audit event (operation `Other("rls_degraded")`, severity Low) with a sanitized controlled-table list and no SQL text; a `log::warn` is emitted when no audit feature is compiled in.
- Configure via `PermissionFacadeConfig::with_rls_policy_join_policy(table, column, value, JoinFailurePolicy::ApplyToPrimaryTableOnly)`; the three-argument `with_rls_policy` keeps the Reject default.

| `permission-facade` 🆕 | RBAC + masking + RLS unified facade, configure once and it applies everywhere (the admin channel skips tenant predicates) |
| `query-dsl` 🆕 | `q!` type-safe query fragment macro, immune to identifier and value injection |
| `repository` / `data-api` 🆕 | Generic `Repository<T>`; entity-to-JSON data API gateway |
| `prepare-cache` 🆕 | Statement-level prepared statement LRU cache with hit-rate metrics |
| `copy` 🆕 | Batch write: COPY wrappers (pg protocol / DuckDB file COPY gated per driver) + multi-row INSERT building (batch_insert, 500-row chunked parameterized) |
| `entity-events` 🆕 | Entity event bus + Outbox persistent dispatch |
| `otel` 🆕 | Health snapshot metrics exported via OTLP/HTTP (stdout fallback) |
| `kit` | trait-kit AsyncKit integration; register once for pool/cache/audit/health capabilities |
| `config-confers` 🆕 | confers hot config reload (atomic `ArcSwap` swap) |
| `retry` | Runtime retry: idempotency check + exponential backoff |
| `failover` | Connection failover: CircuitBreaker state machine coordinated with health checks |
| `replica-routing` | Replica routing read/write splitting: weight/latency-aware selection with half-open recovery |
| `scatter-gather` | Cross-shard aggregate queries with SUM / COUNT / AVG merging |
| `saga` | Saga distributed transactions: persistent log, startup recovery, compensation orchestration |
| `distributed-id` | Snowflake distributed ID generation |

> 🆕 marks capabilities added in 0.6.0-rc.4 and later; see the [Changelog](CHANGELOG.md) for per-item versions (http-health added in 0.6.0-rc.6).

## Security Posture Self-Audit

`dbnexus-cli doctor` reports the security features compiled into the current build as JSON. Use `--profile development` (default, report only) or `--profile production` (judge Required checks).

| Check ID | Feature | Severity |
|---|---|---|
| `rbac` | `permission` | Required |
| `injection-detection` | `sql-parser` | Required |
| `policy-cache` | `cache` | Recommended |
| `rls-and-masking` | `data-protection` | Recommended |
| `pdp` | `permission-engine` | Info |
| `unified-facade` | `permission-facade` | Info |
| `audit` | `audit` | Recommended |
| `jwt-auth` | `authentication` | Recommended |
| `drivers` | any relational driver | Info |

In `production`, a missing Required check yields `NonCompliant` (exit code 1), a missing Recommended check yields `Degraded` (exit 0), otherwise `Compliant` (exit 0). Invalid usage exits 2. The library function `dbnexus::security_posture(PostureProfile)` returns the same report.

## Policy Dry-Run

Before swapping a permission policy, evaluate how it changes access without connecting to a database:

```bash
dbnexus-cli policy-dry-run \
  --baseline current.yaml \
  --candidate next.yaml \
  --queries queries.sql \
  --role reporting
```

`queries.sql` holds one SQL per line (`--` comments and blank lines are skipped). Each query is classified as `allow_to_allow`, `allow_to_deny` (regression, with a missing-grant suggestion), `deny_to_allow`, `deny_to_deny`, or `unparsable`. Exit codes: 0 = no regressions, 1 = at least one `allow_to_deny`, 2 = file/usage error. A missing `--baseline` means an empty (deny-all) baseline.

Coverage boundary: only table-level RBAC (`RolePolicy::allows`, including wildcard tables) is evaluated. RLS predicates, query-origin shape constraints and rate limits are not part of the dry run; treat the report as a table-access diff, not a full semantic guarantee.

## Query Origin Governance

`QueryOrigin` adds a second governance dimension alongside roles. Declare the source when issuing a session:

```rust
use dbnexus::{QueryOrigin, DbPool};

let session = pool
    .get_session_with_origin("reporting", QueryOrigin::Agent("claude-code".into()))
    .await?;
```

`Session::query_rows` then applies the matching `origin_constraints` from the role policy:

```yaml
roles:
  reporting:
    tables: [...]
    origin_constraints:
      - origin_match: agent        # human | service | agent | any
        max_rows: 500             # reject when exceeded; never silently truncate
        deny_columns: [ssn, email] # removed from result rows
```

Merge rules: `agent` rules take precedence over `any` for Agent sources; matched rules take the smallest `max_rows` and the union of `deny_columns`. Admin sessions and sources without matching constraints are unchanged.

Boundary: the origin is declared by the caller at session issuance — it is a library contract, not an in-process enforced identity. MCP tools are the first enforcement point. Row limits reject the entire query; denied columns are removed after retrieval (the data still passes through the driver).

## MCP Tool Surface (`mcp` feature)

`dbnexus-cli mcp-serve` runs a stdio MCP server. Its tools are constrained by the permission engine — no raw SQL tool is exposed:

| Tool | Behavior |
|---|---|
| `dbnexus_query(table, filters, limit)` | Registered endpoint only; column whitelist; `eq`/`contains` filters; `limit` clamped to `max_page_size`; full RBAC/RLS/masking chain via `DataApiGateway` |
| `dbnexus_schema(table)` | Declared endpoint metadata (table, columns, orderable, page limits); no live schema introspection; unregistered tables return a uniform error |
| `dbnexus_explain(sql)` | `Session::explain_access` preview: allow/deny, tables checked, denial reason code, RLS applied/degraded; SQL is not executed |

Configuration is via environment variables: `DBNEXUS_MCP_DATABASE_URL`, `DBNEXUS_MCP_ROLE`, `DBNEXUS_MCP_ENDPOINTS` (JSON array of `{table, columns, max_page_size}`), and optional `DBNEXUS_MCP_OWNER_COLUMN` / `DBNEXUS_MCP_OWNER_VALUE`. Missing required values or invalid JSON exits with code 2.

Operational boundary: stdio mode uses one service role. For multiple agents or roles, run one server process per role; per-session role switching needs a future authenticated transport.

## Feature Flags Reference

`default = []`: no default features; runtime, database driver and functional features must all be enabled explicitly. Embedded (`sqlite`/`duckdb`) and server-side (`postgres`/`mysql`) drivers are strictly mutually exclusive; mixing them fails the build.

### Runtimes (mutually exclusive, pick one)

| Flag | Description | Default |
|------|------|:----:|
| `runtime-tokio-rustls` | Tokio runtime + rustls TLS | No |
| `runtime-tokio-native-tls` | Tokio runtime + native-tls | No |
| `runtime-async-std` | async-std runtime | No |

### Database Drivers

Relational drivers are mutually exclusive by group (compile-time `compile_error!` guards): `postgres` and `mysql` are exclusive, embedded (`sqlite`/`duckdb`) and server-side (`postgres`/`mysql`) are exclusive, `duckdb` and `ladybug` are exclusive; `sqlite` and `duckdb` can coexist. Graph drivers can coexist with relational drivers. `sqlite` / `postgres` / `mysql` are backed by the corresponding sea-orm/sqlx drivers, while `duckdb` (embedded analytical), `ladybug` (formerly Kuzu) and `neo4j` use their own native bindings; see [Choosing a Database Driver](#choosing-a-database-driver) for each driver's database and type.

> ℹ️ **`duckdb` feature status update** (October 2026): the earlier resolution failure caused by the full yank of the upstream `zip` crate no longer applies under the current lockfile — `libduckdb-sys` / `duckdb` are at 1.20000.0 in `Cargo.lock`, whose build-time dependencies no longer include `zip`. Existing projects can follow by refreshing the lockfile (`cargo update -p libduckdb-sys`); the compile-time mutual exclusion between `duckdb` and `ladybug` still applies (see [Choosing a Database Driver](#choosing-a-database-driver)).

### Core Capabilities

| Flag | Description | Default |
|------|------|:----:|
| `permission` | Table-level RBAC; hard-depends on `sql-parser` and enables `yaml` + `cache` | No |
| `sql-parser` | SQL parsing, table extraction and injection detection (the parse-result cache is backed by the oxcache memory backend; does not imply the `cache` feature) | No |
| `macros` | `dbnexus-macros` procedural macros (`db_entity` / `db_repository`) | No |
| `default-no-db` | Driver-less default aggregate (runtime + permission + sql-parser + macros + config-env + with-time) for CI driver-matrix testing | No |

### Data Access & Integrations

<details>
<summary>📋 Table</summary>

| Flag | Description | Default |
|------|------|:----:|
| `cache` | oxcache cache (moka L1 backend) + lock-free `ArcSwap` reads | No |
| `cache-available` | Internal capability marker (no code of its own): implied by `cache` and `oxcache-integration`, lets downstream crates and gating tests detect whether the cache surface is ready | No |
| `oxcache-integration` | OxcacheDbCacheAdapter + `OxcacheQueryCache` query-cache decorator (query_cached/invalidate_table) | No |
| `kit` | trait-kit AsyncKit integration with the full pool/cache/audit/health capability closure | No |
| `repository` | Generic `Repository<T>` CRUD port + `impl_json_repository!` macro | No |
| `data-api` | Data API gateway: entity-to-JSON query endpoints (allowlist + filtering + pagination) | No |
| `prepare-cache` | Statement-level prepared statement LRU cache | No |
| `query-dsl` | `q!` type-safe query fragment macro | No |
| `entity-events` | Entity event bus + Outbox | No |
| `copy` | Batch write: COPY wrappers + multi-row INSERT building (batch_insert) | No |
| `data-protection` | Field masking and row-level security predicate injection | No |
| `permission-facade` | RBAC + masking + RLS unified facade | No |
| `config-confers` | confers hot config reload | No |

</details>

### Observability

| Flag | Description | Default |
|------|------|:----:|
| `metrics` | Prometheus-format metrics export (with slow-query detection) | No |
| `health-check` | Health check module and `health_snapshot` structured export | No |
| `observability` | `metrics` + `health-check` aggregate | No |
| `otel` | OTLP/HTTP JSON envelope export bridge | No |
| `http-health` 🆕 | HTTP health endpoint generator: `HealthRouterBuilder` produces an axum Router mounting `/healthz` (liveness) / `/readyz` (readiness, pool snapshot + circuit breaker) / `/metrics` (Prometheus); generates the Router only, no serve loop; zero HTTP deps in the default build | No |

> Structured logging: the four wiring points (pool acquisition timeout, permission denied, circuit breaker state transitions, slow queries) emit records through the `log` facade unconditionally — a no-op when no logger is installed, so the default build is unchanged. The backend (e.g. inklog's `LoggerManager`) is installed as the global `log` backend **by the consumer**; dbnexus itself depends on no logging-backend crate (inklog already optionally depends on dbnexus, and a reverse edge would form a package-level cycle — see the adapter-direction convention in [ARCHITECTURE.md](ARCHITECTURE.md)).

### Data Management

| Flag | Description | Default |
|------|------|:----:|
| `migration` | Database migrations | No |
| `auto-migrate` | Automatic migration execution | No |
| `sharding` | Data sharding (strategies and session-level routing) | No |
| `global-index` | Cross-shard global index | No |
| `data-management` | Aggregate of the four above | No |

### Distributed Capabilities

<details>
<summary>📋 Table</summary>

| Flag | Description | Default |
|------|------|:----:|
| `retry` | Runtime retry + exponential backoff (idempotency check) | No |
| `failover` | Connection failover (CircuitBreaker + health checks) | No |
| `replica-routing` | Replica routing read/write splitting | No |
| `scatter-gather` | Cross-shard aggregate query executor | No |
| `shard-migration` | Shard migration orchestration | No |
| `saga` | Saga distributed transaction orchestration (persistent recovery) | No |
| `distributed-id` | Snowflake distributed ID | No |
| `distributed-capabilities` | Aggregate of the seven above | No |

</details>

### Security & Compliance

| Flag | Description | Default |
|------|------|:----:|
| `audit` | Audit logging (operations + user context) | No |
| `permission-engine` | Advanced permission engine (depends on `permission`) | No |
| `authentication` | JWT authentication + bcrypt password strength policy | No |
| `security` | `audit` + `permission-engine` aggregate | No |

<details>
<summary>📦 Typing, config sources, pool enhancements and dev-tool flags</summary>

| Flag | Description | Default |
|------|------|:----:|
| `with-json` / `with-time` / `with-chrono` / `with-uuid` | sea-orm type bridges (JSON / time / chrono / UUID fields) | No |
| `validation` | validator-based data validation | No |
| `yaml` | YAML permission/config file parsing | No |
| `config-env` | Environment variable config (no extra dependency) | No |
| `pool-health-check` | Connection pool health checks | No |
| `pool-warmup` | Connection pool warmup | No |
| `test-utils` | Test utilities (tempfile / assert_cmd) | No |
| `cli-tests` | CLI integration test gating | No |

</details>

### Presets

| Preset | Features | Use Case |
|------|------|----------|
| `embedded` | `runtime-tokio-rustls`, `sqlite`, `config-env` | Ultra-minimal for embedded/edge devices |
| `microservice` | `runtime-tokio-rustls`, `postgres`, `permission`, `sql-parser`, `config-env`, `observability` | Microservice deployment |
| `monolith` | `runtime-tokio-rustls`, `postgres`, `permission`, `sql-parser`, `yaml`, `data-management`, `security`, `observability`, all 7 distributed capabilities | Monolithic application |
| `enterprise` | `postgres`, `monolith`, `permission-engine` | Full enterprise features |
| `all-optional` | 26 optional features except database drivers (cache / observability / data-management / security / migration / retry / failover / replica-routing / scatter-gather / shard-migration / saga / distributed-id / repository / data-api / prepare-cache / validation / authentication / query-dsl / entity-events / permission-facade / config-confers / copy / otel / kit / data-protection / http-health) | Full-feature verification (add drivers manually) |

### Usage Examples

<details>
<summary>📄 Code · toml</summary>

```toml
# Embedded/edge devices (minimal)
dbnexus = { version = "0.6.1", features = ["embedded"] }

# Microservices
dbnexus = { version = "0.6.1", features = ["microservice"] }

# Monolithic applications
dbnexus = { version = "0.6.1", features = ["monolith"] }

# Enterprise (full features)
dbnexus = { version = "0.6.1", features = ["enterprise"] }
```

</details>
