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
let config = DbConfigBuilder::new()
    .url("postgresql://localhost/db")
    .max_connections(20)
    .build()?;

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
let config = DbConfigBuilder::new()
    .url("postgresql://localhost/db")
    .max_connections(20)
    .build()?;

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

```rust
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
pub async fn execute(&mut self, sql: &str) -> DbResult<ExecResult>
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
pub async fn begin_transaction(&mut self) -> DbResult<()>
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
pub async fn commit(&mut self) -> DbResult<()>
```

##### `rollback`

回滚当前事务。

```rust
pub async fn rollback(&mut self) -> DbResult<()>
```

##### `is_in_transaction`

检查当前是否在事务中。

```rust
pub fn is_in_transaction(&self) -> bool
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

    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    #[serde(default = "default_min_connections")]
    pub min_connections: u32,

    #[serde(default = "default_idle_timeout")]
    pub idle_timeout: u64,

    #[serde(default = "default_acquire_timeout")]
    pub acquire_timeout: u64,

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
pub fn new(role: String, cache: Arc<AsyncMutex<LruCache<String, RolePolicy>>>) -> Self
```

##### `with_rate_limiter`

创建启用速率限制的上下文。

```rust
pub fn with_rate_limiter(
    role: String,
    cache: Arc<AsyncMutex<LruCache<String, RolePolicy>>>,
    limiter: Arc<RateLimiter>,
) -> Self
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
pub async fn load_policy(&self, yaml: &str) -> DbResult<()>
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

##### `from_yaml`

从 YAML 字符串解析权限配置。

```rust
pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error>
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

    // 按主键查找
    pub async fn find_by_id(session: &Session, id: i64) -> DbResult<Option<MyEntity>>;

    // 查找所有记录
    pub async fn find_all(session: &Session) -> DbResult<Vec<MyEntity>>;

    // 按条件查找
    pub async fn find_by_condition(
        session: &Session,
        condition: Condition
    ) -> DbResult<Vec<MyEntity>>;

    // 更新记录
    pub async fn update(session: &Session, value: MyEntity) -> DbResult<MyEntity>;

    // 按主键删除
    pub async fn delete(session: &Session, id: i64) -> DbResult<()>;

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
    Connection(String),
    ConnectionPool(String),
    Permission(String),
    SqlParse(SqlParseError),
    Transaction(String),
    Migration(String),
    Validation(String),
    Database(sea_orm::DbErr),
    Internal(String),
}
```

### `ConfigError`

配置相关错误。

```rust
pub enum ConfigError {
    FileNotFound(PathBuf),
    InvalidFormat(String),
    MissingField(String),
    EnvVarError(String),
    IoError(io::Error),
    InvalidUrl(String),
    UnsupportedProtocol(String),
    ValidationFailed(String),
}
```

### `SqlParseError`

SQL 解析错误。

```rust
pub struct SqlParseError {
    pub message: String,
    pub sql: String,
}
```

---

## 工具类型

### `ExecResult`

SQL 执行的结果。

```rust
pub struct ExecResult {
    pub rows_affected: u64,
    pub last_insert_id: Option<i64>,
}
```

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
use dbnexus::metrics::MetricsCollector;

let collector = MetricsCollector::new(&pool);
println!("{}", collector.export_prometheus());
```

### 审计（需要 `audit` 特性）

```rust
use dbnexus::audit::AuditLogger;

let logger = AuditLogger::new("/var/log/dbnexus/audit.log");
logger.log_event(audit_event).await?;
```

### 迁移（需要 `migration` 特性）

```rust
use dbnexus::migration::MigrationExecutor;

let executor = MigrationExecutor::new(&pool);
executor.run_migrations().await?;
```

---

更多详细文档，请参见：
- [用户指南](USER_GUIDE.md)
- [架构](ARCHITECTURE.md)
- [Rust 文档](https://docs.rs/dbnexus)
