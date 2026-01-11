# DBNexus

<div align="center">

**An enterprise-grade database abstraction layer for Rust with built-in permission control and connection pooling**

[![Rust Version](https://img.shields.io/badge/rust-1.85+-blue.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-yellow.svg)](https://opensource.org/licenses/MIT)
[![Version](https://img.shields.io/badge/version-0.1.0-green.svg)](https://github.com/Kirky-X/dbnexus)

</div>

## Overview

DBNexus is a Rust-based database abstraction library built on top of Sea-ORM. It provides enterprise-grade features including connection pooling, role-based access control, audit logging, caching, and database sharding.

## Features

- **Connection Pooling**: Efficient database connection management with configurable min/max connections
- **Permission Engine**: Role-based table-level access control with YAML configuration
- **Audit Logging**: Comprehensive audit trail with data sanitization
- **Caching**: LRU cache for permission policies and frequently accessed data
- **Sharding**: Horizontal scaling support for large datasets
- **Metrics**: Prometheus-compatible metrics collection
- **Migrations**: Built-in migration management with automatic support
- **Tracing**: Distributed tracing with OpenTelemetry support

## Supported Databases

- SQLite
- PostgreSQL
- MySQL

## Quick Start

### Installation

```toml
[dependencies]
dbnexus = "0.1"
# Choose one database driver
dbnexus = { version = "0.1", features = ["sqlite"] }
dbnexus = { version = "0.1", features = ["postgres"] }
dbnexus = { version = "0.1", features = ["mysql"] }
```

### Basic Usage

```rust
use dbnexus::{DbPool, DbConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = DbConfig {
        url: "sqlite://example.db".to_string(),
        max_connections: 10,
        min_connections: 2,
        idle_timeout: 300,
        acquire_timeout: 5000,
        permissions_path: None,
        migrations_dir: None,
        auto_migrate: false,
        migration_timeout: 60,
        admin_role: "admin".to_string(),
    };

    let pool = DbPool::with_config(config).await?;
    let session = pool.get_session("admin").await?;

    // Execute queries
    let result = session.execute_raw("SELECT 1").await?;
    println!("Query executed successfully");

    Ok(())
}
```

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DB_URL` | Database connection URL | - |
| `DB_MAX_CONNECTIONS` | Maximum pool size | 10 |
| `DB_MIN_CONNECTIONS` | Minimum pool size | 2 |
| `DB_IDLE_TIMEOUT` | Idle connection timeout (seconds) | 300 |
| `DB_ACQUIRE_TIMEOUT` | Connection acquire timeout (milliseconds) | 3000 |
| `DB_ADMIN_ROLE` | Admin role for DDL operations | "admin" |

### Permission Configuration

Create a `permissions.yaml` file:

```yaml
roles:
  admin:
    - table: "*"
      operations: ["SELECT", "INSERT", "UPDATE", "DELETE"]
  user:
    - table: "users"
      operations: ["SELECT", "INSERT"]
    - table: "orders"
      operations: ["SELECT"]
  reader:
    - table: "*"
      operations: ["SELECT"]
```

## Optional Features

| Feature | Description |
|---------|-------------|
| `metrics` | Prometheus metrics collection |
| `migration` | Database migration support |
| `auto-migrate` | Automatic migration on startup |
| `sharding` | Horizontal sharding support |
| `global-index` | Global index for distributed queries |
| `cache` | LRU cache for permission policies and frequently accessed data |
| `audit` | Comprehensive audit logging |
| `tracing` | OpenTelemetry distributed tracing |
| `permission` | Role-based access control |
| `sql-parser` | SQL parsing and analysis |
| `pool-health-check` | Connection pool health monitoring |
| `config-yaml` | YAML configuration file support |
| `config-toml` | TOML configuration file support |

### Preset Configurations

Two preset configurations are available for common use cases:

#### Minimal (Lite)

```toml
dbnexus = { version = "0.1", features = ["minimal", "sqlite"] }
```

| Feature | Included |
|---------|----------|
| `runtime-tokio-rustls` | ✅ Async runtime with TLS |
| `sqlite` | SQLite driver |
| `config-env` | Environment variable config |
| `sql-parser` | SQL parsing |
| `lru` | LRU cache for permissions |
| `async-trait` | Async trait support |
| `regex` | Regex support |

#### Microservice (Full-featured)

```toml
dbnexus = { version = "0.1", features = ["microservice", "postgres"] }
```

| Feature | Included |
|---------|----------|
| `runtime-tokio-rustls` | ✅ Async runtime with TLS |
| `postgres` | PostgreSQL driver |
| `permission` | Role-based access control |
| `sql-parser` | SQL parsing |
| `config-env` | Environment variable config |
| `pool-health-check` | Connection health monitoring |
| `config-yaml` | YAML configuration |
| `lru` | LRU cache |
| `async-trait` | Async trait support |
| `regex` | Regex support |

### Custom Configuration

Combine database driver with required and optional features:

```toml
# Minimal with cache
dbnexus = { version = "0.1", features = ["minimal", "sqlite", "cache"] }

# Microservice with audit logging
dbnexus = { version = "0.1", features = ["microservice", "postgres", "audit"] }

# Full featured
dbnexus = { version = "0.1", features = ["all-optional", "postgres"] }
```

Enable all optional features:

```toml
dbnexus = { version = "0.1", features = ["all-optional", "postgres"] }
```

## Project Structure

```
dbnexus/
├── dbnexus/           # Core library
├── dbnexus-cli/       # CLI tool
├── dbnexus-macros/    # Procedural macros
├── examples/          # Example code
├── scripts/           # Build and utility scripts
└── docs/              # Documentation
```

## Examples

See the [examples](examples/) directory for complete usage examples:

- Basic database operations
- Permission configuration
- Sharding setup
- Audit logging

## Documentation

- [API Reference](https://docs.rs/dbnexus)
- [Architecture Guide](docs/ARCHITECTURE.md)
- [User Guide](docs/USER_GUIDE.md)
- [API Reference](docs/API_REFERENCE.md)

## Testing

```bash
# Run all tests
cargo test --all

# Run tests with coverage
cargo tarpaulin --output-dir ./target/tarpaulin

# Run specific test suites
cargo test --package dbnexus --lib
cargo test --package dbnexus-cli
```

## Benchmarking

```bash
cargo bench
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Commit your changes
4. Push to the branch
5. Open a Pull Request

## License

This project is licensed under the MIT OR Apache-2.0 License.

## Authors

DBNexus Team

## Version

0.1.0

## Contact

- Repository: https://github.com/Kirky-X/dbnexus
- Issues: https://github.com/Kirky-X/dbnexus/issues
