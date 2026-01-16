# DBNexus

<div align="center">

**Enterprise-grade Database Abstraction Layer for Rust**

[![Crates.io](https://img.shields.io/crates/v/dbnexus)](https://crates.io/crates/dbnexus)
[![Documentation](https://docs.rs/dbnexus/badge.svg)](https://docs.rs/dbnexus)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

**A high-performance, secure, and feature-rich database access layer built on Sea-ORM**

[Quick Start](#quick-start) • [Features](#features) • [Documentation](https://docs.rs/dbnexus) • [Examples](#examples)

</div>

---

## 📖 Overview

DBNexus is an enterprise-grade database abstraction layer for Rust that provides:

- **Session-based Connection Management**: RAII-style automatic connection lifecycle management
- **Declarative Permission Control**: Compile-time permission checks via procedural macros
- **Intelligent Connection Pooling**: Dynamic configuration correction and health checking
- **Enterprise Features**: Metrics, distributed tracing, audit logging, and more

Built on top of [Sea-ORM](https://www.sea-ql.org/SeaORM/), DBNexus adds production-ready features while maintaining the simplicity and ergonomics you love.

## 🚀 Quick Start

### Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
dbnexus = "0.1"
```

### Basic Usage

```rust
use dbnexus::{DbPool, DbEntity, db_crud};

// Define your entity
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
    // Create a connection pool
    let pool = DbPool::new("sqlite::memory:").await?;

    // Get a session with role-based access
    let session = pool.get_session("admin").await?;

    // Insert a user
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    User::insert(&session, user).await?;

    // Query users
    let users = User::find_all(&session).await?;
    println!("Found {} users", users.len());

    Ok(())
}
```

### With Permission Control

```rust
use dbnexus::{DbPool, DbEntity, db_crud, db_permission};

#[derive(DbEntity)]
#[db_entity]
#[table_name = "users"]
#[db_crud]
#[db_permission(roles = ["admin", "manager"], operations = ["SELECT", "INSERT"])]
struct User {
    #[primary_key]
    id: i64,
    name: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("sqlite::memory:").await?;

    // Admin can access
    let session = pool.get_session("admin").await?;
    User::find_all(&session).await?;

    // Regular user will be denied
    let session = pool.get_session("guest").await?;
    User::find_all(&session).await?; // Error: Permission denied

    Ok(())
}
```

## ✨ Features

### Core Features

- **🔒 Permission Control**
  - Table-level access control with roles
  - Compile-time permission verification
  - Support for YAML and RBAC policy providers
  - LRU cache for permission policies

- **🏊 Smart Connection Pooling**
  - RAII-based automatic connection management
  - Dynamic configuration correction
  - Health checking with automatic connection recreation
  - Connection warmup support

- **⚡ High Performance**
  - Zero-cost abstractions
  - LRU caching for frequently accessed data
  - Lock-free counters using atomic operations
  - Async-first design with Tokio

### Enterprise Features

- **📊 Monitoring**
  - Prometheus metrics export
  - Connection pool status monitoring
  - Query performance tracking

- **🔍 Distributed Tracing**
  - OpenTelemetry integration
  - Jaeger support
  - Automatic trace propagation

- **📝 Audit Logging**
  - Automatic audit for all database operations
  - Operation type and timestamp tracking
  - User context capture

- **🗄️ Advanced Database Features**
  - Database migration support
  - Automatic migration execution
  - Data sharding support
  - Global index for cross-database queries

### Developer Experience

- **🎯 Procedural Macros**
  - `#[db_entity]` - Entity definition
  - `#[db_crud]` - Automatic CRUD methods
  - `#[db_permission]` - Permission declarations
  - `#[db_cache]` - Cache annotations
  - `#[db_audit]` - Audit annotations

- **🔧 Flexible Configuration**
  - Environment variables
  - YAML configuration files
  - TOML configuration files
  - Builder pattern API

## 🎨 Feature Flags

DBNexus uses Cargo features to allow you to pick exactly what you need:

### Database Drivers (choose one)

```toml
# SQLite (default)
dbnexus = { version = "0.1", features = ["sqlite"] }

# PostgreSQL
dbnexus = { version = "0.1", features = ["postgres"] }

# MySQL
dbnexus = { version = "0.1", features = ["mysql"] }
```

### Runtime

```toml
# Tokio with RustLS (default)
dbnexus = { version = "0.1", features = ["runtime-tokio-rustls"] }

# Tokio with Native TLS
dbnexus = { version = "0.1", features = ["runtime-tokio-native-tls"] }

# AsyncStd
dbnexus = { version = "0.1", features = ["runtime-async-std"] }
```

### Optional Features

```toml
# Core features
dbnexus = { version = "0.1", features = [
    "permission",      # Permission control
    "sql-parser",      # SQL parsing
    "macros",          # Procedural macros
] }

# Enterprise features
dbnexus = { version = "0.1", features = [
    "metrics",         # Prometheus metrics
    "tracing",         # Distributed tracing
    "audit",           # Audit logging
    "migration",       # Database migration
    "sharding",        # Data sharding
] }

# Configuration
dbnexus = { version = "0.1", features = [
    "config-yaml",     # YAML config support
    "config-toml",     # TOML config support
    "config-env",       # Environment variables (default)
] }
```

### Preset Configurations

```toml
# Minimal for embedded devices
dbnexus = { version = "0.1", default-features = false, features = ["minimal"] }

# Microservice setup
dbnexus = { version = "0.1", default-features = false, features = ["microservice"] }

# Full enterprise features
dbnexus = { version = "0.1", default-features = false, features = ["all-optional"] }
```

See [FEATURES.md](FEATURES.md) for a complete list of all features and their combinations.

## 📚 Documentation

- **[User Guide](USER_GUIDE.md)** - Comprehensive guide for using DBNexus
- **[API Reference](API_REFERENCE.md)** - Complete API documentation
- **[Architecture](ARCHITECTURE.md)** - System architecture and design decisions
- **[Examples](examples/)** - Working code examples
- **[Rust Docs](https://docs.rs/dbnexus)** - API documentation on docs.rs

## 💡 Examples

### Configuration

```rust
use dbnexus::{DbPool, config::DbConfigBuilder};

let config = DbConfigBuilder::new()
    .url("postgresql://user:pass@localhost/db")
    .max_connections(20)
    .min_connections(5)
    .idle_timeout(300)
    .acquire_timeout(5000)
    .build()?;

let pool = DbPool::try_from_config(config).await?;
```

### Environment Variables

```bash
export DATABASE_URL="postgresql://user:pass@localhost/db"
export DB_MAX_CONNECTIONS=20
export DB_MIN_CONNECTIONS=5
export DB_ADMIN_ROLE=admin
```

```rust
let pool = DbPool::new().await?;
```

### Transactions

```rust
use dbnexus::DbPool;

let pool = DbPool::new("sqlite::memory:").await?;
let mut session = pool.get_session("admin").await?;

// Begin transaction
session.begin_transaction().await?;

// Multiple operations
User::insert(&session, user1).await?;
User::insert(&session, user2).await?;

// Commit
session.commit_transaction().await?;
```

### Monitoring

```rust
use dbnexus::{DbPool, metrics::MetricsCollector};

let pool = DbPool::new("postgresql://localhost/db").await?;

// Get pool status
let status = pool.status();
println!("Active: {}, Idle: {}", status.active, status.idle);

// Export Prometheus metrics
let metrics = MetricsCollector::new(&pool);
println!("{}", metrics.export_prometheus());
```

See the [examples/](examples/) directory for more comprehensive examples.

## 🏗️ Architecture

DBNexus follows a layered architecture:

<div align="center">

![DBNexus Architecture](resource/DBNexus.png)

</div>

```
┌─────────────────────────────────────────────────┐
│           Application Layer                    │
│  (Your code using DbPool and Session)       │
└────────────────┬────────────────────────────┘
                 │
┌────────────────▼────────────────────────────┐
│         DBNexus API Layer                  │
│  - DbPool, Session                        │
│  - Permission checking                    │
│  - Transaction management                 │
└────────────────┬────────────────────────────┘
                 │
┌────────────────▼────────────────────────────┐
│         Feature Modules                     │
│  - Config, Permission, Metrics            │
│  - Migration, Sharding, Audit             │
└────────────────┬────────────────────────────┘
                 │
┌────────────────▼────────────────────────────┐
│         Connection Pool                     │
│  - Connection lifecycle management          │
│  - Health checking                        │
│  - RAII guarantees                       │
└────────────────┬────────────────────────────┘
                 │
┌────────────────▼────────────────────────────┐
│         Sea-ORM / SQLx                    │
│  - Database drivers                       │
│  - Query builder                        │
└───────────────────────────────────────────┘
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed architecture documentation.

## 🔒 Security

DBNexus is built with security in mind:

- **No unsafe code** - `#![forbid(unsafe_code)]` in all library code
- **Permission enforcement** - Table-level access control with compile-time verification
- **SQL injection prevention** - Parameterized queries by default
- **Config path validation** - Protection against path traversal attacks
- **Rate limiting** - Permission check rate limiting to prevent abuse

## 🧪 Testing

### Run Tests

```bash
# SQLite tests
cargo test --features sqlite

# PostgreSQL tests
cargo test --features postgres

# MySQL tests
cargo test --features mysql

# All tests (requires Docker)
make test-all
```

### Using Docker

```bash
# Start databases
make docker-up

# Run all tests
make test-all

# Stop databases
make docker-down
```

## 🤝 Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Development Setup

```bash
# Clone repository
git clone https://github.com/Kirky-X/dbnexus.git
cd dbnexus

# Install pre-commit hooks
./scripts/install-pre-commit.sh

# Run tests
cargo test --all-features

# Run linter
cargo clippy --all-features
```

## 📝 License

Licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## 🙏 Acknowledgments

- [Sea-ORM](https://www.sea-ql.org/SeaORM/) - The excellent ORM framework DBNexus is built on
- [SQLx](https://github.com/launchbadge/sqlx) - Async SQL toolkit
- The Rust community for amazing tools and libraries

## 📞 Support

- **Documentation**: https://docs.rs/dbnexus
- **Issues**: https://github.com/Kirky-X/dbnexus/issues
- **Discussions**: https://github.com/Kirky-X/dbnexus/discussions

## 🌟 Star History

If you find DBNexus useful, please consider giving it a star ⭐ on [GitHub](https://github.com/Kirky-X/dbnexus)!

---

<div align="center">

**Built with ❤️ by the DBNexus Team**

[GitHub](https://github.com/Kirky-X/dbnexus) • [Rust](https://www.rust-lang.org) • [Sea-ORM](https://www.sea-ql.org/SeaORM/)

</div>
