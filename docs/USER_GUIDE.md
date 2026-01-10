# DBNexus User Guide

<div align="center">

**Complete guide to using DBNexus**

</div>

## Table of Contents

1. [Installation](#installation)
2. [Quick Start](#quick-start)
3. [Configuration](#configuration)
4. [Connection Pooling](#connection-pooling)
5. [Permission Control](#permission-control)
6. [Audit Logging](#audit-logging)
7. [Database Migrations](#database-migrations)
8. [Sharding](#sharding)
9. [Metrics](#metrics)
10. [CLI Tool](#cli-tool)
11. [Examples](#examples)

---

## Installation

### Cargo.toml

Add DBNexus to your `Cargo.toml`:

```toml
[dependencies]
dbnexus = "0.1"

# Choose your database driver
dbnexus = { version = "0.1", features = ["sqlite"] }
dbnexus = { version = "0.1", features = ["postgres"] }
dbnexus = { version = "0.1", features = ["mysql"] }
```

### Optional Features

Enable additional functionality:

```toml
[features]
default = []

# All features (except database driver)
all-optional = [
    "metrics",
    "migration",
    "auto-migrate",
    "sharding",
    "global-index",
    "cache",
    "audit",
    "tracing",
    "permission-engine"
]
```

---

## Quick Start

### Basic Database Connection

```rust
use dbnexus::{DbPool, DbConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = DbConfig {
        url: "sqlite://example.db".to_string(),
        max_connections: 10,
        min_connections: 2,
        idle_timeout: 300,
        acquire_timeout: 3000,
        permissions_path: None,
        migrations_dir: None,
        auto_migrate: false,
        migration_timeout: 60,
        admin_role: "admin".to_string(),
    };

    let pool = DbPool::with_config(config).await?;
    let session = pool.get_session("admin").await?;

    // Execute a simple query
    let result = session.execute_raw("SELECT 1").await?;
    println!("Query result: {:?}", result);

    Ok(())
}
```

### With Environment Variables

```rust
// Configure from environment
let config = DbConfig::from_env()?;
let pool = DbPool::with_config(config).await?;
```

---

## Configuration

### DbConfig Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `url` | String | Yes | - | Database connection URL |
| `max_connections` | u32 | No | 10 | Maximum pool size |
| `min_connections` | u32 | No | 2 | Minimum pool size |
| `idle_timeout` | u64 | No | 300 | Idle timeout (seconds) |
| `acquire_timeout` | u64 | No | 3000 | Acquire timeout (ms) |
| `permissions_path` | Option<PathBuf> | No | None | Permission config file |
| `migrations_dir` | Option<PathBuf> | No | None | Migration files directory |
| `auto_migrate` | bool | No | false | Auto-run migrations |
| `migration_timeout` | u64 | No | 60 | Migration timeout (seconds) |
| `admin_role` | String | No | "admin" | Admin role name |

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DB_URL` | Database connection URL | - |
| `DB_MAX_CONNECTIONS` | Maximum connections | 10 |
| `DB_MIN_CONNECTIONS` | Minimum connections | 2 |
| `DB_IDLE_TIMEOUT` | Idle timeout (seconds) | 300 |
| `DB_ACQUIRE_TIMEOUT` | Acquire timeout (ms) | 3000 |
| `DB_ADMIN_ROLE` | Admin role name | "admin" |
| `DB_PERMISSIONS_PATH` | Permission config path | - |
| `DB_MIGRATIONS_DIR` | Migrations directory | - |

### Configuration File

Create a `DbConfig` from a TOML file:

```toml
# config.toml
url = "sqlite://example.db"
max_connections = 10
min_connections = 2
idle_timeout = 300
acquire_timeout = 3000
auto_migrate = true
migrations_dir = "./migrations"
admin_role = "admin"
```

```rust
let config = DbConfig::from_toml("config.toml")?;
```

---

## Connection Pooling

### Creating a Pool

```rust
let pool = DbPool::new("sqlite://example.db", 10).await?;
```

### Getting a Session

```rust
let session = pool.get_session("admin").await?;
```

### Session Operations

```rust
// Execute raw SQL
let result = session.execute_raw("SELECT * FROM users").await?;

// Execute with permission checks
let result = session.execute("SELECT * FROM users").await?;

// With transaction
let mut tx = session.begin().await?;
tx.execute("INSERT INTO users (name) VALUES ('John')").await?;
tx.commit().await?;
```

### Pool Status

```rust
let status = pool.status();
println!("Total: {}, Active: {}, Idle: {}",
    status.total, status.active, status.idle);
```

---

## Permission Control

### Permission Configuration File

Create a `permissions.yaml` file:

```yaml
roles:
  admin:
    - table: "*"
      operations: ["SELECT", "INSERT", "UPDATE", "DELETE"]
  manager:
    - table: "users"
      operations: ["SELECT", "INSERT", "UPDATE"]
    - table: "orders"
      operations: ["SELECT", "INSERT"]
  user:
    - table: "users"
      operations: ["SELECT"]
    - table: "orders"
      operations: ["SELECT"]
  guest:
    - table: "products"
      operations: ["SELECT"]
```

### Using Permissions

```rust
let config = DbConfig {
    url: "sqlite://example.db".to_string(),
    permissions_path: Some("./permissions.yaml".into()),
    ..Default::default()
};

let pool = DbPool::with_config(config).await?;

// Admin can do anything
let admin_session = pool.get_session("admin").await?;
admin_session.execute("DROP TABLE users").await?; // Allowed

// Regular user has limited access
let user_session = pool.get_session("user").await?;
match user_session.execute("DROP TABLE users").await {
    Ok(_) => println!("Success"),
    Err(DbError::Permission(msg)) => println!("Denied: {}", msg),
}
```

### Rate Limiting

The permission engine includes rate limiting (100 requests/60s by default):

```rust
// Check remaining requests
let remaining = permission_ctx.remaining_requests("user").await;
println!("Remaining: {}", remaining);

// Reset rate limit
permission_ctx.reset_rate_limit("user").await;
```

---

## Audit Logging

### Basic Usage

```rust
#[cfg(feature = "audit")]
{
    use dbnexus::audit::{AuditLogger, AuditConfig, AuditEvent};

    let config = AuditConfig::default();
    let storage = Arc::new(MemoryAuditStorage::new(1000));
    let logger = AuditLogger::new(config, storage);

    // Log an event
    let event = AuditEvent::create("users", "1", "admin");
    logger.log(event).await?;

    // Query audit logs
    let filters = AuditQueryFilters::default();
    let events = logger.query(&filters).await?;
}
```

### Automatic Audit with Macros

```rust
use dbnexus_macros::db_audit;

#[db_audit(entity = "users", operation = "create")]
async fn create_user(name: &str) -> Result<User, DbError> {
    // Your implementation
    Ok(user)
}
```

### Data Sanitization

Sensitive fields are automatically redacted:

```rust
// Password field will be sanitized
let event = AuditEvent::update("users", "1", "admin",
    Some(r#"{"password": "secret123"}"#.to_string()),
    Some(r#"{"password": "***REDACTED***"}"#.to_string())
);
```

---

## Database Migrations

### Creating Migrations

Create SQL files in your migrations directory:

```sql
-- migrations/001_create_users.sql
-- UP: Create users table
CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    email TEXT UNIQUE NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- DOWN: Drop users table
DROP TABLE users;
```

### Running Migrations

```rust
let pool = DbPool::new("sqlite://example.db", 10).await?;

// Manual migration execution
let executor = MigrationExecutor::new(pool.clone(), "./migrations".into());
executor.run_pending_migrations().await?;

// With auto-migrate
let config = DbConfig {
    url: "sqlite://example.db".to_string(),
    migrations_dir: Some("./migrations".into()),
    auto_migrate: true,
    ..Default::default()
};
let pool = DbPool::with_config(config).await?;
```

### CLI Migration Commands

```bash
# Apply all pending migrations
dbnexus-cli migrate up --url "sqlite://example.db" --dir ./migrations

# Rollback last migration
dbnexus-cli migrate rollback --url "sqlite://example.db" --dir ./migrations

# Generate a new migration
dbnexus-cli migrate generate --name create_users --dir ./migrations

# Check migration status
dbnexus-cli migrate status --url "sqlite://example.db" --dir ./migrations
```

---

## Sharding

### Shard Configuration

```rust
use dbnexus::sharding::{ShardRouter, ShardConfig};

let configs = vec![
    ShardConfig::new("shard0", "sqlite://shard0.db".to_string()),
    ShardConfig::new("shard1", "sqlite://shard1.db".to_string()),
    ShardConfig::new("shard2", "sqlite://shard2.db".to_string()),
];

let router = ShardRouter::with_configs(configs, |key| {
    // Hash-based sharding key
    (key.hash() % 3) as u32
});
```

### Cross-Shard Queries

```rust
// Query all shards
let results = router.broadcast_query("SELECT COUNT(*) FROM users").await?;
```

---

## Metrics

### Enabling Metrics

```toml
[dependencies]
dbnexus = { version = "0.1", features = ["metrics"] }
```

### Collecting Metrics

```rust
#[cfg(feature = "metrics")]
{
    use dbnexus::metrics::MetricsCollector;

    let collector = MetricsCollector::new();
    pool.set_metrics(collector.clone());

    // Get metrics
    let pool_metrics = collector.get_pool_metrics().await;
    println!("Active connections: {}", pool_metrics.active_connections);
}
```

### Prometheus Integration

```rust
#[cfg(feature = "metrics")]
{
    use prometheus::Encoder;

    let encoder = TextEncoder::new();
    let metric_families = collector.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer)?;
}
```

---

## CLI Tool

### Installation

```bash
cargo install dbnexus-cli
```

### Commands

```bash
# Database operations
dbnexus-cli query "SELECT 1" --url "sqlite://example.db"
dbnexus-cli migrate --help

# Generate diff SQL
dbnexus-cli diff --from schema_v1.sql --to schema_v2.sql --output migration.sql

# Pool management
dbnexus-cli pool status --url "sqlite://example.db"
```

### Environment Variables

```bash
export DB_URL="sqlite://example.db"
export DB_MAX_CONNECTIONS=10

dbnexus-cli query "SELECT 1"
```

---

## Examples

### Complete Example with Permissions

See [`examples/permissions.rs`](examples/permissions.rs) for a full example.

### Complete Example with Sharding

See [`examples/sharding.rs`](examples/sharding.rs) for a distributed database example.

---

## Troubleshooting

### Connection Issues

```
Error: DbError::Connection(...)
- Check database URL is correct
- Ensure database server is running
- Verify firewall rules for remote databases
```

### Permission Denied

```
Error: DbError::Permission(...)
- Check role exists in permissions.yaml
- Verify table/operation is allowed
- Check admin role configuration
```

### Migration Failures

```
Error: DbError::Migration(...)
- Check migration SQL syntax
- Verify migrations directory exists
- Check migration timeout settings
```

---

## Best Practices

1. **Pool Sizing**: Set `max_connections` based on expected load
2. **Permissions**: Use least-privilege principle for roles
3. **Migrations**: Always test migrations before production
4. **Monitoring**: Enable metrics for production deployments
5. **Auditing**: Enable audit logging for compliance requirements

---

## Version

0.1.0

## Authors

DBNexus Team
