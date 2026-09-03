# API 参考

DBNexus 的完整 API 文档。

## 目录

- [核心类型](#核心类型)
- [连接池 API](#连接池-api)
- [会话 API](#会话-api)
- [配置 API](#配置-api)
- [权限 API](#权限-api)
- [过程宏](#过程宏)
- [错误类型](#错误类型)
- [特性门控 API](#特性门控-api)
- [0.3.0 新增 API](#030-新增-api)
- [0.4.0 新增 API](#040-新增-api)
- [重试 API](#重试-api)

---

## 核心类型

### `DbPool`

数据库连接的主连接池管理器。

```rust
pub struct DbPool {
    // 字段是私有的
}
```

#### 方法

##### `new`

使用默认配置创建新的连接池。

```rust
pub async fn new(url: &str) -> DbResult<Self>
```

**参数：**
- `url: &str` - 数据库连接 URL

**返回值：**
- `DbResult<DbPool>` - 连接池实例

**示例：**
```rust
let pool = DbPool::new("sqlite::memory:").await?;
```

##### `try_from_config`

从显式配置创建连接池。

```rust
pub async fn try_from_config(config: DbConfig) -> DbResult<Self>
```

**参数：**
- `config: DbConfig` - 数据库配置

**返回值：**
- `DbResult<DbPool>` - 连接池实例

**示例：**
```rust
use dbnexus::{DbConfig, PoolConfig};

let config = DbConfig {
    url: "postgresql://localhost/db".to_string(),
    pool_config: PoolConfig {
        max_connections: 20,
        ..Default::default()
    },
    ..Default::default()
};

let pool = DbPool::try_from_config(config).await?;
```

##### `with_config`

从显式配置创建连接池（带自动修正）。

```rust
pub async fn with_config(config: DbConfig) -> DbResult<Self>
```

**参数：**
- `config: DbConfig` - 数据库配置

**返回值：**
- `DbResult<DbPool>` - 连接池实例

**示例：**
```rust
use dbnexus::{DbConfig, PoolConfig};

let config = DbConfig {
    url: "postgresql://localhost/db".to_string(),
    pool_config: PoolConfig {
        max_connections: 20,
        ..Default::default()
    },
    ..Default::default()
};

let pool = DbPool::with_config(config).await?;
```

##### `try_from`

同步创建未初始化的连接池。

```rust
pub fn try_from(config: &DbConfig) -> Result<Self, ConfigError>
```

##### `get_session`

获取具有基于角色的访问控制的数据库会话。

```rust
pub async fn get_session(&self, role: &str) -> DbResult<Session>
```

**参数：**
- `role: &str` - 用于权限检查的用户角色

**返回值：**
- `DbResult<Session>` - 数据库会话

**错误：**
- `DbError::Permission` - 角色不在权限配置中
- `DbError::ConnectionPool` - 获取连接失败

**示例：**
```rust
let session = pool.get_session("admin").await?;
```

##### `status`

返回当前池状态。

```rust
pub fn status(&self) -> PoolStatus
```

**返回值：**
- `PoolStatus` - 池状态信息

**示例：**
```rust
let status = pool.status();
println!("活跃: {}, 空闲: {}", status.active, status.idle);
```

##### `clean_invalid_connections`

手动触发连接健康检查和清理。

**特性门控：** `#[cfg(feature = "pool-health-check")]`

```rust
#[cfg(feature = "pool-health-check")]
pub async fn clean_invalid_connections(&self) -> u32
```

**返回值：**
- `u32` - 移除的无效连接数

---

### `Session`

基于 RAII 的数据库会话，用于执行查询。

```rust
pub struct Session {
    // 字段是私有的
}
```

#### 方法

##### `execute`

执行带权限检查的 SQL 语句。

```rust
pub async fn execute(&self, sql: &str) -> DbResult<ExecResult>
```

**参数：**
- `sql: &str` - 要执行的 SQL 语句

**返回值：**
- `DbResult<ExecResult>` - 执行结果

**错误：**
- `DbError::Permission` - 权限被拒绝
- `DbError::SqlParse` - 无效的 SQL 语法
- `DbError::Database` - 数据库错误

**示例：**
```rust
let result = session.execute("SELECT * FROM users").await?;
```

##### `execute_raw`

执行不带权限检查的 SQL（用于管理员操作）。

```rust
pub async fn execute_raw(&self, sql: &str) -> DbResult<ExecResult>
```

##### `begin_transaction`

开始数据库事务。

```rust
pub async fn begin_transaction(&self) -> Result<(), DbError>
```

**错误：**
- `DbError::Transaction` - 已在事务中

**示例：**
```rust
session.begin_transaction().await?;
User::insert(&session, user1).await?;
User::insert(&session, user2).await?;
session.commit().await?;
```

##### `commit`

提交当前事务。

```rust
pub async fn commit(&self) -> Result<(), DbError>
```

##### `rollback`

回滚当前事务。

```rust
pub async fn rollback(&self) -> Result<(), DbError>
```

##### `is_in_transaction`

检查当前是否在事务中。

```rust
pub async fn is_in_transaction(&self) -> bool
```

##### `role`

返回当前会话的角色。

```rust
pub fn role(&self) -> &str
```

---

### `PoolStatus`

连接池状态信息。

```rust
pub struct PoolStatus {
    pub total: u32,      // 池中的总连接数
    pub active: u32,     // 当前活跃连接数
    pub idle: u32,       // 空闲连接数（总数 - 活跃）
    pub wait_count: u32,  // 等待连接的次数
    pub max_waiters: u32,  // 最大等待计数（历史峰值）
    pub borrow_count: u64, // 总借用次数
    pub max_active: u32, // 观察到的最大活跃连接数
}
```

---

## 配置 API

基于 `serde` / `serde_yaml_ng` / `serde_json` 直接反序列化的配置管理 API。

### `DbConfig`

数据库配置结构，通过 serde 派生宏实现。

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

    #[serde(default)]
    pub cache_config: CacheConfig,
}
```

> **注意：** `DbConfig` 通过 `#[serde(flatten)]` 将 `PoolConfig` 的字段扁平化到序列化格式中，因此 YAML/JSON 中直接写 `max_connections` 等字段即可。但 Rust 代码中需通过 `config.pool_config.max_connections` 访问。

**`PoolConfig` 结构：**

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

**默认值：**

| 字段 | 默认值 | 环境变量（`from_env`） |
|-------|----------|------------|
| `max_connections` | 20 | `DB_MAX_CONNECTIONS` |
| `min_connections` | 5 | `DB_MIN_CONNECTIONS` |
| `idle_timeout` | 300 (秒) | `DB_IDLE_TIMEOUT` |
| `acquire_timeout` | 5000 (毫秒) | `DB_ACQUIRE_TIMEOUT` |
| `auto_migrate` | `false` | `DB_AUTO_MIGRATE` |
| `migration_timeout` | 60 (秒) | `DB_MIGRATION_TIMEOUT` |
| `admin_role` | `"admin"` | `DB_ADMIN_ROLE` |
| `warmup_timeout` | 30 (秒) | `DB_WARMUP_TIMEOUT` |
| `warmup_retries` | 3 | `DB_WARMUP_RETRIES` |

### `DbConfig` 配置加载方法

#### `from_yaml_str`

从 YAML 字符串加载配置（需要启用 `yaml` feature）。

```rust
#[cfg(feature = "yaml")]
pub fn from_yaml_str(yaml: &str) -> Result<DbConfig, serde_yaml_ng::Error>
```

**示例：**
```rust
#[cfg(feature = "yaml")]
let yaml = r#"
url: "sqlite::memory:"
max_connections: 20
"#;
let config = DbConfig::from_yaml_str(yaml)?;
```

#### `from_json_str`

从 JSON 字符串加载配置。

```rust
pub fn from_json_str(json: &str) -> Result<DbConfig, serde_json::Error>
```

**示例：**
```rust
let json = r#"{"url":"sqlite::memory:","max_connections":20}"#;
let config = DbConfig::from_json_str(json)?;
```

#### `from_env`

从环境变量加载配置（需要启用 `config-env` feature）。

```rust
#[cfg(feature = "config-env")]
pub fn from_env() -> Result<DbConfig, ConfigError>
```

**环境变量：**

| 变量 | 类型 | 默认值 | 描述 |
|-----------|-------|----------|-------------|
| `DATABASE_URL` | String | - | **必需**，数据库连接 URL |
| `DB_MAX_CONNECTIONS` | u32 | 20 | 最大池大小 |
| `DB_MIN_CONNECTIONS` | u32 | 5 | 最小池大小 |
| `DB_IDLE_TIMEOUT` | u64 | 300 | 空闲超时（秒） |
| `DB_ACQUIRE_TIMEOUT` | u64 | 5000 | 获取超时（毫秒） |
| `DB_PERMISSIONS_PATH` | String | - | 权限配置路径 |
| `DB_MIGRATIONS_DIR` | String | - | 迁移目录 |
| `DB_AUTO_MIGRATE` | bool | false | 启用自动迁移 |
| `DB_ADMIN_ROLE` | String | "admin" | 管理员角色名称 |
| `DB_MIGRATION_TIMEOUT` | u64 | 60 | 迁移超时（秒） |
| `DB_WARMUP_TIMEOUT` | u64 | 30 | 预热超时（秒） |
| `DB_WARMUP_RETRIES` | u32 | 3 | 预热重试次数 |

**示例：**
```bash
export DATABASE_URL="postgresql://localhost/db"
export DB_MAX_CONNECTIONS=20
export DB_ADMIN_ROLE=admin
```

```rust
#[cfg(feature = "config-env")]
let config = DbConfig::from_env()?;
```

**YAML 格式示例：**
```yaml
url: "postgresql://localhost/db"
max_connections: 20
min_connections: 5
idle_timeout: 300
acquire_timeout: 5000
auto_migrate: true
admin_role: "admin"
```

**TOML 格式示例：**
```toml
url = "postgresql://localhost/db"
max_connections = 20
min_connections = 5
idle_timeout = 300
acquire_timeout = 5000
auto_migrate = true
admin_role = "admin"
```

---

## 权限 API

### `PermissionAction`

用于权限检查的数据库操作类型。

```rust
pub enum PermissionAction {
    Select,  // SELECT 查询
    Insert,  // INSERT 语句
    Update,  // UPDATE 语句
    Delete,  // DELETE 语句
}
```

### `PermissionContext`

具有缓存和速率限制的权限检查上下文。

```rust
pub struct PermissionContext {
    // 私有字段
}
```

#### 方法

##### `new`

创建新的权限上下文。

```rust
pub fn new(role: String, policy_cache: Arc<Cache<String, RolePolicy>>) -> Self
```

##### `with_cache_size_and_rate_limit`

创建启用自定义缓存大小和速率限制的上下文。

```rust
pub async fn with_cache_size_and_rate_limit(
    role: String,
    cache_capacity: usize,
    max_requests: u32,
    window_secs: u64,
) -> Result<Self, PermissionError>
```

##### `with_config_and_rate_limit`

创建使用 `DbConfig` 配置和速率限制的上下文。

```rust
pub async fn with_config_and_rate_limit(
    role: String,
    config: &crate::foundation::config::DbConfig,
    max_requests: u32,
    window_secs: u64,
) -> Result<Self, PermissionError>
```

##### `check_table_access`

检查当前角色是否可以对表执行操作。

```rust
pub async fn check_table_access(&self, table: &str, action: &PermissionAction) -> bool
```

**返回值：**
- `bool` - 如果允许返回 `true`，如果拒绝返回 `false`

##### `load_policy`

从 YAML 字符串加载权限配置。

```rust
pub async fn load_policy(&self, config: &PermissionConfig) -> Result<(), String>
```

### `PermissionConfig`

权限配置结构。

```rust
pub struct PermissionConfig {
    pub roles: HashMap<String, RolePolicy>,
}
```

**YAML 格式：**
```yaml
roles:
  admin:
    tables:
      - name: "*"
        operations:
          - select
          - insert
          - update
          - delete

  manager:
    tables:
      - name: "users"
        operations:
          - select
          - insert
          - update

  user:
    tables:
      - name: "users"
        operations:
          - select
```

#### 方法

##### `from_yaml_str`

从 YAML 字符串解析权限配置。

```rust
#[cfg(feature = "yaml")]
pub fn from_yaml_str(yaml: &str) -> Result<Self, serde_yaml_ng::Error>
```

##### `deny_all`

创建拒绝所有访问的权限配置。

```rust
pub fn deny_all() -> Self
```

---

## 过程宏

### `#[db_entity]`

统一的属性宏，将结构体标记为数据库实体并生成 CRUD 方法、缓存、审计等功能。该宏替代了之前的 `#[derive(DbEntity)]`、`#[db_crud]`、`#[db_cache]`、`#[db_audit]` 和 `#[db_permission]` 等多个独立宏。

**必需参数：**

| 参数 | 描述 | 必需 |
|-----------|-------------|-----------|
| `table_name = "..."` | 数据库表名 | 是 |
| `primary_key = "..."` | 主键字段名 | 是 |

**可选参数：**

| 参数 | 描述 | 必需 |
|-----------|-------------|-----------|
| `timestamps = true` | 自动管理 `created_at`/`updated_at` 字段 | 否 |
| `soft_delete = true` | 自动注入 `deleted_at` 字段 | 否 |
| `validate` | 启用 validator crate 验证 | 否 |
| `cache(...)` | 启用缓存（需要 `cache` 特性） | 否 |
| `audit(...)` | 启用审计日志（需要 `audit` 特性） | 否 |
| `hooks(...)` | 配置生命周期钩子函数 | 否 |

**`cache(...)` 子参数：**

| 参数 | 描述 | 默认值 |
|-----------|-------------|-----------|
| `ttl` | 缓存存活时间（秒） | 60 |
| `strategy` | 缓存策略 | `"lru"` |
| `max_capacity` | 最大缓存容量 | 5000 |

**`audit(...)` 子参数：**

| 参数 | 描述 | 默认值 |
|-----------|-------------|-----------|
| `table_name` | 审计日志表名 | `"audit_log"` |
| `operations` | 审计的操作列表 | `["INSERT", "UPDATE", "DELETE"]` |
| `roles` | 允许审计的角色列表 | - |
| `log_values` | 是否记录字段值 | `true` |

> **注意：** 权限控制不再通过宏声明，而是由 Session 在运行时根据权限配置（YAML/JSON）进行强制执行。

**生成的方法：**

```rust
impl MyEntity {
    // 插入记录
    pub async fn insert(session: &Session, value: MyEntity) -> DbResult<MyEntity>;

    // 按主键查找（0.4.2：主键泛型化，支持 i64/Uuid/String 等任意主键类型）
    pub async fn find_by_id<PK>(session: &Session, pk: PK) -> DbResult<Option<MyEntity>>
    where PK: Into<<<Entity as sea_orm::EntityTrait>::PrimaryKey as sea_orm::entity::prelude::PrimaryKeyTrait>::ValueType>;

    // 按主键批量查找（0.4.2：主键泛型化，对接 sea-orm `is_in`）
    pub async fn find_by_ids<PK>(session: &Session, pks: Vec<PK>) -> DbResult<Vec<MyEntity>>
    where PK: Into<sea_orm::Value>;

    // 查找所有记录
    pub async fn find_all(session: &Session) -> DbResult<Vec<MyEntity>>;

    // 按条件查找
    pub async fn find_by_condition(
        session: &Session,
        condition: Condition
    ) -> DbResult<Vec<MyEntity>>;

    // 更新记录
    pub async fn update(session: &Session, value: MyEntity) -> DbResult<MyEntity>;

    // 按主键删除（0.4.2：主键泛型化，约束随 soft_delete 宏参数变化）
    //
    // soft_delete=false（默认）：PK: Into<PrimaryKey::ValueType>（对接 Entity::find_by_id）
    // soft_delete=true：         PK: Into<sea_orm::Value>（对接 Column::eq）
    //
    // soft_delete=true 实体还会额外生成 force_delete 方法（约束同 Into<sea_orm::Value>）。
    pub async fn delete<PK>(session: &Session, pk: PK) -> DbResult<()>
    where PK: Into<<<Entity as sea_orm::EntityTrait>::PrimaryKey as sea_orm::entity::prelude::PrimaryKeyTrait>::ValueType>;

    // 按条件删除
    pub async fn delete_many(session: &Session, condition: Condition) -> DbResult<u64>;

    // 记录计数
    pub async fn count(session: &Session) -> DbResult<u64>;
}
```

**示例：**

基本用法：

```rust
use dbnexus::db_entity;
use sea_orm::entity::prelude::*;

#[db_entity(table_name = "users", primary_key = "id")]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct User {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub email: String,
}

// 使用
let user = User {
    id: 1,
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
};

let inserted = User::insert(&session, user).await?;
let found = User::find_by_id(&session, 1).await?;
```

启用缓存和审计：

```rust
use dbnexus::db_entity;
use sea_orm::entity::prelude::*;

#[db_entity(
    table_name = "users",
    primary_key = "id",
    cache(ttl = 60, strategy = "lru", max_capacity = 5000),
    audit(table_name = "audit_log", operations = ["INSERT", "UPDATE", "DELETE"], log_values = true)
)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct User {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub email: String,
}
```

---

## 错误类型

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

    /// 数据验证错误（feature-gated: validation）
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

---

## 工具类型

### `ExecResult`

SQL 执行的结果。直接 re-export 自 `sea_orm`。

```rust
// src/database/pool/mod.rs
pub use sea_orm::ExecResult;
```

`ExecResult` 是 `sea_orm::ExecResult` 的类型别名，包含执行后受影响的行数等信息。

---

## 类型别名

```rust
pub type DbResult<T> = Result<T, DbError>;
pub type Operation = PermissionAction;
```

---

## 特性门控 API

### 指标（需要 `metrics` 特性）

```rust
use dbnexus::MetricsCollector;

let collector = MetricsCollector::new(&pool);
println!("{}", collector.export_prometheus());
```

### 审计（需要 `audit` 特性）

```rust
use dbnexus::AuditLogger;

let logger = AuditLogger::new("/var/log/dbnexus/audit.log");
logger.log_event(audit_event).await?;
```

### 迁移（需要 `migration` 特性）

```rust
use dbnexus::MigrationExecutor;

let executor = MigrationExecutor::new(&pool);
executor.run_migrations().await?;
```

---

## 0.3.0 新增 API

### DuckDB 连接（需要 `duckdb` 特性）

嵌入式分析型数据库支持，作为分析只读旁路接入（绕过 sea-orm，因 sea-orm 2.0.0-rc.37 不支持 DuckDB）。
v0.3.0 性能优化：使用连接池（`Vec<duckdb::Connection>`）替代单 `Mutex<Connection>`，真正并发=N。

```rust
use dbnexus::DuckDbConnection;

// 独立创建（不走 DbPool，因为 DuckDB 绕过 sea-orm）
let conn = DuckDbConnection::new("duckdb::memory:")?;
// 或指定连接池大小
let conn = DuckDbConnection::with_pool_size("duckdb://path/to/analytics.db", 8)?;

// 异步执行（内部 spawn_blocking 桥接同步 duckdb crate）
let result: DuckDbExecResult = conn.execute("INSERT INTO events VALUES (...)").await?;
let rows: Vec<DuckDbRow> = conn.query("SELECT COUNT(*) FROM events").await?;

// 健康检查与池大小查询
conn.health_check().await?;
let size = conn.pool_size();
```

**导出类型：** `DuckDbConnection`、`DuckDbRow`、`DuckDbExecResult`

**URL 格式：** `:memory:` / `duckdb::memory:`（内存）、`duckdb:path/to/file.db`、`duckdb://path/to/file.db`（文件）

### 认证系统（需要 `authentication` 特性）

JWT 认证 + 密码强度验证，基于 `jsonwebtoken` + `bcrypt`。

```rust
use dbnexus::{AuthenticationManager, JwtManager, PasswordHasher, TokenType};

let auth = AuthenticationManager::new(secret);

// 注册用户（执行 validate_strength → hash → insert 完整流程）
let user = auth.register_user("alice", "strong_password", "admin").await?;

// 认证（验证凭据并签发 access token）
let access_token = auth.authenticate(credentials).await?;

// 校验（额外校验 token_type，防止 refresh 用作 access）
let claims = auth.verify_token(&access_token)?;
```

**导出类型：** `AuthenticationManager`、`JwtManager`、`PasswordHasher`、`AuthCredentials`、`JwtClaims`、`TokenType`、`User`、`AuthError`、`AuthResult`

### 健康检查与熔断（需要 `health-check` 特性）

```rust
use dbnexus::{HealthChecker, CircuitBreaker, CircuitBreakerConfig};

let checker = HealthChecker::new(1000); // check_timeout_ms
let status: HealthCheckResult = checker.check().await;
// 熔断器：连续失败达到阈值自动打开
let breaker = CircuitBreaker::new(CircuitBreakerConfig::default());
```

**导出类型：** `HealthChecker`、`HealthStatus`、`CircuitBreaker`、`CircuitBreakerConfig`、`CircuitBreakerState`、`CircuitBreakerError`、`PoolHealthMetrics`

### 分片路由（需要 `sharding` 特性）

```rust
use dbnexus::{ShardRouter, ShardConfig, ShardingStrategy};

let router = ShardRouter::with_strategy("hash", 4);
let shard_id = router.shard_id_for_key("user_123");

// 会话级分片路由（0.3.0 新增，注意：方法在 ShardRouter 上，非 DbPool）
let session = router.get_session_for_shard("user_123", "admin").await?;

// 同时返回分片 ID（便于后续绑定检查）
let (session, shard_id) = router.get_session_for_shard_with_id("user_42", "admin").await?;
```

**API 说明：**

- `ShardRouter::with_strategy(strategy: &str, total_shards: u32) -> Self` — 按策略名创建（同步方法）
- `ShardRouter::with_config(config: &ShardConfig) -> Result<Self, DbError>` — 异步并行初始化所有分片连接池
- `router.shard_id_for_key(shard_key: &str) -> u32` — 根据 key 计算分片 ID
- `router.get_session_for_shard(shard_key, role) -> Result<Session, DbError>` — 获取分片对应的 Session（async）
- `router.get_session_for_shard_with_id(shard_key, role) -> Result<(Session, u32), DbError>` — 同时返回分片 ID
- `router.calculate_shard(timestamp, key) -> u32` — 按时间+key 计算分片

**导出类型：** `ShardRouter`、`ShardConfig`、`ShardingStrategy`、`create_strategy`

### 全局索引（需要 `global-index` 特性）

跨分片查询索引支持，基于 sea-orm 持久化到数据库。

```rust
use dbnexus::{DbPool, GlobalIndex};
use std::sync::Arc;

// 通过 DbPool 统一管理连接生命周期
let pool = DbPool::new("sqlite::memory:").await?;
let index = GlobalIndex::new(Arc::new(pool)).await?;

// 按索引键查询
let entries = index.query_by_index("users", "user_id", "user_123").await?;

// 批量同步
let result = index.batch_sync(entries).await?;
```

**导出类型：** `GlobalIndex`、`IndexEntry`、`SyncEvent`、`SyncResult`

### 权限引擎（需要 `permission-engine` + `permission` 特性）

高级策略决策点（PDP），支持 RBAC + ABAC，内置缓存与速率限制。

```rust
use dbnexus::{PolicyDecisionPoint, RbacPermissionProvider};
use std::sync::Arc;

let provider = Arc::new(RbacPermissionProvider::new());
let pdp = PolicyDecisionPoint::new(provider);
// 默认：缓存 TTL 5 分钟，速率限制 100 请求/分钟
```

**导出类型：** `PolicyDecisionPoint`、`PermissionRule`、`PermissionDecision`、`PermissionSubject`、`PermissionResource`、`RbacPermissionProvider`、`Role`

### SQL 解析器（需要 `sql-parser` 特性）

```rust
use dbnexus::{SqlParser, SqlOperationType, is_ddl_operation, contains_sql_injection};

let parser = SqlParser::new().await;
let op_type = parser.parse_operation_async("SELECT * FROM users").await?; // Option<(String, PermissionAction)>
let is_ddl = is_ddl_operation("CREATE TABLE foo (...)"); // true
let has_injection = contains_sql_injection("'; DROP TABLE--"); // true
```

**导出类型：** `SqlParser`、`SqlOperationType`、`is_ddl_operation`、`contains_sql_injection`

### 敏感数据脱敏（始终可用）

```rust
use dbnexus::{SensitiveMasker, MaskType};

let masked = SensitiveMasker::mask("alice@example.com", MaskType::Email)?;
// => "a***@example.com"
```

**导出类型：** `SensitiveMasker`、`MaskType`、`SensitiveError`

### 结构化错误报告（始终可用）

```rust
use dbnexus::QueryErrorReport;

let report = QueryErrorReport::new(...)
    .with_category(ErrorCategory::PermissionDenied)
    .with_suggestion("请授予 admin 角色 SELECT 权限");
```

**导出类型：** `QueryErrorReport`、`ErrorCategory`

### Kit 统一能力管理（始终可用）

基于 `trait-kit` 的统一能力管理入口。

```rust
use dbnexus::DbNexusKit;

let kit = DbNexusKit::new();
```

**导出类型：** `DbNexusKit`

---

## 0.4.0 新增 API

### 图数据库支持（需要 `ladybug` 或 `neo4j` 特性）

嵌入式图数据库（Ladybug）和服务器端图数据库（Neo4j）支持，通过 `GraphConnection` trait 统一抽象。图数据库与关系型数据库不互斥，可混合使用。

```rust
use dbnexus::{LadybugConnection, GraphConnection, GraphNode, GraphQueryResult};

// Ladybug 嵌入式图数据库
let conn = LadybugConnection::new("ladybug:path/to/graph.db")?;
let result = conn.execute_cypher("MATCH (n) RETURN n").await?;

// Neo4j 服务器端图数据库
use dbnexus::Neo4jConnection;
let conn = Neo4jConnection::new("neo4j://localhost:7687", "user", "pass").await?;
let result = conn.execute_cypher("MATCH (n) RETURN n").await?;
```

**导出类型：** `LadybugConnection`、`Neo4jConnection`、`GraphConnection`、`GraphExecResult`、`GraphNode`、`GraphQueryResult`、`GraphRel`、`GraphRow`、`GraphTransaction`、`GraphValue`

**Session 图事务支持：**

```rust
// Session 支持图事务（execute_cypher）
session.execute_cypher("CREATE (n:User {name: 'Alice'})").await?;
```

**权限控制：** `PermissionAction` 新增 `Traverse` 和 `Match` 变体用于图操作权限控制。

### 国际化格式化（核心特性，始终可用）

基于 ICU4X 的 locale 感知数字/日期/复数/排序格式化。

```rust
use dbnexus::{DbI18nFormatter, I18nError};

let formatter = DbI18nFormatter::new("zh-CN")?;
let formatted = formatter.format_number(1234567.89);
```

**导出类型：** `DbI18nFormatter`、`I18nError`

### trait-kit 集成（需要 `kit` 特性）

基于 `trait-kit` 0.3 的统一能力管理，将 DBNexus 作为 trait-kit 模块集成。

```rust
use dbnexus::DbNexusModule;

let module = DbNexusModule::new(/* ... */);
```

**导出类型：** `DbNexusModule`、`OxcacheDbCacheAdapter`（需要 `oxcache-integration` 特性）

---

## 重试 API

运行时重试 + 指数退避（需要 `retry` 特性）。仅对幂等查询（SELECT / SHOW / EXPLAIN）自动重试，非幂等操作直接执行不重试。

### `RetryPolicy`

重试策略配置。

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
    /// 整体 wall-clock 超时（毫秒），`None` 表示无超时限，默认 `None`
    pub overall_timeout_ms: Option<u64>,
}
```

**方法：**

- `initial_backoff() -> Duration` — 获取初始退避间隔
- `max_backoff() -> Duration` — 获取最大退避间隔

**示例：**
```rust
use dbnexus::RetryPolicy;

let policy = RetryPolicy {
    max_retries: 5,
    initial_backoff_ms: 200,
    ..Default::default()
};
```

### `RetryExecutor`

重试执行器 — 无状态，所有方法为关联函数。

```rust
pub struct RetryExecutor;
```

#### `execute_with_retry`

执行可重试操作（关联函数，非实例方法）。仅当 `sql` 被判定为幂等操作时才自动重试。

```rust
pub async fn execute_with_retry<F, Fut, T>(
    policy: &RetryPolicy,
    operation: F,
    sql: &str,
) -> Result<T, RetryError>
where
    F: Fn() -> Fut + Send + Sync,
    Fut: Future<Output = Result<T, DbError>> + Send,
    T: Send,
```

**参数：**
- `policy: &RetryPolicy` — 重试策略配置
- `operation: F` — 异步闭包，执行实际操作
- `sql: &str` — SQL 字符串，用于幂等性判断

**退避策略：** 第 N 次重试的等待时间 = `min(initial_backoff * multiplier^N, max_backoff)`，当 `jitter = true` 时添加 ±25% 的随机抖动。

**示例：**
```rust
use dbnexus::{RetryExecutor, RetryPolicy};

let policy = RetryPolicy::default();
let result = RetryExecutor::execute_with_retry(&policy, || {
    async move { Ok("success") }
}, "SELECT * FROM users").await;
```

### `RetryError`

重试过程中的错误类型。

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

### `is_idempotent_operation`

判断 SQL 操作是否为幂等操作（可安全重试）。

```rust
pub fn is_idempotent_operation(sql: &str) -> bool
```

`SELECT`、`SHOW`、`EXPLAIN` 为幂等操作，其余（INSERT / UPDATE / DELETE / DDL）均视为非幂等。零分配实现，直接字节级前缀比较。

**示例：**
```rust
use dbnexus::is_idempotent_operation;

assert!(is_idempotent_operation("SELECT * FROM users"));
assert!(!is_idempotent_operation("INSERT INTO users VALUES (1)"));
```

**导出类型：** `RetryPolicy`、`RetryExecutor`、`RetryError`、`is_idempotent_operation`

---

更多详细文档，请参见：
- [用户指南](USER_GUIDE.md)
- [架构](ARCHITECTURE.md)
- [Rust 文档](https://docs.rs/dbnexus)
