# DBNexus 示例索引

本目录是独立子 crate `dbnexus-examples`，收录 DBNexus 的全部功能示例，与 `Cargo.toml` 中的 `[[bin]]` 清单一一对应（共 51 个已注册 bin）。

**Rust 版本要求：1.97.1+**（与 workspace 的 `rust-version` 一致）。

## 目录

- [基础模块 (basic/)](#基础模块-basic)
- [权限模块 (permission/)](#权限模块-permission)
- [安全模块 (security/)](#安全模块-security)
- [配置模块 (config/)](#配置模块-config)
- [数据库模块 (database/)](#数据库模块-database)
- [可观测性模块 (observability/)](#可观测性模块-observability)
- [认证与审计模块 (auth/)](#认证与审计模块-auth)
- [宏模块 (macros/)](#宏模块-macros)
- [图数据库模块 (graph/)](#图数据库模块-graph)
- [分布式能力 (distributed/)](#分布式能力-distributed)
- [可靠性模块 (reliability/)](#可靠性模块-reliability)
- [通用模块 (common/)](#通用模块-common)
- [集成适配器 (integrations/)](#集成适配器-integrations)
- [Kit 能力管理 (kit/)](#kit-能力管理-kit)
- [批量运行与编译](#批量运行与编译)
- [已启用的 feature](#已启用的-feature)
- [相关文档](#相关文档)

运行命令统一为：

```bash
cargo run -p dbnexus-examples --bin <示例名>
```

（在仓库根目录或 `examples/` 目录下均可执行。）

## 基础模块 (basic/)

| 示例 | 说明 | 运行命令 |
|------|------|----------|
| `basic_connection` | 创建 SQLite 内存连接池，演示 `DbPool`、`Session` 与池配置的基础用法 | `cargo run -p dbnexus-examples --bin basic_connection` |
| `basic_crud` | 用 `#[db_entity(...)]` 宏定义实体并执行增删改查 | `cargo run -p dbnexus-examples --bin basic_crud` |
| `basic_transaction` | 演示 `Session` 的事务 API（begin/commit/rollback） | `cargo run -p dbnexus-examples --bin basic_transaction` |

## 权限模块 (permission/)

| 示例 | 说明 | 运行命令 |
|------|------|----------|
| `permission_rbac` | 用 `MemoryPermissionProvider` 演示 admin/manager/guest 三角色的 RBAC 权限控制 | `cargo run -p dbnexus-examples --bin permission_rbac` |
| `permission_yaml` | 用 `YamlPermissionProvider` 从 YAML 字符串解析并加载权限策略 | `cargo run -p dbnexus-examples --bin permission_yaml` |
| `permission_macro` | 演示 `#[db_entity(..., permissions(...))]` 子参数为实体标注角色与操作 | `cargo run -p dbnexus-examples --bin permission_macro` |
| `permission_engine` | 演示策略决策点 `PolicyDecisionPoint` 的配置与权限判定 | `cargo run -p dbnexus-examples --bin permission_engine` |

## 安全模块 (security/)

| 示例 | 说明 | 运行命令 |
|------|------|----------|
| `sql_parser` | 用 `SqlParser` 解析 SELECT/INSERT/UPDATE/DELETE/CREATE/DROP 等语句并提取操作类型 | `cargo run -p dbnexus-examples --bin sql_parser` |
| `sql_injection_detection` | 用 `contains_sql_injection` 检测 UNION/OR 1=1/注释注入/堆叠查询等多种注入模式 | `cargo run -p dbnexus-examples --bin sql_injection_detection` |
| `ddl_guard` | 用 `DdlGuard` 基于 AST 验证 DDL 语句的安全性 | `cargo run -p dbnexus-examples --bin ddl_guard` |
| `sensitive_masker` | 用 `SensitiveMasker` 对手机号、邮箱等不同类型的敏感数据脱敏 | `cargo run -p dbnexus-examples --bin sensitive_masker` |
| `rate_limiter` | 用 `RateLimiter` 基于令牌桶算法进行速率限制 | `cargo run -p dbnexus-examples --bin rate_limiter` |

## 配置模块 (config/)

| 示例 | 说明 | 运行命令 |
|------|------|----------|
| `config_env` | 通过 `DATABASE_URL`、`DB_MAX_CONNECTIONS` 等环境变量创建 `DbConfig` | `cargo run -p dbnexus-examples --bin config_env` |
| `config_yaml` | 通过 YAML 字符串创建 `DbConfig` 并构建连接池 | `cargo run -p dbnexus-examples --bin config_yaml` |
| `config_toml` | 通过 TOML 字符串创建 `DbConfig` 并构建连接池 | `cargo run -p dbnexus-examples --bin config_toml` |
| `config_presets` | 对比 embedded/microservice/monolith/enterprise 四种预设的 feature 差异 | `cargo run -p dbnexus-examples --bin config_presets` |

## 数据库模块 (database/)

| 示例 | 说明 | 运行命令 |
|------|------|----------|
| `database_sqlite` | 演示 SQLite 内存模式与文件模式两种运行方式的 DDL/DML 操作 | `cargo run -p dbnexus-examples --bin database_sqlite` |
| `database_postgres` | 连接 PostgreSQL 执行基本操作；无可用服务时优雅降级退出 | `cargo run -p dbnexus-examples --bin database_postgres` |
| `database_mysql` | 连接 MySQL 执行基本操作；无可用服务时优雅降级退出 | `cargo run -p dbnexus-examples --bin database_mysql` |
| `migration` | 用 `MigrationExecutor` 定义、应用迁移，查看历史并手动回滚（执行反向 SQL） | `cargo run -p dbnexus-examples --bin migration` |
| `sharding` | 用 `ShardRouter` 演示 yearly/monthly/daily/hash 分片策略与路由 | `cargo run -p dbnexus-examples --bin sharding` |
| `global_index` | 用 `GlobalIndex` 管理跨分片全局索引，并展示 `SyncResult`/`SyncEvent` 类型 | `cargo run -p dbnexus-examples --bin global_index` |
| `pool_management` | 演示连接池 pool-warmup、pool-health-check、auto-migrate 三大生命周期特性 | `cargo run -p dbnexus-examples --bin pool_management` |
| `duckdb_query` | 用 `DuckDbConnection` 演示 DuckDB 内存库的建表、插入与分析查询 | `cargo run -p dbnexus-examples --bin duckdb_query` |

## 可观测性模块 (observability/)

| 示例 | 说明 | 运行命令 |
|------|------|----------|
| `metrics_prometheus` | 用 `MetricsCollector` 同步池状态、记录查询指标并导出 Prometheus 格式 | `cargo run -p dbnexus-examples --bin metrics_prometheus` |
| `health_check` | 演示 `HealthChecker` 健康检查与 `CircuitBreaker` 熔断器的完整流程 | `cargo run -p dbnexus-examples --bin health_check` |
| `latency_histogram` | 演示 `LatencyHistogram` 延迟直方图、百分位统计与慢查询记录 | `cargo run -p dbnexus-examples --bin latency_histogram` |

## 认证与审计模块 (auth/)

| 示例 | 说明 | 运行命令 |
|------|------|----------|
| `authentication_jwt` | 用 `JwtManager` 生成并验证 access/refresh 两类 JWT token | `cargo run -p dbnexus-examples --bin authentication_jwt` |
| `authentication_password` | 用 `PasswordHasher`（bcrypt）与 `AuthenticationManager` 演示密码哈希与用户认证 | `cargo run -p dbnexus-examples --bin authentication_password` |
| `audit_logging` | 用 `AuditLogger` 演示审计日志完整流程（敏感字段、告警操作、容量等配置） | `cargo run -p dbnexus-examples --bin audit_logging` |

## 宏模块 (macros/)

| 示例 | 说明 | 运行命令 |
|------|------|----------|
| `macros_db_entity` | 用 `#[db_entity]` 宏定义多实体（User/Product/Order）与关系 | `cargo run -p dbnexus-examples --bin macros_db_entity` |
| `macros_db_crud` | 演示宏生成的完整 CRUD 方法，含批量插入、批量删除与分页查询 | `cargo run -p dbnexus-examples --bin macros_db_crud` |
| `macros_db_audit` | 演示 `audit(...)` 子参数生成的审计常量与审计日志集成 | `cargo run -p dbnexus-examples --bin macros_db_audit` |
| `macros_db_cache` | 演示 `cache(...)` 子参数生成的缓存配置常量与方法 | `cargo run -p dbnexus-examples --bin macros_db_cache` |
| `macros_soft_delete_unique` | `soft_delete` 配合复合唯一约束 `UNIQUE(email, deleted_at)` 解决软删除后的唯一冲突 | `cargo run -p dbnexus-examples --bin macros_soft_delete_unique` |
| `macros_db_entity_v2` | 演示 timestamps、validate、hooks 三大行为特性 | `cargo run -p dbnexus-examples --bin macros_db_entity_v2` |
| `macros_advanced_query` | 演示宏生成的高级查询与批量操作（schema/query/paginate/batch） | `cargo run -p dbnexus-examples --bin macros_advanced_query` |

## 图数据库模块 (graph/)

| 示例 | 说明 | 运行命令 |
|------|------|----------|
| `graph_neo4j` | 演示 `Neo4jConnection` 的 URL 解析、环境变量凭据回退与连接失败时的优雅降级（无服务器时仅演示降级） | `cargo run -p dbnexus-examples --bin graph_neo4j` |

`graph_ladybug`（Ladybug 嵌入式图数据库，DDL/节点/关系操作）**未注册为 `[[bin]]`**：它需要 `ladybug` feature，而该 feature 与 `duckdb` 存在 mbedtls 链接冲突，故需单独编译（见 `Cargo.toml` 内注释）：

```bash
cargo build --bin graph_ladybug --no-default-features \
  --features "runtime-tokio-rustls,sqlite,cache,ladybug"
```

## 分布式能力 (distributed/)

| 示例 | 说明 | 运行命令 |
|------|------|----------|
| `distributed_id` | 用 `SnowflakeIdGenerator` 生成并解析分布式 ID | `cargo run -p dbnexus-examples --bin distributed_id` |
| `saga` | 用 `SagaOrchestrator` 演示正向动作 + 补偿动作的分布式事务编排 | `cargo run -p dbnexus-examples --bin saga` |
| `scatter_gather` | 用 `ScatterGatherExecutor` 演示跨分片并行查询与结果聚合 | `cargo run -p dbnexus-examples --bin scatter_gather` |
| `replica_routing` | 用 `ReplicaConfig` 演示副本配置与读写分离路由 | `cargo run -p dbnexus-examples --bin replica_routing` |
| `shard_migration` | 用 `ShardMigrationOrchestrator` 演示分片迁移编排 | `cargo run -p dbnexus-examples --bin shard_migration` |

## 可靠性模块 (reliability/)

| 示例 | 说明 | 运行命令 |
|------|------|----------|
| `retry` | 用 `RetryExecutor` 演示重试策略（最大重试、指数退避、抖动） | `cargo run -p dbnexus-examples --bin retry` |
| `failover` | 用 `FailoverConfig` 演示连接故障转移链的配置与使用 | `cargo run -p dbnexus-examples --bin failover` |

## 通用模块 (common/)

| 示例 | 说明 | 运行命令 |
|------|------|----------|
| `error_handling` | 演示 `QueryErrorReport` 与 `ErrorCategory` 的结构化错误报告 | `cargo run -p dbnexus-examples --bin error_handling` |
| `i18n_formatting` | 用 `DbI18nFormatter` 演示 locale 感知的数字、日期等格式化 | `cargo run -p dbnexus-examples --bin i18n_formatting` |
| `cache_standalone` | 实现自定义 `DbCacheProvider` 并演示其独立使用 | `cargo run -p dbnexus-examples --bin cache_standalone` |

## 集成适配器 (integrations/)

| 示例 | 说明 | 运行命令 |
|------|------|----------|
| `oxcache_adapter` | 用 `OxcacheDbCacheAdapter` 将 oxcache 缓存后端适配为 `DbCacheProvider` | `cargo run -p dbnexus-examples --bin oxcache_adapter` |

## Kit 能力管理 (kit/)

| 示例 | 说明 | 运行命令 |
|------|------|----------|
| `kit_usage` | 用 `DbNexusModule` 与 trait-kit 的 `AsyncKit` 演示模块注册、构建与连接池能力获取 | `cargo run -p dbnexus-examples --bin kit_usage` |
| `kit_advanced` | 演示通过 AsyncKit 依赖注入驱动的多能力数据库操作 | `cargo run -p dbnexus-examples --bin kit_advanced` |

## 批量运行与编译

```bash
# 在 examples/ 目录下逐个运行全部 51 个已注册示例
bash test_all_examples.sh

# 编译全部示例
cargo build -p dbnexus-examples --all-targets
```

说明：

- `database_postgres`、`database_mysql`、`graph_neo4j` 在没有可用数据库服务时会优雅降级退出，无需预先部署服务即可运行。
- `graph_ladybug` 不在上述清单内（未注册为 bin），编译方式见[图数据库模块](#图数据库模块-graph)。

## 已启用的 feature

示例所需的 feature 已在 `Cargo.toml` 的 `dbnexus` 依赖中统一启用：

- **数据库**：`sqlite`、`duckdb`、`neo4j`（`ladybug` 因与 `duckdb` 的 mbedtls 链接冲突未启用，仅供 `graph_ladybug` 单独编译时使用）
- **运行时**：`runtime-tokio-rustls`
- **安全与权限**：`permission`、`permission-engine`、`sql-parser`、`authentication`、`validation`
- **配置**：`yaml`、`config-toml`、`config-env`
- **数据管理**：`cache`、`migration`、`auto-migrate`、`sharding`、`global-index`、`pool-warmup`、`pool-health-check`
- **可观测性**：`metrics`、`health-check`、`audit`
- **宏**：`macros`
- **分布式**：`retry`、`failover`、`replica-routing`、`scatter-gather`、`shard-migration`、`saga`、`distributed-id`
- **集成**：`oxcache-integration`、`kit`
- **序列化**：`with-json`、`with-time`

## 相关文档

- [用户指南](../docs/USER_GUIDE.md)
- [API 参考](../docs/API_REFERENCE.md)
- [安全文档](../docs/SECURITY.md)
- [示例清单 Cargo.toml](Cargo.toml)

欢迎贡献新示例：请遵循现有示例文件的结构（文件头版权注释 + `//!` 文档 + 运行说明），并在 `Cargo.toml` 中注册对应的 `[[bin]]`。
