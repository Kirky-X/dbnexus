<div align="center">

# 📖 User Guide

### Complete Guide to Using dbnexus

[🏠 Home](../README.md) • [📚 Docs](README.md) • [🎯 Examples](../examples/) • [❓ FAQ](FAQ.md)

---

</div>

## 📋 Table of Contents

- [Introduction](#introduction)
- [Getting Started](#getting-started)
  - [Prerequisites](#prerequisites)
  - [Installation](#installation)
  - [First Steps](#first-steps)
- [Core Concepts](#core-concepts)
- [Basic Usage](#basic-usage)
  - [Initialization](#initialization)
  - [Configuration](#configuration)
  - [Database Migrations](#database-migrations)
  - [Basic Operations](#basic-operations)
- [Advanced Usage](#advanced-usage)
  - [Database Sharding](#database-sharding)
  - [Caching](#caching)
  - [Audit Logging](#audit-logging)
  - [Metrics and Monitoring](#metrics-and-monitoring)
  - [Error Handling](#error-handling)
- [Best Practices](#best-practices)
- [Common Patterns](#common-patterns)
- [Troubleshooting](#troubleshooting)
- [Next Steps](#next-steps)

---

## Introduction

<div align="center">

### 🎯 What You'll Learn

</div>

<table>
<tr>
<td width="25%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/rocket.png" width="64"><br>
<b>Quick Start</b><br>
Get up and running in 5 minutes
</td>
<td width="25%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/settings.png" width="64"><br>
<b>Configuration</b><br>
Customize to your needs
</td>
<td width="25%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/code.png" width="64"><br>
<b>Best Practices</b><br>
Learn the right way
</td>
<td width="25%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/rocket-take-off.png" width="64"><br>
<b>Advanced Topics</b><br>
Master the details
</td>
</tr>
</table>

**dbnexus** is a powerful database abstraction layer for Rust that provides multi-database support, connection pooling, session management, and advanced features like sharding, caching, and audit logging. This guide will walk you through everything from basic setup to advanced usage patterns.

> 💡 **Tip**: This guide assumes basic knowledge of Rust and database concepts. If you're new to Rust, check out the [Rust Book](https://doc.rust-lang.org/book/) first.

---

## Getting Started

### Prerequisites

Before you begin, ensure you have the following installed:

<table>
<tr>
<td width="50%">

**Required**
- ✅ Rust 1.75+ (stable)
- ✅ Cargo (comes with Rust)
- ✅ Git

</td>
<td width="50%">

**Optional**
- 🔧 IDE with Rust support (VS Code, IntelliJ IDEA)
- 🔧 Docker (for containerized databases)
- 🔧 Database server (PostgreSQL, MySQL, or SQLite)

</td>
</tr>
</table>

<details>
<summary><b>🔍 Verify Your Installation</b></summary>

```bash
# Check Rust version
rustc --version
# Expected: rustc 1.75.0 (or higher)

# Check Cargo version
cargo --version
# Expected: cargo 1.75.0 (or higher)

# Check Git version
git --version
# Expected: git version 2.x.x
```

</details>

### Installation

<div align="center">

#### Choose Your Installation Method

</div>

<table>
<tr>
<td width="50%">

**📦 Using Cargo (Recommended)**

```bash
# Add to Cargo.toml
[dependencies]
dbnexus = { version = "0.1", features = ["sqlite"] }

# Or for PostgreSQL
dbnexus = { version = "0.1", features = ["postgres"] }

# Or for MySQL
dbnexus = { version = "0.1", features = ["mysql"] }

# With optional features
dbnexus = { version = "0.1", features = ["postgres", "metrics", "cache", "audit", "sharding"] }
```

</td>
<td width="50%">

**🐙 From Source**

```bash
git clone https://github.com/yourusername/dbnexus
cd dbnexus
cargo build --release
```

</td>
</tr>
</table>

<details>
<summary><b>⚙️ Feature Flags</b></summary>

**Database Drivers (mutually exclusive, must choose exactly one):**
- `sqlite` - SQLite database support
- `postgres` - PostgreSQL database support
- `mysql` - MySQL database support

**Optional Features:**
- `metrics` - Prometheus metrics integration
- `cache` - Result caching support
- `audit` - Audit logging support
- `sharding` - Database sharding support
- `global-index` - Global index support
- `permission-engine` - Permission engine support
- `migration` - Database migration support

**Runtime Options (mutually exclusive):**
- `runtime-tokio-rustls` (default) - Tokio runtime with rustls
- `runtime-tokio-native-tls` - Tokio runtime with native TLS
- `runtime-async-std` - async-std runtime

</details>

### First Steps

Let's verify your installation with a simple example:

```rust
use dbnexus::{DbPool, DbEntity, db_crud};

#[derive(DbEntity)]
#[db_entity]
#[table_name = "users"]
#[db_crud]
struct User {
    #[primary_key]
    id: i64,
    name: String,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("sqlite::memory:").await?;
    let session = pool.get_session("admin").await?;
    
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    
    User::insert(&session, user).await?;
    println!("✅ dbnexus is ready!");
    
    Ok(())
}
```

<details>
<summary><b>🎬 Run the Example</b></summary>

```bash
# Create a new project
cargo new hello-dbnexus
cd hello-dbnexus

# Add dependency
cargo add dbnexus --features sqlite

# Copy the code above to src/main.rs

# Run it!
cargo run
```

**Expected Output:**
```
✅ dbnexus is ready!
```

</details>

---

## Core Concepts

Understanding these core concepts will help you use dbnexus effectively.

<div align="center">

### 🧩 Key Components

</div>

```mermaid
graph TD
    A[Your Application] --> B[DbPool]
    B --> C[DbSession]
    C --> D[DbEntity]
    C --> E[PermissionEngine]
    B --> F[Connection Pool]
    C --> G[Cache Layer]
    C --> H[Audit Logger]
    C --> I[Metrics Collector]
    
    style A fill:#e1f5ff
    style B fill:#81d4fa
    style C fill:#4fc3f7
    style D fill:#29b6f6
    style E fill:#29b6f6
    style F fill:#0288d1
    style G fill:#0288d1
    style H fill:#0288d1
    style I fill:#0288d1
```

### 1️⃣ DbPool: Connection Pool Manager

**What it is:** Manages database connections and provides sessions for database operations.

**Why it matters:** Efficient connection management is crucial for performance and resource utilization.

**Example:**
```rust
use dbnexus::DbPool;

// Create a connection pool
let pool = DbPool::new("postgres://user:pass@localhost/db").await?;

// Get a session
let session = pool.get_session("admin").await?;
```

<details>
<summary><b>📚 Learn More</b></summary>

The DbPool:
- Manages a pool of database connections
- Provides sessions with automatic permission checking
- Supports multiple database backends (SQLite, PostgreSQL, MySQL)
- Handles connection lifecycle automatically
- Configurable pool size and timeout settings

</details>

### 2️⃣ DbSession: Database Session

**What it is:** Represents a database session with associated user context and permissions.

**Key Features:**
- ✅ Automatic permission checking
- ✅ Transaction support
- ✅ Audit logging
- ✅ Result caching
- ✅ Metrics collection

**Example:**
```rust
let session = pool.get_session("admin").await?;

// Use session for database operations
User::insert(&session, user).await?;
let users = User::find_all(&session).await?;
```

### 3️⃣ DbEntity: Database Entity

**What it is:** A declarative macro that generates CRUD operations for your structs.

<table>
<tr>
<td width="50%">

**Manual Approach**
```rust
// Write all CRUD operations manually
fn insert_user(conn: &Connection, user: &User) -> Result<()> {
    // Manual SQL and mapping
}
```

</td>
<td width="50%">

**With DbEntity**
```rust
#[derive(DbEntity)]
#[db_entity]
#[table_name = "users"]
#[db_crud]
struct User {
    #[primary_key]
    id: i64,
    name: String,
}

// CRUD operations auto-generated
User::insert(&session, user).await?;
```

</td>
</tr>
</table>

### 4️⃣ PermissionEngine: Access Control

**What it is:** Fine-grained permission system for database operations.

**Key Features:**
- ✅ Role-based access control
- ✅ Row-level security
- ✅ Column-level permissions
- ✅ Custom permission rules

**Example:**
```rust
// Define permissions
#[derive(Permission)]
#[permission]
struct UserPermission {
    resource: String,
    action: String,
    condition: String,
}

// Check permissions
let has_access = session.check_permission("users", "read")?;
```

---

## Basic Usage

### Initialization

Every application must initialize the database pool before use:

```rust
use dbnexus::DbPool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Simple initialization with connection string
    let pool = DbPool::new("sqlite::memory:").await?;
    
    // Or with configuration
    let config = dbnexus::Config::builder()
        .database_url("postgres://user:pass@localhost/db")
        .max_connections(10)
        .build()?;
    
    let pool = DbPool::from_config(config).await?;
    
    Ok(())
}
```

<div align="center">

| Method | Use Case | Performance | Complexity |
|--------|----------|-------------|------------|
| `DbPool::new()` | Quick start, development | ⚡ Fast | 🟢 Simple |
| `DbPool::from_config()` | Production, custom needs | ⚡⚡ Optimized | 🟡 Moderate |

</div>

### Configuration

<details open>
<summary><b>⚙️ Configuration Options</b></summary>

```rust
use dbnexus::Config;
use std::time::Duration;

let config = Config::builder()
    // Database connection
    .database_url("postgres://user:pass@localhost/db")
    
    // Connection pool settings
    .max_connections(20)
    .min_connections(5)
    .connection_timeout(Duration::from_secs(30))
    .idle_timeout(Duration::from_secs(600))
    
    // Performance settings
    .enable_cache(true)
    .cache_ttl(Duration::from_secs(3600))
    
    // Logging
    .enable_audit(true)
    .log_level("info")
    
    // Metrics
    .enable_metrics(true)
    .metrics_port(9090)
    
    .build()?;
```

</details>

<table>
<tr>
<th>Option</th>
<th>Type</th>
<th>Default</th>
<th>Description</th>
</tr>
<tr>
<td><code>database_url</code></td>
<td>String</td>
<td>Required</td>
<td>Database connection string</td>
</tr>
<tr>
<td><code>max_connections</code></td>
<td>usize</td>
<td>10</td>
<td>Maximum connections in pool</td>
</tr>
<tr>
<td><code>min_connections</code></td>
<td>usize</td>
<td>1</td>
<td>Minimum connections to maintain</td>
</tr>
<tr>
<td><code>connection_timeout</code></td>
<td>Duration</td>
<td>30s</td>
<td>Connection acquisition timeout</td>
</tr>
<tr>
<td><code>enable_cache</code></td>
<td>bool</td>
<td>false</td>
<td>Enable result caching</td>
</tr>
<tr>
<td><code>enable_audit</code></td>
<td>bool</td>
<td>false</td>
<td>Enable audit logging</td>
</tr>
<tr>
<td><code>enable_metrics</code></td>
<td>bool</td>
<td>false</td>
<td>Enable Prometheus metrics</td>
</tr>
</table>

### Database Migrations

dbnexus provides powerful database migration support with both automatic and manual migration capabilities. This feature is available when the `auto-migrate` feature is enabled.

#### Auto-Migrate

Automatic migrations run when the connection pool is initialized, ensuring your database schema is always up-to-date.

```rust
use dbnexus::{DbPool, DbConfig};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = DbConfig {
        url: "sqlite://./data/app.db".to_string(),
        migrations_dir: Some(PathBuf::from("./migrations")),
        auto_migrate: true,
        migration_timeout: 60,
        ..Default::default()
    };
    
    // Migrations run automatically during pool creation
    let pool = DbPool::with_config(config).await?;
    
    Ok(())
}
```

**Environment Variables:**

| Variable | Description | Default |
|----------|-------------|---------|
| `DB_AUTO_MIGRATE` | Enable automatic migrations | `false` |
| `DB_MIGRATIONS_DIR` | Directory containing migration files | None |
| `DB_MIGRATION_TIMEOUT` | Timeout for migration operations (seconds) | `60` |

#### Manual Migrations

You can also run migrations manually at any time:

```rust
let pool = DbPool::new("sqlite://./data/app.db").await?;

// Run all pending migrations
let applied = pool.run_migrations(Path::new("./migrations")).await?;
println!("Applied {} migrations", applied);

// Or use auto-migrate explicitly
let applied = pool.run_auto_migrate().await?;
```

#### Migration File Format

Migration files should be named `{version}_{description}.sql`:

```sql
-- Migration: create_users_table
-- Version: 1

-- UP
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    email TEXT UNIQUE,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- DOWN
DROP TABLE users;
```

**Key Points:**

- Files must have `.sql` extension
- Version numbers should be unique and incrementing
- `-- UP:` section contains the migration SQL
- `-- DOWN:` section (optional) contains rollback SQL

#### Migration Features

- **Automatic Version Tracking**: dbnexus creates a `dbnexus_migrations` table to track applied migrations
- **Idempotent Operations**: Running migrations multiple times is safe - already-applied migrations are skipped
- **Transaction Support**: Each migration runs in a transaction for safety
- **Multiple Database Support**: Works with SQLite, PostgreSQL, and MySQL

<div align="center">

| Feature | Status |
|---------|--------|
| Auto-migrate on startup | ✅ Supported |
| Manual migration control | ✅ Supported |
| Rollback support | ✅ Supported |
| Migration history | ✅ Supported |
| Multi-database SQL generation | ✅ Supported |

</div>

### Basic Operations

<div align="center">

#### 📝 CRUD Operations with DbEntity

</div>

First, define your entity:

```rust
use dbnexus::{DbEntity, db_crud};

#[derive(DbEntity)]
#[db_entity]
#[table_name = "users"]
#[db_crud]
struct User {
    #[primary_key]
    id: i64,
    name: String,
    email: String,
    age: i32,
}
```

Then use the auto-generated CRUD operations:

<table>
<tr>
<td width="50%">

**Create**
```rust
let user = User {
    id: 1,
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
    age: 30,
};

User::insert(&session, user).await?;
```

**Read**
```rust
// Find by primary key
let user = User::find_by_id(&session, 1).await?;

// Find all
let users = User::find_all(&session).await?;

// Find with condition
let users = User::find_by_condition(&session, "age > 25").await?;
```

</td>
<td width="50%">

**Update**
```rust
let mut user = User::find_by_id(&session, 1).await?;
user.age = 31;
User::update(&session, &user).await?;

// Or update specific fields
User::update_field(&session, 1, "age", 31).await?;
```

**Delete**
```rust
User::delete(&session, 1).await?;

// Or delete by condition
User::delete_by_condition(&session, "age < 18").await?;
```

</td>
</tr>
</table>

<details>
<summary><b>🎯 Complete Example</b></summary>

```rust
use dbnexus::{DbPool, DbEntity, db_crud};

#[derive(DbEntity)]
#[db_entity]
#[table_name = "users"]
#[db_crud]
struct User {
    #[primary_key]
    id: i64,
    name: String,
    email: String,
    age: i32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("sqlite::memory:").await?;
    let session = pool.get_session("admin").await?;
    
    // Create
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
        age: 30,
    };
    User::insert(&session, user).await?;
    println!("✅ Created user: Alice");
    
    // Read
    let loaded = User::find_by_id(&session, 1).await?;
    println!("📖 Loaded: {} ({})", loaded.name, loaded.email);
    
    // Update
    let mut updated = loaded.clone();
    updated.age = 31;
    User::update(&session, &updated).await?;
    println!("✏️ Updated age to 31");
    
    // Find all
    let all_users = User::find_all(&session).await?;
    println!("📊 Total users: {}", all_users.len());
    
    // Delete
    User::delete(&session, 1).await?;
    println!("🗑️ Deleted user");
    
    Ok(())
}
```

</details>

---

## Advanced Usage

### Database Sharding

Sharding allows you to distribute data across multiple database instances for scalability.

```rust
use dbnexus::sharding::{ShardManager, ShardStrategy};

let shard_manager = ShardManager::builder()
    .strategy(ShardStrategy::Hash)
    .shard_count(4)
    .add_shard("shard1", "postgres://localhost/shard1")
    .add_shard("shard2", "postgres://localhost/shard2")
    .add_shard("shard3", "postgres://localhost/shard3")
    .add_shard("shard4", "postgres://localhost/shard4")
    .build()?;

// Operations are automatically routed to the correct shard
let session = shard_manager.get_session("admin", "user_123").await?;
User::insert(&session, user).await?;
```

<details>
<summary><b>📋 Sharding Strategies</b></summary>

| Strategy | Description | Use Case |
|----------|-------------|----------|
| `Hash` | Consistent hashing | Even distribution |
| `Range` | Range-based partitioning | Time-series data |
| `Modulo` | Simple modulo operation | Simple distribution |
| `Custom` | Custom sharding logic | Specific requirements |

</details>

### Caching

Enable caching to reduce database load and improve performance.

```rust
use dbnexus::cache::Cache;
use std::time::Duration;

let cache = Cache::builder()
    .max_size(10_000)
    .ttl(Duration::from_secs(3600))
    .build()?;

// Cache is automatically used when enabled in config
let config = Config::builder()
    .enable_cache(true)
    .cache_ttl(Duration::from_secs(3600))
    .build()?;

// Manual cache usage
let cache_key = format!("user:{}", user_id);
if let Some(user) = cache.get(&cache_key)? {
    return Ok(user);
}

let user = User::find_by_id(&session, user_id).await?;
cache.set(&cache_key, user.clone())?;
```

### Audit Logging

Track all database operations for compliance and debugging.

```rust
use dbnexus::audit::AuditLogger;

let audit_logger = AuditLogger::builder()
    .log_level("info")
    .log_file("/var/log/dbnexus/audit.log")
    .build()?;

// Enable in config
let config = Config::builder()
    .enable_audit(true)
    .audit_logger(audit_logger)
    .build()?;

// Audit events are automatically logged
User::insert(&session, user).await?;
// Logs: INSERT INTO users VALUES (...) by admin at 2024-01-01 12:00:00
```

### Metrics and Monitoring

Enable Prometheus metrics for monitoring database operations.

```rust
use dbnexus::metrics::MetricsCollector;

let metrics = MetricsCollector::builder()
    .port(9090)
    .path("/metrics")
    .build()?;

// Enable in config
let config = Config::builder()
    .enable_metrics(true)
    .metrics_collector(metrics)
    .build()?;

// Metrics are automatically collected
// Available metrics:
// - dbnexus_query_duration_seconds
// - dbnexus_query_total
// - dbnexus_query_errors_total
// - dbnexus_connection_pool_size
// - dbnexus_cache_hits_total
// - dbnexus_cache_misses_total
```

### Error Handling

<div align="center">

#### 🚨 Handling Errors Gracefully

</div>

```rust
use dbnexus::{Error, ErrorKind};

async fn handle_operation() -> Result<(), Error> {
    match User::find_by_id(&session, user_id).await {
        Ok(user) => {
            println!("Found user: {}", user.name);
            Ok(())
        }
        Err(e) => {
            match e.kind() {
                ErrorKind::NotFound => {
                    println!("⚠️ User not found, creating new...");
                    create_user(user_id).await?;
                    Ok(())
                }
                ErrorKind::PermissionDenied => {
                    eprintln!("❌ Access denied");
                    Err(e)
                }
                ErrorKind::ConnectionError => {
                    println!("🔌 Connection error, retrying...");
                    retry_operation().await?;
                    Ok(())
                }
                ErrorKind::QueryError => {
                    eprintln!("❌ Query error: {}", e);
                    Err(e)
                }
                _ => {
                    eprintln!("❌ Unexpected error: {}", e);
                    Err(e)
                }
            }
        }
    }
}
```

<details>
<summary><b>📋 Error Types</b></summary>

| Error Type | Description | Recovery Strategy |
|------------|-------------|-------------------|
| `NotFound` | Record doesn't exist | Create or return 404 |
| `AlreadyExists` | Duplicate record | Use existing or update |
| `PermissionDenied` | Access violation | Request permissions |
| `ConnectionError` | Database connection failed | Retry with backoff |
| `QueryError` | SQL execution failed | Fix query or data |
| `ValidationError` | Invalid input data | Validate and retry |
| `TimeoutError` | Operation timed out | Retry with timeout |

</details>

---

## Best Practices

<div align="center">

### 🌟 Follow These Guidelines

</div>

### ✅ DO's

<table>
<tr>
<td width="50%">

**Initialize Pool Once**
```rust
#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize pool at the start
    let pool = DbPool::new(&db_url).await?;
    
    // Use Arc for sharing across threads
    let pool = Arc::new(pool);
    
    do_work(pool).await?;
    Ok(())
}
```

</td>
<td width="50%">

**Use Sessions Properly**
```rust
// Get session for each operation
let session = pool.get_session("admin").await?;
User::insert(&session, user).await?;

// Or reuse session within transaction
let session = pool.get_session("admin").await?;
session.begin_transaction().await?;
User::insert(&session, user1).await?;
User::insert(&session, user2).await?;
session.commit().await?;
```

</td>
</tr>
<tr>
<td width="50%">

**Handle Errors Properly**
```rust
match User::find_by_id(&session, id).await {
    Ok(user) => process_user(user),
    Err(Error::NotFound) => return None,
    Err(e) => return Err(e.into()),
}
```

</td>
<td width="50%">

**Use Transactions for Multiple Operations**
```rust
session.begin_transaction().await?;

User::insert(&session, user).await?;
Order::insert(&session, order).await?;

session.commit().await?;
// or
session.rollback().await?;
```

</td>
</tr>
</table>

### ❌ DON'Ts

<table>
<tr>
<td width="50%">

**Don't Ignore Errors**
```rust
// ❌ Bad
let _ = User::insert(&session, user).await;

// ✅ Good
User::insert(&session, user).await?;
```

</td>
<td width="50%">

**Don't Block Async Context**
```rust
// ❌ Bad (in async fn)
std::thread::sleep(duration);

// ✅ Good
tokio::time::sleep(duration).await;
```

</td>
</tr>
<tr>
<td width="50%">

**Don't Create Multiple Pools**
```rust
// ❌ Bad
let pool1 = DbPool::new(&db_url).await?;
let pool2 = DbPool::new(&db_url).await?;

// ✅ Good
let pool = DbPool::new(&db_url).await?;
let pool = Arc::new(pool);
```

</td>
<td width="50%">

**Don't Forget to Close Connections**
```rust
// ❌ Bad - connections leak
let pool = DbPool::new(&db_url).await?;
// forget pool

// ✅ Good - pool handles cleanup
{
    let pool = DbPool::new(&db_url).await?;
    // use pool
} // pool is dropped, connections closed
```

</td>
</tr>
</table>

### 💡 Tips and Tricks

> **🔥 Performance Tip**: Use connection pooling efficiently:
> ```rust
> let config = Config::builder()
>     .max_connections(20)  // Set based on your workload
>     .min_connections(5)  // Keep some connections ready
>     .build()?;
> ```

> **🔒 Security Tip**: Never hardcode database credentials:
> ```rust
> // ❌ Bad
> let db_url = "postgres://user:password@localhost/db";
> 
> // ✅ Good
> let db_url = env::var("DATABASE_URL")?;
> ```

> **📊 Monitoring Tip**: Enable metrics in production:
> ```rust
> Config::builder()
>     .enable_metrics(true)
>     .metrics_port(9090)
>     .build()?
> ```

> **🎯 Caching Tip**: Cache frequently accessed data:
> ```rust
> Config::builder()
>     .enable_cache(true)
>     .cache_ttl(Duration::from_secs(3600))
>     .build()?
> ```

---

## Common Patterns

### Pattern 1: Repository Pattern

```rust
use dbnexus::{DbPool, DbEntity, db_crud};

#[derive(DbEntity)]
#[db_entity]
#[table_name = "users"]
#[db_crud]
struct User {
    #[primary_key]
    id: i64,
    name: String,
    email: String,
}

struct UserRepository {
    pool: Arc<DbPool>,
}

impl UserRepository {
    fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }
    
    async fn find_by_email(&self, email: &str) -> Result<Option<User>, Error> {
        let session = self.pool.get_session("admin").await?;
        let users = User::find_by_condition(&session, &format!("email = '{}'", email)).await?;
        Ok(users.into_iter().next())
    }
    
    async fn create(&self, user: User) -> Result<User, Error> {
        let session = self.pool.get_session("admin").await?;
        User::insert(&session, user).await?;
        Ok(user)
    }
}
```

### Pattern 2: Service Layer

```rust
use dbnexus::DbPool;
use std::sync::Arc;

struct UserService {
    user_repo: Arc<UserRepository>,
    audit_logger: Arc<AuditLogger>,
}

impl UserService {
    async fn register_user(&self, name: String, email: String) -> Result<User, Error> {
        // Check if user exists
        if let Some(_) = self.user_repo.find_by_email(&email).await? {
            return Err(Error::AlreadyExists);
        }
        
        // Create user
        let user = User {
            id: generate_id(),
            name,
            email,
        };
        
        self.user_repo.create(user.clone()).await?;
        
        // Log audit
        self.audit_logger.log("user_registered", &user).await?;
        
        Ok(user)
    }
}
```

### Pattern 3: Transactional Operations

```rust
async fn transfer_funds(
    pool: Arc<DbPool>,
    from_id: i64,
    to_id: i64,
    amount: i64,
) -> Result<(), Error> {
    let session = pool.get_session("admin").await?;
    
    // Begin transaction
    session.begin_transaction().await?;
    
    // Check balance
    let from_account = Account::find_by_id(&session, from_id).await?;
    if from_account.balance < amount {
        session.rollback().await?;
        return Err(Error::InsufficientFunds);
    }
    
    // Update balances
    Account::update_balance(&session, from_id, from_account.balance - amount).await?;
    Account::update_balance(&session, to_id, to_account.balance + amount).await?;
    
    // Commit transaction
    session.commit().await?;
    
    Ok(())
}
```

---

## Troubleshooting

<details>
<summary><b>❓ Problem: Connection pool exhausted</b></summary>

**Solution:**
```rust
// Increase pool size
let config = Config::builder()
    .max_connections(50)
    .connection_timeout(Duration::from_secs(60))
    .build()?;
```

**Diagnosis:**
- Check if you're properly closing sessions
- Monitor connection pool metrics
- Review your connection usage patterns

</details>

<details>
<summary><b>❓ Problem: Slow query performance</b></summary>

**Diagnosis:**
1. Enable query logging
2. Check database indexes
3. Review query execution plans
4. Consider caching

**Solution:**
```rust
// Enable query logging
let config = Config::builder()
    .log_level("debug")
    .build()?;

// Enable caching for frequent queries
let config = Config::builder()
    .enable_cache(true)
    .cache_ttl(Duration::from_secs(3600))
    .build()?;
```

</details>

<details>
<summary><b>❓ Problem: Permission denied errors</b></summary>

**Solution:**
```rust
// Check user permissions
let session = pool.get_session("admin").await?;
let has_permission = session.check_permission("users", "read")?;

// Grant necessary permissions
session.grant_permission("user", "users", "read").await?;
```

</details>

<details>
<summary><b>❓ Problem: High memory usage</b></summary>

**Solution:**
```rust
// Reduce cache size
let config = Config::builder()
    .cache_max_size(1000)  // Reduce from default
    .build()?;

// Reduce connection pool size
let config = Config::builder()
    .max_connections(10)
    .build()?;
```

</details>

<details>
<summary><b>❓ Problem: Database migration conflicts</b></summary>

**Solution:**
```rust
// Use migrate-cli for proper migration management
// Install: cargo install dbnexus-migrate-cli

// Create migration
dbnexus-migrate create add_users_table

// Run migrations
dbnexus-migrate up

// Rollback if needed
dbnexus-migrate down
```

</details>

<div align="center">

**💬 Still need help?** [Open an issue](https://github.com/yourusername/dbnexus/issues) or check our [FAQ](FAQ.md)

</div>

---

## Next Steps

<div align="center">

### 🎯 Continue Your Journey

</div>

<table>
<tr>
<td width="33%" align="center">
<a href="../examples/">
<img src="https://img.icons8.com/fluency/96/000000/code.png" width="64"><br>
<b>💻 Examples</b>
</a><br>
Real-world code samples
</td>
<td width="33%" align="center">
<a href="ARCHITECTURE.md">
<img src="https://img.icons8.com/fluency/96/000000/settings.png" width="64"><br>
<b>🔧 Architecture</b>
</a><br>
Deep dive into design
</td>
<td width="33%" align="center">
<a href="API_REFERENCE.md">
<img src="https://img.icons8.com/fluency/96/000000/book.png" width="64"><br>
<b>📖 API Reference</b>
</a><br>
Complete API documentation
</td>
</tr>
</table>

---

<div align="center">

**[📖 API Reference](API_REFERENCE.md)** • **[❓ FAQ](FAQ.md)** • **[🐛 Report Issue](https://github.com/yourusername/dbnexus/issues)**

Made with ❤️ by the dbnexus Team

[⬆ Back to Top](#-user-guide)

</div>
