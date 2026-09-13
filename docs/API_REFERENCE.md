# 📘 Dbnexus API 参考

DBNexus 的公开 API 文档。所有签名均以 [docs.rs/dbnexus](https://docs.rs/dbnexus) 生成的最新文档为准；使用入门见 [📖 用户指南](USER_GUIDE.md)。

## 📋 目录

<details open>
<summary>📑 目录</summary>

- [🧩 核心类型](#-核心类型)
  - [连接池 DbPool](#连接池-dbpool)
  - [会话 Session](#会话-session)
  - [池状态 PoolStatus](#池状态-poolstatus)
- [⚙️ 配置 API](#️-配置-api)
- [🔐 权限 API](#-权限-api)
- [🏷️ 过程宏](#️-过程宏)
- [⚠️ 错误类型](#️-错误类型)
- [🧰 类型别名](#-类型别名)
- [🚪 特性门控 API 总览](#-特性门控-api-总览)
- [📊 可观测性 API](#-可观测性-api)
- [🗄️ DuckDB 数据库](#️-duckdb-数据库)
- [🔑 认证系统](#-认证系统)
- [🌐 图数据库](#-图数据库)
- [🗺️ 分片与分布式能力](#️-分片与分布式能力)
- [🔎 SQL 解析与安全工具](#-sql-解析与安全工具)
- [🧩 Kit 与缓存集成](#-kit-与缓存集成)
- [🌍 国际化](#-国际化)
- [📚 相关文档](#-相关文档)

</details>

---

## 🧩 核心类型

### 连接池 DbPool

数据库连接的主连接池管理器。

```rust
pub struct DbPool { /* 字段私有 */ }
```

#### 方法总览

| 方法 | 签名要点 | 说明 |
|------|----------|------|
| `new` | `async fn new(url: &str) -> DbResult<Self>` | 使用默认配置创建连接池 |
| `with_config` | `async fn with_config(config: DbConfig) -> DbResult<Self>` | 从显式配置创建（带自动修正） |
| `try_from_config` | `async fn try_from_config(config: DbConfig) -> DbResult<Self>` | 从显式配置创建（严格模式） |
| `try_from` | `fn try_from(config: &DbConfig) -> Result<Self, ConfigError>` | 同步创建未初始化的连接池 |
| `get_session` | `async fn get_session(&self, role: &str) -> DbResult<Session>` | 获取带角色权限检查的会话 |
| `query_rows` | `async fn query_rows(&self, sql: &str, role: &str) -> DbResult<Vec<serde_json::Value>>` | 统一行查询：解析 → 权限 → 执行 → JSON 出口（0.6.0-rc.3） |
| `status` | `fn status(&self) -> PoolStatus` | 返回当前池状态快照 |
| `config` | `fn config(&self) -> &DbConfig` | 返回生效配置 |
| `clean_invalid_connections` | `async fn clean_invalid_connections(&self) -> u32` | 手动触发连接健康检查与清理（`pool-health-check`） |
| `health_snapshot` | `async fn health_snapshot(&self) -> serde_json::Value` | 池饱和度/副本状态/慢查询计数汇总为单个 JSON（`health-check`） |
| `set_cache_provider` | `fn set_cache_provider(&mut self, provider: Arc<dyn DbCacheProvider + Send + Sync>)` | 注入缓存实现（`cache` / `oxcache-integration`） |
| `set_ddl_guard` | `fn set_ddl_guard(&self, guard: Arc<dyn DdlGuardPolicy>)` | 注入统一 DDL 守卫策略（`sql-parser`） |
| `enable_prepare_cache` | `fn enable_prepare_cache(&self, capacity: usize)` | 启用语句级 prepared statement LRU 缓存（`prepare-cache`） |
| `prepare_cache_stats` | `fn prepare_cache_stats(&self) -> Option<PrepareCacheStats>` | 语句缓存命中率指标（`prepare-cache`） |
| `set_permission_config` | `async fn set_permission_config(&self, config: &PermissionConfig)` | 运行时更新权限配置（`permission`） |

#### 错误

`get_session` 可能返回：

| 错误 | 场景 |
|------|------|
| `DbError::Permission` | 角色不在权限配置中 |
| `DbError::ConnectionPool` | 获取连接失败 |

**示例**：

```rust
use dbnexus::DbPool;

let pool = DbPool::new("sqlite::memory:").await?;
let session = pool.get_session("admin").await?;

// 统一行查询（返回真实数据行）
let rows = pool.query_rows("SELECT id, name FROM users WHERE id = 1", "admin").await?;
```

### 会话 Session

基于 RAII 的数据库会话，丢弃时连接自动归还连接池。

```rust
pub struct Session { /* 字段私有 */ }
```

#### 方法总览

| 方法 | 签名要点 | 说明 |
|------|----------|------|
| `role` | `fn role(&self) -> &str` | 返回当前会话角色 |
| `execute` | `async fn execute(&self, sql: &str) -> DbResult<ExecResult>` | 执行带权限检查的 SQL |
| `execute_raw` | `async fn execute_raw(&self, sql: &str) -> DbResult<ExecResult>` | 安全管道执行（解析 → 逐表权限 → 慢查询计时，0.6.0-rc.3 接线） |
| `query_rows` | `async fn query_rows(&self, sql: &str) -> DbResult<Vec<serde_json::Value>>` | 行查询，返回 JSON 数据行 |
| `execute_raw_ddl` | `async fn execute_raw_ddl(&self, sql: &str) -> DbResult<ExecResult>` | 执行 DDL，仅限 admin 角色，经 `DdlGuard` 校验 |
| `execute_cached` | `async fn execute_cached(&self, sql: &str) -> DbResult<ExecResult>` | 语句级缓存感知执行（`prepare-cache`） |
| `batch_execute` | `async fn batch_execute(&self, sqls: Vec<&str>) -> DbResult<Vec<DbResult<ExecResult>>>` | 批量执行多条语句 |
| `begin_transaction` | `async fn begin_transaction(&self) -> Result<(), DbError>` | 开始事务（已在事务中返回 `DbError::Transaction`） |
| `commit` | `async fn commit(&self) -> Result<(), DbError>` | 提交当前事务 |
| `rollback` | `async fn rollback(&self) -> Result<(), DbError>` | 回滚当前事务 |
| `is_in_transaction` | `async fn is_in_transaction(&self) -> bool` | 检查是否在事务中 |
| `query_cache_get` / `query_cache_set` | `async fn query_cache_get(&self, key: &str) -> Option<Vec<u8>>` 等 | 只读查询结果缓存读写（`cache`，0.6.0-rc.3） |

`execute` 可能返回的错误：

| 错误 | 场景 |
|------|------|
| `DbError::Permission` | 权限被拒绝 |
| `DbError::SqlParse` | 无效的 SQL 语法 |
| `DbError::Database` | 数据库错误 |

**示例**：

```rust
let result = session.execute("SELECT * FROM users").await?;
```

### 池状态 PoolStatus

连接池状态快照。

```rust
pub struct PoolStatus {
    pub total: u32,        // 池中的总连接数
    pub active: u32,       // 当前活跃连接数
    pub idle: u32,         // 空闲连接数（总数 - 活跃）
    pub wait_count: u32,   // 当前等待连接的请求数
    pub max_waiters: u32,  // 最大等待计数（历史峰值）
    pub borrow_count: u64, // 总借用次数
    pub max_active: u32,   // 观察到的最大活跃连接数（历史峰值）
}
```

> 池内部的计数由原子类型维护，热路径无锁；`status()` 返回的是某一时刻的快照值。

---

## ⚙️ 配置 API

配置类型基于 `serde` 直接反序列化实现，多配置源（环境变量 / YAML / JSON / TOML / 程序化）见[用户指南](USER_GUIDE.md#-配置)。

### `DbConfig`

```rust
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct DbConfig {
    pub url: String,

    /// 连接池配置（通过 `#[serde(flatten)]` 扁平化，保持序列化向后兼容）
    #[serde(flatten)]
    pub pool_config: PoolConfig,

    #[serde(default)]
    pub permissions_path: Option<String>,

    #[serde(default)]
    pub migrations_dir: Option<PathBuf>,

    #[serde(default)]
    pub auto_migrate: bool,

    #[serde(default = "default_migration_timeout")]
    pub migration_timeout: u64,

    #[serde(default = "default_admin_role")]
    pub admin_role: String,

    #[serde(default = "default_warmup_timeout")]
    pub warmup_timeout: u64,

    #[serde(default = "default_warmup_retries")]
    pub warmup_retries: u32,
}
```

### `PoolConfig`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    #[serde(default = "default_min_connections")]
    pub min_connections: u32,

    #[serde(default = "default_idle_timeout")]
    pub idle_timeout: u64,

    #[serde(default = "default_acquire_timeout")]
    pub acquire_timeout: u64,
}
```

**默认值与环境变量**：

| 字段 | 默认值 | 环境变量（`from_env`） |
|------|--------|------------------------|
| `url` | 必需 | `DATABASE_URL` |
| `max_connections` | 20 | `DB_MAX_CONNECTIONS` |
| `min_connections` | 5 | `DB_MIN_CONNECTIONS` |
| `idle_timeout` | 300（秒） | `DB_IDLE_TIMEOUT` |
| `acquire_timeout` | 5000（毫秒） | `DB_ACQUIRE_TIMEOUT` |
| `permissions_path` | 无 | `DB_PERMISSIONS_PATH` |
| `migrations_dir` | 无 | `DB_MIGRATIONS_DIR` |
| `auto_migrate` | `false` | `DB_AUTO_MIGRATE` |
| `migration_timeout` | 60（秒） | `DB_MIGRATION_TIMEOUT` |
| `admin_role` | `"admin"` | `DB_ADMIN_ROLE` |
| `warmup_timeout` | 30（秒） | `DB_WARMUP_TIMEOUT` |
| `warmup_retries` | 3 | `DB_WARMUP_RETRIES` |

### 配置加载方法

| 方法 | 签名 | 特性要求 |
|------|------|----------|
| `DbConfig::from_env` | `fn from_env() -> Result<DbConfig, ConfigError>` | `config-env` |
| `DbConfig::from_yaml_str` | `fn from_yaml_str(yaml: &str) -> Result<DbConfig, serde_yaml_ng::Error>` | `yaml` |
| `DbConfig::from_json_str` | `fn from_json_str(json: &str) -> Result<DbConfig, serde_json::Error>` | 无 |

**示例**：

```rust
let config = DbConfig::from_yaml_str(
    r#"
url: "sqlite::memory:"
max_connections: 20
"#,
)?;
let pool = DbPool::with_config(config).await?;
```

---

## 🔐 权限 API

### `PermissionAction`

用于权限检查的数据库操作类型：

```rust
pub enum PermissionAction {
    Select,    // SELECT 查询
    Insert,    // INSERT 语句
    Update,    // UPDATE 语句
    Delete,    // DELETE 语句
    // 以下变体仅在启用图数据库特性时可用：
    Traverse,  // 图遍历（ladybug / neo4j）
    Match,     // 图匹配（ladybug / neo4j）
}
```

### `PermissionContext`

具有缓存和速率限制的权限检查上下文。

| 方法 | 签名要点 | 说明 |
|------|----------|------|
| `new` | `fn new(role: String, policy_cache: Arc<Cache<String, RolePolicy>>) -> Self` | 以共享策略缓存创建 |
| `with_cache_size_and_rate_limit` | `async fn with_cache_size_and_rate_limit(role: String, cache_capacity: usize, max_requests: u32, window_secs: u64) -> Result<Self, PermissionError>` | 自定义缓存大小与令牌桶限流 |
| `with_config_and_rate_limit` | `async fn with_config_and_rate_limit(role: String, config: &DbConfig, max_requests: u32, window_secs: u64) -> Result<Self, PermissionError>` | 从 `DbConfig` 创建并附带限流 |
| `check_table_access` | `async fn check_table_access(&self, table: &str, operation: &PermissionAction) -> bool` | 检查当前角色能否对表执行操作 |
| `load_policy` | `async fn load_policy(&self, config: &PermissionConfig) -> Result<(), String>` | 加载权限配置 |

> 权限缓存带 TTL 与 singleflight 请求合并（防缓存击穿），支持后台热加载。

### `PermissionConfig`

```rust
pub struct PermissionConfig {
    pub roles: HashMap<String, RolePolicy>,
}
```

| 方法 | 签名 | 特性要求 |
|------|------|----------|
| `from_yaml_str` | `fn from_yaml_str(yaml: &str) -> Result<Self, serde_yaml_ng::Error>` | `yaml` |
| `deny_all` | `fn deny_all() -> Self` | 创建拒绝所有访问的配置 |

YAML 格式见[用户指南 · 权限控制](USER_GUIDE.md#-权限控制)。

### 权限引擎 `PolicyDecisionPoint`（`permission-engine` 特性）

高级策略决策点（PDP），支持 RBAC + ABAC，内置缓存与速率限制。

```rust
use dbnexus::{PolicyDecisionPoint, RbacPermissionProvider};
use std::sync::Arc;

// 默认：缓存 TTL 5 分钟，速率限制 100 请求/分钟
let provider = Arc::new(RbacPermissionProvider::new());
let pdp = PolicyDecisionPoint::new(provider);

// 或经构建器自定义
let pdp = PolicyDecisionPoint::builder()
    .provider(provider)
    .cache_ttl_seconds(600)
    .rate_limit(200, 60)
    .build();
```

**导出类型**：`PolicyDecisionPoint`、`PermissionRule`、`PermissionDecision`、`PermissionSubject`、`PermissionResource`、`RbacPermissionProvider`、`Role`

---

## 🏷️ 过程宏

### `#[db_entity]`

统一的属性宏，将结构体标记为数据库实体，生成 Sea-ORM 实体胶水、带权限检查的 CRUD 方法以及缓存/审计配置。该宏整合了早期版本分散的 `db_crud` / `db_cache` / `db_audit` 等独立宏（`#[db_entity]` 由 `macros` 特性门控，经 `dbnexus::db_entity` 导入）。

**必需参数**：

| 参数 | 描述 |
|------|------|
| `table_name = "..."` | 数据库表名 |
| `primary_key = "..."` | 主键字段名 |

**可选参数**：

| 参数 | 描述 |
|------|------|
| `timestamps = true` | 自动管理 `created_at` / `updated_at`（经 `ActiveModelBehavior::before_save`） |
| `soft_delete = true` | 自动注入 `deleted_at` 字段并改写 `find*` / `delete` 语义，额外生成 `force_delete` |
| `validate` | 集成 `validator` crate（`validation` 特性） |
| `cache(...)` | 生成缓存配置（`cache` 特性） |
| `audit(...)` | 生成审计配置（`audit` 特性） |
| `hooks(...)` | 6 个生命周期钩子：`before_insert` / `after_insert` / `before_update` / `after_update` / `before_delete` / `after_delete`；编排顺序为 校验 → 时间戳 → 用户钩子 |
| `schema(backend)` | 生成 `migration::schema::Table`（基于 `sea_orm::Schema::create_table_from_entity`） |

> 权限控制不通过宏声明，而是由 `Session` 在运行时根据权限配置（YAML/JSON）强制执行。

**生成的方法**：

| 方法 | 签名要点 |
|------|----------|
| `insert` | `async fn insert(session: &Session, value: Self) -> DbResult<Self>` |
| `find_by_id` | `async fn find_by_id<PK>(session: &Session, pk: PK) -> DbResult<Option<Self>>`（主键泛型化，支持 i64 / Uuid / String 等） |
| `find_by_ids` | `async fn find_by_ids<PK>(session: &Session, pks: Vec<PK>) -> DbResult<Vec<Self>>`（对接 sea-orm `is_in`） |
| `find_all` | `async fn find_all(session: &Session) -> DbResult<Vec<Self>>` |
| `find_by_condition` | `async fn find_by_condition(session: &Session, condition: Condition) -> DbResult<Vec<Self>>` |
| `count` | `async fn count(session: &Session) -> DbResult<u64>` |
| `exists` | `async fn exists(session: &Session, pk) -> DbResult<bool>` |
| `update` | `async fn update(session: &Session, value: Self) -> DbResult<Self>` |
| `delete` | `async fn delete<PK>(session: &Session, pk: PK) -> DbResult<()>`（约束随 `soft_delete` 参数变化） |
| `delete_many` | `async fn delete_many(session: &Session, condition: Condition) -> DbResult<u64>` |
| `insert_many` | `async fn insert_many(session: &Session, models: Vec<Self>) -> DbResult<InsertResult>` |
| `update_many` | `async fn update_many(session: &Session, filter: Condition, updates: Self) -> DbResult<u64>` |
| `paginate` | `async fn paginate(session: &Session, page_size: u64) -> DbResult<Paginator>` |
| `query` | `async fn query(session: &Session) -> DbResult<Select<Self>>`（返回 Sea-ORM 原生查询构建器） |
| `find_with_deleted` / `find_only_deleted` / `force_delete` | 仅 `soft_delete = true` 实体生成 |
| `table_name()` / `primary_key_column()` | 表名与主键列名常量访问 |
| `schema(backend)` | 生成迁移用 `Table` 结构（配合 `migration` 特性） |

**示例**：

```rust
use dbnexus::db_entity;
use dbnexus::sea_orm::entity::prelude::*;

#[db_entity(table_name = "users", primary_key = "id")]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub email: String,
}

// 使用
let user = Model { id: 1, name: "Alice".to_string(), email: "alice@example.com".to_string() };
let inserted = Model::insert(&session, user).await?;
let found = Model::find_by_id(&session, 1).await?;
```

### `#[db_repository]`

泛型仓储宏（`repository` 特性，0.6.0-rc.3 新增）：为实体生成 `Repository<T>` CRUD 端口实现，配合 `JsonRepository` 参考实现（标识符白名单 + SQL 转义，复用 `query_rows` 管道）与 `impl_json_repository!` 实现宏。

---

## ⚠️ 错误类型

### `DbError`

数据库操作错误。

```rust
pub enum DbError {
    /// 数据库连接错误
    Connection(#[from] sea_orm::DbErr),

    /// 配置错误
    Config(String),

    /// 权限错误
    Permission(String),

    /// 事务错误
    Transaction(String),

    /// 迁移错误
    Migration(String),

    /// 缓存操作错误
    Cache(String),

    /// 数据验证错误（validation 特性）
    #[cfg(feature = "validation")]
    Validation(String),
}
```

### `ConfigError`

配置相关错误。

```rust
pub enum ConfigError {
    /// 缺少必填字段
    MissingField(String),
    /// 缺少 URL
    MissingUrl,
    /// 无效缓存容量
    InvalidCacheCapacity(String),
    /// 无效值
    InvalidValue { key: String, message: String },
    /// 无效格式
    InvalidFormat(String),
    /// 文件未找到
    FileNotFound(String),
    /// IO 错误
    IoError(String),
    /// 无效 URL
    InvalidUrl(String),
    /// 不支持的协议
    UnsupportedProtocol(String),
    /// 解析错误
    ParseError(String),
    /// 验证错误
    ValidationError(String),
}
```

### `SqlParseError`

SQL 解析错误。

```rust
pub enum SqlParseError {
    /// SQL 解析失败（语法错误或无效结构）
    ParseError(String),
    /// 不支持的 SQL 语句类型
    UnsupportedStatement(String),
    /// 空 SQL 语句
    EmptyStatement,
    /// 检测到多条 SQL 语句（仅允许单条语句）
    MultipleStatements,
    /// SQL 语句包含变量（可能为动态 SQL 注入）
    ContainsVariables(String),
}
```

### 统一错误报告（0.6.0-rc.3）

| 类型 | 说明 |
|------|------|
| `ErrorCode` | 统一错误码表（分段数值码 + 机器可读名） |
| `UnifiedDbError` | 顶层统一错误结构，与 `DbError` / `DbNexusError` 双向 `From` 兼容 |
| `QueryErrorReport` | 结构化错误报告：分类 + 消息 + 修复建议 |
| `ErrorCategory` | 错误分类（权限不足 / 注入风险 / 语法错误 / 分片冲突等） |

```rust
use dbnexus::{ErrorCategory, QueryErrorReport};

let report = QueryErrorReport::new(
    ErrorCategory::PermissionDenied,
    "SELECT on users denied for role guest",
    "请授予 guest 角色 users 表的 SELECT 权限",
);
```

---

## 🧰 类型别名

```rust
pub type DbResult<T> = Result<T, DbError>;
pub type Operation = PermissionAction;
```

`ExecResult` 直接 re-export 自 `sea_orm`，包含执行后受影响的行数等信息。

---

## 🚪 特性门控 API 总览

| 特性 | 主要导出类型 / 方法 |
|------|---------------------|
| `metrics` | `MetricsCollector`（Prometheus 导出、慢查询检测） |
| `health-check` | `HealthChecker`、`CircuitBreaker`、`health_snapshot()` |
| `otel` | 健康快照指标 OTLP/HTTP 导出桥 |
| `audit` | `AuditLogger`、`AuditEvent`、`AuditStorage` |
| `permission` | `PermissionContext`、`PermissionConfig`、`PermissionAction` |
| `permission-engine` | `PolicyDecisionPoint`、`RbacPermissionProvider` |
| `permission-facade` | `PermissionFacade`（RBAC + 脱敏 + RLS 统一门面） |
| `data-protection` | 查询出口字段脱敏与行级安全谓词注入 |
| `authentication` | `AuthenticationManager`、`JwtManager`、`PasswordHasher` |
| `sql-parser` | `SqlParser`、`DdlGuard`、`InjectionEngine`、`is_ddl_operation`、`contains_sql_injection` |
| `sharding` | `ShardRouter`、`ShardConfig`、`create_strategy` |
| `global-index` | `GlobalIndex`、`IndexEntry`、`SyncResult` |
| `retry` | `RetryPolicy`、`RetryExecutor`、`RetryError`、`is_idempotent_operation` |
| `failover` | `FailoverConfig`、`CircuitBreaker` 协同 |
| `replica-routing` | `ReplicaConfig`（读写分离路由） |
| `scatter-gather` | 跨分片 Scatter-Gather 查询执行器 |
| `saga` | Saga 分布式事务编排（持久化恢复） |
| `distributed-id` | Snowflake 分布式 ID 生成器 |
| `duckdb` | `DuckDbConnection`、`DuckDbRow`、`DuckDbExecResult` |
| `ladybug` / `neo4j` | `LadybugConnection` / `Neo4jConnection`、`GraphConnection` |
| `cache` / `oxcache-integration` | `DbCacheProvider`、`OxcacheDbCacheAdapter` |
| `prepare-cache` | `Session::execute_cached`、`PrepareCacheStats` |
| `query-dsl` | `q!` 宏、`QueryFragment`、`DslCondition`、`DslOp` |
| `repository` | `Repository<T>`、`JsonRepository`、`impl_json_repository!` |
| `data-api` | `DataApiGateway`（实体到 JSON 查询端点） |
| `entity-events` | `EntityEvent`、`EntityEventBus`、`DbOutboxStore`、`OutboxDispatcher` |
| `copy` | COPY FROM STDIN 语句构建与 text 行编码 |
| `kit` | `DbNexusModule` 及卫星模块（见 [Kit 与缓存集成](#-kit-与缓存集成)） |
| `validation` | `DbError::Validation` 变体 + validator 集成 |
| `pool-health-check` | `DbPool::clean_invalid_connections`、`validate_and_recreate_connections` |

---

## 📊 可观测性 API

### 指标收集 `MetricsCollector`（`metrics` 特性）

```rust
use dbnexus::MetricsCollector;

let collector = MetricsCollector::new();

let pool_metrics = collector.pool_status();          // PoolMetrics { total, active, idle }
if let Some(stats) = collector.get_query_stats("SELECT") {
    println!("P99 延迟: {}ns", stats.latency_percentiles.p99_ns);
}

println!("{}", collector.export_prometheus());        // Prometheus 文本格式
```

慢查询检测：`Session::execute_raw` 在所有返回路径前计时，超过 `SlowQueryConfig` 阈值自动记录到 `MetricsCollector`。

### 健康检查与熔断（`health-check` 特性）

```rust
use dbnexus::{CircuitBreaker, CircuitBreakerConfig, HealthChecker};

let checker = HealthChecker::new(1000); // check_timeout_ms（毫秒）
let status: HealthCheckResult = checker.check().await;

// 熔断器：连续失败达到阈值自动打开，半开恢复
let breaker = CircuitBreaker::new(CircuitBreakerConfig::default());
```

`HealthCheckResult` 携带 `HealthStatus`（`Healthy` / `Unhealthy(String)` / `Degraded(String)`）、检查耗时与详细信息。

### 审计日志 `AuditLogger`（`audit` 特性）

```rust
use dbnexus::AuditLogger;

let logger = AuditLogger::new(); // 默认内存存储（容量 10000）
// 记录操作与用户上下文；admin 绕过权限的操作同样被记录
```

自定义存储与配置经 `AuditLogger::with_config(config: AuditConfig, storage: Arc<dyn AuditStorage>)` 注入；0.6.0-rc.3 新增 `DbAuditStorage` 数据库存储实现（`audit` + `sql-parser` 门控）与 `PermissionAuditChain` HMAC 链式签名审计链。

---

## 🗄️ DuckDB 数据库

需要 `duckdb` 特性。嵌入式分析型数据库以分析只读旁路接入（绕过 sea-orm），0.3.0 起使用连接池支持真正并行查询。

```rust
use dbnexus::DuckDbConnection;

// 独立创建（不走 DbPool，因为 DuckDB 绕过 sea-orm）
let conn = DuckDbConnection::new("duckdb::memory:")?;
// 或指定连接池大小
let conn = DuckDbConnection::with_pool_size("duckdb://path/to/analytics.db", 8)?;

// 异步执行（内部 spawn_blocking 桥接同步 duckdb crate）
let result: DuckDbExecResult = conn.execute("INSERT INTO events VALUES (1)").await?;
let rows: Vec<DuckDbRow> = conn.query("SELECT COUNT(*) FROM events").await?;

// 健康检查与池大小查询
conn.health_check().await?;
let size = conn.pool_size();
```

**URL 格式**：`:memory:` / `duckdb::memory:`（内存）、`duckdb:path/to/file.db`、`duckdb://path/to/file.db`（文件）

**导出类型**：`DuckDbConnection`、`DuckDbRow`、`DuckDbExecResult`

---

## 🔑 认证系统

需要 `authentication` 特性。JWT 认证 + 密码强度验证，基于 `jsonwebtoken` + `bcrypt`。

| 方法 | 签名要点 | 说明 |
|------|----------|------|
| `AuthenticationManager::new` | `fn new(jwt_secret: &[u8]) -> AuthResult<Self>` | 创建管理器（密钥不少于 32 字节） |
| `with_config` | `fn with_config(jwt_secret: &[u8], access_exp_secs: u64, refresh_exp_secs: u64) -> AuthResult<Self>` | 自定义过期时间 |
| `register_user` | `async fn register_user(username: &str, password: &str, role: &str) -> AuthResult<()>` | 校验强度 → 哈希 → 入库完整流程 |
| `authenticate` | `async fn authenticate(credentials: AuthCredentials) -> AuthResult<String>` | 验证凭据并签发 access token |
| `verify_token` | `fn verify_token(token: &str) -> AuthResult<JwtClaims>` | 校验 JWT（同步） |
| `refresh_token` | `async fn refresh_token(refresh_token: &str) -> AuthResult<String>` | 刷新访问令牌 |

`JwtManager` 额外提供 `verify_access_token` / `verify_refresh_token`（区分校验 `token_type`，防止刷新令牌冒用为访问令牌）与可选分布式撤销缓存（`oxcache-integration` 特性，撤销条目 TTL 等于令牌剩余有效期）。

**导出类型**：`AuthenticationManager`、`JwtManager`、`PasswordHasher`、`AuthCredentials`、`JwtClaims`、`TokenType`、`User`、`AuthError`、`AuthResult`

---

## 🌐 图数据库

需要 `ladybug` 或 `neo4j` 特性。嵌入式图数据库（Ladybug）和服务器端图数据库（Neo4j）通过 `GraphConnection` trait 统一抽象，可与关系型数据库混合使用。

```rust
use dbnexus::{GraphConnection, LadybugConnection};

// Ladybug 嵌入式图数据库
let conn = LadybugConnection::new("ladybug:path/to/graph.db")?;
let result = conn.execute_cypher("MATCH (n) RETURN n").await?;

// Neo4j 服务器端图数据库
use dbnexus::Neo4jConnection;
let conn = Neo4jConnection::new("neo4j://localhost:7687", "user", "pass").await?;
```

**Session 图事务支持**：

```rust
// Session 经图操作互斥串行化执行 Cypher，支持 begin/commit/rollback
session.execute_cypher("CREATE (n:User {name: 'Alice'})").await?;
```

> 裸 Cypher 执行已废弃：统一使用 `execute_cypher_with_params` 参数化通道（默认实现拒绝静默回退）。

**权限控制**：`PermissionAction` 的 `Traverse` 和 `Match` 变体用于图操作权限控制。

**导出类型**：`LadybugConnection`、`Neo4jConnection`、`GraphConnection`、`GraphExecResult`、`GraphNode`、`GraphQueryResult`、`GraphRel`、`GraphRow`、`GraphTransaction`、`GraphValue`

---

## 🗺️ 分片与分布式能力

### 分片路由 `ShardRouter`（`sharding` 特性）

| 方法 | 签名要点 | 说明 |
|------|----------|------|
| `new` | `fn new<S: ShardingStrategy + 'static>(strategy: S, total_shards: u32) -> Self` | 以策略实例创建 |
| `with_strategy` | `fn with_strategy(strategy: &str, total_shards: u32) -> Self` | 按策略名创建（同步） |
| `with_config` | `async fn with_config(config: &ShardConfig) -> Result<Self, DbError>` | 异步并行初始化所有分片连接池 |
| `shard_id_for_key` | `fn shard_id_for_key(shard_key: &str) -> u32` | 根据 key 计算分片 ID |
| `calculate_shard` | `fn calculate_shard(timestamp: DateTime<Utc>, key: &str) -> u32` | 按时间 + key 计算分片 |
| `get_session_for_shard` | `async fn get_session_for_shard(shard_key, role) -> Result<Session, DbError>` | 获取分片对应的 Session |
| `get_session_for_shard_with_id` | `async fn get_session_for_shard_with_id(shard_key, role) -> Result<(Session, u32), DbError>` | 同时返回分片 ID |

`create_strategy(name)` 工厂支持 `"yearly"`、`"monthly"`、`"daily"`、`"hash"`、`"consistent-hash"`；未知名称回退默认的 `YearlyStrategy`。

**导出类型**：`ShardRouter`、`ShardConfig`、`ShardingStrategy`、`create_strategy`

### 全局索引 `GlobalIndex`（`global-index` 特性）

跨分片查询索引支持，基于 sea-orm 持久化到数据库。

```rust
use dbnexus::{DbPool, GlobalIndex};
use std::sync::Arc;

let pool = DbPool::new("sqlite::memory:").await?;
let index = GlobalIndex::new(Arc::new(pool)).await?;

// 按索引键查询
let entries = index.query_by_index("users", "user_id", "user_123").await?;

// 批量同步
let result = index.batch_sync(entries).await?;
```

**导出类型**：`GlobalIndex`、`IndexEntry`、`SyncEvent`、`SyncResult`

### 运行时重试（`retry` 特性）

仅对幂等查询（SELECT / SHOW / EXPLAIN）自动重试，非幂等操作直接执行不重试。

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// 最大重试次数（不含首次执行），默认 3
    pub max_retries: u32,
    /// 初始退避间隔（毫秒），默认 100
    pub initial_backoff_ms: u64,
    /// 最大退避间隔上限（毫秒），默认 5000
    pub max_backoff_ms: u64,
    /// 退避增长倍数，默认 2.0
    pub multiplier: f64,
    /// 是否添加随机抖动（避免 thundering herd），默认 true
    pub jitter: bool,
    /// 整体 wall-clock 超时（毫秒），`None` 表示无超时限制，默认 `None`
    pub overall_timeout_ms: Option<u64>,
}
```

退避策略：第 N 次重试的等待时间 = `min(initial_backoff * multiplier^N, max_backoff)`；`jitter = true` 时添加 ±25% 的随机抖动。

```rust
pub struct RetryExecutor;

impl RetryExecutor {
    /// 关联函数（非实例方法）。仅当 sql 被判定为幂等操作时才自动重试。
    pub async fn execute_with_retry<F, Fut, T>(
        policy: &RetryPolicy,
        operation: F,
        sql: &str,
    ) -> Result<T, RetryError>
    where
        F: Fn() -> Fut + Send + Sync,
        Fut: Future<Output = Result<T, DbError>> + Send,
        T: Send;
}
```

```rust
pub enum RetryError {
    /// 重试次数耗尽，包含最后一次错误
    Exhausted { attempts: u32, last_error: DbError },
    /// 非幂等操作被拒绝重试
    NonRetryable(DbError),
    /// 整体超时
    Timeout { timeout_ms: u64, last_error: DbError },
}
```

```rust
use dbnexus::is_idempotent_operation;

assert!(is_idempotent_operation("SELECT * FROM users"));
assert!(!is_idempotent_operation("INSERT INTO users VALUES (1)"));
```

**导出类型**：`RetryPolicy`、`RetryExecutor`、`RetryError`、`is_idempotent_operation`

> 其余分布式能力（`failover` / `replica-routing` / `scatter-gather` / `shard-migration` / `saga` / `distributed-id`）的入口类型见[特性门控 API 总览](#-特性门控-api-总览)，端到端用法见 `examples/` 中的同名示例。

---

## 🔎 SQL 解析与安全工具

### `SqlParser`（`sql-parser` 特性）

```rust
use dbnexus::{SqlParser, SqlOperationType, contains_sql_injection, is_ddl_operation};

let parser = SqlParser::new().await;

// 返回 Result<Option<(表名, PermissionAction)>, SqlParseError>
// 仅 DML 操作返回 Some；DDL / DCL / 事务 / 其他返回 None
let op = parser.parse_operation_async("SELECT * FROM users").await?;
let op_sync = parser.parse_operation("SELECT * FROM users"); // 同步版本

assert!(is_ddl_operation("CREATE TABLE foo (id INT)"));
assert!(contains_sql_injection("'; DROP TABLE--"));
```

`SqlOperationType` 变体：`Select`、`Insert`、`Update`、`Delete`、`Ddl`（CREATE / ALTER / DROP / TRUNCATE）、`Dcl`（GRANT / REVOKE）、`Transaction`（BEGIN / COMMIT / ROLLBACK）、`Other`。

**导出类型**：`SqlParser`、`SqlOperationType`、`SqlParseError`、`is_ddl_operation`、`contains_sql_injection`

### 注入检测 `InjectionEngine`（`sql-parser` 特性）

统一注入检测引擎：关系型 / DDL / 图三处规则表合并为单一注册表（0.6.0-rc.3），按类别（`RuleCategory`）扫描。

```rust
use dbnexus::{InjectionEngine, RuleCategory};

let engine = InjectionEngine::global();
let hits = engine.scan_ddl("DROP DATABASE production");
```

### DDL 守卫 `DdlGuard`（`sql-parser` 特性）

基于 AST 的 DDL 验证，配合统一注入检测引擎拦截危险模式。完整说明与白名单见[用户指南 · DDL 安全守卫](USER_GUIDE.md#ddl-安全守卫)。

```rust
use dbnexus::{DdlGuard, DdlValidationResult};

let guard = DdlGuard::new();
match guard.validate("CREATE TABLE users (id INT)") {
    Ok(DdlValidationResult::Allowed) => println!("放行"),
    Ok(DdlValidationResult::Forbidden(reason)) => println!("拦截: {}", reason),
    Ok(DdlValidationResult::ParseError(err)) => println!("解析失败: {}", err),
    Err(err) => println!("校验错误: {}", err),
}
```

守卫策略可替换：`DdlGuardPolicy` 端口 + `AuditingDdlGuard`（审计装饰器）/ `DryRunDdlGuard`（干跑），经 `DbPoolBuilder::ddl_guard` / `DbPool::set_ddl_guard` 注入。

### 敏感数据脱敏 `SensitiveMasker`（始终可用）

```rust
use dbnexus::{MaskType, SensitiveMasker};

let masked = SensitiveMasker::mask("alice@example.com", MaskType::Email)?;
// => "a***@example.com"
```

`MaskType` 变体：`Phone`、`Email`、`IdCard`、`BankCard`、`Name`、`Address`（均含 Unicode 安全处理）。

**导出类型**：`SensitiveMasker`、`MaskType`、`SensitiveError`

---

## 🧩 Kit 与缓存集成

### trait-kit 集成（`kit` 特性）

基于 trait-kit 的统一能力管理。`kit` 特性隐含池 / 缓存 / 审计 / 健康全能力闭包，注册即全能力可 `require`：

```rust
use dbnexus::DbNexusModule;

let module = DbNexusModule::new();
```

**导出类型**：`DbNexusModule`、`DbNexusCacheModule`、`DbNexusAuditModule`、`DbNexusHealthModule`、`DbHealthCapability`、`DbNexusBuildObserver`（卫星模块为 0.6.0-rc.3 新增，四能力均可独立注册）

### 缓存 Provider（`cache` / `oxcache-integration` 特性）

`DbCacheProvider` trait 是缓存抽象层：`Session` 经注入的 Provider 驱动只读查询结果缓存（`query_cache_get` / `query_cache_set`），`cache_provider` 以 `ArcSwapOption` 无锁读取。

```rust
use dbnexus::{DbCacheProvider, DbPoolBuilder};

let pool = DbPoolBuilder::new()
    .config(config)
    .cache_provider(custom_cache) // Arc<dyn DbCacheProvider + Send + Sync>
    .build()
    .await?;
```

**导出类型**：`DbCacheProvider`、`OxcacheDbCacheAdapter`（`oxcache-integration` 特性，适配 oxcache）

---

## 🌍 国际化

基于 ICU4X 的 locale 感知格式化（核心特性，始终可用）。

```rust
use dbnexus::DbI18nFormatter;

let formatter = DbI18nFormatter::new("zh-CN")?;
let formatted = formatter.format_number(1234567.89)?;
```

**导出类型**：`DbI18nFormatter`、`I18nError`

---

## 📚 相关文档

| 文档 | 说明 |
|------|------|
| [📖 用户指南](USER_GUIDE.md) | 从安装到进阶的完整使用教程 |
| [🏗️ 架构文档](ARCHITECTURE.md) | 设计理念、模块划分、数据流与安全/性能设计 |
| [📦 在线 API 文档](https://docs.rs/dbnexus) | docs.rs 自动生成的最新文档 |
