# dbnexus FAQ

## General Questions

### What is dbnexus?

dbnexus is a high-performance, feature-rich database abstraction library for Rust that provides a unified interface for multiple database backends. It offers advanced features like connection pooling, session management, permission control, caching, audit logging, sharding, and Prometheus metrics integration.

### What databases does dbnexus support?

dbnexus currently supports the following databases through Sea-ORM:
- PostgreSQL
- MySQL
- SQLite
- Microsoft SQL Server

### What are the key features of dbnexus?

Key features include:
- Multi-database support with unified API
- Connection pool management
- Session-based database access
- Declarative entity definitions with macros
- Permission-based access control
- Built-in caching support
- Comprehensive audit logging
- Database sharding support
- Prometheus metrics integration
- Transaction management
- Async/await support

### Is dbnexus production-ready?

Yes, dbnexus is designed for production use with robust error handling, connection pooling, and comprehensive testing. However, as with any database library, thorough testing in your specific environment is recommended.

## Installation

### How do I add dbnexus to my project?

Add dbnexus to your `Cargo.toml`:

```toml
[dependencies]
dbnexus = "0.1"
```

### What are the feature flags?

dbnexus uses feature flags to enable optional functionality:

```toml
[dependencies]
dbnexus = { version = "0.1", features = ["postgres", "mysql", "permission", "cache", "audit", "sharding", "metrics"] }
```

Available features:
- `postgres`: PostgreSQL support
- `mysql`: MySQL support
- `sqlite`: SQLite support
- `mssql`: Microsoft SQL Server support
- `permission`: Permission control system
- `cache`: Caching support
- `audit`: Audit logging
- `sharding`: Database sharding
- `metrics`: Prometheus metrics

### What are the minimum Rust version requirements?

dbnexus requires Rust 1.75 or later due to its use of async/await and modern Rust features.

## Usage

### How do I initialize a database connection pool?

```rust
use dbnexus::DbPool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("postgres://user:password@localhost/dbname").await?;
    Ok(())
}
```

### How do I define a database entity?

Use the `DbEntity` macro:

```rust
use dbnexus::DbEntity;

#[derive(Clone, Debug, DbEntity)]
#[db_entity(table_name = "users")]
pub struct User {
    #[db_entity(primary_key)]
    pub id: i32,
    pub name: String,
    pub email: String,
    #[db_entity(created_at)]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    #[db_entity(updated_at)]
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}
```

### How do I perform CRUD operations?

```rust
use dbnexus::{DbEntity, DbPool, DbSession};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("postgres://user:password@localhost/dbname").await?;
    let session = pool.get_session().await?;

    // Create
    let user = User {
        id: 0,
        name: "John Doe".to_string(),
        email: "john@example.com".to_string(),
        created_at: None,
        updated_at: None,
    };
    let created_user = user.insert(&session).await?;

    // Read
    let found_user = User::find_by_id(1, &session).await?;

    // Update
    let mut user = found_user.unwrap();
    user.name = "Jane Doe".to_string();
    let updated_user = user.update(&session).await?;

    // Delete
    updated_user.delete(&session).await?;

    Ok(())
}
```

### How do I use transactions?

```rust
use dbnexus::{DbEntity, DbPool, DbSession};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("postgres://user:password@localhost/dbname").await?;
    let session = pool.get_session().await?;

    let result = session.transaction(|tx| async move {
        let user = User {
            id: 0,
            name: "John Doe".to_string(),
            email: "john@example.com".to_string(),
            created_at: None,
            updated_at: None,
        };
        let created_user = user.insert(&tx).await?;
        Ok::<_, dbnexus::DbError>(created_user)
    }).await?;

    Ok(())
}
```

## Performance

### How does connection pooling work?

dbnexus uses Sea-ORM's connection pool which maintains a pool of database connections that can be reused. This eliminates the overhead of creating new connections for each query. The pool size can be configured based on your application's needs.

### How can I optimize performance?

Performance optimization tips:
- Use connection pooling with appropriate pool size
- Enable caching for frequently accessed data
- Use batch operations for bulk inserts/updates
- Leverage database indexes properly
- Use sharding for large datasets
- Monitor metrics to identify bottlenecks
- Use prepared statements

### Does dbnexus support caching?

Yes, dbnexus supports caching through the `cache` feature. You can enable caching for entities and queries:

```rust
use dbnexus::DbEntity;

#[derive(Clone, Debug, DbEntity)]
#[db_entity(table_name = "users", cache = true, cache_ttl = 300)]
pub struct User {
    // ... fields
}
```

## Security

### How does permission control work?

dbnexus provides a permission-based access control system through the `permission` feature. You can define permissions and check them before operations:

```rust
use dbnexus::permission::{Permission, PermissionContext};

let permission = Permission::new("users", "read");
let context = PermissionContext::new(user_id);
let has_permission = permission.check(&context).await?;
```

### How do I enable audit logging?

Enable the `audit` feature and configure audit logging:

```rust
use dbnexus::audit::AuditLogger;

let audit_logger = AuditLogger::new(pool.clone());
audit_logger.log_operation("create", "users", user_id, &user).await?;
```

### Are database credentials secure?

dbnexus supports environment variables and configuration files for storing database credentials. Never hardcode credentials in your source code. Use environment variables or secure secret management systems.

## Troubleshooting

### Why am I getting connection errors?

Common causes:
- Database server is not running
- Incorrect connection URL
- Network connectivity issues
- Firewall blocking connections
- Database user lacks necessary permissions

Check your connection string and ensure the database is accessible.

### Why are my queries slow?

Possible reasons:
- Missing database indexes
- Inefficient queries
- Connection pool too small
- Network latency
- Large result sets

Use the metrics feature to monitor query performance and identify bottlenecks.

### How do I debug issues?

Enable debug logging:

```rust
env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
```

Check the logs for detailed error messages and stack traces.

## Contributing

### How can I contribute to dbnexus?

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on:
- Setting up the development environment
- Running tests
- Submitting pull requests
- Coding standards

### What is the contribution process?

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality
5. Ensure all tests pass
6. Submit a pull request with a clear description

### Where can I report bugs?

Report bugs using the GitHub issue tracker. Please include:
- A clear description of the bug
- Steps to reproduce
- Expected vs actual behavior
- Environment details (OS, Rust version, database type)
- Relevant code snippets

## Licensing

### What license does dbnexus use?

dbnexus is licensed under the MIT License. See the [LICENSE](../LICENSE) file for details.

### Can I use dbnexus in commercial projects?

Yes, dbnexus is free to use in commercial projects under the MIT License.

### Can I modify dbnexus?

Yes, you are free to modify dbnexus as long as you comply with the MIT License terms, including preserving the copyright notice and license text.

## Advanced Topics

### How do I implement database sharding?

Enable the `sharding` feature and configure sharding strategy:

```rust
use dbnexus::sharding::{ShardConfig, ShardStrategy};

let config = ShardConfig::new()
    .with_strategy(ShardStrategy::Hash)
    .with_shard_count(4);

let pool = DbPool::new_with_sharding("postgres://...", &config).await?;
```

### How do I integrate Prometheus metrics?

Enable the `metrics` feature and configure metrics:

```rust
use dbnexus::metrics::MetricsCollector;

let metrics = MetricsCollector::new();
metrics.start_server(9090).await?;
```

Metrics are available at `http://localhost:9090/metrics`.

### Can I use dbnexus with multiple databases?

Yes, you can create multiple `DbPool` instances for different databases:

```rust
let postgres_pool = DbPool::new("postgres://...").await?;
let mysql_pool = DbPool::new("mysql://...").await?;
```

## Support

### Where can I get help?

- Documentation: [docs/](../docs/)
- GitHub Issues: [Issues](https://github.com/yourorg/dbnexus/issues)
- Discussions: [GitHub Discussions](https://github.com/yourorg/dbnexus/discussions)

### Is there a community forum?

Join our GitHub Discussions for community support and discussions about dbnexus.

### How do I stay updated?

Watch the GitHub repository for releases and updates. Follow the project on social media for announcements.
