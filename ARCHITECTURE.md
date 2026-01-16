# DBNexus Architecture

## Table of Contents

- [Overview](#overview)
- [Design Principles](#design-principles)
- [System Architecture](#system-architecture)
- [Module Architecture](#module-architecture)
- [Core Components](#core-components)
- [Data Flow](#data-flow)
- [Security Architecture](#security-architecture)
- [Performance Architecture](#performance-architecture)
- [Scalability Architecture](#scalability-architecture)

---

## Overview

DBNexus is an enterprise-grade database abstraction layer built on top of Sea-ORM. The architecture follows a **layered design** with clear separation of concerns, enabling developers to choose exactly the features they need while maintaining consistency and simplicity.

### Key Architectural Goals

1. **Modularity** - Feature-gated compilation for minimal binaries
2. **Safety** - RAII-based resource management and compile-time guarantees
3. **Performance** - Zero-cost abstractions and async-first design
4. **Extensibility** - Pluggable components (permission engines, cache strategies, etc.)
5. **Observability** - Built-in metrics, tracing, and audit logging

---

## Design Principles

### 1. RAII Resource Management

All database connections are managed using Rust's RAII (Resource Acquisition Is Initialization) pattern:

```rust
{
    let session = pool.get_session("admin").await?;
    // Use session...
    // Connection automatically released when session is dropped
}
```

**Benefits:**
- Automatic connection cleanup
- No manual resource management needed
- Exception-safe guarantee

### 2. Feature-Gated Architecture

Features are organized into logical groups:

```
Core Features (always available):
  - config
  - pool
  - entity

Optional Core Features:
  - permission
  - sql-parser
  - macros

Enterprise Features:
  - metrics
  - tracing
  - audit
  - migration
  - sharding
  - global-index
  - cache
```

### 3. Async-First Design

All I/O operations use `async/await` with Tokio:

- `AsyncMutex` for thread-safe state
- `Notify` for efficient condition waiting
- `tokio::spawn` for background tasks

### 4. Type-Safe Abstractions

Compile-time guarantees prevent common errors:

- **Database Driver Mutual Exclusion**: Only one database driver can be enabled
- **Permission Verification**: Compile-time role validation
- **Type Safety**: All database operations are type-safe

---

## System Architecture

### High-Level Layer Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                     Application Layer                     │
│              (User code using DBNexus)                   │
└────────────────────┬──────────────────────────────────────┘
                     │
┌────────────────────▼──────────────────────────────────────┐
│                   DBNexus API Layer                     │
│  ┌──────────┬──────────┬──────────┬─────────────┐ │
│  │ DbPool   │ Session  │  Macros  │   Types     │ │
│  └──────────┴──────────┴──────────┴─────────────┘ │
└────────────────────┬──────────────────────────────────────┘
                     │
┌────────────────────▼──────────────────────────────────────┐
│                  Feature Modules                         │
│  ┌─────────┬──────────┬─────────┬──────────────┐  │
│  │Config   │Permission│ Parser  │   Metrics    │  │
│  ├─────────┼──────────┼─────────┼──────────────┤  │
│  │ Audit   │  Cache   │ Sharding│   Tracing    │  │
│  ├─────────┼──────────┼─────────┼──────────────┤  │
│  │Migration│GlobalIdx│ PermEng │    etc.      │  │
│  └─────────┴──────────┴─────────┴──────────────┘  │
└────────────────────┬──────────────────────────────────────┘
                     │
┌────────────────────▼──────────────────────────────────────┐
│               Connection Pool Layer                     │
│  ┌──────────────────────────────────────────────────┐  │
│  │  Connection Queue (AsyncMutex<Vec<Conn>>)     │  │
│  │  + Atomic Counters (active/total/wait)        │  │
│  │  + Notify (condition variable replacement)      │  │
│  └──────────────────────────────────────────────────┘  │
└────────────────────┬──────────────────────────────────────┘
                     │
┌────────────────────▼──────────────────────────────────────┐
│                Sea-ORM / SQLx Layer                     │
│  ┌──────────────────────────────────────────────────┐  │
│  │   Database Drivers (PostgreSQL/MySQL/SQLite)    │  │
│  │   Query Builder & Type System                  │  │
│  └──────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────┘
```

### Component Interaction

```
Application
    │
    │ 1. pool.get_session("role")
    ▼
DbPool (acquire connection)
    │
    │ 2. Permission check
    ▼
PermissionContext (validate role)
    │
    │ 3. Return Session
    ▼
Session (holds connection + role)
    │
    │ 4. session.execute("SELECT...")
    ▼
SQLParser (extract table + operation)
    │
    │ 5. Check table permission
    ▼
PermissionContext (allow/deny)
    │
    │ 6. Execute query
    ▼
Database (return result)
    │
    │ 7. Session dropped
    ▼
DbPool (release connection)
```

---

## Module Architecture

### Core Modules

#### 1. Configuration Module (`config.rs`)

**Responsibility:** Centralized configuration management

**Key Components:**
```rust
pub struct DbConfig {
    pub url: String,                    // Database connection URL
    pub max_connections: u32,           // Maximum pool size
    pub min_connections: u32,           // Minimum pool size
    pub idle_timeout: u64,              // Idle connection timeout
    pub acquire_timeout: u64,           // Connection acquisition timeout
    pub permissions_path: Option<String>, // Permission config path
    pub migrations_dir: Option<PathBuf>, // Migration directory
    pub auto_migrate: bool,             // Auto-migrate flag
    pub migration_timeout: u64,         // Migration timeout
    pub admin_role: String,             // Admin role name
}

pub struct DbConfigBuilder {
    // Chain API for building config
}

pub struct ConfigLoader {
    // Load from env vars, YAML, TOML
}

pub struct ConfigCorrector {
    // Auto-correct invalid values
}
```

**Configuration Sources Priority:**
1. Environment variables (highest)
2. YAML/TOML config files
3. Built-in defaults (lowest)

**Security Features:**
- Path traversal attack prevention
- URL protocol whitelist
- Configuration validation

#### 2. Connection Pool Module (`pool/`)

**Responsibility:** Manage database connection lifecycle

**Architecture:**

```
DbPool (Arc<DbPoolInner>)
├── idle_connections: AsyncMutex<Vec<DatabaseConnection>>
├── connection_available: Notify
├── active_count: AtomicU32
├── total_count: AtomicU32
├── wait_count: AtomicU32
├── max_active: AtomicU32
├── policy_cache: Arc<AsyncMutex<LruCache>>
├── config: DbConfig
└── admin_role: String

Session
├── connection: Option<DatabaseConnection>
├── pool: Arc<DbPool>
├── role: String
├── transaction: Option<DatabaseTransaction>
└── permission_ctx: PermissionContext
```

**Connection Acquisition Flow:**

```rust
async fn acquire_connection(&self) -> DbResult<DatabaseConnection> {
    // 1. Try to get from idle queue
    let mut idle = self.idle_connections.lock().await;
    if let Some(conn) = idle.pop() {
        self.active_count.fetch_add(1, Ordering::SeqCst);
        return Ok(conn);
    }

    // 2. Check if max connections reached
    if self.total_count.load(Ordering::SeqCst) >= self.config.max_connections {
        drop(idle);
        self.wait_count.fetch_add(1, Ordering::SeqCst);
        let notified = self.connection_available.notified();
        notified.await; // Wait efficiently using Notify
        // Retry...
    }

    // 3. Create new connection
    let conn = self.create_connection().await?;
    self.total_count.fetch_add(1, Ordering::SeqCst);
    return Ok(conn);
}
```

**RAII Implementation:**

```rust
impl Drop for Session {
    fn drop(&mut self) {
        if let Some(conn) = self.connection.take() {
            self.pool.release_connection(conn);
        }
    }
}
```

#### 3. Permission Module (`permission.rs`)

**Responsibility:** Role-based access control (RBAC)

**Architecture:**

```
PermissionContext
├── role: String
├── policy_cache: Arc<AsyncMutex<LruCache<String, RolePolicy>>>
├── rate_limiter: Option<Arc<RateLimiter>>
└── config: PermissionConfig

PermissionConfig
└── roles: HashMap<String, RolePolicy>

RolePolicy
└── tables: Vec<TablePermission>

TablePermission
├── name: String (table name or "*")
└── operations: HashSet<PermissionAction>
```

**Permission Check Flow:**

```
1. Rate limit check
   └─> Block if exceeded
   └─> Reset after time window

2. LRU cache lookup
   └─> Cache hit: return cached decision
   └─> Cache miss: continue

3. Load policy from config
   └─> Parse YAML config
   └─> Build role policy map

4. Check table access
   └─> Is role allowed for table?
   └─> Is operation allowed?

5. Cache decision
   └─> Store in LRU cache

6. Return allow/deny
```

**Performance Optimizations:**

- **LRU Cache**: Default 256 entries, reduces config loading
- **Rate Limiting**: 100 requests/minute, prevents abuse
- **Async Locks**: Non-blocking for concurrent requests

### Optional Feature Modules

#### 4. Metrics Module (`metrics.rs`)

**Responsibility:** Performance metrics collection

**Metrics Tracked:**

| Metric | Description |
|---------|-------------|
| `pool_connections_active` | Currently active connections |
| `pool_connections_idle` | Idle connections |
| `pool_connections_total` | Total connections |
| `query_latency_p50` | 50th percentile latency |
| `query_latency_p99` | 99th percentile latency |
| `query_throughput` | Queries per second |

**Data Structures:**

```rust
pub struct MetricsCollector {
    latency_histogram: LatencyHistogram,
    latency_percentiles: LatencyPercentiles,
    query_counter: AtomicU64,
    error_counter: AtomicU64,
}

pub struct LatencyPercentiles {
    p50: AtomicU64,
    p90: AtomicU64,
    p95: AtomicU64,
    p99: AtomicU64,
    p99_9: AtomicU64,
}
```

#### 5. Audit Module (`audit.rs`)

**Responsibility:** Complete operation audit trail

**Audit Event Structure:**

```rust
pub struct AuditEvent {
    pub event_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub operation: AuditOperation,
    pub severity: AuditSeverity,
    pub result: AuditResult,
    pub table_name: Option<String>,
    pub record_id: Option<String>,
    pub user_role: Option<String>,
    pub sql_statement: Option<String>,
    pub error_message: Option<String>,
    pub execution_time_ms: Option<u64>,
}
```

**Audit Flow:**

```
1. Before operation
   ├─> Generate UUID
   ├─> Record start time
   └─> Log request details

2. Execute operation
   └─> Capture SQL and parameters

3. After operation
   ├─> Record end time
   ├─> Capture result (success/failure)
   ├─> Build AuditEvent
   └─> Persist to audit log
```

#### 6. Cache Module (`cache.rs`)

**Responsibility:** Entity data caching

**Cache Architecture:**

```
CacheManager<T>
├── cache: LruCache<CacheKey, CacheEntry<T>>
├── config: CacheConfig
└── stats: CacheStats

CacheEntry<T>
├── value: T
├── created_at: DateTime<Utc>
├── expires_at: DateTime<Utc>
├── access_count: AtomicU32
└── last_accessed: AtomicU64

CacheConfig
├── capacity: usize (max entries)
├── ttl: Duration (time-to-live)
├── cleanup_interval: Duration
└── enabled: bool
```

**Cache Strategy:**

- **LRU Eviction**: Least recently used entries are evicted first
- **TTL Expiration**: Entries expire after configured time
- **Write-Through**: Cache is updated on writes

#### 7. Sharding Module (`sharding.rs`)

**Responsibility:** Data distribution across shards

**Sharding Strategies:**

```rust
pub enum ShardingStrategy {
    Yearly,    // One shard per year
    Monthly,    // One shard per month
    Daily,      // One shard per day
    Hash,       // Consistent hash sharding
}

pub trait ShardingStrategy: Send + Sync {
    fn get_shard_name(&self, timestamp: DateTime<Utc>) -> String;
    fn calculate_shard(&self, key: &str, total_shards: usize) -> usize;
}
```

**Example: Monthly Sharding**

```rust
impl ShardingStrategy for MonthlyStrategy {
    fn get_shard_name(&self, timestamp: DateTime<Utc>) -> String {
        format!("{}_{}", timestamp.year(), timestamp.month())
    }
}
```

#### 8. Global Index Module (`global_index.rs`)

**Responsibility:** Cross-shard indexing

**Global Index Architecture:**

```
GlobalIndex
├── local_index: LruCache<String, Vec<IndexEntry>>
├── sync_events: Channel<SyncEvent>
├── sync_task: JoinHandle<()>
└── config: GlobalIndexConfig

IndexEntry
├── key: String
├── shard_name: String
├── record_id: String
└── updated_at: DateTime<Utc>
```

**Sync Flow:**

```
1. Write operation on Shard A
   └─> Generate SyncEvent::Insert

2. Publish to sync channel
   └─> Background task picks up event

3. Update global index
   └─> Add/Update index entry

4. Global query
   └─> Query global index
   └─> Route to correct shard(s)
```

---

## Core Components

### 1. Procedural Macros System

**Purpose:** Compile-time code generation for boilerplate reduction

**Macros Provided:**

| Macro | Purpose |
|--------|---------|
| `#[db_entity]` | Map struct to database table |
| `#[db_crud]` | Generate CRUD methods |
| `#[db_permission]` | Generate permission checks |
| `#[db_cache]` | Generate cache annotations |
| `#[db_audit]` | Generate audit annotations |

**Macro Expansion Example:**

**Input:**
```rust
#[derive(DbEntity)]
#[db_entity]
#[table_name = "users"]
#[db_crud]
#[db_permission(roles = ["admin", "manager"])]
struct User {
    #[primary_key]
    id: i64,
    name: String,
}
```

**Generated Code (simplified):**
```rust
impl User {
    // CRUD methods
    pub async fn insert(session: &Session, value: User) -> DbResult<User> { /* ... */ }
    pub async fn find_by_id(session: &Session, id: i64) -> DbResult<Option<User>> { /* ... */ }
    pub async fn update(session: &Session, value: User) -> DbResult<User> { /* ... */ }
    pub async fn delete(session: &Session, id: i64) -> DbResult<()> { /* ... */ }

    // Permission methods
    pub const ALLOWED_ROLES: &[&str] = &["admin", "manager"];
    pub fn check_permission(ctx: &PermissionContext) -> DbResult<()> {
        if !Self::ALLOWED_ROLES.contains(&ctx.role()) {
            return Err(DbError::Permission(...));
        }
        Ok(())
    }

    // Entity methods
    pub const TABLE_NAME: &str = "users";
    pub const PRIMARY_KEY: &str = "id";
}
```

### 2. SQL Parser

**Purpose:** Extract operation type and target table from SQL

**Supported Operations:**

```rust
pub enum SqlOperationType {
    Select,    // SELECT queries
    Insert,    // INSERT statements
    Update,    // UPDATE statements
    Delete,    // DELETE statements
    Ddl,       // CREATE/ALTER/DROP/TRUNCATE
    Dcl,       // GRANT/REVOKE
    Transaction, // BEGIN/COMMIT/ROLLBACK
    Other,      // Everything else
}
```

**Usage:**

```rust
let parser = SqlParser::new();
let (table_name, operation) = parser.parse_operation("SELECT * FROM users WHERE id = 1")?;

// Returns: ("users", SqlOperationType::Select)
```

### 3. Health Check System

**Purpose:** Maintain connection pool health

**Architecture:**

```
Background Task (tokio::spawn)
    │
    ├──> Interval tick (every N seconds)
    │
    ├──> Validate idle connections
    │   ├─> Execute "SELECT 1"
    │   ├─> If valid: keep
    │   └─> If invalid: remove
    │
    └─> Recreate connections to maintain min_connections
```

**Health Check Implementation:**

```rust
pub async fn validate_and_recreate_connections(&self) -> Result<u32, sea_orm::DbErr> {
    let mut invalid_count = 0;

    for conn in idle_connections.drain(..) {
        let is_valid = timeout(Duration::from_secs(2), conn.execute_raw("SELECT 1"))
            .await
            .is_ok_and(|result| result.is_ok());

        if is_valid {
            valid_connections.push(conn);
        } else {
            invalid_count += 1;
        }
    }

    // Recreate to maintain minimum
    let needed = min_connections - valid_connections.len();
    for _ in 0..needed {
        let new_conn = create_connection().await?;
        valid_connections.push(new_conn);
    }

    Ok(invalid_count)
}
```

---

## Data Flow

### Query Flow

```
1. Application calls User::find_by_id(&session, 1)
   │
2. #[db_crud] generated method checks permission
   │
3. Session.check_permission("users", "SELECT")
   │
   ├──> PermissionContext.check_table_access()
   │   ├──> Rate limit check
   │   ├──> LRU cache lookup
   │   └─> Load policy & evaluate
   │
4. Build Sea-ORM query
   │
5. Session.execute(query)
   │
6. SQL parser validates operation type
   │
7. Execute via Sea-ORM
   │
8. Return result
   │
9. Audit log entry (if enabled)
```

### Write Flow (with Transaction)

```
1. Application calls User::insert(&session, user)
   │
2. Session.begin_transaction()
   │
3. Permission check (INSERT on "users")
   │
4. Insert via Sea-ORM
   │
5. Cache invalidation (if caching enabled)
   │
6. Audit log (if audit enabled)
   │
7. Session.commit_transaction()
   │
8. Return success
   │
   (If error: Session.rollback_transaction())
```

---

## Security Architecture

### Defense in Depth

```
Layer 1: Compile-time Guarantees
├── Unsafe code forbidden
├── Database driver mutual exclusion
└── Permission macro validation

Layer 2: Runtime Permission Checks
├── Role-based table access
├── Operation-level permissions
└── Rate limiting on permission checks

Layer 3: SQL Injection Protection
├── Parameterized queries
├── SQL parser validation
├── DDL operation blocking
└── Multi-statement prevention

Layer 4: Configuration Security
├── Path traversal prevention
├── URL whitelist
└── Environment variable sanitization

Layer 5: Audit Trail
├── Complete operation logging
├── User context tracking
└── Error capture
```

### Permission Model

```
Permission Config (YAML)
└── Roles
    ├── admin
    │   └── tables: [*]  (all operations)
    ├── manager
    │   ├── tables: [users, orders]
    │   └── operations: [SELECT, INSERT, UPDATE]
    └── user
        └── tables: [users]
            └── operations: [SELECT]

Permission Check Algorithm:
1. Get role from session
2. Lookup role policy (or use cache)
3. For "*": grant all access
4. For specific table: check operation list
5. Return Allow/Deny
```

---

## Performance Architecture

### Zero-Cost Abstractions

```rust
// Feature-gated compilation
#[cfg(feature = "metrics")]
pub fn track_metric(&self, name: &str, value: u64) {
    // Metrics code
}

#[cfg(not(feature = "metrics"))]
pub fn track_metric(&self, name: &str, value: u64) {
    // No-op - compiled away
}
```

### Lock-Free Counters

```rust
pub struct PoolStatus {
    pub total: AtomicU32,      // Lock-free
    pub active: AtomicU32,     // Lock-free
    pub wait_count: AtomicU32,  // Lock-free
}
```

### Asynchronous Operations

- All I/O uses `async/await`
- `AsyncMutex` for protecting shared state
- `Notify` instead of condition variables (avoids busy waiting)

### Connection Pooling

```
Strategy: Pool + LRU

Benefits:
├── Reuse connections (avoid TCP handshake)
├── Limit maximum connections (prevent exhaustion)
├── Maintain minimum (avoid cold starts)
└── Health checking (remove dead connections)
```

---

## Scalability Architecture

### Horizontal Scaling (Sharding)

```
Application
    │
    ├─> ShardRouter
    │   ├──> YearlyStrategy → shard_2024
    │   ├──> MonthlyStrategy → shard_2024_01
    │   └─> HashStrategy → shard_{hash(key) % N}
    │
    └─> Query Routing
        └─> Global Index (optional)
            └─> Route to correct shard
```

### Vertical Scaling (Caching)

```
Query Request
    │
    ├─> Check Cache
    │   ├──> Hit: Return cached value
    │   └─> Miss: Continue
    │
    ├─> Database Query
    │
    └─> Update Cache (write-through)
```

### Feature-Based Scaling

```
Minimal Deployment:
├── SQLite
├── config-env
├── lru
└── sql-parser

Microservice Deployment:
├── PostgreSQL
├── permission
├── pool-health-check
└── config-yaml

Enterprise Deployment:
├── All optional features
├── metrics
├── tracing
├── audit
└── sharding
```

---

## Conclusion

DBNexus architecture is designed with:

1. **Modularity** - Clear separation of concerns, feature-gated
2. **Safety** - RAII, compile-time guarantees, no unsafe code
3. **Performance** - Async-first, lock-free where possible, efficient pooling
4. **Security** - Multi-layer defense, RBAC, audit trail
5. **Extensibility** - Pluggable components, trait-based design
6. **Observability** - Metrics, tracing, audit logging built-in

This architecture enables DBNexus to scale from embedded devices to enterprise deployments while maintaining simplicity and ergonomics.

For more details on specific components, see:
- [API Reference](API_REFERENCE.md)
- [User Guide](USER_GUIDE.md)
- [Feature Documentation](FEATURES.md)
