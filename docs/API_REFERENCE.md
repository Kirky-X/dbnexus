<div align="center">

# 📘 API Reference

### Complete API Documentation

[🏠 Home](../README.md) • [📖 User Guide](USER_GUIDE.md) • [🏗️ Architecture](ARCHITECTURE.md)

---

</div>

## 📋 Table of Contents

- [Overview](#overview)
- [Core API](#core-api)
  - [Database Pool](#database-pool)
  - [Database Session](#database-session)
  - [Entity Operations](#entity-operations)
  - [Configuration](#configuration)
- [Database Adapters](#database-adapters)
- [Permission Engine](#permission-engine)
- [Cache Manager](#cache-manager)
- [Error Handling](#error-handling)
- [Type Definitions](#type-definitions)
- [Examples](#examples)

---

## Overview

<div align="center">

### 🎯 API Design Principles

</div>

<table>
<tr>
<td width="25%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/easy.png" width="64"><br>
<b>Simple</b><br>
Intuitive and easy to use
</td>
<td width="25%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/security-checked.png" width="64"><br>
<b>Safe</b><br>
Type-safe and secure by default
</td>
<td width="25%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/module.png" width="64"><br>
<b>Composable</b><br>
Build complex workflows easily
</td>
<td width="25%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/documentation.png" width="64"><br>
<b>Well-documented</b><br>
Comprehensive documentation
</td>
</tr>
</table>

---

## Core API

### Database Pool

<div align="center">

#### 🚀 DbPool - Connection Pool Management

</div>

---

#### `DbPool::new()`

Create a new database connection pool.

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
pub async fn new(database_url: &str) -> Result<Self, DbError>
```

</td>
</tr>
<tr>
<td><b>Description</b></td>
<td>Initializes a new connection pool with the specified database URL. Must be called before creating sessions.</td>
</tr>
<tr>
<td><b>Parameters</b></td>
<td>

- `database_url: &str` - Database connection URL (e.g., "sqlite://db.sqlite", "postgresql://user:pass@localhost/db")

</td>
</tr>
<tr>
<td><b>Returns</b></td>
<td><code>Result&lt;DbPool, DbError&gt;</code> - Ok on success, DbError on failure</td>
</tr>
<tr>
<td><b>Errors</b></td>
<td>

- `DbError::ConnectionError` - Failed to connect to database
- `DbError::InvalidConfig` - Invalid configuration

</td>
</tr>
</table>

**Example:**

```rust
use dbnexus::DbPool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("sqlite://./db.sqlite").await?;
    println!("✅ Database pool created successfully");
    Ok(())
}
```

---

#### `DbPool::get_session()`

Create a new database session for a user.

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
pub async fn get_session(&self, user_id: &str) -> Result<DbSession, DbError>
```

</td>
</tr>
<tr>
<td><b>Parameters</b></td>
<td>

- `user_id: &str` - User identifier for permission checking

</td>
</tr>
<tr>
<td><b>Returns</b></td>
<td><code>Result&lt;DbSession, DbError&gt;</code> - New session instance</td>
</tr>
</table>

**Example:**

```rust
let session = pool.get_session("user123").await?;
```

---

#### `DbPool::close()`

Close the connection pool and release all resources.

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
pub async fn close(self) -> Result<(), DbError>
```

</td>
</tr>
<tr>
<td><b>Returns</b></td>
<td><code>Result&lt;(), DbError&gt;</code></td>
</tr>
</table>

---

### Database Session

<div align="center">

#### 🔐 DbSession - Session-based Database Access

</div>

---

#### `DbSession`

A database session that tracks user context and enforces permissions.

<table>
<tr>
<td width="30%"><b>Type</b></td>
<td width="70%">

```rust
pub struct DbSession {
    pool: DatabaseConnection,
    user_id: String,
    permission_engine: Arc<PermissionEngine>,
    cache_manager: Option<Arc<CacheManager>>,
}
```

</td>
</tr>
</table>

---

#### `DbSession::begin_transaction()`

Begin a new database transaction.

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
pub async fn begin_transaction(&self) -> Result<Transaction, DbError>
```

</td>
</tr>
<tr>
<td><b>Returns</b></td>
<td><code>Result&lt;Transaction, DbError&gt;</code> - Transaction handle</td>
</tr>
</table>

**Example:**

```rust
let tx = session.begin_transaction().await?;
tx.commit().await?;
```

---

#### `DbSession::execute()`

Execute a raw SQL query.

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
pub async fn execute(&self, query: &str) -> Result<QueryResult, DbError>
```

</td>
</tr>
<tr>
<td><b>Parameters</b></td>
<td>

- `query: &str` - SQL query string

</td>
</tr>
<tr>
<td><b>Returns</b></td>
<td><code>Result&lt;QueryResult, DbError&gt;</code></td>
</tr>
</table>

---

### Entity Operations

<div align="center">

#### 📦 DbEntity - Entity CRUD Operations

</div>

---

#### `DbEntity` Trait

Core trait for database entities with automatic CRUD operations.

<table>
<tr>
<td width="30%"><b>Definition</b></td>
<td width="70%">

```rust
#[async_trait]
pub trait DbEntity: ModelTrait + Sized {
    async fn insert(&self, session: &DbSession) -> Result<Self, DbError>;
    async fn update(&self, session: &DbSession) -> Result<Self, DbError>;
    async fn delete(&self, session: &DbSession) -> Result<(), DbError>;
    async fn find_by_id(id: i32, session: &DbSession) -> Result<Option<Self>, DbError>;
    async fn find_all(session: &DbSession) -> Result<Vec<Self>, DbError>;
}
```

</td>
</tr>
</table>

---

#### `db_entity!` Macro

Derive macro to implement DbEntity trait for a struct.

<table>
<tr>
<td width="30%"><b>Usage</b></td>
<td width="70%">

```rust
use dbnexus::db_entity;

#[db_entity]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub email: String,
}
```

</td>
</tr>
</table>

---

#### `DbEntity::insert()`

Insert a new entity into the database.

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
async fn insert(&self, session: &DbSession) -> Result<Self, DbError>
```

</td>
</tr>
<tr>
<td><b>Parameters</b></td>
<td>

- `session: &DbSession` - Database session

</td>
</tr>
<tr>
<td><b>Returns</b></td>
<td><code>Result&lt;Self, DbError&gt;</code> - Inserted entity with generated ID</td>
</tr>
<tr>
<td><b>Errors</b></td>
<td>

- `DbError::PermissionDenied` - User lacks insert permission
- `DbError::DuplicateKey` - Unique constraint violation
- `DbError::QueryError` - Database query error

</td>
</tr>
</table>

**Example:**

```rust
let user = User {
    id: 0,
    name: "John Doe".to_string(),
    email: "john@example.com".to_string(),
};

let inserted = user.insert(&session).await?;
println!("Inserted user with ID: {}", inserted.id);
```

---

#### `DbEntity::update()`

Update an existing entity in the database.

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
async fn update(&self, session: &DbSession) -> Result<Self, DbError>
```

</td>
</tr>
<tr>
<td><b>Parameters</b></td>
<td>

- `session: &DbSession` - Database session

</td>
</tr>
<tr>
<td><b>Returns</b></td>
<td><code>Result&lt;Self, DbError&gt;</code> - Updated entity</td>
</tr>
<tr>
<td><b>Errors</b></td>
<td>

- `DbError::PermissionDenied` - User lacks update permission
- `DbError::NotFound` - Entity not found
- `DbError::QueryError` - Database query error

</td>
</tr>
</table>

**Example:**

```rust
let mut user = User::find_by_id(1, &session).await?.unwrap();
user.name = "Jane Doe".to_string();
let updated = user.update(&session).await?;
```

---

#### `DbEntity::delete()`

Delete an entity from the database.

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
async fn delete(&self, session: &DbSession) -> Result<(), DbError>
```

</td>
</tr>
<tr>
<td><b>Parameters</b></td>
<td>

- `session: &DbSession` - Database session

</td>
</tr>
<tr>
<td><b>Returns</b></td>
<td><code>Result&lt;(), DbError&gt;</code></td>
</tr>
<tr>
<td><b>Errors</b></td>
<td>

- `DbError::PermissionDenied` - User lacks delete permission
- `DbError::NotFound` - Entity not found
- `DbError::QueryError` - Database query error

</td>
</tr>
</table>

**Example:**

```rust
let user = User::find_by_id(1, &session).await?.unwrap();
user.delete(&session).await?;
```

---

#### `DbEntity::find_by_id()`

Find an entity by its primary key.

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
async fn find_by_id(id: i32, session: &DbSession) -> Result<Option<Self>, DbError>
```

</td>
</tr>
<tr>
<td><b>Parameters</b></td>
<td>

- `id: i32` - Primary key value
- `session: &DbSession` - Database session

</td>
</tr>
<tr>
<td><b>Returns</b></td>
<td><code>Result&lt;Option&lt;Self&gt;, DbError&gt;</code> - Entity if found</td>
</tr>
</table>

**Example:**

```rust
if let Some(user) = User::find_by_id(1, &session).await? {
    println!("Found user: {}", user.name);
}
```

---

#### `DbEntity::find_all()`

Find all entities of a given type.

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
async fn find_all(session: &DbSession) -> Result<Vec<Self>, DbError>
```

</td>
</tr>
<tr>
<td><b>Parameters</b></td>
<td>

- `session: &DbSession` - Database session

</td>
</tr>
<tr>
<td><b>Returns</b></td>
<td><code>Result&lt;Vec&lt;Self&gt;, DbError&gt;</code> - All entities</td>
</tr>
</table>

**Example:**

```rust
let users = User::find_all(&session).await?;
for user in users {
    println!("User: {}", user.name);
}
```

---

### Configuration

<div align="center">

#### ⚙️ Configuration Management

</div>

---

#### `Config`

Configuration struct for customizing database behavior.

<table>
<tr>
<td width="30%"><b>Type</b></td>
<td width="70%">

```rust
pub struct Config {
    pub database_url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connection_timeout: Duration,
    pub idle_timeout: Duration,
    pub cache_enabled: bool,
    pub cache_ttl: Duration,
    pub audit_enabled: bool,
    pub metrics_enabled: bool,
}
```

</td>
</tr>
</table>

---

#### `Config::from_url()`

Create configuration from database URL.

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
pub fn from_url(url: &str) -> Result<Self, DbError>
```

</td>
</tr>
<tr>
<td><b>Parameters</b></td>
<td>

- `url: &str` - Database connection URL

</td>
</tr>
<tr>
<td><b>Returns</b></td>
<td><code>Result&lt;Config, DbError&gt;</code></td>
</tr>
</table>

**Example:**

```rust
let config = Config::from_url("postgresql://localhost/mydb")?;
```

---

## Database Adapters

<div align="center">

#### 🗄️ Supported Database Backends

</div>

### Supported Databases

<details open>
<summary><b>🔹 SQLite</b></summary>

**Feature Flag:** `sqlite`

**Connection URL:** `sqlite://path/to/database.sqlite`

**Characteristics:**
- Lightweight, serverless database
- Single file storage
- Zero configuration
- Ideal for development and small applications

**Example:**
```rust
let pool = DbPool::new("sqlite://./db.sqlite").await?;
```

</details>

<details>
<summary><b>🔹 PostgreSQL</b></summary>

**Feature Flag:** `postgres`

**Connection URL:** `postgresql://user:password@localhost/database`

**Characteristics:**
- Full-featured, enterprise-grade database
- ACID compliant
- Advanced features (JSON, arrays, etc.)
- Excellent for production workloads

**Example:**
```rust
let pool = DbPool::new(
    "postgresql://user:pass@localhost/mydb"
).await?;
```

</details>

<details>
<summary><b>🔹 MySQL</b></summary>

**Feature Flag:** `mysql`

**Connection URL:** `mysql://user:password@localhost/database`

**Characteristics:**
- Popular open-source database
- High performance
- Wide compatibility
- Good for web applications

**Example:**
```rust
let pool = DbPool::new(
    "mysql://user:pass@localhost/mydb"
).await?;
```

</details>

### Feature Selection

**Cargo.toml:**
```toml
[dependencies]
dbnexus = { version = "0.1", features = ["postgres"] }
```

**Note:** Only one database feature can be enabled at a time.

---

## Permission Engine

<div align="center">

#### 🛡️ Permission Control System

</div>

---

#### `PermissionEngine`

Manages and enforces database access permissions.

<table>
<tr>
<td width="30%"><b>Type</b></td>
<td width="70%">

```rust
pub struct PermissionEngine {
    roles: HashMap<String, Role>,
    permissions: HashMap<String, Permission>,
}
```

</td>
</tr>
</table>

---

#### `PermissionEngine::new()`

Create a new permission engine.

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
pub fn new() -> Self
```

</td>
</tr>
<tr>
<td><b>Returns</b></td>
<td><code>PermissionEngine</code> - New instance</td>
</tr>
</table>

---

#### `PermissionEngine::add_role()`

Add a new role to the engine.

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
pub fn add_role(&mut self, role: Role)
```

</td>
</tr>
<tr>
<td><b>Parameters</b></td>
<td>

- `role: Role` - Role definition

</td>
</tr>
</table>

---

#### `PermissionEngine::check_permission()`

Check if a user has a specific permission.

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
pub fn check_permission(
    &self,
    user_id: &str,
    resource: &str,
    action: &str
) -> bool
```

</td>
</tr>
<tr>
<td><b>Parameters</b></td>
<td>

- `user_id: &str` - User identifier
- `resource: &str` - Resource name (e.g., "users")
- `action: &str` - Action (e.g., "read", "write", "delete")

</td>
</tr>
<tr>
<td><b>Returns</b></td>
<td><code>bool</code> - true if permission granted</td>
</tr>
</table>

**Example:**

```rust
let has_permission = permission_engine.check_permission(
    "user123",
    "users",
    "write"
);
```

---

## Cache Manager

<div align="center">

#### ⚡ Caching Layer

</div>

---

#### `CacheManager`

Manages in-memory caching of query results.

<table>
<tr>
<td width="30%"><b>Type</b></td>
<td width="70%">

```rust
pub struct CacheManager {
    cache: Arc<Mutex<LruCache<String, CacheEntry>>>,
    ttl: Duration,
}
```

</td>
</tr>
</table>

---

#### `CacheManager::new()`

Create a new cache manager.

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
pub fn new(ttl: Duration) -> Result<Self, DbError>
```

</td>
</tr>
<tr>
<td><b>Parameters</b></td>
<td>

- `ttl: Duration` - Time-to-live for cache entries

</td>
</tr>
<tr>
<td><b>Returns</b></td>
<td><code>Result&lt;CacheManager, DbError&gt;</code></td>
</tr>
</table>

---

#### `CacheManager::get()`

Retrieve a value from cache.

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
pub fn get<T>(&self, key: &str) -> Option<T>
where
    T: DeserializeOwned,
```

</td>
</tr>
<tr>
<td><b>Parameters</b></td>
<td>

- `key: &str` - Cache key

</td>
</tr>
<tr>
<td><b>Returns</b></td>
<td><code>Option&lt;T&gt;</code> - Cached value if exists and not expired</td>
</tr>
</table>

---

#### `CacheManager::set()`

Store a value in cache.

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
pub fn set<T>(&self, key: &str, value: &T) -> Result<(), DbError>
where
    T: Serialize,
```

</td>
</tr>
<tr>
<td><b>Parameters</b></td>
<td>

- `key: &str` - Cache key
- `value: &T` - Value to cache

</td>
</tr>
<tr>
<td><b>Returns</b></td>
<td><code>Result&lt;(), DbError&gt;</code></td>
</tr>
</table>

---

#### `CacheManager::invalidate()`

Invalidate a cache entry.

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
pub fn invalidate(&self, key: &str)
```

</td>
</tr>
<tr>
<td><b>Parameters</b></td>
<td>

- `key: &str` - Cache key to invalidate

</td>
</tr>
</table>

---

## Error Handling

<div align="center">

#### 🚨 Error Types and Handling

</div>

### `DbError` Enum

```rust
pub enum DbError {
    ConnectionError(String),
    QueryError(String),
    PermissionDenied(String),
    NotFound(String),
    DuplicateKey(String),
    InvalidConfig(String),
    SerializationError(String),
    CacheError(String),
    MigrationError(String),
    Custom(String),
}
```

### Error Handling Pattern

<table>
<tr>
<td width="50%">

**Pattern Matching**
```rust
match operation() {
    Ok(result) => {
        println!("Success: {:?}", result);
    }
    Err(DbError::PermissionDenied(msg)) => {
        eprintln!("Permission denied: {}", msg);
    }
    Err(DbError::NotFound(msg)) => {
        eprintln!("Not found: {}", msg);
    }
    Err(e) => {
        eprintln!("Error: {:?}", e);
    }
}
```

</td>
<td width="50%">

**? Operator**
```rust
async fn process_user(
    id: i32,
    session: &DbSession
) -> Result<User, DbError> {
    let mut user = User::find_by_id(id, session)
        .await?
        .ok_or(DbError::NotFound(
            "User not found".to_string()
        ))?;
    
    user.name = "Updated".to_string();
    user.update(session).await?;
    
    Ok(user)
}
```

</td>
</tr>
</table>

---

## Type Definitions

### Common Types

<table>
<tr>
<td width="50%">

**Database Connection**
```rust
pub type DatabaseConnection = 
    sea_orm::DatabaseConnection;
```

**Transaction**
```rust
pub type Transaction = 
    sea_orm::Transaction;
```

**Query Result**
```rust
pub type QueryResult = 
    sea_orm::QueryResult;
```

</td>
<td width="50%">

**DbResult**
```rust
pub type DbResult<T> = 
    Result<T, DbError>;
```

**UserId**
```rust
pub type UserId = String;
```

**ResourceId**
```rust
pub type ResourceId = String;
```

</td>
</tr>
</table>

---

## Examples

<div align="center">

### 💡 Common Usage Patterns

</div>

### Example 1: Basic CRUD Operations

```rust
use dbnexus::{DbPool, db_entity};

#[db_entity]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("sqlite://./db.sqlite").await?;
    let session = pool.get_session("admin").await?;
    
    let user = User {
        id: 0,
        name: "John Doe".to_string(),
        email: "john@example.com".to_string(),
    };
    
    let inserted = user.insert(&session).await?;
    println!("✅ Inserted user with ID: {}", inserted.id);
    
    let found = User::find_by_id(inserted.id, &session).await?;
    println!("✅ Found user: {:?}", found);
    
    Ok(())
}
```

### Example 2: Transaction Handling

```rust
async fn transfer_funds(
    from_id: i32,
    to_id: i32,
    amount: f64,
    session: &DbSession
) -> Result<(), DbError> {
    let tx = session.begin_transaction().await?;
    
    let mut from = Account::find_by_id(from_id, session)
        .await?
        .ok_or(DbError::NotFound(
            "Source account not found".to_string()
        ))?;
    
    let mut to = Account::find_by_id(to_id, session)
        .await?
        .ok_or(DbError::NotFound(
            "Target account not found".to_string()
        ))?;
    
    from.balance -= amount;
    to.balance += amount;
    
    from.update(session).await?;
    to.update(session).await?;
    
    tx.commit().await?;
    
    Ok(())
}
```

### Example 3: Permission Checking

```rust
use dbnexus::{DbPool, PermissionEngine, Role, Permission};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("sqlite://./db.sqlite").await?;
    
    let mut permission_engine = PermissionEngine::new();
    
    let admin_role = Role {
        name: "admin".to_string(),
        permissions: vec![
            Permission::new("users", "read"),
            Permission::new("users", "write"),
            Permission::new("users", "delete"),
        ],
    };
    
    permission_engine.add_role(admin_role);
    
    let session = pool.get_session("admin").await?;
    
    let has_permission = permission_engine.check_permission(
        "admin",
        "users",
        "delete"
    );
    
    println!("✅ Has delete permission: {}", has_permission);
    
    Ok(())
}
```

### Example 4: Advanced Configuration

```rust
use dbnexus::{DbPool, Config};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_url("postgresql://localhost/mydb")?;
    
    let pool = DbPool::new_with_config(config).await?;
    
    let session = pool.get_session("user123").await?;
    
    Ok(())
}
```

---

<div align="center">

**[📖 User Guide](USER_GUIDE.md)** • **[🏗️ Architecture](ARCHITECTURE.md)** • **[🏠 Home](../README.md)**

Made with ❤️ by the Documentation Team

[⬆ Back to Top](#-api-reference)
</div>
