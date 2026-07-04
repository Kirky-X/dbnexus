# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2026-07-03

### ⚠️ BREAKING CHANGES

- **版本号升级**: workspace 版本号 `0.2.0 → 0.3.0`（含 `Cargo.toml` + `macros/Cargo.toml` + `examples/Cargo.toml`）
- **README 重命名**: `README.md → README_EN.md`，`README_zh.md → README.md`（中文为主 README）
- **编译期互斥规则修复**: `sqlite`/`postgres`/`mysql` 现在真正互斥（原规则允许同时启用，与文档"Must enable exactly one"矛盾）
- **`observability` 预设修正**: 从 `["metrics", "health-check"]` 改为 `["metrics", "health-check", "tracing"]`（与 README 描述一致）
- **`dbentity-v2-rewrite` 变更归档**: 88/88 任务完成，归档到 `openspec/changes/archive/`

### Added

#### 新特性

- **`duckdb` feature**: DuckDB OLAP 后端支持，基于 `duckdb` crate（同步）+ `tokio::task::spawn_blocking` 桥接，作为分析只读旁路接入（绕过 sea-orm，因 sea-orm 2.0.0-rc.37 不支持 DuckDB）
  - 新增 `DbConnection::DuckDb` 枚举变体
  - 新增 `DuckDbConnection` 连接包装器，支持 `execute()`/`query()` 异步接口
  - `DatabaseType::DuckDb` 支持 `duckdb:` URL 前缀解析
- **`tracing` feature**: OpenTelemetry 分布式追踪，基于 `tracing` + `tracing-opentelemetry` + `opentelemetry-otlp`
  - 新增 `TracingGuard::init_with_otlp()` API
  - 集成 `tracing` 模块导出
- **结构化错误报告**: `QueryErrorReport` 按"权限不足/注入风险/语法错误/分片冲突"分类，含修复建议
- **连接串智能解析**: 用 `url` crate 替代 `starts_with("sqlite:")` 硬编码前缀匹配，支持标准 URI 格式
- **权限缓存 TTL + 动态热加载**: `PermissionCache` 新增 `expire_after` + 后台异步刷新任务
- **分片路由集成 Session**: `pool.get_session_for_shard(key, role)` 自动路由到对应分片

#### 工程化护栏

- `.github/dependabot.yml`: cargo + github-actions 依赖自动更新 PR
- `.github/codeql.yml`: 语义级安全扫描工作流
- `.editorconfig`: 跨编辑器一致性（Rust 4 空格缩进、UTF-8、LF 换行）
- `.pre-commit-config.yaml`: 集成 cargo-fmt / cargo-clippy / cargo-deny Rust 专有 hook

#### 测试覆盖

- `tests/authentication/`: AuthenticationManager/JwtManager/PasswordHasher 外部测试
- `tests/security/sensitive_masker_tests.rs`: SensitiveMasker/MaskType 外部测试（含 Unicode 安全测试）
- `tests/kit/kit_integration_tests.rs`: DbNexusKit 外部测试
- `benches/permission_engine_bench.rs`: 权限引擎基准测试
- `benches/sharding_bench.rs`: 分片路由基准测试
- `benches/metrics_bench.rs`: 指标收集基准测试

### Changed

- **`cache` feature 描述修正**: 从 "LRU" 改为 "oxcache（内部 moka L1 后端）"
- **协议兼容数据库文档化**: CockroachDB/YugabyteDB/TiDB/MariaDB/Aurora 无需代码改动即可使用，在 README 中明确说明

### Performance

- **DuckDB 连接池化**: 替换 DuckDB 单一 Mutex 为连接池（`DuckDbConnection`），支持 `with_pool_size(url, pool_size)` 并发查询
- **Session 短锁模式**: 避免 `Session` 持锁期间执行 async DB 调用，降低锁竞争
- **MetricsCollector 原子化**: 移除不必要的 `RwLock`，改用原子操作释放性能
- **SqlParser 全局共享单例**: `SqlParser::shared().await` 返回 `Arc<SqlParser>`，避免重复创建解析器实例

### Fixed

#### 正确性修复（diting 审查）

- **`mask_email` UTF-8 字节切片 panic**: 非 ASCII 本地部分（中文/emoji）按字节切片会 panic，改用 `chars()` 安全处理
- **`ShardRouter::default()` 除零 panic**: 默认 `total_shards=0` 导致 `% total_shards` 除零，改为 `total_shards=1`（单一分片语义）+ 防御性检查
- **`shard_id_for_key` 哈希器误用**: `shard_key.hash(&hasher)` 应为 `&mut hasher`（预存编译错误）
- **DuckDB `permit` 提前 drop**: `execute()`/`query()` 中 permit 在 `handle.await` 前 drop，信号量在 spawn_blocking 任务完成前释放，失去并发限制作用
- **`warmup_connections` 静默丢弃失败**: 全部失败时仍返回 `Ok(())`，改为全部失败返回 `Err` + 部分失败 warn 日志（规则 12）
- **`validate_role_name` 静默 fail-open**: 无权限配置时静默使用安全默认策略，添加 warn 日志显性化（规则 12，CRITICAL 风险保守修复）

#### 安全修复（diting 审查）

- **`verify_token` 不校验 `token_type`**: refresh token 可用作 access token（权限提升风险），新增 `verify_access_token`/`verify_refresh_token` 方法额外校验 `token_type`
- **`add_user` 无法验证密码强度**: 接收已哈希 `password_hash`，新增 `register_user(username, password, role)` 方法执行 `validate_strength → hash → insert` 完整流程

#### 安全审查修复（tiangang SAST + diting 6 CRITICAL/5 HIGH）

- **DuckDB SQL 注入防护**: `execute_duckdb_raw` 添加 admin role + DdlGuard AST 验证（对齐 `execute_raw_ddl` 行为），非 admin role 拒绝 DDL
- **令牌桶竞态修复**: RateLimiter 内部状态更新改为原子操作
- **`try_from` panic 修复**: `DbPool::try_from` 在 `permission` feature 启用时返回错误而非 panic
- **singleflight 违约修复**: 权限缓存 singleflight 协调逻辑修正
- **`eprintln!` 替换**: 所有 `eprintln!` 替换为 `tracing::warn!`/`tracing::error!`
- **`sql-parser` 依赖加固**: 显式声明 sql-parser 依赖防止权限绕过
- **假 DI setter 修复**: 移除误导性的依赖注入 setter
- **`Engine*` 双重导出修复**: 消除类型重复导出
- **空 `impl_` 目录清理**: 移除空实现目录
- **冗余预加载移除**: 优化启动时无用的预加载逻辑
- **`deny.toml` 添加 RUSTSEC-2023-0071**: 安全公告加入 cargo-deny 策略

#### 最终回归修复（T024）

- **DuckDB DDL 设计 bug**: `execute_duckdb_raw` 文档声明支持 DDL 但代码无条件拒绝，修复为 admin role + DdlGuard AST 验证后执行（对齐 `execute_raw_ddl`）
- **DuckDB 健康检查查询失败**: `SELECT 1 AS health` 因 `parse_operation_async` 返回 `Ok(None)` 被拒绝，修复为 admin role 允许执行（对齐 `execute` 的 None 路径）
- **`sqlite3://` scheme 缺失**: `DatabaseType::from_url` 不支持 `sqlite3://` 格式，添加支持
- **sharding flaky test 修复**: `test_shard_router_key_consistency` 假设两个特定 key 必映射不同 shard（10% 碰撞概率），改为验证确定性属性 + 统计分布属性

### Maintenance（v0.3.0 发版前加固）

#### 依赖升级（9 个 Cargo 依赖）

- **sqlparser**: 0.47 → 0.62（破坏性 API：`Statement` 枚举变体改为元组结构体，更新 `classify_statement` 与 `statement_type_name` 的模式匹配）
- **opentelemetry**: 0.27 → 0.32（`TracerProvider`/`Span` 等 API 重构）
- **opentelemetry_sdk**: 0.27 → 0.32
- **opentelemetry-otlp**: 0.27 → 0.32
- **tracing-opentelemetry**: 0.28 → 0.33
- **criterion**: 0.5 → 0.8（修复 `BenchmarkGroup` deprecation）
- **toml**: 1.0 → 1.1
- **parking_lot**: 0.12 patch 升级
- **url**: 2.5 patch 升级

#### CI 加固（9 个 GitHub Actions 升级 + 2 个不存在 action 修复）

- **actions/checkout**: v4 → v7（5 处）
- **actions/cache**: v4 → v6（3 处）
- **codecov/codecov-action**: v4 → v7
- **github/codeql-action/init**: v3 → v4
- **github/codeql-action/analyze**: v3 → v4
- **softprops/action-gh-release**: v2 → v3（2 处）
- **swatinmurthy/cache-for-rust@v1** → **Swatinem/rust-cache@v2**（4 处，原 action 仓库不存在，GitHub API 返回 404）
- **actions/attest-release-assets@v1** → **actions/attest-build-provenance@v3**（原 action 不存在，GitHub API 返回 404）
- **Swatinem/rust-cache 参数修复**: 移除不支持的 `path`/`restore-keys` 参数，改用 `shared-key` 区分缓存命名空间

#### CI 验证规则增强

- **clippy `--all-features` 修复**: lint job 缺少 `--all-features` 参数，feature-gated 代码从未被 lint
- **cargo doc check 新增**: lint job 新增 `cargo doc --all-features --no-deps --workspace`，提前捕获 rustdoc 警告

#### 文档警告修复（28 个 rustdoc 警告）

- **`src/domain/migration/converter.rs`**: 28 个 "unclosed HTML tag" 警告修复。markdown 表格中的泛型类型（如 `Option<TableRef>`、`Vec<ColumnDef>`、`Char(Option<u32>)`）未用反引号包裹，rustdoc 误判为 HTML 标签起始。全部加反引号包裹。

## [0.2.0] - 2026-06-27

### ⚠️ BREAKING CHANGES - ALL USERS MUST UPDATE

This is a **major breaking change** that affects **100% of users**. There is **NO backward compatibility** and **NO automatic migration path**.

#### Core Changes

- **Removed from default features**: `cache` is no longer enabled by default
- **Feature dependencies**: `permission`, `permission-engine`, and `sql-parser` now **require** `cache` feature
- **Compilation failure**: Code will not compile without correct feature flags
- **`foundation::pool` deprecated**: All database operations must use `database::pool` for Session management
- **Legacy `db_crud` macro removed**: Replaced by the unified `#[db_entity]` macro with explicit `primary_key` specification

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

- **`foundation::pool`**: Deprecated; all database operations must use `database::pool` for Session management.

- **Legacy `db_crud` macro**: Removed; replaced by the unified `#[db_entity]` macro with explicit `primary_key` specification.

- **`minimal` preset**: Replaced by `embedded` (different feature set).

- **Implicit cache dependency**: Cache is now optional and must be explicitly enabled.

- **Fallback behaviors**: No fallback or no-op implementations — compilation fails if required features are missing.

### Added

- **`#[db_entity]` unified macro** (`macros`): Single attribute macro replacing 5 separate macros (`db_entity`, `db_crud`, `db_audit`, `db_cache`, soft_delete). Supports `table_name`, `primary_key`, `timestamps`, `soft_delete`, `validate`, `hooks(...)` parameters.
  - **`schema(backend)`**: Generates `migration::schema::Table` from Entity via `sea_orm::Schema::create_table_from_entity` with custom converter.
  - **`query(session)`**: Returns Sea-ORM native `Select<E>` for chaining `filter/order_by/limit`.
  - **`paginate(session, page_size)`**: Returns Sea-ORM paginator with `num_items/num_pages/fetch_page`.
  - **`insert_many(session, models)`**: Batch insert returning `InsertResult` with `last_insert_id`.
  - **`update_many(session, filter, updates)`**: Conditional batch update returning affected row count.
  - **`find_with_deleted(session)`**: Find including soft-deleted records.
  - **`find_only_deleted(session)`**: Find only soft-deleted records.
  - **`force_delete(session, id)`**: Hard delete bypassing soft delete.
  - **`timestamps = true`**: Auto-manages `created_at` / `updated_at` via `ActiveModelBehavior::before_save` — insert sets both, update sets only `updated_at`.
  - **`validate`**: Integrates `validator` crate with `#[derive(Validate)]` and field-level `#[validate(...)]` attributes. Validation runs before timestamps in `before_save`.
  - **`hooks(...)`**: 6 event hooks — `before_insert`/`after_insert`/`before_update`/`after_update`/`before_delete`/`after_delete`. Hook orchestration order: `validate → timestamps → user_hooks` (any failure short-circuits).
  - **`soft_delete = true`**: Rewrites `find*` and `delete` semantics to auto-filter/set `deleted_at` column. Auto-injects `deleted_at: Option<time::OffsetDateTime>` field.

- **`with-time` feature**: Enables `sea-orm/with-time` dependency for `OffsetDateTime` timestamp support. Included in default features.

- **`validation` feature**: Gates `validator` crate with `features = ["derive"]` for `#[derive(Validate)]` macro support.

- **New combined features**:
  - **`cache`**: Independent cache feature (was implicitly enabled, now explicit)
  - **`observability`**: Combined feature for metrics + tracing + health-check
  - **`data-management`**: Combined feature for migration + auto-migrate + sharding + global-index
  - **`security`**: Combined feature for audit + permission-engine
  - **`bench`**: Performance testing dependencies (criterion)
  - **`test-utils`**: Testing utilities (tempfile, assert_cmd)

- **New presets**:
  - **`embedded`**: Ultra-minimal (runtime-tokio-rustls, sqlite, config-env) for embedded/edge devices
  - **`microservice`**: Optimized for microservice deployment (postgres, permission, sql-parser, config-env, observability)
  - **`monolith`**: Complete for monolithic applications (postgres, permission, sql-parser, config-yaml, data-management, security, observability)
  - **`enterprise`**: Full enterprise features (postgres + monolith + permission-engine)

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

- **Macro validation logic**: Fixed bug where `validate` + `timestamps = true` combination destroyed `ActiveValue::Set` state during `before_save`. Validation now clones `ActiveModel` for read-only validation, preserving original `Set/Unchanged/NotSet` states so UPDATE operations correctly persist changed fields.

- **Sea-ORM 2.0 `ActiveModelBehavior` signatures**: `after_save` receives `Model` (not `ActiveModel`); `after_delete` takes `self` by value and returns `Result<Self, DbErr>`.

- **Hook function signatures**: `before_*` must be `fn(&mut ActiveModel) -> Result<(), E>`, `after_*` must be `fn(&Model) -> Result<(), E>`.

- **Feature reorganization**:
  - **`permission`**: Now requires `cache` feature
  - **`sql-parser`**: Now requires `cache` feature
  - **`permission-engine`**: Now requires `cache` feature
  - **`minimal` preset**: Removed and replaced with `embedded` (truly minimal, no caching)

- **Dependency updates**:
  - **`oxcache`**: Changed to optional dependency (was required)
  - **`regex`**: Removed duplicate declarations across features
  - **`once_cell`**: Removed duplicate declarations across features

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
