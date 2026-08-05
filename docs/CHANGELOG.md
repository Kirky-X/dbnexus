# 更新日志

本项目所有显著变更都将记录在此文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
本项目遵循 [语义化版本](https://semver.org/lang/zh-CN/spec/v2.0.0.html)。

## [Unreleased]

## [0.5.0] - 2026-08-04

### Added

- **分布式能力示例（7 个新示例）**：补全 `examples/` 中缺失的核心功能模块示例
  - `distributed_id`：Snowflake ID 生成器，批量生成、ID 解析、多线程并发、错误场景
  - `saga`：Saga 分布式事务编排，自定义 SagaAction、成功/失败+补偿场景、SagaLog
  - `scatter_gather`：跨分片 Scatter-Gather 查询，聚合函数、PartialFailurePolicy
  - `replica_routing`：副本路由读写分离、ReplicaConfig、FailoverConfig 协同
  - `shard_migration`：分片迁移编排器、并行/串行模式、OrchestratedMigrationResult
  - `retry`：运行时重试+指数退避、幂等性判断、自定义策略、重试耗尽
  - `failover`：连接故障转移链、CircuitBreaker 状态机、与 DbConfig 集成
- 新增 `distributed-capabilities` 聚合 feature 的 examples 透传

### Fixed

- **连接池测试断言同步**：`test_pool_status_and_config`、`test_connection_pool_trait_methods` 更新断言以匹配池预创建 min_connections 行为
- **DuckDB 测试特性守卫**：`test_create_connection_duckdb_not_enabled` 添加 `#[cfg(not(feature = "duckdb"))]` 避免特性启用时误跑
- **Kit 健康检查测试同步**：`health_check_unhealthy_before_first_use`、`full_kit_with_lifecycle_health_observer` 更新预期以匹配池预创建连接后 Healthy 状态
- **文档幽灵条目清理**：移除 `examples/README.md` 中不存在的 `tracing_otlp` 条目

### Changed

- `docs/API_REFERENCE.md`：新增分布式能力模块 API 文档
- `docs/ARCHITECTURE.md`：更新架构图反映分布式能力模块

## [0.4.4] - 2026-07-23

### Changed

- 依赖版本约束移除波浪号（`~`）：`dashmap ~6.2`→`6.2`、`jsonwebtoken ~10`→`10`、`bcrypt ~0.19`→`0.19`，统一为 Major.Minor 精确格式
- 依赖刷新：`cargo update` 将 lockfile 锁定到最新兼容版本（含 oxcache 0.3.9→0.3.12、trait-kit 0.3.0→0.3.1、tokio 1.52.4→1.53.1、uuid 1.23.5→1.24.0、duckdb 1.10504→1.10505、arrow 58.3→58.4 等）

### Security

- `cargo deny check advisories bans` 通过，无已知漏洞（unmaintained 警告为既有传递依赖，已在 deny.toml 中登记）

### 说明

- 回归测试采用项目 CI 支持的特性组合 `--no-default-features --features sqlite,default-no-db,all-optional`（覆盖 dashmap/jsonwebtoken/bcrypt 所在的 cache/security 特性），全部通过；`--all-features` 因 duckdb 与 lbug 各自捆绑 mbedtls 导致重复符号链接冲突，属既有架构限制（CI 从不使用 `--all-features`），非本次依赖变更引入

## [0.4.3] - 2026-07-22

### 修复

- **[fix-cache-key]** `#[db_entity]` 宏生成的 `cache_key(id: i64)` 从硬编码 i64 改为泛型 `cache_key<PK: Display>(id: PK)`，修复 Uuid/String 主键实体启用 `cache(...)` 时调用 `cache_key` 的编译错误（0.4.2 标注的已知限制）。新增 3 个回归测试覆盖 Uuid/i64/String 三种主键类型的 cache_key 调用

### 测试

- 新增 `tests/e2e_advanced.rs`（89 个测试）：覆盖 SensitiveMasker(11)、CircuitBreaker(12)、ShardRouter(13)、GlobalIndex(10)、Authentication(14)、i18n(16)、Tracing(5)、PoolHealthMetrics(13) 共 8 个模块的边界与异常场景。2 个测试标记为 `#[ignore]`（需 OTLP collector）

### 维护

- 移除未使用依赖：toml（main）、serde（macros）、serde/time/toml（cli）等

## [0.4.2] - 2026-07-22

### 修复

- **[fix-pk-types]** `#[db_entity]` 宏生成的主键参数从硬编码 `i64` 改为泛型，修复 Uuid/String 主键实体调用时 `the trait bound 'uuid::Uuid: From<i64>' is not satisfied` 编译错误。具体约束：`find_by_id` / `delete`（soft_delete=false 分支）改为 `PK: Into<<<Entity as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType>`（对接 `Entity::find_by_id`）；`find_by_ids` / `delete`（soft_delete=true 分支）/ `force_delete` 改为 `PK: Into<sea_orm::Value>`（对接 `Column::eq` / `is_in`）。完全向后兼容（i64 主键无需修改调用代码）。新增 `tests/entity/unit/db_entity_pk_types_test.rs` 11 个回归测试覆盖 i64/Uuid/String 三种主键 × 编译期/签名/ActiveModel/soft_delete=true 分支验证
- **[known-limitation]** `cache_key(id: i64)` 仍硬编码 i64（与本次泛型化方向不一致），Uuid/String 主键实体启用 `cache(...)` 时调用 `cache_key` 会编译失败。此问题超出本次修复范围（规则 6 外科手术式修改），计划在 0.4.3 修复

### 安全修复

- **[vuln-0001]** 为 admin 绕过操作新增审计日志，并对默认 `admin_role` 发出警告
- **[vuln-0002]** `add_user` 校验 bcrypt hash 格式，拒绝畸形密码哈希
- **[vuln-0003]** 表名提取从朴素字符串匹配替换为 `SqlParser`，消除注入隐患
- **[vuln-0004]** 密码策略增强：引入黑名单 + 复杂度要求
- **[vuln-0005]** 新增 `execute_cypher_with_params` 并废弃裸 Cypher 执行，加入注入防护
- **[vuln-0006]** 放宽 `jsonwebtoken` 约束至 `~10`，接纳 CVE-2026-25537 补丁版本

### 重构

- graph/pool：`HD-1` `execute_cypher_with_params` 默认实现返回错误而非静默回退（Liskov 注入风险消除）
- auth：`HD-3` `add_user_unchecked` 强化 `pub(crate)` 约束文档 + 职责分离测试；`HD-4` `PasswordPolicy` 可配置 + 自定义黑名单
- pool：`HD-5`/`MD-4` 抽出 `audit.rs` 分离安全审计逻辑；`MD-1` 抽出 `execute_cypher_in_transaction` 复用图事务分发；`MD-3`/`LD-4` 简化 `batch_execute_in_transaction`；`LD-2` `execute_with_operation` 复用 `record_metrics_and_mark_write`
- `HD-2`/`MD-2`/`LD-1` 误报文档说明（trait 解耦 / `Send+Sync` 必要性 / `Mutex` 保护时序）
- examples/tests/benches：扩展 L1 重导出隔离 + 补 `tokio`
- lib.rs：文档化 L1/L2/L3 重导出范围

### 性能

- audit_admin_bypass 热路径移除 `eprintln!`

## [0.4.0] - 2026-07-13

### 新增

- **图数据库支持**：新增 `LadybugConnection`（嵌入式图 DB，基于 lbug 0.18）和 `Neo4jConnection`（服务器端图 DB，基于 neo4rs 0.8），通过 `GraphConnection` trait 统一抽象
- **图事务**：`Session` 新增 `execute_cypher` 方法，支持图事务分发（begin/commit/rollback），通过 `GraphTransaction` trait 抽象图事务句柄
- **宏扩展**：`#[db_entity]` 新增 `exists()` 和 `find_by_ids()` 方法；新增 `#[db_graph]` 宏用于图实体代码生成；宏参数类型校验增强（非字符串字面量报错）
- **权限模型**：`PermissionAction` 新增 `Traverse` 和 `Match` 变体用于图操作权限控制
- **DatabaseType**：新增 `Ladybug` 和 `Neo4j` 变体，`from_url` 识别 `ladybug:`/`neo4j:`/`neo4j+s:`/`neo4j+ssc:` scheme
- **DbConnection**：新增 `Ladybug(Arc<LadybugConnection>)` 和 `Neo4j(Arc<Neo4jConnection>)` 变体，`as_graph()`/`is_graph()` 方法
- **feature gate**：新增 `ladybug` 和 `neo4j` feature（与关系型 DB 不互斥，允许混合使用）

### 变更

- `trait-kit` 依赖升级 `0.2` → `0.3`
- `oxcache` 依赖升级 `0.3.4` → `0.3.8`（trait-kit 0.3 支持）
- `parse_url`（Neo4j）返回 `Result`，无凭据时从环境变量读取或返回明确错误（LOW-001）
- `LadybugConnection` 使用 `Arc<Semaphore>` 限制并发数（替代 `Vec<Connection>` 连接池，因 lbug 生命周期限制）

### 修复

- **HIGH-001**：图事务并发竞态 — `execute_cypher` 的 take → 锁外 await → put back 模式在并发下绕过事务隔离，新增 `graph_op_mutex` 串行化图操作
- **FM-3.1**（RPN=216）：`execute_cypher` panic 时事务隔离被绕过 — PoisonGuard RAII 模式在 unwinding 时设置 poisoned 标记
- **FM-3.6**（RPN=294）：`Session::Drop` 不处理图事务 — 通过级联 Drop 解决（LadybugTransaction actor 自动 ROLLBACK，Neo4jTransaction::Drop spawn rollback）
- **FM-2.2**（RPN=168）：`Neo4jTransaction` 无 Drop impl — 实现 Drop，`try_lock` + `spawn` rollback task
- **FM-1.6**（RPN=144）：`begin_graph_txn` 无并发限制 — `LadybugTransaction` 持有 `OwnedSemaphorePermit` 直到 commit/rollback
- **M-48**：`LadybugConnection::parse_url` 路径遍历校验 — 拒绝包含 `..` 的路径
- **M-49**：`Neo4jConnection::parse_url` 错误信息不回显原始 URL — 避免凭据泄露
- **MED-004**：`LadybugConnection::execute_cypher` 行获取逻辑重复 — 去重为 `execute_cypher_on_conn` 共享函数
- **MED-006**：宏参数类型不匹配时静默忽略 — 4 个参数 + hooks 加 `else` 分支报错

### ⚠️ BREAKING CHANGES

- `default` feature 改为空数组 `[]`（之前包含 runtime-tokio-rustls/sqlite/permission/sql-parser/macros/config-env/with-time 7 个特性）
- 新增 `default-no-db` 聚合特性，提供不含数据库驱动的常用功能集
- 用户必须显式启用 runtime + driver + features，推荐 `default-features = false, features = ["default-no-db", "sqlite"]`
- pre-commit 脚本和 CI 统一使用 `--no-default-features --features sqlite,default-no-db,all-optional` 命令

## [0.3.4] - 2026-07-12

### ⚠️ BREAKING CHANGES

- `error` 模块从 `src/common/error.rs` 迁移到 `src/error.rs`，导入路径 `crate::common::error::` → `crate::error::`

## [0.3.3] - 2026-07-11

### 变更

- **权限导出去重**: 消除重复的权限模块导出
- **RUSTSEC-2023-0071 文档化**: 记录 `rsa` crate 安全公告（上游暂无修复，已在 `deny.toml` 中声明忽略）

### 变更（Phase 6 前置）

- **edition 2024 升级**: workspace 从 edition 2021 升级到 edition 2024
- **rust-version 1.85**: 最低支持的 Rust 版本提升至 1.85（edition 2024 要求）
- **MIT license 统一**: workspace 统一采用 MIT 许可证，所有子 crate（dbnexus、dbnexus-macros、dbnexus-examples）均使用同一许可证

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

[0.4.4]: https://github.com/Kirky-X/dbnexus/compare/v0.4.3...v0.4.4
[0.4.3]: https://github.com/Kirky-X/dbnexus/compare/v0.4.2...v0.4.3
[0.4.2]: https://github.com/Kirky-X/dbnexus/compare/v0.4.0...v0.4.2
[0.4.0]: https://github.com/Kirky-X/dbnexus/compare/v0.3.4...v0.4.0
[0.3.4]: https://github.com/Kirky-X/dbnexus/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/Kirky-X/dbnexus/compare/v0.3.0...v0.3.3
[0.3.0]: https://github.com/Kirky-X/dbnexus/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/Kirky-X/dbnexus/compare/v0.1.2...v0.2.0
