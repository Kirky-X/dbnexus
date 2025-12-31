<div align="center">

# DB Nexus

<p>
  <a href="https://github.com/Kirky-X/dbnexus/actions/workflows/ci.yml"><img src="https://github.com/Kirky-X/dbnexus/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-yellow.svg" alt="License"></a>
  <a href="https://crates.io/crates/dbnexus"><img src="https://img.shields.io/crates/v/dbnexus.svg" alt="Crates.io"></a>
  <a href="https://docs.rs/dbnexus"><img src="https://docs.rs/dbnexus/badge.svg" alt="Documentation"></a>
  <a href="https://crates.io/crates/dbnexus"><img src="https://img.shields.io/crates/d/dbnexus.svg" alt="Downloads"></a>
</p>

<p align="center">
  <strong>DB Nexus is an enterprise-grade database abstraction layer built on Sea-ORM, providing a high-performance and secure Rust database access solution.</strong>
</p>

<p align="center">
  <a href="#-features">Features</a> •
  <a href="#-quick-start">Quick Start</a> •
  <a href="#-documentation">Documentation</a> •
  <a href="#-examples">Examples</a> •
  <a href="#-contributing">Contributing</a>
</p>

</div>

---

## ✨ Features

### Core Features

- **Multi-Database Support**: SQLite, PostgreSQL, and MySQL support via feature gates
- **Session Mechanism**: RAII-based automatic database connection lifecycle management
- **Permission Control**: Declarative macros for automatic permission check code generation
- **Connection Pool Management**: Dynamic configuration and health checks
- **Monitoring Metrics**: Prometheus metrics export
- **Migration Tools**: Automated schema change management
- **Sharding Support**: Horizontal sharding and global index support
- **Cache Layer**: Pluggable cache abstraction
- **Audit Logging**: Complete operation audit trail
- **Pluggable Permission Engine**: Support for custom permission policies

---

## 🎯 Use Cases

- **Enterprise Applications**: Large-scale systems requiring strict permission control
- **Microservice Architecture**: Multi-database and multi-tenant scenarios
- **High-Concurrency Systems**: Systems requiring connection pool and cache optimization
- **Audit Requirements**: Systems requiring complete operation logs
- **Data Sensitivity**: Applications requiring fine-grained permission control

---

## 🚀 Quick Start

### Installation

Add the dependency in your `Cargo.toml`:

```toml
[dependencies]
dbnexus = { version = "0.1", features = ["sqlite"] }
```

### Basic Usage

```rust
use dbnexus::{DbPool, DbEntity, db_crud};

#[derive(DbEntity)]
#[db_entity]
#[table_name = "users")]
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
    Ok(())
}
```

### Running Examples

```bash
# Quick start example
cargo run --example quickstart --features sqlite

# Permission control example
cargo run --example permissions --features sqlite

# Transaction example
cargo run --example transactions --features sqlite
```

---

## 📚 Documentation

- [User Guide](docs/USER_GUIDE.md) - Detailed usage instructions and best practices
- [API Reference](docs/API_REFERENCE.md) - Complete API reference
- [Architecture Documentation](docs/ARCHITECTURE.md) - System architecture and design decisions
- [FAQ](docs/FAQ.md) - Frequently asked questions
- [Contributing Guide](docs/CONTRIBUTING.md) - How to contribute to the project

---

## 🎨 Examples

### Quick Start Example

```rust
use dbnexus::{DbPool, DbEntity, db_crud};

#[derive(DbEntity)]
#[db_entity]
#[table_name = "users")]
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
    
    let inserted = User::insert(&session, user).await?;
    println!("Inserted user: {}", inserted.name);
    
    Ok(())
}
```

### Permission Control Example

```rust
use dbnexus::{DbEntity, db_permission, db_crud};

#[derive(DbEntity)]
#[db_entity]
#[table_name = "users")]
#[db_permission(role = "admin", actions = ["read", "write", "delete"])]
#[db_permission(role = "user", actions = ["read"])]
#[db_crud]
struct User {
    #[primary_key]
    id: i64,
    name: String,
    email: String,
}
```

<div align="center">

**[View More Examples →](examples/)**

</div>

---

## 🏗️ Project Structure

```
dbnexus/
├── dbnexus/              # Core library
│   ├── src/
│   │   ├── lib.rs       # Library entry point
│   │   ├── pool.rs      # Connection pool management
│   │   ├── session.rs   # Session mechanism
│   │   ├── permission.rs # Permission control
│   │   ├── config.rs    # Configuration management
│   │   ├── cache.rs     # Cache layer
│   │   ├── audit.rs     # Audit logging
│   │   ├── sharding.rs  # Sharding support
│   │   ├── global_index.rs # Global index
│   │   ├── metrics.rs   # Monitoring metrics
│   │   ├── migration.rs # Migration tools
│   │   ├── tracing.rs   # Distributed tracing
│   │   ├── permission_engine.rs # Pluggable permission engine
│   │   ├── entity.rs    # Entity conversion
│   │   └── generated_roles.rs # Generated permission roles
│   └── tests/           # Integration tests
│       ├── pool_integration.rs
│       ├── permission_integration.rs
│       ├── cache_integration.rs
│       ├── audit_integration.rs
│       ├── sharding_integration.rs
│       ├── migration_integration.rs
│       ├── multi_db_integration.rs
│       ├── session_transaction.rs
│       ├── cli_integration.rs
│       └── concurrency_integration.rs
├── dbnexus-macros/      # Procedural macros
│   └── src/
│       └── lib.rs       # Macro definitions
├── migrate-cli/         # Migration CLI tool
│   └── src/
│       └── main.rs      # CLI entry point
├── examples/            # Example code
│   ├── quickstart.rs    # Quick start
│   ├── permissions.rs   # Permission control
│   └── transactions.rs  # Transaction handling
├── docs/                # Documentation
│   ├── USER_GUIDE.md    # User guide
│   ├── API_REFERENCE.md  # API reference
│   ├── ARCHITECTURE.md  # Architecture documentation
│   ├── FAQ.md           # Frequently asked questions
│   ├── CONTRIBUTING.md  # Contributing guide
│   ├── prd.md           # Product requirements document
│   ├── task.md          # Task document
│   ├── tdd.md           # TDD guide
│   ├── test.md          # Test documentation
│   └── uat.md           # User acceptance testing
├── scripts/             # Script tools
│   ├── init-sqlite.sql
│   ├── init-mysql.sql
│   ├── init-postgres.sql
│   ├── generate-sql.sh
│   └── test-databases.sh
├── Cargo.toml           # Workspace configuration
├── Cargo.lock           # Dependency lock file
├── Makefile             # Build script
├── rustfmt.toml         # Code formatting configuration
├── deny.toml            # Dependency audit configuration
└── tarpaulin.toml       # Test coverage configuration
```

---

## ⚙️ Configuration

### Basic Configuration

```toml
[dependencies]
dbnexus = { version = "0.1", features = ["sqlite", "cache", "audit"] }
```

### Feature Options

| Feature | Description | Default |
|---------|-------------|---------|
| `sqlite` | SQLite database support | - |
| `postgres` | PostgreSQL database support | - |
| `mysql` | MySQL database support | - |
| `cache` | Cache layer support | false |
| `audit` | Audit logging support | false |
| `sharding` | Sharding support | false |
| `global-index` | Global index support | false |
| `metrics` | Prometheus metrics export | false |
| `migration` | Migration tools | false |
| `permission-engine` | Pluggable permission engine | false |
| `tracing` | Distributed tracing support | false |

**Note**: Database features (sqlite, postgres, mysql) are mutually exclusive; you can only select one.

---

## 🧪 Testing

```bash
# Run all tests
cargo test --all-features

# Run specific test
cargo test pool_integration --features sqlite

# Run tests and generate coverage report
cargo tarpaulin --out Html --all-features

# Run integration tests
cargo test --test '*' --all-features
```

### Test Coverage

| Test Type | Test File | Coverage Content |
|-----------|-----------|------------------|
| Connection Pool Tests | pool_integration.rs | Connection pool creation, retrieval, health checks |
| Permission Tests | permission_integration.rs | Permission checks, role management |
| Cache Tests | cache_integration.rs | Cache read/write, invalidation strategies |
| Audit Tests | audit_integration.rs | Audit log recording, queries |
| Sharding Tests | sharding_integration.rs | Sharding routing, global indexes |
| Migration Tests | migration_integration.rs | Schema changes, version management |
| Multi-Database Tests | multi_db_integration.rs | Multi-database connections, transactions |
| Session Tests | session_transaction.rs | Session lifecycle, transactions |
| CLI Tests | cli_integration.rs | Command-line tool functionality |
| Concurrency Tests | concurrency_integration.rs | Concurrency safety, lock contention |

---

## 📊 Performance

### Benchmarks

```bash
# Run benchmarks
cargo bench
```

### Performance Features

- **Zero-Copy**: Uses Rust's ownership system to avoid unnecessary copying
- **Async I/O**: Async runtime based on Tokio
- **Connection Pool**: Efficient connection reuse and management
- **Cache**: LRU cache to reduce database access
- **Batch Operations**: Support for batch insert and update operations

---

## 🔒 Security

### Security Features

- **Compile-Time Safety**: Rust's type system and borrow checker
- **Permission Control**: Role-based table-level permission control
- **Audit Logging**: Complete operation audit trail
- **SQL Injection Protection**: Using parameterized queries
- **Connection Security**: Support for TLS-encrypted connections

### Security Best Practices

1. Always use parameterized queries
2. Enable audit logging for critical operations
3. Configure roles using the principle of least privilege
4. Regularly update dependency versions
5. Use TLS encryption in production environments

---

## 🗺️ Roadmap

### v0.1.0 (Current Version)

- [x] Multi-database support
- [x] Session mechanism
- [x] Permission control
- [x] Connection pool management
- [x] Basic caching
- [x] Audit logging
- [x] Migration tools
- [x] Basic documentation and examples

### v0.2.0 (Planned)

- [ ] Advanced sharding strategies
- [ ] Distributed transaction support
- [ ] More database drivers
- [ ] Performance optimization
- [ ] More examples and tutorials

### v1.0.0 (Future)

- [ ] Complete plugin system
- [ ] Multi-language bindings
- [ ] Enterprise-grade features
- [ ] Cloud-native support

---

## 🤝 Contributing

We welcome contributions in any form!

### How to Contribute

1. Fork this repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Create a Pull Request

### Development Guidelines

- Follow Rust coding standards
- Write unit tests and integration tests
- Update relevant documentation
- Ensure CI passes

See the [Contributing Guide](docs/CONTRIBUTING.md) for more details.

---

## 📄 License

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

This project uses the MIT License.

---

## 🙏 Acknowledgments

This project is based on the following excellent open-source projects:

- [Sea-ORM](https://github.com/SeaQL/sea-orm) - Async ORM framework
- [Tokio](https://tokio.rs/) - Async runtime
- [Serde](https://serde.rs/) - Serialization/deserialization framework
- [Prometheus](https://prometheus.io/) - Monitoring metrics system

Thanks to all contributors for their support!

---

## 📞 Contact

- **GitHub Issues**: [Report Issues](https://github.com/dbnexus/dbnexus/issues)
- **GitHub Discussions**: [Join Discussions](https://github.com/dbnexus/dbnexus/discussions)
- **Documentation**: [docs.rs/dbnexus](https://docs.rs/dbnexus)

---

<div align="center">

### If this project is helpful to you, please give us a ⭐️!

**Built with ❤️ by DB Nexus Team**

[⬆ Back to Top](#db-nexus)

---

<sub>© 2025 DB Nexus Team. All rights reserved.</sub>

</div>
