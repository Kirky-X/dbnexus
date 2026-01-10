# DBNexus Architecture

<div align="center">

**Enterprise-grade database abstraction layer architecture**

</div>

## Overview

DBNexus is built on a modular architecture that separates concerns while maintaining tight integration between components. The architecture follows a layered approach with clear boundaries between core functionality and optional extensions.

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Application Layer                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │   CLI Tool   │  │   Examples   │  │   Tests      │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
│                                                                  │
├──────────────────────────┬─────────────────────────────────────┤
│                          │                                     │
│  ┌──────────────────────▼──────────────────────┐               │
│  │              Core Library (dbnexus)          │               │
│  │  ┌─────────────────────────────────────────┐│               │
│  │  │              Connection Pool             ││               │
│  │  │        (DbPool, Session Management)      ││               │
│  │  └─────────────────────────────────────────┘│               │
│  │                                             │               │
│  │  ┌─────────────────────────────────────────┐│               │
│  │  │            Permission Engine             ││               │
│  │  │     (Role Policies, Access Control)      ││               │
│  │  └─────────────────────────────────────────┘│               │
│  │                                             │               │
│  │  ┌─────────────────────────────────────────┐│               │
│  │  │            Audit System                  ││               │
│  │  │      (Logging, Sanitization, Alerts)     ││               │
│  │  └─────────────────────────────────────────┘│               │
│  │                                             │               │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  │               │
│  │  │ Caching  │  │Metrics   │  │ Tracing  │  │               │
│  │  └──────────┘  └──────────┘  └──────────┘  │               │
│  └──────────────────────────┬──────────────────┘               │
│                             │                                    │
├─────────────────────────────▼────────────────────────────────────┤
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │                     Sea-ORM Layer                           │  │
│  │     (Query Builder, Transaction Management, Migrations)     │  │
│  └────────────────────────────────────────────────────────────┘  │
│                                                                  │
├─────────────────────────────▼────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │   SQLite     │  │  PostgreSQL  │  │    MySQL     │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. Connection Pool (pool.rs)

The connection pool manages database connections efficiently:

- **DbPool**: Main pool orchestrator
  - Configuration management
  - Connection creation and recycling
  - Background health checks
  - Graceful shutdown

- **Session**: Individual connection wrapper
  - Permission context attachment
  - Transaction management
  - Query execution

**Key Features**:
- Configurable min/max connections
- Idle connection timeout
- Connection validation on acquire
- Automatic reconnection for maintaining min connections

### 2. Permission Engine (permission.rs, permission_engine.rs)

Role-based access control system:

- **PermissionConfig**: YAML-based policy definition
- **RolePolicy**: Role to table/operation mappings
- **PermissionContext**: Runtime permission checks
- **RateLimiter**: Request rate limiting (100 req/60s default)

**Permission Model**:
```yaml
roles:
  <role_name>:
    - table: <table_name|*>
      operations: [SELECT, INSERT, UPDATE, DELETE]
```

### 3. Audit System (audit.rs)

Comprehensive audit logging:

- **AuditEvent**: Individual audit records
- **AuditLogger**: Event logging interface
- **AuditStorage**: Storage backends (memory, future: database)
- **Data Sanitization**: Automatic sensitive data redaction

**Logged Operations**:
- CREATE, READ, UPDATE, DELETE
- DDL operations
- Permission denials

### 4. Metrics (metrics.rs)

Prometheus-compatible metrics collection:

- **PoolMetrics**: Connection pool statistics
- **LatencyStorage**: Query latency tracking
- **Percentile Calculation**: P50, P95, P99 latencies

### 5. Sharding (sharding.rs)

Horizontal scaling support:

- **ShardRouter**: Request routing to shards
- **ShardConfig**: Per-shard configuration
- **GlobalIndex**: Cross-shard queries

## Configuration Flow

```
Application Config
       │
       ▼
┌──────────────┐
│  DbConfig    │ ─────► ConfigCorrector (validation/fixes)
└──────────────┘              │
       │                      ▼
       │              ┌──────────────┐
       │              │  DbPool      │
       │              └──────────────┘
       │                     │
       ▼                     ▼
┌──────────────�     ┌──────────────┐
│Permissions   │     │  Sessions    │
│(YAML Config) │────►│(with Context)│
└──────────────┘     └──────────────┘
```

## Error Handling

DBNexus uses a hierarchical error system:

```
DbError (top-level)
 ├── Config (configuration errors)
 ├── Connection (connection issues)
 ├── Permission (access denied)
 ├── Migration (migration failures)
 └── Internal (unexpected errors)
```

## Concurrency Model

- **Async Runtime**: Tokio multi-threaded runtime
- **Connection Pool**: Arc<Mutex> for thread-safe access
- **Permission Cache**: LRU cache with async mutex
- **Rate Limiter**: RwLock for concurrent reads/writes

## Performance Considerations

### Connection Pool Sizing

```
max_connections = (CPU cores * 2) + disk_count
min_connections = 25% of max_connections
```

### Query Optimization

- Prepared statements for repeated queries
- Connection validation with 2-second timeout
- Rate limiting to prevent abuse

### Memory Management

- Sliding window for latency samples (max 10,000)
- LRU cache for permission policies (256 entries default)
- Automatic cleanup of idle connections

## Security Measures

1. **SQL Injection Prevention**: Parameterized queries throughout
2. **Rate Limiting**: 100 requests/60s per role
3. **Data Sanitization**: Automatic password/secret redaction
4. **Audit Logging**: Security events with target="security"
5. **Configurable Admin Role**: DB_ADMIN_ROLE environment variable

## Extension Points

### Custom Storage Backends

Implement the `AuditStorage` trait for custom audit storage.

### Custom Health Checks

Extend `validate_and_recreate_connections` for database-specific checks.

### Custom Sharding Strategies

Implement `ShardRouter` for application-specific routing logic.

## Dependencies

### Core Dependencies

- **tokio**: Async runtime
- **sea-orm**: ORM and query builder
- **tracing**: Structured logging
- **serde**: Serialization

### Optional Dependencies

- **prometheus**: Metrics export
- **opentelemetry**: Distributed tracing
- **sqlx**: Alternative database driver support

## Version

0.1.0

## Authors

DBNexus Team
