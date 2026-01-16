# DBNexus User Guide

Complete guide for using DBNexus in your applications.

## Table of Contents

- [Getting Started](#getting-started)
- [Installation](#installation)
- [Configuration](#configuration)
- [Defining Entities](#defining-entities)
- [Working with Connections](#working-with-connections)
- [CRUD Operations](#crud-operations)
- [Permission Control](#permission-control)
- [Transactions](#transactions)
- [Advanced Features](#advanced-features)
- [Best Practices](#best-practices)
- [Troubleshooting](#troubleshooting)

---

## Getting Started

This guide will take you from zero to production-ready database usage with DBNexus.

### What You'll Learn

- How to set up DBNexus in your project
- How to define database entities
- How to perform CRUD operations
- How to implement role-based access control
- How to configure and optimize connections
- How to use advanced features like caching and metrics

### Prerequisites

- Rust 1.85 or later
- Basic knowledge of Rust and SQL
- A database (PostgreSQL, MySQL, or SQLite)

---

## Installation

### 1. Add Dependency

Add to your `Cargo.toml`:

```toml
[dependencies]
dbnexus = "0.1"
tokio = { version = "1.42", features = ["rt-multi-thread", "macros"] }
```

### 2. Choose Features

Select the features you need:

```toml
# SQLite with basic features
dbnexus = { version = "0.1", features = ["sqlite", "permission", "sql-parser"] }

# PostgreSQL with enterprise features
dbnexus = { version = "0.1", features = [
    "postgres",
    "permission",
    "metrics",
    "tracing",
    "audit"
] }

# Minimal for embedded
dbnexus = { version = "0.1", default-features = false, features = ["minimal"] }
```

See [README.md](README.md#feature-flags) for complete feature list.

### 3. Enable Database Driver

Choose one database driver:

```toml
# SQLite (default)
dbnexus = { version = "0.1", features = ["sqlite"] }

# PostgreSQL
dbnexus = { version = "0.1", features = ["postgres"] }

# MySQL
dbnexus = { version = "0.1", features = ["mysql"] }
```

**Important:** Only one database driver can be enabled at a time.

### 4. Verify Installation

```rust
use dbnexus::DbPool;

fn main() {
    println!("DBNexus is ready!");
}
```

---

## Configuration

### Quick Start with Environment Variables

The simplest way to configure DBNexus is using environment variables.

#### Step 1: Set Environment Variables

```bash
export DATABASE_URL="postgresql://user:password@localhost/mydb"
export DB_MAX_CONNECTIONS=20
export DB_MIN_CONNECTIONS=5
export DB_ADMIN_ROLE=admin
```

#### Step 2: Load Configuration

```rust
use dbnexus::DbPool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new().await?;
    println!("Connected!");
    Ok(())
}
```

### Using Configuration Files

#### YAML Configuration

Create `dbnexus.yaml`:

```yaml
url: "postgresql://localhost/mydb"
max_connections: 20
min_connections: 5
idle_timeout: 300
acquire_timeout: 5000
auto_migrate: true
admin_role: admin
```

#### Load YAML Configuration

```rust
use dbnexus::config::ConfigLoader;

let config = ConfigLoader::from_yaml_file("dbnexus.yaml")?;
let pool = DbPool::try_from_config(config).await?;
```

#### TOML Configuration

Create `dbnexus.toml`:

```toml
url = "postgresql://localhost/mydb"
max_connections = 20
min_connections = 5
idle_timeout = 300
acquire_timeout = 5000
auto_migrate = true
admin_role = "admin"
```

#### Load TOML Configuration

```rust
#[cfg(feature = "config-toml")]
use dbnexus::config::ConfigLoader;

let config = ConfigLoader::from_toml_file("dbnexus.toml")?;
let pool = DbPool::try_from_config(config).await?;
```

### Using the Builder Pattern

For programmatic configuration:

```rust
use dbnexus::{DbPool, config::DbConfigBuilder};

let config = DbConfigBuilder::new()
    .url("postgresql://localhost/mydb")
    .max_connections(20)
    .min_connections(5)
    .idle_timeout(300)
    .acquire_timeout(5000)
    .auto_migrate(true)
    .admin_role("admin")
    .build()?;

let pool = DbPool::try_from_config(config).await?;
```

### Configuration Parameters

| Parameter | Type | Default | Description |
|-----------|-------|----------|-------------|
| `url` | `String` | Required | Database connection URL |
| `max_connections` | `u32` | 20 | Maximum pool size |
| `min_connections` | `u32` | 5 | Minimum pool size |
| `idle_timeout` | `u64` | 300 | Idle connection timeout (seconds) |
| `acquire_timeout` | `u64` | 5000 | Connection acquisition timeout (ms) |
| `permissions_path` | `Option<String>` | None | Path to permissions config |
| `migrations_dir` | `Option<PathBuf>` | None | Migration directory |
| `auto_migrate` | `bool` | false | Auto-run migrations |
| `migration_timeout` | `u64` | 60 | Migration timeout (seconds) |
| `admin_role` | `String` | "admin" | Admin role name |

---

## Defining Entities

### Basic Entity Definition

Define a struct that maps to a database table:

```rust
use dbnexus::{DbEntity, db_crud};

#[derive(DbEntity)]
#[db_entity]
#[table_name = "users"]
#[db_crud]
pub struct User {
    #[primary_key]
    pub id: i64,
    pub name: String,
    pub email: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
```

**Required Attributes:**

- `#[derive(DbEntity)]` - Enables DBNexus entity features
- `#[db_entity]` - Marks struct as database entity
- `#[table_name = "..."]` - Specifies table name
- `#[db_crud]` - Generates CRUD methods
- `#[primary_key]` - Marks primary key field

### Entity with Permission Control

Add role-based access control:

```rust
#[derive(DbEntity)]
#[db_entity]
#[table_name = "users"]
#[db_crud]
#[db_permission(roles = ["admin", "manager"])]
pub struct User {
    #[primary_key]
    pub id: i64,
    pub name: String,
}
```

### Entity with Operation-Level Control

Specify allowed operations:

```rust
#[derive(DbEntity)]
#[db_entity]
#[table_name = "users"]
#[db_crud]
#[db_permission(
    roles = ["admin", "manager"],
    operations = ["SELECT", "INSERT", "UPDATE"]
)]
pub struct User {
    #[primary_key]
    pub id: i64,
    pub name: String,
}
```

### Entity with Caching

Enable caching for read operations:

```rust
#[derive(DbEntity)]
#[db_entity]
#[table_name = "users"]
#[db_crud]
#[db_cache]
pub struct User {
    #[primary_key]
    pub id: i64,
    pub name: String,
}
```

### Entity with Audit Logging

Enable audit logging for all operations:

```rust
#[derive(DbEntity)]
#[db_entity]
#[table_name = "users"]
#[db_crud]
#[db_audit]
pub struct User {
    #[primary_key]
    pub id: i64,
    pub name: String,
}
```

### Complex Entity Example

```rust
use dbnexus::{DbEntity, db_crud, db_permission, db_cache, db_audit};

#[derive(DbEntity)]
#[db_entity]
#[table_name = "orders"]
#[db_crud]
#[db_permission(
    roles = ["admin", "sales_manager"],
    operations = ["SELECT", "INSERT", "UPDATE", "DELETE"]
)]
#[db_cache]
#[db_audit]
pub struct Order {
    #[primary_key]
    pub id: i64,
    pub user_id: i64,
    pub amount: f64,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
```

---

## Working with Connections

### Getting a Session

Use `get_session()` to acquire a database connection:

```rust
use dbnexus::DbPool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("postgresql://localhost/mydb").await?;

    // Get session with role
    let session = pool.get_session("admin").await?;

    // Use session...
    // Connection automatically released when dropped

    Ok(())
}
```

### Session Lifecycle

Sessions are RAII-managed:

```rust
{
    let session = pool.get_session("admin").await?;
    // Connection is active here

    User::find_all(&session).await?;
    User::insert(&session, new_user).await?;

} // Connection automatically released here
```

### Checking Pool Status

Monitor connection pool health:

```rust
let status = pool.status();

println!("Total connections: {}", status.total);
println!("Active connections: {}", status.active);
println!("Idle connections: {}", status.idle);
println!("Wait count: {}", status.wait_count);
println!("Max active observed: {}", status.max_active);
```

### Manual Health Check

Trigger connection pool health check:

```rust
let invalid_count = pool.clean_invalid_connections().await?;
println!("Removed {} invalid connections", invalid_count);
```

---

## CRUD Operations

### Create (Insert)

Insert a new record:

```rust
let user = User {
    id: 1,
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
    created_at: chrono::Utc::now(),
};

let inserted = User::insert(&session, user).await?;
println!("Inserted user: {}", inserted.name);
```

### Read (Select)

#### Find by Primary Key

```rust
let user = User::find_by_id(&session, 1).await?;
if let Some(user) = user {
    println!("Found user: {}", user.name);
}
```

#### Find All Records

```rust
let users = User::find_all(&session).await?;
println!("Found {} users", users.len());
```

#### Find by Condition

```rust
use dbnexus::entity::*;

let condition = Condition::all()
    .add(Column::Name.like("%Alice%"))
    .add(Column::CreatedAt.gte(chrono::Utc::now() - chrono::Duration::days(7)));

let users = User::find_by_condition(&session, condition).await?;
```

#### Count Records

```rust
let count = User::count(&session).await?;
println!("Total users: {}", count);
```

### Update

Update an existing record:

```rust
let mut user = User::find_by_id(&session, 1).await?.unwrap();
user.email = "alice_new@example.com".to_string();
user.updated_at = chrono::Utc::now();

let updated = User::update(&session, user).await?;
println!("Updated user: {}", updated.email);
```

### Delete

#### Delete by Primary Key

```rust
User::delete(&session, 1).await?;
println!("Deleted user with ID 1");
```

#### Delete by Condition

```rust
use dbnexus::entity::*;

let condition = Column::CreatedAt.lt(chrono::Utc::now() - chrono::Duration::days(365));
let deleted_count = User::delete_many(&session, condition).await?;
println!("Deleted {} old users", deleted_count);
```

---

## Permission Control

### Setting Up Permissions

Create a `permissions.yaml` file:

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

### Defining Permissions on Entities

```rust
#[derive(DbEntity)]
#[db_entity]
#[table_name = "users"]
#[db_crud]
#[db_permission(roles = ["admin", "manager"])]
pub struct User {
    #[primary_key]
    pub id: i64,
    pub name: String,
}
```

### Using Permissions

```rust
// Admin can do everything
let admin_session = pool.get_session("admin").await?;
User::insert(&admin_session, user).await?;
User::delete(&admin_session, 1).await?;

// Manager can only select/insert/update on users
let manager_session = pool.get_session("manager").await?;
User::find_all(&manager_session).await?; // OK
User::insert(&manager_session, user).await?; // OK
User::delete(&manager_session, 1).await?; // Error: Permission denied

// User can only select
let user_session = pool.get_session("user").await?;
User::find_all(&user_session).await?; // OK
User::insert(&user_session, user).await?; // Error: Permission denied
```

### Wildcard Tables

Use `"*"` to grant access to all tables:

```yaml
roles:
  admin:
    tables:
      - name: "*"  # All tables
        operations:
          - select
          - insert
          - update
          - delete
```

### Operation-Level Control

Restrict specific operations:

```yaml
roles:
  readonly:
    tables:
      - name: "reports"
        operations:
          - select  # Only SELECT allowed
```

---

## Transactions

### Basic Transaction

Begin, commit, and rollback:

```rust
use dbnexus::DbPool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("postgresql://localhost/mydb").await?;
    let mut session = pool.get_session("admin").await?;

    // Begin transaction
    session.begin_transaction().await?;

    // Perform operations
    User::insert(&session, user1).await?;
    User::insert(&session, user2).await?;

    // Commit transaction
    session.commit_transaction().await?;

    Ok(())
}
```

### Transaction with Error Handling

```rust
session.begin_transaction().await?;

match perform_operations(&session).await {
    Ok(_) => {
        session.commit_transaction().await?;
    }
    Err(e) => {
        eprintln!("Error: {}", e);
        session.rollback_transaction().await?;
    }
}
```

### RAII Transaction Guard

Use RAII pattern for automatic rollback:

```rust
use dbnexus::DbPool;

struct TransactionGuard<'a> {
    session: &'a mut Session,
}

impl<'a> TransactionGuard<'a> {
    fn new(session: &'a mut Session) -> Self {
        session.begin_transaction().await.ok();
        Self { session }
    }

    pub async fn commit(self) {
        self.session.commit_transaction().await.ok();
    }
}

impl<'a> Drop for TransactionGuard<'a> {
    fn drop(&mut self) {
        if self.session.is_in_transaction() {
            let _ = self.session.rollback_transaction().now_or_never();
        }
    }
}

// Usage
{
    let tx = TransactionGuard::new(&mut session);
    User::insert(&session, user).await?;
    tx.commit().await; // Explicitly commit
} // Or implicitly rollback on drop
```

---

## Advanced Features

### Caching

Enable caching for read-heavy operations:

```rust
#[derive(DbEntity)]
#[db_entity]
#[table_name = "products"]
#[db_crud]
#[db_cache]
pub struct Product {
    #[primary_key]
    pub id: i64,
    pub name: String,
    pub price: f64,
}

// Use cached read
let product = Product::find_cached(&session, 1).await?;

// Invalidate cache on write
Product::invalidate_cache(&session, 1).await?;
```

### Metrics

Enable Prometheus metrics:

```toml
[dependencies.dbnexus]
version = "0.1"
features = ["metrics"]
```

Collect and export metrics:

```rust
use dbnexus::metrics::MetricsCollector;

let collector = MetricsCollector::new(&pool);

// Get pool metrics
let pool_metrics = collector.get_pool_metrics();
println!("Active connections: {}", pool_metrics.active);

// Get query metrics
let query_metrics = collector.get_query_metrics();
println!("P99 latency: {}ms", query_metrics.latency_p99);

// Export Prometheus format
let prometheus_metrics = collector.export_prometheus();
println!("{}", prometheus_metrics);
```

### Audit Logging

Enable audit logging:

```toml
[dependencies.dbnexus]
version = "0.1"
features = ["audit"]
```

Audit logging is automatic with `#[db_audit]`:

```rust
#[derive(DbEntity)]
#[db_entity]
#[table_name = "sensitive_data"]
#[db_crud]
#[db_audit]
pub struct SensitiveData {
    #[primary_key]
    pub id: i64,
    pub data: String,
}

// All operations are automatically logged
SensitiveData::insert(&session, data).await?;
SensitiveData::find_by_id(&session, 1).await?;
```

### Distributed Tracing

Enable OpenTelemetry tracing:

```toml
[dependencies.dbnexus]
version = "0.1"
features = ["tracing"]
```

Initialize tracing:

```rust
use dbnexus::tracing::TracingGuard;

// Initialize tracing
let _guard = TracingGuard::init_with_otlp("http://localhost:4317")?;

// All DB operations are automatically traced
let session = pool.get_session("admin").await?;
User::find_all(&session).await?;
```

---

## Best Practices

### 1. Use Environment Variables for Sensitive Data

Never hardcode credentials:

```rust
// ❌ Bad
let url = "postgresql://user:password@localhost/db";

// ✅ Good
let url = std::env::var("DATABASE_URL")?;
```

### 2. Always Use Transactions for Multi-Step Operations

```rust
// ❌ Bad
User::insert(&session, user1).await?;
User::insert(&session, user2).await?;
// If user2 fails, user1 is still inserted!

// ✅ Good
session.begin_transaction().await?;
User::insert(&session, user1).await?;
User::insert(&session, user2).await?;
session.commit_transaction().await?;
// Both succeed or both fail
```

### 3. Use RAII for Resource Management

```rust
// ❌ Bad
let session = pool.get_session("admin").await?;
User::find_all(&session).await?;
pool.release_connection(session); // Easy to forget

// ✅ Good
{
    let session = pool.get_session("admin").await?;
    User::find_all(&session).await?;
} // Connection automatically released
```

### 4. Configure Appropriate Pool Sizes

For low-traffic applications:
```rust
.min_connections(1)
.max_connections(5)
```

For high-traffic applications:
```rust
.min_connections(10)
.max_connections(100)
```

### 5. Use Permission Control

Even for internal tools:

```rust
// ❌ Bad
let session = pool.get_session("root").await?;
// Full access, no checks

// ✅ Good
let session = pool.get_session("read_only").await?;
// Limited access, explicit permissions
```

### 6. Monitor Pool Status

Regularly check pool health:

```rust
let status = pool.status();
if status.wait_count > 1000 {
    eprintln!("Warning: High connection wait count");
}
```

### 7. Handle Errors Gracefully

```rust
// ❌ Bad
let user = User::find_by_id(&session, 1).await?.unwrap();
// Panics on error

// ✅ Good
match User::find_by_id(&session, 1).await {
    Ok(Some(user)) => { /* Use user */ }
    Ok(None) => { /* Handle not found */ }
    Err(e) => { /* Handle error */ }
}
```

### 8. Use Type-Safe Operations

```rust
// ❌ Bad
session.execute("DELETE FROM users WHERE id = 1").await?;
// No type safety, prone to errors

// ✅ Good
User::delete(&session, 1).await?;
// Type-safe, automatic permission checks
```

---

## Troubleshooting

### Connection Pool Exhaustion

**Symptom:** `DbError::ConnectionPool("Connection pool exhausted")`

**Solutions:**

1. Increase pool size:
   ```rust
   .max_connections(50)
   ```

2. Check for connection leaks:
   ```rust
   let status = pool.status();
   println!("Active: {}, Total: {}", status.active, status.total);
   ```

3. Ensure sessions are dropped:
   ```rust
   // Use RAII pattern
   {
       let session = pool.get_session("admin").await?;
       // Use session...
   } // Connection released
   ```

### Permission Denied Errors

**Symptom:** `DbError::Permission("Permission denied...")`

**Solutions:**

1. Check role is in permissions config:
   ```yaml
   roles:
     my_role:  # Must match exactly
   ```

2. Verify operation is allowed:
   ```yaml
   roles:
     my_role:
       tables:
         - name: "users"
           operations:
             - select  # Ensure operation is listed
   ```

3. Check role casing:
   ```rust
   // Must match config exactly
   let session = pool.get_session("My_Role").await?; // ❌ Wrong
   let session = pool.get_session("my_role").await?; // ✅ Correct
   ```

### Database Connection Errors

**Symptom:** `DbError::Connection("Failed to connect...")`

**Solutions:**

1. Verify URL format:
   ```bash
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

**Symptom:** Queries take too long

**Solutions:**

1. Enable and check metrics:
   ```rust
   let collector = MetricsCollector::new(&pool);
   println!("P99 latency: {}ms", collector.get_query_metrics().latency_p99);
   ```

2. Use indexes:
   ```sql
   CREATE INDEX idx_users_email ON users(email);
   ```

3. Optimize queries:
   ```rust
   // ❌ Bad: Fetches all
   let users = User::find_all(&session).await?;
   let filtered: Vec<_> = users.into_iter()
       .filter(|u| u.name.contains("Alice"))
       .collect();

   // ✅ Good: Filter in database
   let users = User::find_by_condition(&session, Column::Name.like("%Alice%")).await?;
   ```

### High Memory Usage

**Symptom:** Application uses excessive memory

**Solutions:**

1. Reduce pool size:
   ```rust
   .max_connections(10)
   ```

2. Enable connection idle timeout:
   ```rust
   .idle_timeout(60)  // Close idle connections faster
   ```

3. Check for cache size:
   ```rust
   #[db_cache(capacity = 100)]  // Limit cache entries
   ```

---

## Example: Complete Application

```rust
use dbnexus::{DbPool, DbEntity, db_crud, db_permission};
use chrono::Utc;

#[derive(DbEntity)]
#[db_entity]
#[table_name = "users"]
#[db_crud]
#[db_permission(roles = ["admin", "manager"])]
pub struct User {
    #[primary_key]
    pub id: i64,
    pub name: String,
    pub email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize connection pool
    let pool = DbPool::new("postgresql://localhost/mydb").await?;
    println!("✓ Connected to database");

    // Admin operations
    {
        let admin_session = pool.get_session("admin").await?;
        println!("✓ Admin session acquired");

        // Insert users
        let user1 = User {
            id: 1,
            name: "Alice".to_string(),
            email: "alice@example.com".to_string(),
        };
        User::insert(&admin_session, user1).await?;
        println!("✓ Inserted user: Alice");

        let user2 = User {
            id: 2,
            name: "Bob".to_string(),
            email: "bob@example.com".to_string(),
        };
        User::insert(&admin_session, user2).await?;
        println!("✓ Inserted user: Bob");
    }

    // Manager operations
    {
        let manager_session = pool.get_session("manager").await?;
        println!("✓ Manager session acquired");

        // Query users
        let users = User::find_all(&manager_session).await?;
        println!("✓ Found {} users", users.len());

        // Update user
        if let Some(mut user) = User::find_by_id(&manager_session, 1).await? {
            user.email = "alice_new@example.com".to_string();
            User::update(&manager_session, user).await?;
            println!("✓ Updated user email");
        }
    }

    // Pool status
    let status = pool.status();
    println!("\n📊 Pool Status:");
    println!("  Total: {}", status.total);
    println!("  Active: {}", status.active);
    println!("  Idle: {}", status.idle);

    println!("\n✨ Application completed successfully!");

    Ok(())
}
```

---

For more information:
- [API Reference](API_REFERENCE.md)
- [Architecture](ARCHITECTURE.md)
- [Rust Docs](https://docs.rs/dbnexus)
