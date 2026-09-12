<div align="center">

<img src="docs/assets/dbnexus.png" alt="DBNexus Logo" width="180">

[![CI Status](https://github.com/Kirky-X/dbnexus/actions/workflows/ci.yml/badge.svg)](https://github.com/Kirky-X/dbnexus/actions/workflows/ci.yml) [![Version](https://img.shields.io/crates/v/dbnexus.svg)](https://crates.io/crates/dbnexus) [![Docs.rs](https://docs.rs/dbnexus/badge.svg)](https://docs.rs/dbnexus) [![Downloads](https://img.shields.io/crates/d/dbnexus.svg)](https://crates.io/crates/dbnexus) [![License](https://img.shields.io/crates/l/dbnexus.svg)](LICENSE) [![Rust](https://img.shields.io/badge/rust-1.97.1%2B-orange.svg)](https://www.rust-lang.org/) [![Coverage](https://codecov.io/gh/Kirky-X/dbnexus/branch/main/graph/badge.svg)](https://codecov.io/gh/Kirky-X/dbnexus)

**[中文](README.md)** | English

**Enterprise-grade Database Abstraction Layer for Rust**

[✨ Features](#-features) • [🚀 Quick Start](#-quick-start) • [📚 Documentation](#-documentation) • [💻 Examples](#-examples) • [🤝 Contributing](#-contributing)

</div>

---

## 📋 Table of Contents

<details open>
<summary>📑 Table of Contents</summary>

- [✨ Features](#-features)
- [🚀 Quick Start](#-quick-start)
  - [📦 Installation](#-installation)
  - [💡 Minimal Example](#-minimal-example)
  - [🧭 Core Concepts](#-core-concepts)
- [🎨 Feature Flags](#-feature-flags)
- [📚 Documentation](#-documentation)
- [💻 Examples](#-examples)
- [🏗️ Architecture](#️-architecture)
- [🔗 Core Execution Path](#-core-execution-path)
- [🌐 Database Support](#-database-support)
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

DBNexus is built on Sea-ORM and provides a **declarative** database access approach: one macro defines the entity, one permission layer guards every SQL statement, and one set of features tailors the build.

<div align="center">

<table>
<tr>
<td align="center" width="25%">🔒<br><b>Secure by Design</b><br>Unsafe forbidden crate-wide; table-level RBAC covers JOINs and subqueries</td>
<td align="center" width="25%">🧩<br><b>Declarative Macros</b><br><code>#[db_entity]</code> generates permission-checked CRUD methods</td>
<td align="center" width="25%">🗄️<br><b>Multi-Database</b><br>SQLite / PostgreSQL / MySQL / DuckDB / Ladybug / Neo4j</td>
<td align="center" width="25%">📊<br><b>Observable & Reliable</b><br>Prometheus metrics, health checks, retry and circuit breaker</td>
</tr>
</table>

</div>

### 🎯 Core Foundation (no optional features required)

| Capability | Description |
|------|------|
| **Connection Pooling** | RAII-style connection lifecycle; pool state maintained with atomics, lock-free on hot paths |
| **Transactions** | Complete transaction management with `begin_transaction` / `commit` / rollback and RAII guarantees |
| **Unified Errors** | `ErrorCode` table + `QueryErrorReport` structured error reporting (unified in 0.6.0-rc.3) |
| **Configuration** | `DbConfig` / `PoolConfig` with environment variable / YAML / TOML sources |
| **Internationalization** | ICU4X + Fluent locale-aware formatting (core feature, always compiled) |

### ⚙️ Core Optional Features (`default-no-db` aggregate)

| Capability | Description |
|------|------|
| **Permission Control** (`permission`) | Role-based table-level access control (RBAC); hard-depends on `sql-parser` to prevent injection bypass |
| **SQL Parsing** (`sql-parser`) | Operation type and table extraction, injection detection, cached parse results |
| **Procedural Macros** (`macros`) | `#[db_entity]` / `#[db_repository]` generate permission-checked CRUD |
| **Environment Config** (`config-env`) | `DbConfig::from_env` reads configuration directly from environment variables |

### ⚡ Enterprise Features (opt-in)

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
| `data-protection` 🆕 | Field-level masking (mask/hash/truncate) and row-level security predicate injection |
| `permission-facade` 🆕 | RBAC + masking + RLS unified facade, configure once and it applies everywhere |
| `query-dsl` 🆕 | `q!` type-safe query fragment macro, immune to identifier and value injection |
| `repository` / `data-api` 🆕 | Generic `Repository<T>`; entity-to-JSON data API gateway |
| `prepare-cache` 🆕 | Statement-level prepared statement LRU cache with hit-rate metrics |
| `copy` 🆕 | COPY FROM STDIN statement building (pg protocol path gated per driver) |
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

> 🆕 marks capabilities added in 0.6.0-rc.3; see the [Changelog](docs/CHANGELOG.md) for the full list.

---

## 🚀 Quick Start

### 📦 Installation

Requirements: Rust **1.97.1+** (pinned by `rust-toolchain.toml`, edition 2024), plus at least one runtime and one database driver feature.

```bash
cargo add dbnexus --features runtime-tokio-rustls,sqlite,permission,macros
cargo add tokio --features rt-multi-thread,macros
```

```toml
[dependencies]
dbnexus = { version = "0.6.0-rc.3", features = ["runtime-tokio-rustls", "sqlite", "permission", "macros"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

> `permission` force-enables `sql-parser` and `cache` (verified at compile time to prevent SQL injection bypassing permission checks). `default = []`: every feature must be enabled explicitly.

### 💡 Minimal Example

Define an entity and get macro-generated CRUD (adapted from [examples/src/basic/basic_crud.rs](examples/src/basic/basic_crud.rs); `ActiveModelBehavior` is implemented by the macro):

```rust
use dbnexus::{DbPool, db_entity};
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

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Pool + admin session (RAII: the connection returns to the pool on drop)
    let pool = DbPool::new("sqlite::memory:").await?;
    let session = pool.get_session("admin").await?;

    // Macro-generated CRUD: every statement goes through parsing and table-level permission checks
    let user = Model { id: 1, name: "Alice".to_string(), email: "alice@example.com".to_string() };
    Model::insert(&session, user).await?;

    let users = Model::find_all(&session).await?;
    println!("Found {} users", users.len());
    Ok(())
}
```

<details>
<summary>🎬 Permission Control: unauthorized roles are denied</summary>

```rust
// The admin role is allowed by default (safe default without a permission config file)
let session = pool.get_session("admin").await?;
Model::find_all(&session).await?;

// A guest role not defined in the policy is denied; tables not granted are denied too
let session = pool.get_session("guest").await?;
Model::find_all(&session).await?; // Error: permission denied
```

</details>

### 🧭 Core Concepts

| Concept | One-liner |
|------|-----------|
| `DbPool` | Pool entry point, built from a URL or `DbConfig`, owns the connection lifecycle |
| `Session` | Role-scoped session handle, RAII connection return, hosts transactions and execution channels |
| `#[db_entity]` | One macro generates the Sea-ORM entity model + 8 permission-checked CRUD methods |
| Permission policy | Role → table → action RBAC policy (memory or YAML); JOIN/subquery tables are checked too |
| Feature gating | Driver mutual exclusion enforced at compile time via `compile_error!`; disabled capabilities cost nothing |

---

## 🎨 Feature Flags

`default = []`: no default features; runtime, database driver and functional features must all be enabled explicitly. Embedded (`sqlite`/`duckdb`) and server-side (`postgres`/`mysql`) drivers are strictly mutually exclusive; mixing them fails the build.

### Runtimes (mutually exclusive, pick one)

| Flag | Description | Default |
|------|------|:----:|
| `runtime-tokio-rustls` | Tokio runtime + rustls TLS | No |
| `runtime-tokio-native-tls` | Tokio runtime + native-tls | No |
| `runtime-async-std` | async-std runtime | No |

### Database Drivers

Pick exactly one relational driver (compile-time mutual exclusion); graph drivers can coexist with relational drivers.

| Flag | Description | Default |
|------|------|:----:|
| `sqlite` | Embedded SQLite (sea-orm/sqlx-sqlite) | No |
| `postgres` | PostgreSQL (sea-orm/sqlx-postgres) | No |
| `mysql` | MySQL (sea-orm/sqlx-mysql) | No |
| `duckdb` | Embedded analytical DuckDB (new in 0.3.0) | No |
| `ladybug` | Embedded Ladybug graph database, formerly Kuzu (new in 0.4.0) | No |
| `neo4j` | Neo4j graph database server (new in 0.4.0) | No |

### Core Capabilities

| Flag | Description | Default |
|------|------|:----:|
| `permission` | Table-level RBAC; hard-depends on `sql-parser` and enables `yaml` + `cache` | No |
| `sql-parser` | SQL parsing, table extraction and injection detection; auto-enables `cache` | No |
| `macros` | `dbnexus-macros` procedural macros (`db_entity` / `db_repository`) | No |
| `default-no-db` | Driver-less default aggregate (runtime + permission + sql-parser + macros + config-env + with-time) for CI driver-matrix testing | No |

### Data Access & Integrations

| Flag | Description | Default |
|------|------|:----:|
| `cache` | oxcache cache (moka L1 backend) + lock-free `ArcSwap` reads | No |
| `oxcache-integration` | OxcacheDbCacheAdapter | No |
| `kit` | trait-kit AsyncKit integration with the full pool/cache/audit/health capability closure | No |
| `repository` | Generic `Repository<T>` CRUD port + `impl_json_repository!` macro | No |
| `data-api` | Data API gateway: entity-to-JSON query endpoints (allowlist + filtering + pagination) | No |
| `prepare-cache` | Statement-level prepared statement LRU cache | No |
| `query-dsl` | `q!` type-safe query fragment macro | No |
| `entity-events` | Entity event bus + Outbox | No |
| `copy` | COPY FROM STDIN batch-write statement building | No |
| `data-protection` | Field masking and row-level security predicate injection | No |
| `permission-facade` | RBAC + masking + RLS unified facade | No |
| `config-confers` | confers hot config reload | No |

### Observability

| Flag | Description | Default |
|------|------|:----:|
| `metrics` | Prometheus-format metrics export (with slow-query detection) | No |
| `health-check` | Health check module and `health_snapshot` structured export | No |
| `observability` | `metrics` + `health-check` aggregate | No |
| `otel` | OTLP/HTTP JSON envelope export bridge | No |

### Data Management

| Flag | Description | Default |
|------|------|:----:|
| `migration` | Database migrations | No |
| `auto-migrate` | Automatic migration execution | No |
| `sharding` | Data sharding (strategies and session-level routing) | No |
| `global-index` | Cross-shard global index | No |
| `data-management` | Aggregate of the four above | No |

### Distributed Capabilities

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
| `json` | Direct serde_json deserialization support | No |
| `yaml` | YAML permission/config file parsing | No |
| `config-toml` | TOML config support (no extra dependency) | No |
| `config-env` | Environment variable config (no extra dependency) | No |
| `pool-health-check` | Connection pool health checks | No |
| `pool-warmup` | Connection pool warmup | No |
| `dev` / `dev-full` | Developer convenience aggregates | No |
| `bench` | criterion benchmark dependencies | No |
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
| `all-optional` | 15 optional features except database drivers (cache / observability / data-management / security / migration / retry / failover / replica-routing / scatter-gather / shard-migration / saga / distributed-id / repository / data-api / prepare-cache) | Full-feature verification (add drivers manually) |

### Usage Examples

```toml
# Embedded/edge devices (minimal)
dbnexus = { version = "0.6.0-rc.3", features = ["embedded"] }

# Microservices
dbnexus = { version = "0.6.0-rc.3", features = ["microservice"] }

# Monolithic applications
dbnexus = { version = "0.6.0-rc.3", features = ["monolith"] }

# Enterprise (full features)
dbnexus = { version = "0.6.0-rc.3", features = ["enterprise"] }
```

---

## 📚 Documentation

| Document | Description |
|------|------|
| [📖 User Guide](docs/USER_GUIDE.md) | Complete tutorial from installation to advanced usage |
| [📘 API Reference](docs/API_REFERENCE.md) | Detailed description of all public APIs |
| [🏗️ Architecture](docs/ARCHITECTURE.md) | Design philosophy, module breakdown, data flow, security/performance design |
| [📊 Performance Baseline](docs/PERFORMANCE.md) | End-to-end benchmark data and reproduction commands |
| [🔒 Security](docs/SECURITY.md) | Defense-in-depth design, best practices and vulnerability reporting |
| [📋 Changelog](docs/CHANGELOG.md) | Change records for every version |
| [🤝 Contributing](docs/CONTRIBUTING.md) | TDD workflow, code conventions, commit/PR process |
| [🧪 Test Scenarios](docs/TEST_SCENARIOS.md) | Test pyramid baseline, driver-group matrix and E2E scenarios |
| [📦 Online API Docs](https://docs.rs/dbnexus) | Latest documentation auto-generated by docs.rs |
| [📦 crates.io](https://crates.io/crates/dbnexus) | Release page |

---

## 💻 Examples

All examples live in [examples/](examples/) (a separate crate `dbnexus-examples`, managed in the workspace with `publish = false`), **51 binary targets** in total (as of 0.6.0-rc.3):

```bash
cd examples

# Run a single example (each example is a bin target)
cargo run --bin basic_crud --features "sqlite,permission,macros"

# Build all examples
cargo build --all-targets
```

| Module | Examples | Description |
|------|------|------|
| Basics | `basic_connection`, `basic_crud`, `basic_transaction` | Connection pool / `#[db_entity]` CRUD / transactions |
| Config | `config_env`, `config_yaml`, `config_toml`, `config_presets` | Env vars / YAML / TOML / preset comparison |
| Database | `database_sqlite`, `database_postgres`, `database_mysql`, `duckdb_query`, `migration`, `sharding`, `global_index`, `pool_management` | Driver connections / OLAP queries / migration / sharding / global index / pool management |
| Permission | `permission_rbac`, `permission_yaml`, `permission_macro`, `permission_engine` | RBAC / YAML policies / macro permissions / permission engine |
| Security | `sql_parser`, `sql_injection_detection`, `ddl_guard`, `sensitive_masker`, `rate_limiter` | SQL parsing / injection detection / DDL guard / masking / rate limiting |
| Auth & Audit | `authentication_jwt`, `authentication_password`, `audit_logging` | JWT / password hashing / audit logging |
| Observability | `metrics_prometheus`, `health_check`, `latency_histogram` | Metrics / health check & circuit breaker / latency histogram |
| Macros | `macros_db_entity`, `macros_db_crud`, `macros_db_audit`, `macros_db_cache`, `macros_soft_delete_unique`, `macros_db_entity_v2`, `macros_advanced_query` | Full macro capabilities (CRUD/audit/cache/soft-delete/hooks/pagination) |
| Graph Databases | `graph_ladybug`*, `graph_neo4j` | Ladybug embedded graph DB / Neo4j server |
| Distributed Capabilities | `distributed_id`, `saga`, `scatter_gather`, `replica_routing`, `shard_migration` | Snowflake ID / Saga / cross-shard aggregation / read-write splitting / shard migration |
| Reliability | `retry`, `failover` | Retry with backoff / circuit-breaker failover |
| Internationalization | `i18n_formatting` | ICU4X locale-aware formatting |
| Integrations & Cache | `oxcache_adapter`, `cache_standalone` | oxcache adapter / custom cache provider |
| Kit | `kit_usage`, `kit_advanced` | Capability registration / multi-capability composition |
| Common | `error_handling` | Structured error reporting |

\* `graph_ladybug` is not registered as a bin due to an mbedtls link conflict with `duckdb`; build it separately: `cargo build --bin graph_ladybug --no-default-features --features "runtime-tokio-rustls,sqlite,cache,ladybug"`.

See [examples/README.md](examples/README.md) for full details.

### 📝 Code Snippets

<details>
<summary>⚙️ Advanced configuration and environment variables</summary>

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

</details>

<details>
<summary>🔄 Transactions and monitoring</summary>

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

</details>

---

## 🏗️ Architecture

DBNexus follows a layered module design: `foundation` provides the config and error base; the `database` module hosts the connection pool, Session, migrations, sharding, Saga and scatter-gather; the `access` module concentrates SQL parsing, the permission engine, authentication and masking; the `domain` module holds domain abstractions for permission/audit/migration; `observability` and `reliability` provide metrics/health and retry/failover respectively. All optional capabilities are trimmed at compile time via feature gates, and the `dbnexus-macros` proc-macro crate generates permission-checked CRUD code for entities at compile time.

```mermaid
flowchart TD
    APP["Application code"]
    MAC["dbnexus-macros proc macros<br/>db_entity and db_repository"]
    API["database module<br/>DbPool / Session / transactions / migration / sharding / Saga"]
    ACC["access module<br/>sql_parser / permission / auth / masking"]
    DOM["domain module<br/>permission / audit / migration abstractions"]
    OBS["observability module<br/>metrics / health / otel"]
    REL["reliability module<br/>retry"]
    STO["storage module<br/>global_index"]
    INT["integrations module<br/>oxcache / trait-kit"]
    I18N["i18n module<br/>ICU4X locale formatting"]
    FND["foundation module<br/>config / error"]
    DRV["Database driver layer<br/>Sea-ORM / SQLx / lbug / neo4rs"]
    DB[("SQLite / PostgreSQL / MySQL<br/>DuckDB / Ladybug / Neo4j")]

    MAC -.->|generates permission-checked CRUD at compile time| APP
    APP --> API
    API --> ACC
    ACC --> DOM
    API --> OBS
    API --> REL
    API --> STO
    API --> INT
    API --> I18N
    ACC --> FND
    API --> FND
    API --> DRV
    DRV --> DB
```

See the [Architecture document](docs/ARCHITECTURE.md) for design philosophy, module breakdown, data flow, and security/performance design.

---

## 🔗 Core Execution Path

The real execution pipeline of `Session::execute_raw` under the `sql-parser` + `permission` feature combination (source: [src/database/pool/session.rs](src/database/pool/session.rs)):

```mermaid
sequenceDiagram
    autonumber
    participant App as Application
    participant Pool as DbPool
    participant Sess as Session
    participant Parser as SqlParser shared
    participant Perm as Permission Context
    participant DB as Sea-ORM Driver

    App->>Pool: get_session with role
    Pool->>Pool: validate role and permission config
    Pool-->>App: return Session handle
    App->>Sess: execute_raw with SQL
    Sess->>Sess: reject DDL statements
    Sess->>Parser: parse_single
    Parser-->>Sess: operation type and all table names
    Sess->>Perm: per-table permission check
    alt any table denied
        Sess-->>App: return permission denied error
    else all allowed
        Sess->>DB: execute_unprepared
        DB-->>Sess: execution result
        Sess-->>App: return ExecResult
    end
```

Path notes:

- **Safe default on parse failure**: the admin role is allowed, non-admin roles are denied; no silent fallback
- **Full cross-table coverage**: every target table in JOINs / subqueries goes through the permission check (completed in rc.2)
- **Automatic idempotent retry**: with the `retry` feature, idempotent statements such as SELECT are retried with exponential backoff; write statements are never retried
- **Slow-query observability**: with the `metrics` feature, execution time is recorded and anything above the `SlowQueryConfig` threshold lands in `MetricsCollector` (wired in rc.3)
- **Auditable admin**: the admin role bypasses per-table permission checks, but bypass operations are recorded by the `audit` feature

---

## 🌐 Database Support

| Driver feature | Database | Type | Introduced |
|----------|--------|------|----------|
| `sqlite` | SQLite | Embedded relational | Initial |
| `postgres` | PostgreSQL | Server relational | Initial |
| `mysql` | MySQL | Server relational | Initial |
| `duckdb` | DuckDB | Embedded analytical | 0.3.0 |
| `ladybug` | Ladybug (formerly Kuzu) | Embedded graph | 0.4.0 |
| `neo4j` | Neo4j | Graph server | 0.4.0 |

Protocol-compatible databases (no extra feature needed, just use the corresponding protocol driver):

| Database | Compatible Protocol | Description |
|--------|----------|------|
| CockroachDB | PostgreSQL | Distributed SQL database |
| YugabyteDB | PostgreSQL | Distributed PostgreSQL |
| TiDB | MySQL | Distributed HTAP database |
| MariaDB | MySQL | MySQL-compatible fork |
| Aurora | PostgreSQL/MySQL | AWS cloud-native database |

> Known limitation: enabling `duckdb` and `ladybug` together hits an mbedtls duplicate-symbol link conflict; verify multiple drivers with grouped feature combinations (see the [Roadmap](#️-roadmap)).

---

## 🧪 Testing

### Test strategy matrix

| Layer | Location | Description |
|------|------|------|
| Unit tests | `#[cfg(test)]` in `src/**` | Self-tests for error/config/domain modules, run with `--lib` |
| Integration tests | `tests/**` (unit / integration layout) | Explicitly registered `[[test]]` targets, feature-gated |
| E2E scenarios | `tests/e2e/` | Boundary/exception/distributed end-to-end, isolated via `cfg(feature)` |
| Container-level | `postgres_testcontainers` / `mysql_testcontainers` | testcontainers with per-test container isolation |
| Doc tests | doc tests | Run separately in CI via `cargo test --doc` |
| Benchmarks | [benches/](benches/) | 5 criterion benchmarks (see [Performance](#-performance)) |

### Test scale (as of 0.6.0-rc.3)

| Metric | Value | Source |
|------|------|------|
| Total test functions | 2389 `#[test]` / `#[tokio::test]` | grep count (src 1066 + tests 1321 + macros 2) |
| Registered test targets | 79 `[[test]]` | `Cargo.toml` |
| Driver-group full runs | sqlite 1712 / postgres 1276 / mysql 1276 / duckdb 1300 passed | [docs/TEST_SCENARIOS.md](docs/TEST_SCENARIOS.md) |
| Coverage gate | ≥ 80% line coverage | `.github/workflows/ci.yml` (llvm-cov) |

### Commands (identical to CI)

```bash
# Full test suite (CI standard feature combination; sqlite/postgres/mysql/duckdb drivers are mutually exclusive, do NOT use --all-features)
cargo test --no-default-features --features sqlite,default-no-db,all-optional --workspace --exclude dbnexus-examples --exclude dbnexus-macros

# Run integration tests against another database backend (CI provides PostgreSQL 15 / MySQL 8.0 via service containers)
cargo test --no-default-features --features postgres,default-no-db,all-optional --workspace --exclude dbnexus-examples --exclude dbnexus-macros

# Doc tests
cargo test --no-default-features --features sqlite,default-no-db,all-optional --doc --workspace --exclude dbnexus-examples --exclude dbnexus-macros
```

> Embedded (`sqlite`/`duckdb`) and server-side (`postgres`/`mysql`) drivers are strictly mutually exclusive at compile time (`compile_error!`); verify multiple drivers via grouped feature combinations. Scenario definitions and the driver-group matrix are documented in [docs/TEST_SCENARIOS.md](docs/TEST_SCENARIOS.md).

---

## 📊 Performance

DBNexus follows the zero-cost abstraction principle; its performance characteristics are guaranteed at the design level:

- **Zero-cost feature gating**: optional capabilities are compiled out via `#[cfg(feature = ...)]` — a no-op with zero overhead when disabled
- **Lock-free hot paths**: pool state uses atomics; permission config is read via lock-free `ArcSwap`; atomics use `AcqRel` ordering
- **Async first**: all I/O uses `async/await`; read-heavy state uses `RwLock`
- **Pool strategy**: LRU connection reuse, max-connection limits, minimum-connection warmup, health checks to evict dead connections
- **Hot path optimizations** (0.5.1): static SQL injection detection pattern table, pre-indexed Saga compensation lookup, `Session` concurrent-read optimization, `DbConfig` Arc sharing

### End-to-end benchmarks

The repository ships 5 criterion benchmarks: `permission_bench`, `permission_engine_bench`, `sharding_bench`, `metrics_bench`, `e2e_bench` (under [benches/](benches/)). Run:

```bash
cargo bench
```

The end-to-end baseline below is excerpted from [docs/PERFORMANCE.md](docs/PERFORMANCE.md) (2026-09-11, WSL2 linux x86_64, 12 logical cores, rustc 1.97.1, bench profile, sqlite temp-file database; single-sample local reference, not an SLA):

| Benchmark | Path | Baseline (median) |
|------|------|----------------|
| `e2e_pool/get_session_admin` | `DbPool::get_session` handle acquisition (permission check + pool accounting) | ≈ 0.24 µs |
| `e2e_query/query_rows_select_single` | `DbPool::query_rows` full row-query pipeline (parse → permission → execute → JSON output) | ≈ 480 µs |
| `e2e_write/execute_raw_insert_x64` | `Session::execute_raw` INSERT loop, 64 rows/iteration | ≈ 708 ms/iteration (≈ 11 ms/row) |

### Historical optimization comparison

Cumulative results of two platform optimization rounds, excerpted from [benches/baseline-after.md](benches/baseline-after.md) (2026-08-14, Linux x86_64, release profile, lto=thin):

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

DBNexus is built with security in mind; its defense-in-depth design spans five layers (full design in the [Security document](docs/SECURITY.md)):

| Layer | Mechanisms |
|------|------|
| Compile-time | Crate-wide `#![forbid(unsafe_code)]`; mixing embedded and server-side drivers fails via `compile_error!`; missing feature dependencies fail the build, no silent degradation |
| Runtime permissions | Table-level RBAC covering JOIN/subquery cross-table paths; TTL permission cache + singleflight against cache stampede; token-bucket rate limiting |
| Injection defense | Parameterized queries by default; `SqlParser` table extraction and the unified `InjectionEngine` (Unicode normalization aware); `DdlGuard` AST validation with DDL restricted to admin; parameterized channel for graph queries |
| Auth & config | JWT access/refresh token distinction and TTL-based revocation cache; bcrypt password policy; path traversal validation; URL parse errors never echo credentials |
| Audit & masking | `audit` logs with full operation and user context (admin bypasses recorded); `SensitiveMasker` multi-type masking and row-level security |

Supply-chain security: CI runs `cargo deny check` (licenses/advisories/duplicates, exemptions documented in [deny.toml](deny.toml)) and `cargo audit` ([audit.toml](audit.toml)), plus CodeQL semantic scanning, Dependabot automatic updates, and pre-commit private-key scanning.

**Vulnerability reporting**: please do not report through public issues. Use the private GitHub [Security Advisories](https://github.com/Kirky-X/dbnexus/security/advisories/new) channel ("Report a vulnerability"). Response commitment: acknowledgment within 48 hours, initial assessment within 7 days (see [SECURITY.md](SECURITY.md)).

---

## 🗺️ Roadmap

### Short Term (0.6.0 stable release)

- [ ] Release 0.6.0 stable: after rc validation, update the trait-kit 0.5.0 / oxcache 0.5.0 dependency requirements per the workspace propagation table, verify with `cargo publish --dry-run`, then tag to trigger the automated release.yml publish
- [x] Align the declared MSRV with actual dependency requirements — unified to 1.97.1 per workspace CONFIG_BASELINE (2026-09-06), covering the 1.94 transitive requirement
- [ ] Restore regular MySQL integration test runs (testcontainers ready, currently blocked by database service availability)

### Mid Term

- [ ] Resolve the mbedtls duplicate-symbol link conflict when `duckdb` and `ladybug` are enabled together (multi-driver verification currently uses grouped feature combinations)
- [ ] Follow up on Medium items filed during code quality reviews

> Items compiled from the [CHANGELOG.md](docs/CHANGELOG.md) and repository acceptance records.

---

## 🤝 Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](docs/CONTRIBUTING.md) first for the TDD workflow, code conventions, and commit/PR process.

### Development environment

| Item | Requirement |
|----|------|
| Toolchain | Rust 1.97.1 (pinned by `rust-toolchain.toml`, edition 2024) |
| Git hooks | lefthook / pre-commit (install via `./scripts/install-pre-commit.sh`; bypassing with `--no-verify` is forbidden) |
| Commit messages | Conventional Commits (`feat` / `fix` / `docs` / `refactor` etc., enforced by the commit-msg hook) |
| Quality gates | `cargo fmt --check`, `cargo clippy -D warnings`, `cargo deny check`, `cargo audit`, ≥ 80% line coverage |

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
| 0.6.0-rc.3 | 2026-09-10 | Unified `query_rows` row-query API; Saga persistent recovery; field-level masking and row-level security (`data-protection`); permission facade and query DSL; COPY batch writes and the OTel export bridge; `e2e_bench` end-to-end benchmark; ops CLI `migrate`/`health`/`user` subcommands |
| 0.6.0-rc.2 | 2026-09-03 | Removed the `tracing` feature; strict four-driver mutual exclusion (compile-time `compile_error!`); completed JOIN/subquery cross-table permission checks; `h2` security upgrade |
| 0.5.1 | 2026-08-06 | Hot path performance optimizations (static injection detection table, `Session` RwLock, `DbConfig` Arc sharing, etc.); removed deprecated builder methods |

---

## 📄 License

This project is licensed under **MIT + Commons Clause**: the MIT license with the additional Commons Clause condition — the right to sell the Software or use it commercially is not granted without separate written authorization. See [LICENSE](LICENSE).

---

## 🙏 Acknowledgments

- [Sea-ORM](https://www.sea-ql.org/SeaORM/) - The excellent ORM framework DBNexus is built on
- [SQLx](https://github.com/launchbadge/sqlx) - Async SQL toolkit
- [sqlparser](https://github.com/apache/datafusion-sqlparser-rs) - SQL dialect parsing powering permission checks and injection defense
- [ICU4X](https://github.com/unicode-org/icu4x) - Unicode internationalization components powering locale-aware formatting
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
