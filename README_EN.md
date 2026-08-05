<div align="center">

<a name="dbnexus"></a>

<img src="docs/assets/logo.png" alt="DBNexus Logo" width="200" style="margin-bottom: 16px;">

<p>
  <!-- CI/CD Status -->
  <a href="https://github.com/Kirky-X/dbnexus/actions/workflows/ci.yml">
    <img src="https://github.com/Kirky-X/dbnexus/actions/workflows/ci.yml/badge.svg" alt="CI Status" style="display:inline;margin:0 4px;">
  </a>
  <!-- Version -->
  <a href="https://crates.io/crates/dbnexus">
    <img src="https://img.shields.io/crates/v/dbnexus.svg" alt="Version" style="display:inline;margin:0 4px;">
  </a>
  <!-- Documentation -->
  <a href="https://docs.rs/dbnexus">
    <img src="https://docs.rs/dbnexus/badge.svg" alt="Documentation" style="display:inline;margin:0 4px;">
  </a>
  <!-- Downloads -->
  <a href="https://crates.io/crates/dbnexus">
    <img src="https://img.shields.io/crates/d/dbnexus.svg" alt="Downloads" style="display:inline;margin:0 4px;">
  </a>
  <!-- License -->
  <a href="https://github.com/Kirky-X/dbnexus/blob/main/LICENSE">
    <img src="https://img.shields.io/crates/l/dbnexus.svg" alt="License" style="display:inline;margin:0 4px;">
  </a>
  <!-- Rust Version -->
  <a href="https://www.rust-lang.org/">
    <img src="https://img.shields.io/badge/rust-1.91+-orange.svg" alt="Rust 1.91+" style="display:inline;margin:0 4px;">
  </a>
</p>

[中文](./README.md) | English

<p align="center">
  <strong>Enterprise-grade Database Abstraction Layer for Rust</strong>
</p>

<p align="center">
  <a href="#features" style="color:#3B82F6;">✨ Features</a> •
  <a href="#quick-start" style="color:#3B82F6;">🚀 Quick Start</a> •
  <a href="#documentation" style="color:#3B82F6;">📚 Documentation</a> •
  <a href="#examples" style="color:#3B82F6;">💻 Examples</a> •
  <a href="#contributing" style="color:#3B82F6;">🤝 Contributing</a>
</p>

</div>

---

### 🎯 A high-performance, secure, and feature-rich database access layer built on Sea-ORM

DBNexus provides a **declarative** database access approach:

| ✨ Type Safe | 🔒 Permission Control | 🏊 Smart Pooling | 📊 Enterprise Monitoring |
|:---------:|:----------:|:--------------:|:--------:|
| Compile-time checks | Table-level RBAC | RAII auto-management | Prometheus metrics |

```rust
use dbnexus::{DbPool, db_entity};
use sea_orm::entity::prelude::*;

#[db_entity(table_name = "users", primary_key = "id")]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub email: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("sqlite::memory:").await?;
    let session = pool.get_session("admin").await?;
    let user = Model { id: 1, name: "Alice".to_string(), email: "alice@example.com".to_string() };
    Model::insert(&session, user).await?;
    Ok(())
}
```

---

## 📋 Table of Contents

<details open style="border-radius:8px; padding:16px; border:1px solid #E2E8F0;">
<summary style="cursor:pointer; font-weight:600; color:#1E293B;">📑 Table of Contents (Click to expand)</summary>

- [✨ Features](#features)
- [🚀 Quick Start](#quick-start)
  - [📦 Installation](#installation)
  - [💡 Basic Usage](#basic-usage)
  - [🔒 Permission Control](#permission-control)
- [🎨 Feature Flags](#feature-flags)
- [📚 Documentation](#documentation)
- [💻 Examples](#examples)
- [🏗️ Architecture](#architecture)
- [🔒 Security](#security)
- [🧪 Testing](#testing)
- [🤝 Contributing](#contributing)
- [📋 Changelog](#changelog)
- [📄 License](#license)
- [🙏 Acknowledgments](#acknowledgments)

</details>

---

## <span id="features">✨ Features</span>

<div align="center" style="margin: 24px 0;">

| 🎯 Core Features | ⚡ Enterprise Features |
|:----------:|:----------:|
| Always Available | Optional |

</div>

<table style="width:100%; border-collapse: collapse;">
<tr>
<td width="50%" style="vertical-align:top; padding: 16px; border-radius:8px; border:1px solid #E2E8F0;">

### 🎯 Core Features (Always Available)

| Status | Feature | Description |
|:----:|------|------|
| ✅ | **Connection Pooling** | RAII-style automatic connection lifecycle management |
| ✅ | **Permission Control** | Role-based table-level access control (RBAC) |
| ✅ | **Procedural Macros** | Auto-generate CRUD methods and permission checks |
| ✅ | **SQL Parser** | Extract operation type and target table |
| ✅ | **Transaction Support** | Complete transaction management |
| ✅ | **Multi-Database Support** | SQLite, PostgreSQL, MySQL, DuckDB, Ladybug, Neo4j |

</td>
<td width="50%" style="vertical-align:top; padding: 16px; border-radius:8px; border:1px solid #E2E8F0;">

### ⚡ Enterprise Features

| Status | Feature | Description |
|:----:|------|------|
| 🔍 | **Metrics Monitoring** | Prometheus metrics export (`metrics` feature) |
| 📊 | **Distributed Tracing** | OpenTelemetry integration (`tracing` feature) |
| 📝 | **Audit Logging** | Automatic audit for all operations (`audit` feature) |
| 🗄️ | **Database Migration** | Automatic migration execution (`migration` feature) |
| 🔀 | **Data Sharding** | Support for sharding strategies (`sharding` feature) |
| 🌐 | **Global Index** | Cross-shard queries (`global-index` feature) |
| 💾 | **Caching** | oxcache cache (moka L1 backend internally) (`cache` feature) |
| 🔐 | **Permission Engine** | Advanced permission system (`permission-engine` feature) |
| 🛡️ | **JWT Authentication** | JWT + password strength validation (`authentication` feature) |
| 🌍 | **Internationalization** | ICU4X locale-aware formatting (core feature, always available) |
| 🔁 | **Retry** | Exponential backoff + idempotency check (`retry` feature) |
| 🔄 | **Failover** | CircuitBreaker state machine (`failover` feature) |
| 🌐 | **Replica Routing** | Read/write splitting (`replica-routing` feature) |
| 📡 | **Scatter-Gather** | Cross-shard aggregate queries (`scatter-gather` feature) |
| 🧩 | **Saga Transactions** | Distributed transaction orchestration (`saga` feature) |
| 🔢 | **Distributed ID** | Snowflake ID generation (`distributed-id` feature) |

</td>
</tr>
</table>

### 📦 Feature Presets

| Preset | Features | Use Case |
|------|------|----------|
| <span style="color:#166534; padding:4px 8px; border-radius:4px;">embedded</span> | `runtime-tokio-rustls`, `sqlite`, `config-env` | Ultra-minimal for embedded/edge devices |
| <span style="color:#1E40AF; padding:4px 8px; border-radius:4px;">microservice</span> | `runtime-tokio-rustls`, `postgres`, `permission`, `sql-parser`, `config-env`, `observability` | Microservice deployment |
| <span style="color:#7C3AED; padding:4px 8px; border-radius:4px;">monolith</span> | `runtime-tokio-rustls`, `postgres`, `permission`, `sql-parser`, `yaml`, `data-management`, `security`, `observability` | Monolithic application |
| <span style="color:#991B1B; padding:4px 8px; border-radius:4px;">enterprise</span> | `postgres`, `monolith`, `permission-engine` | Full enterprise features |
| <span style="color:#64748B; padding:4px 8px; border-radius:4px;">all-optional</span> | `cache`, `observability`, `data-management`, `security`, `migration`, `distributed-capabilities` | 6 aggregate features (manually add database drivers and other features) |

---

## <span id="quick-start">🚀 Quick Start</span>

### <span id="installation">📦 Installation</span>

Add this to your `Cargo.toml`:

```toml
[dependencies]
dbnexus = { version = "0.5", default-features = false, features = ["runtime-tokio-rustls", "sqlite", "permission", "sql-parser", "macros", "config-env"] }
tokio = { version = "1.52", features = ["rt-multi-thread", "macros"] }
sea-orm = { version = "2.0.0-rc.42", features = ["macros"] }
```

### <span id="basic-usage">💡 Basic Usage</span>

<div align="center" style="margin: 24px 0;">

#### 🎬 5-Minute Quick Start

</div>

<table style="width:100%; border-collapse: collapse;">
<tr>
<td width="50%" style="padding: 16px; vertical-align:top;">

**Step 1: Define Entity**

```rust
use dbnexus::{DbPool, db_entity};
use sea_orm::entity::prelude::*;

#[db_entity(table_name = "users", primary_key = "id")]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub email: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

</td>
<td width="50%" style="padding: 16px; vertical-align:top;">

**Step 2: Create Connection Pool**

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("sqlite::memory:").await?;
    let session = pool.get_session("admin").await?;
    Ok(())
}
```

</td>
</tr>
<tr>
<td width="50%" style="padding: 16px; vertical-align:top;">

**Step 3: Insert Data**

```rust
let user = Model {
    id: 1,
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
};
Model::insert(&session, user).await?;
```

</td>
<td width="50%" style="padding: 16px; vertical-align:top;">

**Step 4: Query Data**

```rust
let users = Model::find_all(&session).await?;
println!("Found {} users", users.len());
```

</td>
</tr>
</table>

### <span id="permission-control">🔒 Permission Control</span>

```rust
use dbnexus::{DbPool, db_entity};
use sea_orm::entity::prelude::*;

#[db_entity(table_name = "users", primary_key = "id")]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Admin can access
let session = pool.get_session("admin").await?;
Model::find_all(&session).await?;

// Regular user will be denied
let session = pool.get_session("guest").await?;
Model::find_all(&session).await?; // Error: Permission denied
```

---

## <span id="feature-flags">🎨 Feature Flags</span>

### ⚠️ BREAKING CHANGE in v0.2.0

**All users must update their Cargo.toml:**

**Version 0.1.x → 0.2.0 is a breaking change.** The `cache` feature is no longer enabled by default, and several features now explicitly require `cache` to be enabled.

### ⚠️ BREAKING CHANGE in v0.4.0

- `default` feature is now an empty array `[]` (previously included 7 features)
- Users must explicitly enable runtime + database driver + desired features
- Recommended: `default-features = false, features = ["default-no-db", "sqlite"]`

### Database Drivers (choose one)

```toml
# SQLite (default, embedded)
dbnexus = { version = "0.5", features = ["sqlite"] }

# PostgreSQL
dbnexus = { version = "0.5", features = ["postgres"] }

# MySQL
dbnexus = { version = "0.5", features = ["mysql"] }

# DuckDB (embedded analytical database, new in 0.3.0)
dbnexus = { version = "0.5", features = ["duckdb"] }

# Ladybug (embedded graph database, new in 0.4.0)
dbnexus = { version = "0.5", features = ["ladybug"] }

# Neo4j (graph database server, new in 0.4.0)
dbnexus = { version = "0.5", features = ["neo4j"] }
```

### Protocol-Compatible Databases

DBNexus supports the following protocol-compatible databases via standard protocols (no extra feature needed, just use the corresponding protocol driver):

| Database | Compatible Protocol | Description |
|----------|---------------------|-------------|
| CockroachDB | PostgreSQL | Distributed SQL database |
| YugabyteDB | PostgreSQL | Distributed PostgreSQL |
| TiDB | MySQL | Distributed HTAP database |
| MariaDB | MySQL | MySQL-compatible fork |
| Aurora | PostgreSQL/MySQL | AWS cloud-native database |

### Runtime

```toml
# Tokio with RustLS (default)
dbnexus = { version = "0.5", features = ["runtime-tokio-rustls"] }

# Tokio with Native TLS
dbnexus = { version = "0.5", features = ["runtime-tokio-native-tls"] }

# AsyncStd
dbnexus = { version = "0.5", features = ["runtime-async-std"] }
```

### Core Features

```toml
# Permission control (auto-enables sql-parser + yaml + cache features; sql-parser is mandatory to prevent SQL injection bypass)
dbnexus = { version = "0.5", features = ["permission"] }

# SQL parsing (auto-enables cache feature)
dbnexus = { version = "0.5", features = ["sql-parser"] }

# Procedural macros
dbnexus = { version = "0.5", features = ["macros"] }
```

### Using Presets (Recommended)

```toml
# Embedded/Edge devices (minimal)
dbnexus = { version = "0.5", features = ["embedded"] }

# Microservices
dbnexus = { version = "0.5", features = ["microservice"] }

# Monolithic applications
dbnexus = { version = "0.5", features = ["monolith"] }

# Enterprise (all features)
dbnexus = { version = "0.5", features = ["enterprise"] }
```

### Optional Features

```toml
# Observability (metrics + tracing + health-check)
dbnexus = { version = "0.5", features = ["observability"] }

# Data management (migration + sharding + global-index)
dbnexus = { version = "0.5", features = ["data-management"] }

# Security (audit + permission-engine)
dbnexus = { version = "0.5", features = ["security"] }

# Individual features
dbnexus = { version = "0.5", features = [
    "metrics",          # Prometheus metrics
    "tracing",          # Distributed tracing
    "audit",            # Audit logging
    "migration",        # Database migration
    "sharding",         # Data sharding
    "global-index",     # Cross-shard global index
    "permission-engine", # Advanced permission engine (requires cache)
    "authentication",   # JWT authentication + password strength validation
    "distributed-capabilities" # Distributed capabilities aggregate (retry/failover/saga/scatter-gather etc.)
    # i18n is now a core feature, always available without explicit enabling
] }
```

### Configuration

```toml
dbnexus = { version = "0.5", features = [
    "yaml",            # YAML config support
    "config-toml",     # TOML config support
    "config-env",      # Environment variables (default)
] }
```

---

## <span id="documentation">📚 Documentation</span>

<div align="center" style="margin: 24px 0;">

<table style="width:100%; max-width: 800px;">
<tr>
<td align="center" width="33%" style="padding: 16px;">
<a href="docs/USER_GUIDE.md" style="text-decoration:none;">
<div style="padding: 24px; border-radius:12px; transition: transform 0.2s;">
<b style="color:#1E293B;">📖 User Guide</b>
</div>
</a>
<br><span style="color:#64748B;">Comprehensive guide</span>
</td>
<td align="center" width="33%" style="padding: 16px;">
<a href="https://docs.rs/dbnexus" style="text-decoration:none;">
<div style="padding: 24px; border-radius:12px; transition: transform 0.2s;">
<b style="color:#1E293B;">📘 API Reference</b>
</div>
</a>
<br><span style="color:#64748B;">Complete API docs</span>
</td>
<td align="center" width="33%" style="padding: 16px;">
<a href="examples/" style="text-decoration:none;">
<div style="padding: 24px; border-radius:12px; transition: transform 0.2s;">
<b style="color:#1E293B;">💻 Examples</b>
</div>
</a>
<br><span style="color:#64748B;">Code examples</span>
</td>
</tr>
</table>

</div>

### 📖 Additional Resources

| Resource | Description |
|----------|-------------|
| 📖 [User Guide](docs/USER_GUIDE.md) | Comprehensive guide for using DBNexus |
| 📘 [API Reference](docs/API_REFERENCE.md) | Complete API documentation |
| 🏗️ [Architecture](docs/ARCHITECTURE.md) | System architecture and design decisions |
| 📦 [Examples](examples/) | Working code examples |

---

## <span id="examples">💻 Examples</span>

<div align="center" style="margin: 24px 0;">

### 💡 Real-world Examples

</div>

<table style="width:100%; border-collapse: collapse;">
<tr>
<td width="50%" style="padding: 16px; border-radius:8px; border:1px solid #E2E8F0; vertical-align:top;">

#### 📝 Advanced Configuration

```rust
use dbnexus::{DbPool, DbConfig, PoolConfig};

let config = DbConfig {
    url: "postgresql://user:pass@localhost/db".to_string(),
    pool_config: PoolConfig {
        max_connections: 20,
        min_connections: 5,
        idle_timeout: 300,
        acquire_timeout: 5000,
    },
    ..Default::default()
};

let pool = DbPool::with_config(config).await?;
```

</td>
<td width="50%" style="padding: 16px; border-radius:8px; border:1px solid #E2E8F0; vertical-align:top;">

#### 🔧 Environment Variables

```bash
export DATABASE_URL="postgresql://user:pass@localhost/db"
export DB_MAX_CONNECTIONS=20
export DB_MIN_CONNECTIONS=5
export DB_ADMIN_ROLE=admin
```

```rust
let config = dbnexus::DbConfig::from_env()?;
let pool = dbnexus::DbPool::with_config(config).await?;
```

</td>
</tr>
<tr>
<td width="50%" style="padding: 16px; border-radius:8px; border:1px solid #E2E8F0; vertical-align:top;">

#### 🔄 Transactions

```rust
let session = pool.get_session("admin").await?;

// Begin transaction
session.begin_transaction().await?;

// Multiple operations
Model::insert(&session, user1).await?;
Model::insert(&session, user2).await?;

// Commit
session.commit().await?;
```

</td>
<td width="50%" style="padding: 16px; border-radius:8px; border:1px solid #E2E8F0; vertical-align:top;">

#### 📊 Monitoring

```rust
use dbnexus::{DbPool, MetricsCollector};

let pool = DbPool::new("postgresql://localhost/db").await?;

// Get pool status
let status = pool.status();
println!("Active: {}, Idle: {}", status.active, status.idle);

// Export Prometheus metrics
let metrics = MetricsCollector::new();
println!("{}", metrics.export_prometheus());
```

</td>
</tr>
</table>

<div align="center" style="margin: 24px 0;">

**[📂 View all examples →](examples/)**

---

> **Note**: `dbnexus-examples` is set to `publish = false` and managed within the workspace.

</div>

---

## <span id="architecture">🏗️ Architecture</span>

<div align="center" style="margin: 24px 0;">

### 🏗️ System Architecture

</div>

<div align="center">

</div>

```mermaid
graph TD
    A[Application Layer<br/>Your code using DbPool and Session] --> B[DBNexus API Layer<br/>DbPool, Session<br/>Permission checking<br/>Transaction management]
    B --> C[Feature Modules<br/>Config, Permission, Metrics<br/>Migration, Sharding, Audit]
    C --> D[Connection Pool<br/>Connection lifecycle management<br/>Health checking<br/>RAII guarantees]
    D --> E[Sea-ORM / SQLx<br/>Database drivers<br/>Query builder]
```

See [ARCHITECTURE.md](docs/ARCHITECTURE.md) for detailed architecture documentation.

---

## <span id="security">🔒 Security</span>

<div align="center" style="margin: 24px 0;">

### 🛡️ Security Features

</div>

DBNexus is built with security in mind:

- **No unsafe code** - `#![forbid(unsafe_code)]` in all library code
- **Permission enforcement** - Table-level access control with compile-time verification
- **SQL injection prevention** - Parameterized queries by default
- **Config path validation** - Protection against path traversal attacks
- **Rate limiting** - Permission check rate limiting to prevent abuse

---

## <span id="testing">🧪 Testing</span>

<div align="center" style="margin: 24px 0;">

### 🎯 Run Tests

</div>

```bash
# Run all tests
cargo test --all-features

# Run tests for a specific database (requires Docker environment)
cargo test --features "runtime-tokio-rustls postgres"
```

---

## <span id="contributing">🤝 Contributing</span>

<div align="center" style="margin: 24px 0;">

Contributions are welcome! Please check [CONTRIBUTING.md](docs/CONTRIBUTING.md) for detailed contribution guidelines.

</div>

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

---

## <span id="changelog">📋 Changelog</span>

See [CHANGELOG.md](docs/CHANGELOG.md).

---

## <span id="license">📄 License</span>

<div align="center" style="margin: 24px 0;">

This project is licensed under **MIT**:

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

</div>

---

## <span id="acknowledgments">🙏 Acknowledgments</span>

<div align="center" style="margin: 24px 0;">

### 🌟 Built on Excellent Tools

</div>

- [Sea-ORM](https://www.sea-ql.org/SeaORM/) - The excellent ORM framework DBNexus is built on
- [SQLx](https://github.com/launchbadge/sqlx) - Async SQL toolkit
- The Rust community for amazing tools and libraries

---

## 📞 Support

<div align="center" style="margin: 24px 0;">

<table style="width:100%; max-width: 600px;">
<tr>
<td align="center" width="33%">
<a href="https://github.com/Kirky-X/dbnexus/issues">
<div style="padding: 16px; border-radius:8px;">
<b style="color:#991B1B;">📋 Issues</b>
</div>
</a>
<br><span style="color:#64748B;">Report bugs and issues</span>
</td>
<td align="center" width="33%">
<a href="https://github.com/Kirky-X/dbnexus/discussions">
<div style="padding: 16px; border-radius:8px;">
<b style="color:#1E40AF;">💬 Discussions</b>
</div>
</a>
<br><span style="color:#64748B;">Ask questions and share ideas</span>
</td>
<td align="center" width="33%">
<a href="https://github.com/Kirky-X/dbnexus">
<div style="padding: 16px; border-radius:8px;">
<b style="color:#1E293B;">🐙 GitHub</b>
</div>
</a>
<br><span style="color:#64748B;">View source code</span>
</td>
</tr>
</table>

</div>

---

## ⭐ Star History

<div align="center">

[![Star History Chart](https://api.star-history.com/svg?repos=Kirky-X/dbnexus&type=Date)](https://star-history.com/#Kirky-X/dbnexus&Date)

</div>

---

<div align="center" style="margin: 32px 0; padding: 24px; border-radius: 12px;">

### 💝 Support This Project

If you find this project useful, please consider giving it a ⭐️!

**Built with ❤️ by Kirky.X**

---

**[⬆ Back to Top](#dbnexus)**

---

<sub>© 2026 Kirky.X. All rights reserved.</sub>

</div>
