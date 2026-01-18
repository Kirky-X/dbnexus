<div align="center">

<img src="resource/DBNexus.png" alt="DBNexus Logo" width="200" style="margin-bottom: 16px;">

<p>
  <!-- CI/CD 状态 -->
  <a href="https://github.com/Kirky-X/dbnexus/actions/workflows/ci.yml">
    <img src="https://github.com/Kirky-X/dbnexus/actions/workflows/ci.yml/badge.svg" alt="CI Status" style="display:inline;margin:0 4px;">
  </a>
  <!-- 版本 -->
  <a href="https://crates.io/crates/dbnexus">
    <img src="https://img.shields.io/crates/v/dbnexus.svg" alt="Version" style="display:inline;margin:0 4px;">
  </a>
  <!-- 文档 -->
  <a href="https://docs.rs/dbnexus">
    <img src="https://docs.rs/dbnexus/badge.svg" alt="Documentation" style="display:inline;margin:0 4px;">
  </a>
  <!-- 下载量 -->
  <a href="https://crates.io/crates/dbnexus">
    <img src="https://img.shields.io/crates/d/dbnexus.svg" alt="Downloads" style="display:inline;margin:0 4px;">
  </a>
  <!-- 许可证 -->
  <a href="https://github.com/Kirky-X/dbnexus/blob/main/LICENSE">
    <img src="https://img.shields.io/crates/l/dbnexus.svg" alt="License" style="display:inline;margin:0 4px;">
  </a>
  <!-- Rust 版本 -->
  <a href="https://www.rust-lang.org/">
    <img src="https://img.shields.io/badge/rust-1.85+-orange.svg" alt="Rust 1.85+" style="display:inline;margin:0 4px;">
  </a>
</p>

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
use dbnexus::{DbPool, DbEntity, db_crud, db_permission};

#[derive(DbEntity)]
#[table_name = "users"]
#[db_crud]
#[db_permission(roles = ["admin", "manager"], operations = ["SELECT", "INSERT"])]
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
    let user = User { id: 1, name: "Alice".to_string(), email: "alice@example.com".to_string() };
    User::insert(&session, user).await?;
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
| ✅ | **Multi-Database Support** | SQLite, PostgreSQL, MySQL |

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
| 💾 | **Caching** | LRU cache support (`cache` feature) |

</td>
</tr>
</table>

### 📦 Feature Presets

| Preset | Features | Use Case |
|------|------|----------|
| <span style="color:#166534; padding:4px 8px; border-radius:4px;">minimal</span> | `runtime-tokio-rustls`, `sqlite`, `config-env`, `lru`, `regex`, `sql-parser` | Minimal for embedded devices |
| <span style="color:#1E40AF; padding:4px 8px; border-radius:4px;">microservice</span> | `runtime-tokio-rustls`, `postgres`, `permission`, `sql-parser`, `config-env`, `pool-health-check`, `config-yaml`, `regex`, `lru` | Microservice setup |
| <span style="color:#991B1B; padding:4px 8px; border-radius:4px;">all-optional</span> | All enterprise features | Full enterprise features |

---

## <span id="quick-start">🚀 Quick Start</span>

### <span id="installation">📦 Installation</span>

Add this to your `Cargo.toml`:

```toml
[dependencies]
dbnexus = "0.1.1"
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
use dbnexus::{DbPool, DbEntity, db_crud};

#[derive(DbEntity)]
#[table_name = "users"]
#[db_crud]
struct User {
    #[primary_key]
    id: i64,
    name: String,
    email: String,
}
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
let user = User {
    id: 1,
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
};
User::insert(&session, user).await?;
```

</td>
<td width="50%" style="padding: 16px; vertical-align:top;">

**Step 4: Query Data**

```rust
let users = User::find_all(&session).await?;
println!("Found {} users", users.len());
```

</td>
</tr>
</table>

### <span id="permission-control">🔒 Permission Control</span>

```rust
use dbnexus::{DbPool, DbEntity, db_crud, db_permission};

#[derive(DbEntity)]
#[table_name = "users"]
#[db_crud]
#[db_permission(roles = ["admin", "manager"], operations = ["SELECT", "INSERT"])]
struct User {
    #[primary_key]
    id: i64,
    name: String,
}

// Admin can access
let session = pool.get_session("admin").await?;
User::find_all(&session).await?;

// Regular user will be denied
let session = pool.get_session("guest").await?;
User::find_all(&session).await?; // Error: Permission denied
```

---

## <span id="feature-flags">🎨 Feature Flags</span>

### Database Drivers (choose one)

```toml
# SQLite (default)
dbnexus = { version = "0.1.1", features = ["sqlite"] }

# PostgreSQL
dbnexus = { version = "0.1.1", features = ["postgres"] }

# MySQL
dbnexus = { version = "0.1.1", features = ["mysql"] }
```

### Runtime

```toml
# Tokio with RustLS (default)
dbnexus = { version = "0.1.1", features = ["runtime-tokio-rustls"] }

# Tokio with Native TLS
dbnexus = { version = "0.1.1", features = ["runtime-tokio-native-tls"] }

# AsyncStd
dbnexus = { version = "0.1.1", features = ["runtime-async-std"] }
```

### Optional Features

```toml
# Core features
dbnexus = { version = "0.1.1", features = [
    "permission",      # Permission control
    "sql-parser",      # SQL parsing
    "macros",          # Procedural macros
] }

# Enterprise features
dbnexus = { version = "0.1.1", features = [
    "metrics",         # Prometheus metrics
    "tracing",         # Distributed tracing
    "audit",           # Audit logging
    "migration",       # Database migration
    "sharding",        # Data sharding
] }

# Configuration
dbnexus = { version = "0.1.1", features = [
    "config-yaml",     # YAML config support
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
<a href="USER_GUIDE.md" style="text-decoration:none;">
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
| 📖 [User Guide](USER_GUIDE.md) | Comprehensive guide for using DBNexus |
| 📘 [API Reference](API_REFERENCE.md) | Complete API documentation |
| 🏗️ [Architecture](ARCHITECTURE.md) | System architecture and design decisions |
| 📦 [Examples](examples/) | Working code examples |

---

## <span id="examples">💻 Examples</span>

<div align="center" style="margin: 24px 0;">

### 💡 Real-world Examples

</div>

<table style="width:100%; border-collapse: collapse;">
<tr>
<td width="50%" style="padding: 16px; border-radius:8px; border:1px solid #E2E8F0; vertical-align:top;">

#### 📝 Configuration

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
let pool = DbPool::new().await?;
```

</td>
</tr>
<tr>
<td width="50%" style="padding: 16px; border-radius:8px; border:1px solid #E2E8F0; vertical-align:top;">

#### 🔄 Transactions

```rust
let mut session = pool.get_session("admin").await?;

// Begin transaction
session.begin_transaction().await?;

// Multiple operations
User::insert(&session, user1).await?;
User::insert(&session, user2).await?;

// Commit
session.commit_transaction().await?;
```

</td>
<td width="50%" style="padding: 16px; border-radius:8px; border:1px solid #E2E8F0; vertical-align:top;">

#### 📊 Monitoring

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

</td>
</tr>
</table>

<div align="center" style="margin: 24px 0;">

**[📂 View all examples →](examples/)**

</div>

---

## <span id="architecture">🏗️ Architecture</span>

<div align="center" style="margin: 24px 0;">

### 🏗️ System Architecture

</div>

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

---

## <span id="contributing">🤝 Contributing</span>

<div align="center" style="margin: 24px 0;">

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

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

## <span id="license">📄 License</span>

<div align="center" style="margin: 24px 0;">

This project is dual-licensed under **MIT / Apache-2.0**:

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE-MIT)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE-APACHE)

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

**Built with ❤️ by the DBNexus Team**

---

**[⬆ Back to Top](#dbnexus)**

---

<sub>© 2024 DBNexus Project. All rights reserved.</sub>

</div>