# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Removed

- **`confers` configuration framework**: Removed the optional `confers` dependency and the `confers` feature flag entirely. The project now relies on `serde` / `serde_yaml_ng` / `serde_json` for direct deserialization.
  - Removed `confers = ["dep:confers"]` feature and the `confers` path dependency from `Cargo.toml`.
  - Removed `confers/yaml` and `confers/json` from the `config-yaml` feature; the `yaml` feature now drives YAML support directly.
  - Removed `confers/yaml` from the `permission-engine` feature.
  - Removed `confers` from the `required-features` of the `core_session_transaction` and `cross_cutting_benchmarks` test targets.
  - Removed `CacheConfig::from_confers`, `DbConfig::from_confers`, and `PermissionConfig::from_confers`.
  - Added `CacheConfig::from_yaml_str` / `CacheConfig::from_json_value`, `DbConfig::from_yaml_str` / `DbConfig::from_json_str`, and `PermissionConfig::from_yaml_str` / `PermissionConfig::from_json_str` as serde-based replacements.
  - Removed `#[cfg(feature = "confers")]` gates from `DbPool::load_permission_config` / `parse_permission_yaml` / `parse_permission_json`, `YamlPermissionProvider::new` / `parse_yaml_content` / `refresh`, `parse_permission_yaml_async`, and `permission_engine::YamlPermissionProvider::load_config`. The corresponding `#[cfg(not(feature = "confers"))]` Err branches were deleted.
  - Deleted `tests/confers_oxcache_integration.rs` (all tests were `#[ignore]`d and referenced an unimplemented `with_confers()` method).
  - Updated `enterprise` preset to no longer include `confers`.

### Added

- **Cache stampede protection** (`permission`): `PermissionContext` now uses singleflight request coalescing to prevent thundering herd when multiple requests hit an uncached role simultaneously. Concurrent cache-miss requests are collapsed into a single load; followers wait for the leader's result. A `stampede_events` counter tracks how many times coalescing occurred. New `get_cache_metrics()` method returns `(hit_rate, miss_count, stampede_count, cache_size)`.

- **Connection pool alerting** (`pool`): `acquire_connection` now tracks `wait_count` (current waiters) and `max_waiters` (historical peak) with CAS-safe updates. Timeout paths trigger tiered log alerts: warn ≥3s, error ≥5s, critical ≥10s.

- **Acquire duration histogram** (`metrics` feature): `MetricsCollector` now records `connection_acquire_duration` histograms with 100ms/500ms/1s/3s/5s/10s buckets. Slow acquires (>1s) increment `slow_acquires` counter. Timeout events are classified by level and counted separately.

- **Prometheus metrics export** (`metrics` feature): `MetricsCollector` now exports Prometheus-format metrics via `to_prometheus()`:
  - `dbnexus_pool_connections_total / active / idle` (gauges)
  - `dbnexus_connection_acquire_slow_total` (counter)
  - `dbnexus_connection_timeout_total{level="warn|error|critical"}` (counter)
  - `dbnexus_pool_acquire_duration_seconds` (histogram)

- **Rate limiter burst capacity**: `RateLimiter::new()` now accepts a `burst_capacity: u32` parameter (default = `max_requests`) to allow initial token count to exceed the steady-state refill rate. New `update_config(max_requests, window_duration)` method for runtime reconfiguration. New `with_defaults(max_requests, window_duration, max_buckets)` convenience constructor.

### Changed

- **Performance**: All metrics recording in `acquire_connection` is gated behind `#[cfg(feature = "metrics")]` — zero overhead when the feature is disabled.

## [0.2.0] - 2026-02-04

### ⚠️ BREAKING CHANGES - ALL USERS MUST UPDATE

This is a **major breaking change** that affects **100% of users**. There is **NO backward compatibility** and **NO automatic migration path**.

#### Core Changes

- **Removed from default features**: `cache` is no longer enabled by default
- **Feature dependencies**: `permission`, `permission-engine`, and `sql-parser` now **require** `cache` feature
- **Compilation failure**: Code will not compile without correct feature flags

#### Required Action for All Users

**Before (v0.1.x):**
```toml
dbnexus = "0.1"
# or
dbnexus = { version = "0.1", features = ["postgres"] }
```

**After (v0.2.0) - Choose ONE:**

```toml
# Option 1: Use presets (RECOMMENDED)
dbnexus = { version = "0.2", features = ["microservice"] }

# Option 2: Explicit features
dbnexus = { version = "0.2", features = [
    "postgres",
    "permission",
    "cache",
    "observability"
] }

# Option 3: Ultra-minimal (embedded)
dbnexus = { version = "0.2", features = ["embedded"] }
```

### Added

#### New Features

- **`cache`**: Independent cache feature (was implicitly enabled, now explicit)
- **`observability`**: Combined feature for metrics + tracing + health-check
- **`data-management`**: Combined feature for migration + auto-migrate + sharding + global-index
- **`security`**: Combined feature for audit + permission-engine
- **`bench`**: Performance testing dependencies (criterion)
- **`test-utils`**: Testing utilities (tempfile, assert_cmd)

#### New Presets

- **`embedded`**: Ultra-minimal configuration for embedded/edge devices
  - Features: runtime-tokio-rustls, sqlite, config-env
  - Use case: Resource-constrained environments

- **`microservice`**: Optimized for microservice deployment
  - Features: postgres, permission, sql-parser, config-env, observability
  - Use case: Containerized microservices

- **`monolith`**: Complete configuration for monolithic applications
  - Features: postgres, permission, sql-parser, config-yaml, data-management, security, observability
  - Use case: Traditional monolithic architectures

- **`enterprise`**: Full enterprise features
  - Features: postgres + monolith + permission-engine
  - Use case: Large-scale enterprise applications

### Changed

#### Feature Reorganization

- **`permission`**: Now requires `cache` feature
- **`sql-parser`**: Now requires `cache` feature
- **`permission-engine`**: Now requires `cache` feature
- **`minimal` preset**: Removed and replaced with `embedded` (truly minimal, no caching)

#### Dependency Updates

- **`oxcache`**: Changed to optional dependency (was required)
- **`regex`**: Removed duplicate declarations across features
- **`once_cell`**: Removed duplicate declarations across features

### Removed

- **`minimal` preset**: Replaced by `embedded` (different feature set)
- **Implicit cache dependency**: Cache is now optional and must be explicitly enabled
- **Fallback behaviors**: No fallback or no-op implementations - compilation fails if required features are missing

### Migration Guide

#### Step 1: Update Version

```toml
# In your Cargo.toml
[dependencies]
dbnexus = "0.2"  # Update from 0.1.x
```

#### Step 2: Choose a Configuration

**For most users:**
```toml
dbnexus = { version = "0.2", features = ["microservice"] }
```

**For embedded/edge devices:**
```toml
dbnexus = { version = "0.2", features = ["embedded"] }
```

**For full enterprise features:**
```toml
dbnexus = { version = "0.2", features = ["enterprise"] }
```

**For custom configuration:**
```toml
dbnexus = { version = "0.2", features = [
    "postgres",     # or mysql/sqlite
    "permission",    # requires cache
    "cache",         # REQUIRED by permission
    "observability"
] }
```

#### Step 3: Build and Test

```bash
cargo clean
cargo build
# If you get compilation errors about missing features,
# add the required features to your Cargo.toml
```

### Important Notes

- **No automatic migration**: You must manually update Cargo.toml
- **No compatibility layers**: v0.1.x and v0.2.0 are completely incompatible
- **Feature combinations**: Ensure `cache` is enabled if using `permission`, `permission-engine`, or `sql-parser`
- **Compilation errors**: If compilation fails, the error message will indicate which feature is required

### Performance Impact

**Without cache feature:**
- Binary size: Reduced by 5-10%
- Compile time: Reduced by 15-20%
- Runtime performance: May be significantly slower (100x for permission checks, 10x for SQL parsing)

**Recommendation**: Enable `cache` feature for production use unless targeting embedded devices.

### Testing

All existing tests have been updated to use correct feature combinations. Test each preset before deploying:

```bash
# Test embedded preset
cargo test --features "embedded"

# Test microservice preset
cargo test --features "microservice"

# Test all features
cargo test --all-features
```

### Support

For migration assistance, see:
- [Feature Flags Documentation](#feature-flags)
- [Examples](examples/)
- [GitHub Issues](https://github.com/Kirky-X/dbnexus/issues)

---

## [0.1.2] - Previous Release

### Features
- Connection pooling with RAII lifecycle management
- Permission control (RBAC)
- Procedural macros for CRUD and permission checks
- SQL parser
- Transaction support
- Multi-database support (SQLite, PostgreSQL, MySQL)
- Enterprise features (metrics, tracing, audit, migration, sharding, etc.)

### Known Issues
- Cache feature was implicitly enabled, could not be disabled
- Documentation inconsistencies with Cargo.toml
- No practical presets for common use cases
