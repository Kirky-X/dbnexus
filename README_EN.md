<div align="center">

<img src="docs/assets/dbnexus.png" alt="DBNexus Logo" width="200">

[![CI Status](https://github.com/Kirky-X/dbnexus/actions/workflows/ci.yml/badge.svg)](https://github.com/Kirky-X/dbnexus/actions/workflows/ci.yml) [![Version](https://img.shields.io/crates/v/dbnexus.svg)](https://crates.io/crates/dbnexus) [![Docs.rs](https://docs.rs/dbnexus/badge.svg)](https://docs.rs/dbnexus) [![Downloads](https://img.shields.io/crates/d/dbnexus.svg)](https://crates.io/crates/dbnexus) [![License](https://img.shields.io/crates/l/dbnexus.svg)](LICENSE) [![Rust](https://img.shields.io/badge/rust-1.97.1%2B-orange.svg)](https://www.rust-lang.org/) [![Coverage](https://codecov.io/gh/Kirky-X/dbnexus/branch/main/graph/badge.svg)](https://codecov.io/gh/Kirky-X/dbnexus)

**[中文](README.md)** | English

**Enterprise-grade Database Abstraction Layer for Rust**

[✨ Features](#-features) • [🚀 Quick Start](#-quick-start) • [📚 Documentation](#-documentation) • [💻 Examples](#-examples) • [🤝 Contributing](#-contributing)

</div>

---

## 📋 Table of Contents

<details open>
<summary>📑 Table of Contents (Click to expand)</summary>

- [✨ Features](#-features)
- [🚀 Quick Start](#-quick-start)
  - [📦 Installation](#-installation)
  - [💡 Basic Usage](#-basic-usage)
  - [🔒 Permission Control](#-permission-control)
- [🎨 Feature Flags](#-feature-flags)
- [📚 Documentation](#-documentation)
- [💻 Examples](#-examples)
- [🏗️ Architecture](#️-architecture)
- [🧪 Testing](#-testing)
- [📊 Performance](#-performance)
- [🔒 Security](#-security)
- [🗺️ Roadmap](#️-roadmap)
- [🤝 Contributing](#-contributing)
- [📋 Changelog](#-changelog)
- [📄 License](#-license)
- [🙏 Acknowledgments](#-acknowledgments)
- [📞 Contact & Support](#-contact--support)
- [⭐ Star History](#-star-history)

</details>

---

## ✨ Features

A high-performance, secure, and feature-rich database access layer built on Sea-ORM. DBNexus provides a **declarative** database access approach:

| ✨ Type Safe | 🔒 Permission Control | 🏊 Smart Pooling | 📊 Enterprise Monitoring |
|:---------:|:----------:|:--------------:|:--------:|
| Compile-time checks | Table-level RBAC | RAII auto-management | Prometheus metrics |

### 🎯 Core Features (Always Available)

| Status | Feature | Description |
|:----:|------|------|
| ✅ | **Connection Pooling** | RAII-style automatic connection lifecycle management |
| ✅ | **Permission Control** | Role-based table-level access control (RBAC) |
| ✅ | **Procedural Macros** | Auto-generate CRUD methods and permission checks |
| ✅ | **SQL Parser** | Extract operation type and target table |
| ✅ | **Transaction Support** | Complete transaction management |
| ✅ | **Multi-Database Support** | SQLite, PostgreSQL, MySQL, DuckDB, Ladybug, Neo4j |

### ⚡ Enterprise Features (Optional)

| Status | Feature | Description |
|:----:|------|------|
| 🔍 | **Metrics Monitoring** | Prometheus metrics export (`metrics` feature) |
| 📝 | **Audit Logging** | Automatic audit for all operations (`audit` feature) |
| 🗄️ | **Database Migration** | Automatic migration execution (`migration` feature) |
| 🔀 | **Data Sharding** | Support for sharding strategies (`sharding` feature) |
| 🌐 | **Global Index** | Cross-shard queries (`global-index` feature) |
| 💾 | **Caching** | oxcache cache (moka L1 backend internally) (`cache` feature) |
| 🩺 | **Permission Health Check** | Memory provider validates policy table capacity, YAML provider validates policy file readability (`permission` feature) |
| 🔐 | **Permission Engine** | Advanced permission system (`permission-engine` feature) |
| 🛡️ | **JWT Authentication** | JWT + password strength validation (`authentication` feature) |
| 🌍 | **Internationalization** | ICU4X locale-aware formatting (core feature, always available) |
| 🔁 | **Retry** | Exponential backoff + idempotency check (`retry` feature) |
| 🔄 | **Failover** | CircuitBreaker state machine (`failover` feature) |
| 🌐 | **Replica Routing** | Read/write splitting (`replica-routing` feature) |
| 📡 | **Scatter-Gather** | Cross-shard aggregate queries (`scatter-gather` feature) |
| 🧩 | **Saga Transactions** | Distributed transaction orchestration (`saga` feature) |
| 🔢 | **Distributed ID** | Snowflake ID generation (`distributed-id` feature) |

### 📦 Feature Presets

| Preset | Features | Use Case |
|------|------|----------|
| `embedded` | `runtime-tokio-rustls`, `sqlite`, `config-env` | Ultra-minimal for embedded/edge devices |
| `microservice` | `runtime-tokio-rustls`, `postgres`, `permission`, `sql-parser`, `config-env`, `observability` | Microservice deployment |
| `monolith` | `runtime-tokio-rustls`, `postgres`, `permission`, `sql-parser`, `yaml`, `data-management`, `security`, `observability`, `distributed-capabilities` | Monolithic application (includes all 7 distributed capabilities) |
| `enterprise` | `postgres`, `monolith`, `permission-engine` | Full enterprise features |
| `all-optional` | `cache`, `observability`, `data-management`, `security`, `migration`, `retry`, `failover`, `replica-routing`, `scatter-gather`, `shard-migration`, `saga`, `distributed-id` | 12 individual features (manually add database drivers and other features) |

---

## 🚀 Quick Start

### 📦 Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
dbnexus = { version = "0.6.0-rc.2", default-features = false, features = ["runtime-tokio-rustls", "sqlite", "permission", "sql-parser", "macros", "config-env"] }
tokio = { version = "1.52", features = ["rt-multi-thread", "macros"] }
sea-orm = { version = "2.0.0-rc.42", features = ["macros"] }
```

### 💡 Basic Usage

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

**Step 2: Create Connection Pool**

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("sqlite::memory:").await?;
    let session = pool.get_session("admin").await?;
    Ok(())
}
```

**Step 3: Insert Data**

```rust
let user = Model {
    id: 1,
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
};
Model::insert(&session, user).await?;
```

**Step 4: Query Data**

```rust
let users = Model::find_all(&session).await?;
println!("Found {} users", users.len());
```

<details>
<summary>🎬 Complete Example (Runnable)</summary>

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

</details>

### 🔒 Permission Control

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

## 🎨 Feature Flags

### Database Drivers (choose one)

```toml
# SQLite (embedded)
dbnexus = { version = "0.6.0-rc.2", default-features = false, features = ["runtime-tokio-rustls", "sqlite"] }

# PostgreSQL
dbnexus = { version = "0.6.0-rc.2", features = ["postgres"] }

# MySQL
dbnexus = { version = "0.6.0-rc.2", features = ["mysql"] }

# DuckDB (embedded analytical database, new in 0.3.0)
dbnexus = { version = "0.6.0-rc.2", features = ["duckdb"] }

# Ladybug (embedded graph database, new in 0.4.0)
dbnexus = { version = "0.6.0-rc.2", features = ["ladybug"] }

# Neo4j (graph database server, new in 0.4.0)
dbnexus = { version = "0.6.0-rc.2", features = ["neo4j"] }
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

### Runtimes

```toml
# Tokio with RustLS (default)
dbnexus = { version = "0.6.0-rc.2", features = ["runtime-tokio-rustls"] }

# Tokio with Native TLS
dbnexus = { version = "0.6.0-rc.2", features = ["runtime-tokio-native-tls"] }

# AsyncStd
dbnexus = { version = "0.6.0-rc.2", features = ["runtime-async-std"] }
```

### Core Features

```toml
# Permission control (auto-enables sql-parser + yaml + cache features; sql-parser is mandatory to prevent SQL injection bypass)
dbnexus = { version = "0.6.0-rc.2", features = ["permission"] }

# SQL parsing (auto-enables cache feature)
dbnexus = { version = "0.6.0-rc.2", features = ["sql-parser"] }

# Procedural macros
dbnexus = { version = "0.6.0-rc.2", features = ["macros"] }
```

### Using Presets (Recommended)

```toml
# Embedded/Edge devices (minimal)
dbnexus = { version = "0.6.0-rc.2", features = ["embedded"] }

# Microservices
dbnexus = { version = "0.6.0-rc.2", features = ["microservice"] }

# Monolithic applications
dbnexus = { version = "0.6.0-rc.2", features = ["monolith"] }

# Enterprise (all features)
dbnexus = { version = "0.6.0-rc.2", features = ["enterprise"] }
```

### Optional Features

```toml
# Observability (metrics + health-check)
dbnexus = { version = "0.6.0-rc.2", features = ["observability"] }

# Data management (migration + sharding + global-index)
dbnexus = { version = "0.6.0-rc.2", features = ["data-management"] }

# Security (audit + permission-engine)
dbnexus = { version = "0.6.0-rc.2", features = ["security"] }

# Individual features
dbnexus = { version = "0.6.0-rc.2", features = [
    "metrics",          # Prometheus metrics
    "audit",            # Audit logging
    "migration",        # Database migration
    "sharding",         # Data sharding
    "global-index",     # Cross-shard global index
    "permission-engine", # Advanced permission engine (requires cache)
    "authentication",   # JWT authentication + password strength validation
    "distributed-capabilities" # Distributed capabilities aggregate (retry/failover/replica-routing/scatter-gather/shard-migration/saga/distributed-id)
    # i18n is now a core feature, always available without explicit enabling
] }
```

### Configuration

```toml
dbnexus = { version = "0.6.0-rc.2", features = [
    "yaml",            # YAML config support
    "config-toml",     # TOML config support
    "config-env",      # Environment variables (default)
] }
```

---

## 📚 Documentation

| Document | Description |
|----------|-------------|
| [📖 User Guide](docs/USER_GUIDE.md) | Complete tutorial from installation to advanced usage |
| [📘 API Reference](docs/API_REFERENCE.md) | Detailed description of all public APIs |
| [🏗️ Architecture](docs/ARCHITECTURE.md) | Design philosophy and internal implementation |
| [🔒 Security](docs/SECURITY.md) | Security design and best practices |
| [📋 Changelog](docs/CHANGELOG.md) | Change records for every version |
| [🤝 Contributing](docs/CONTRIBUTING.md) | How to participate in development |
| [📦 Online API Docs](https://docs.rs/dbnexus) | Latest documentation auto-generated by docs.rs |

---

## 💻 Examples

All examples live in [examples/](examples/) (a separate crate `dbnexus-examples`, managed in the workspace):

```bash
cd examples

# Run a single example (each example is a bin target)
cargo run --bin basic_connection

# Build all examples
cargo build --all-targets
```

| Module | Examples | Description |
|--------|----------|-------------|
| Basics | `basic_connection`, `basic_crud`, `basic_transaction` | Connection pool / `#[db_entity]` CRUD / transactions |
| Config | `config_env`, `config_yaml`, `config_toml`, `config_presets` | Env vars / YAML / TOML / preset comparison |
| Database | `database_sqlite`, `database_postgres`, `database_mysql`, `duckdb_query`, `migration`, `sharding`, `global_index`, `pool_management` | Driver connections / OLAP queries / migration / sharding / global index / pool management |
| Permission | `permission_rbac`, `permission_yaml`, `permission_macro`, `permission_engine` | RBAC / YAML policies / macro permissions / permission engine |
| Security | `sql_parser`, `sql_injection_detection`, `ddl_guard`, `sensitive_masker`, `rate_limiter` | SQL parsing / injection detection / DDL guard / masking / rate limiting |
| Auth & Audit | `authentication_jwt`, `authentication_password`, `audit_logging` | JWT / password hashing / audit logging |
| Observability | `metrics_prometheus`, `health_check`, `latency_histogram` | Metrics / health check & circuit breaker / latency histogram |
| Macros | `macros_db_entity`, `macros_db_crud`, `macros_db_audit`, `macros_db_cache`, `macros_soft_delete_unique`, `macros_db_entity_v2`, `macros_advanced_query` | Full macro capabilities (CRUD/audit/cache/soft-delete/hooks/pagination) |
| Graph Databases | `graph_ladybug`*, `graph_neo4j` | Ladybug embedded graph DB / Neo4j server (*`graph_ladybug` is not registered as a bin due to an mbedtls link conflict with `duckdb`; build separately: `cargo build --bin graph_ladybug --no-default-features --features "runtime-tokio-rustls,sqlite,cache,ladybug"`) |
| Distributed Capabilities | `distributed_id`, `saga`, `scatter_gather`, `replica_routing`, `shard_migration` | Snowflake ID / Saga / cross-shard aggregation / read-write splitting / shard migration |
| Reliability | `retry`, `failover` | Retry with backoff / circuit-breaker failover |
| Internationalization | `i18n_formatting` | ICU4X locale-aware formatting |
| Integrations & Cache | `oxcache_adapter`, `cache_standalone` | oxcache adapter / custom cache provider |
| Kit | `kit_usage`, `kit_advanced` | Capability registration / multi-capability composition |
| Common | `error_handling` | Structured error reporting |

See [examples/README.md](examples/README.md) for full details.

> **Note**: `dbnexus-examples` is set to `publish = false` and managed within the workspace.

### 📝 Code Snippets

#### Advanced Configuration

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

#### Environment Variables

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

#### Transactions

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

#### Monitoring

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

---

## 🏗️ Architecture

```mermaid
graph TD
    A[Application Layer<br/>Your code using DbPool and Session] --> B[DBNexus API Layer<br/>DbPool, Session<br/>Permission checking<br/>Transaction management]
    B --> C[Feature Modules<br/>Config, Permission, Metrics<br/>Migration, Sharding, Audit]
    C --> D[Connection Pool<br/>Connection lifecycle management<br/>Health checking<br/>RAII guarantees]
    D --> E[Sea-ORM / SQLx<br/>Database drivers<br/>Query builder]
```

See the [Architecture document](docs/ARCHITECTURE.md) for design philosophy, module breakdown, data flow, and security/performance design.

---

## 🧪 Testing

```bash
# Full test suite (CI standard feature combination; sqlite/postgres/mysql/duckdb drivers are mutually exclusive, do NOT use --all-features)
cargo test --no-default-features --features sqlite,default-no-db,all-optional --workspace --exclude dbnexus-examples --exclude dbnexus-macros

# Run integration tests with another database backend (PostgreSQL/MySQL require Docker)
cargo test --no-default-features --features postgres,default-no-db,all-optional --workspace --exclude dbnexus-examples --exclude dbnexus-macros
```

> Embedded (`sqlite`/`duckdb`) and server-side (`postgres`/`mysql`) drivers are strictly mutually exclusive at compile time (`compile_error!`); verify multiple drivers via grouped feature combinations.

---

## 📊 Performance

DBNexus follows the zero-cost abstraction principle; its performance characteristics are guaranteed at the design level:

- **Zero-cost feature gating**: optional capabilities such as `metrics` are compiled out via `#[cfg(feature = ...)]` — a no-op with zero overhead when disabled
- **Lock-free counters**: pool state (`PoolStatus`) is maintained with atomic types, lock-free on hot paths
- **Async first**: all I/O uses `async/await`; read-heavy state uses `RwLock`; atomics use `AcqRel` ordering to reduce unnecessary global synchronization
- **Connection pool strategy**: pool + LRU connection reuse (avoids handshake overhead), max-connection limits, minimum-connection warmup, health checks to evict dead connections
- **Hot path optimizations** (0.5.1): static SQL injection detection pattern table, pre-indexed Saga compensation lookup, `Session` concurrent-read optimization, `DbConfig` Arc sharing, etc.

### Benchmarks

The repository ships 4 criterion benchmarks: `permission_bench`, `permission_engine_bench`, `sharding_bench`, `metrics_bench` (under [benches/](benches/)):

```bash
cargo bench
```

The table below shows cumulative results of two optimization rounds on Linux x86_64 (release profile, lto=thin) as of 2026-08-14 (full report in [benches/baseline-after.md](benches/baseline-after.md)):

| Benchmark | Original Baseline | After Optimization | Cumulative Change |
|-----------|-------------------|--------------------|-------------------|
| shard_id_for_key | 2.9725 – 3.0496 µs | 2.8871 – 2.9188 µs | -3.9% |
| enforce_shard_binding_conflict | 5.8179 – 5.9490 µs | 5.4019 – 5.5934 µs | -5.5% |
| prometheus_export | 4.2057 – 4.3566 µs | 4.0485 – 4.1789 µs | -3.1% |
| histogram_record | 882.21 – 888.70 ns | 796.62 – 804.15 ns | -9.4% |
| permission_cache_hit | 8.8730 – 8.9906 µs | 8.1415 – 8.1825 µs | -8.3% |
| permission_cache_miss | 3.6778 – 3.7570 µs | 3.5827 – 3.6179 µs | -2.8% |

All 6 benchmarks improved with no regressions; the average cumulative improvement is about 5.5%.

---

## 🔒 Security

DBNexus is built with security in mind:

- **No unsafe code** — `#![forbid(unsafe_code)]` in all library code
- **Permission enforcement** — role-based table-level access control (RBAC), covering JOIN/subquery cross-table paths
- **SQL injection prevention** — parameterized queries by default, `SqlParser` table extraction and injection detection, `DdlGuard` AST validation
- **Config path validation** — protection against path traversal attacks; connection URL parse errors never echo credentials
- **Rate limiting** — token-bucket rate limiting on permission checks to prevent abuse

For the full defense-in-depth design, vulnerability reporting process, and security best practices, see the [Security document](docs/SECURITY.md).

---

## 🗺️ Roadmap

### Short Term (0.6.0 stable release)

- [ ] Release 0.6.0 stable: after rc validation, update the trait-kit 0.5.0 / oxcache 0.5.0 dependency requirements per the workspace propagation table, verify with `cargo publish --dry-run`, then tag to trigger the automated release.yml publish
- [x] Align the declared MSRV with actual dependency requirements — unified to 1.97.1 per workspace CONFIG_BASELINE (2026-09-06), covering the 1.94 transitive requirement
- [ ] Restore regular MySQL integration test runs (testcontainers ready, currently blocked by database service availability)

### Mid Term

- [ ] Resolve the mbedtls duplicate-symbol link conflict when `duckdb` and `ladybug` are enabled together (multi-driver verification currently uses grouped feature combinations)
- [ ] Follow up on Medium items filed during code quality reviews

> Items compiled from the workspace acceptance plan and the [CHANGELOG.md](docs/CHANGELOG.md).

---

## 🤝 Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](docs/CONTRIBUTING.md) first for the TDD workflow, code conventions, and commit/PR process.

### Development Setup

```bash
# Clone repository
git clone https://github.com/Kirky-X/dbnexus.git
cd dbnexus

# Install pre-commit hooks
./scripts/install-pre-commit.sh

# Run tests (CI feature combination)
cargo test --no-default-features --features sqlite,default-no-db,all-optional

# Run linter
cargo clippy --no-default-features --features sqlite,default-no-db,all-optional --all-targets -- -D warnings
```

---

## 📋 Changelog

See [CHANGELOG.md](docs/CHANGELOG.md) for the full version history.

| Version | Date | Highlights |
|---------|------|------------|
| 0.6.0-rc.2 | 2026-09-03 | Removed the `tracing` feature; strict four-driver mutual exclusion (compile-time `compile_error!`); completed JOIN/subquery cross-table permission checks; `h2` security upgrade |
| 0.5.1 | 2026-08-06 | Hot path performance optimizations (static injection detection table, `Session` RwLock, `DbConfig` Arc sharing, etc.); removed deprecated builder methods |
| 0.5.0 | 2026-08-04 | Added 7 distributed capability examples (`saga`/`scatter_gather`/`replica_routing` etc.); API and architecture doc sync |

---

## 📄 License

This project is licensed under the [MIT License](LICENSE).

---

## 🙏 Acknowledgments

- [Sea-ORM](https://www.sea-ql.org/SeaORM/) - The excellent ORM framework DBNexus is built on
- [SQLx](https://github.com/launchbadge/sqlx) - Async SQL toolkit
- The Rust community for amazing tools and libraries

---

## 📞 Contact & Support

| Channel | Purpose |
|---------|---------|
| [📋 Issues](https://github.com/Kirky-X/dbnexus/issues) | Report bugs and issues |
| [💬 Discussions](https://github.com/Kirky-X/dbnexus/discussions) | Ask questions and share ideas |
| [🐙 GitHub](https://github.com/Kirky-X/dbnexus) | View source code |

Please do not report security vulnerabilities through public issues; see the [vulnerability reporting process](docs/SECURITY.md) in the Security document.

---

## ⭐ Star History

[![Star History Chart](https://api.star-history.com/svg?repos=Kirky-X/dbnexus&type=Date)](https://star-history.com/#Kirky-X/dbnexus&Date)

### 💝 Support This Project

If you find this project useful, please consider giving it a ⭐️!

---

<div align="center">

**Built with ❤️ by Kirky.X**

[⬆ Back to Top](#readme)

<sub>© 2026 Kirky.X. All rights reserved.</sub>

</div>
