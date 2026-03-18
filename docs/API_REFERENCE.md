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

基于 [confers](https://crates.io/crates/confique) 库实现的配置管理 API。

### `DbConfig`

数据库配置结构，通过 confers 派生宏实现。

```rust
#[cfg_attr(feature = "confers", derive(Config))]
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct DbConfig {
    #[cfg_attr(feature = "confers", config(env = "DATABASE_URL"))]
    pub url: String,

    #[cfg_attr(feature = "confers", config(default = DEFAULT_MAX_CONNECTIONS, env = "DB_MAX_CONNECTIONS"))]
    pub max_connections: u32,

    #[cfg_attr(feature = "confers", config(default = DEFAULT_MIN_CONNECTIONS, env = "DB_MIN_CONNECTIONS"))]
    pub min_connections: u32,

    #[cfg_attr(feature = "confers", config(default = DEFAULT_IDLE_TIMEOUT, env = "DB_IDLE_TIMEOUT"))]
    pub idle_timeout: u64,

    #[cfg_attr(feature = "confers", config(default = DEFAULT_ACQUIRE_TIMEOUT, env = "DB_ACQUIRE_TIMEOUT"))]
    pub acquire_timeout: u64,

    #[cfg_attr(feature = "confers", config(env = "DB_PERMISSIONS_PATH"))]
    pub permissions_path: Option<String>,

    #[cfg_attr(feature = "confers", config(env = "DB_MIGRATIONS_DIR"))]
    pub migrations_dir: Option<PathBuf>,

    #[cfg_attr(feature = "confers", config(default = false, env = "DB_AUTO_MIGRATE"))]
    pub auto_migrate: bool,

    #[cfg_attr(feature = "confers", config(default = DEFAULT_MIGRATION_TIMEOUT, env = "DB_MIGRATION_TIMEOUT"))]
    pub migration_timeout: u64,

    #[cfg_attr(feature = "confers", config(default = DEFAULT_ADMIN_ROLE, env = "DB_ADMIN_ROLE"))]
    pub admin_role: String,

    #[cfg_attr(feature = "confers", config(default = DEFAULT_WARMUP_TIMEOUT, env = "DB_WARMUP_TIMEOUT"))]
    pub warmup_timeout: u64,

    #[cfg_attr(feature = "confers", config(default = DEFAULT_WARMUP_RETRIES, env = "DB_WARMUP_RETRIES"))]
    pub warmup_retries: u32,
}
```

**默认值：**

| 字段 | 默认值 | 环境变量 |
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

### `DbConfigBuilder`

基于 confers 库的配置构建器，提供链式 API 用于构建 `DbConfig` 实例。

```rust
#[cfg(feature = "confers")]
pub struct DbConfigBuilder {
    builder: confique::Builder<'static, DbConfig>,
}
```

#### 方法

##### `new`

创建新的构建器实例。

```rust
#[cfg(feature = "confers")]
pub fn new() -> Self
```

##### `url`

设置数据库连接 URL（通过环境变量传递给 confers）。

```rust
#[cfg(feature = "confers")]
pub fn url(self, url: &str) -> Self
```

##### `max_connections`

设置最大池大小（通过环境变量传递给 confers）。

```rust
#[cfg(feature = "confers")]
pub fn max_connections(self, max: u32) -> Self
```

##### `min_connections`

设置最小池大小（通过环境变量传递给 confers）。

```rust
#[cfg(feature = "confers")]
pub fn min_connections(self, min: u32) -> Self
```

##### `idle_timeout`

设置空闲连接超时（秒）（通过环境变量传递给 confers）。

```rust
#[cfg(feature = "confers")]
pub fn idle_timeout(self, timeout: u64) -> Self
```

##### `acquire_timeout`

设置连接获取超时（毫秒）（通过环境变量传递给 confers）。

```rust
#[cfg(feature = "confers")]
pub fn acquire_timeout(self, timeout: u64) -> Self
```

##### `permissions_path`

设置权限配置文件的路径（通过环境变量传递给 confers）。

```rust
#[cfg(feature = "confers")]
pub fn permissions_path(self, path: &str) -> Self
```

##### `auto_migrate`

启用自动数据库迁移（通过环境变量传递给 confers）。

```rust
#[cfg(feature = "confers")]
pub fn auto_migrate(self, enabled: bool) -> Self
```

##### `admin_role`

设置管理员角色名称（通过环境变量传递给 confers）。

```rust
#[cfg(feature = "confers")]
pub fn admin_role(self, role: &str) -> Self
```

##### `file`

添加配置文件源（confers 特性）。

```rust
#[cfg(feature = "confers")]
pub fn file<P: AsRef<Path>>(self, path: P) -> Self
```

##### `env`

启用环境变量加载（confers 特性）。

```rust
#[cfg(feature = "confers")]
pub fn env(self) -> Self
```

##### `build`

构建 `DbConfig` 实例。

```rust
#[cfg(feature = "confers")]
pub fn build(self) -> Result<DbConfig, ConfigError>
```

**示例：**
```rust
#[cfg(feature = "confers")]
let config = DbConfigBuilder::new()
    .url("postgresql://localhost/db")
    .max_connections(20)
    .min_connections(5)
    .idle_timeout(300)
    .acquire_timeout(5000)
    .permissions_path("/etc/dbnexus/permissions.yaml")
    .auto_migrate(true)
    .admin_role("superuser")
    .build()?;
```

### `DbConfig` 配置加载方法

#### `from_env`

从环境变量加载配置（基于 confers 实现）。

```rust
#[cfg(feature = "confers")]
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
#[cfg(feature = "confers")]
let config = DbConfig::from_env()?;
```

#### `from_file`

从配置文件加载配置（基于 confers 实现）。

```rust
#[cfg(feature = "confers")]
pub fn from_file<P: AsRef<Path>>(path: P) -> Result<DbConfig, ConfigError>
```

**支持格式：** TOML, YAML, JSON (取决于启用的 confers 特性)

**示例：**
```toml
# config.toml
url = "postgresql://localhost/db"
max_connections = 20
min_connections = 5
idle_timeout = 300
acquire_timeout = 5000
auto_migrate = true
admin_role = "admin"
```

#### `builder`

获取 confers 构建器用于多源配置加载。

```rust
#[cfg(feature = "confers")]
pub fn builder() -> confique::Builder<'static, Self>
```

**示例：**
```rust
#[cfg(feature = "confers")]
let config = DbConfig::builder()
    .env()
    .file("config.toml")
    .load()?;
```

> **注意**：原有的 `from_yaml_file` 和 `from_toml_file` 方法已被移除，
> 请使用 `from_file` 方法配合相应的 confers 特性。

**TOML 格式：**
```toml
url = "postgresql://localhost/db"
max_connections = 20
min_connections = 5
idle_timeout = 300
acquire_timeout = 5000
auto_migrate = true
admin_role = "admin"
```

#### `from_config_files`

自动检测并从标准路径加载配置。

```rust
pub fn from_config_files() -> Result<DbConfig, ConfigError>
```

**搜索顺序：**
1. `./dbnexus.yaml`
2. `./dbnexus.toml`
3. `./config/dbnexus.yaml`
4. `./config/dbnexus.toml`
5. `~/.config/dbnexus/config.yaml`
6. `~/.dbnexus/config.toml`

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

### `#[derive(DbEntity)]`

将结构体标记为数据库实体的派生宏。

**必需属性：**

| 属性 | 描述 | 必需 |
|-----------|-------------|-----------|
| `#[derive(DbEntity, DeriveEntityModel, DeriveModel, DeriveActiveModel)]` | 启用 DBNexus 和 Sea-ORM 实体特性 | 是 |
| `#[sea_orm(table_name = "...")]` | 数据库表名 | 是 |
| `#[sea_orm(primary_key)]` | 标记主键字段 | 是 |

**可选属性：**

| 属性 | 描述 | 必需 |
|-----------|-------------|-----------|
| `#[db_crud]` | 生成 CRUD 方法 | 否 |
| `#[db_permission(...)]` | 添加权限控制 | 否 |
| `#[db_cache]` | 启用缓存（需要 `cache` 特性） | 否 |
| `#[db_audit]` | 启用审计日志（需要 `audit` 特性） | 否 |

### `#[db_crud]`

自动为实体生成 CRUD 方法。

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
```rust
use dbnexus::{DbEntity, db_crud};
use sea_orm::entity::prelude::*;

#[derive(DbEntity, DeriveEntityModel, DeriveModel, DeriveActiveModel)]
#[sea_orm(table_name = "users")]
#[db_crud]
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

### `#[db_permission]`

声明实体的基于角色的访问控制。

**属性：**

```rust
#[db_permission(
    roles = ["admin", "manager"],
    operations = ["SELECT", "INSERT", "UPDATE"],
    config = "permissions.yaml"
)]
```

**参数：**

| 参数 | 类型 | 必需 | 描述 |
|-----------|-------|-----------|-------------|
| `roles` | `Vec<&str>` | 是 | 允许访问此实体的角色列表 |
| `operations` | `Vec<&str>` | 否 | 允许的操作列表（SELECT, INSERT, UPDATE, DELETE） |
| `config` | `&str` | 否 | 用于编译时验证的权限配置文件路径 |

**生成的方法：**

```rust
impl MyEntity {
    pub const ALLOWED_ROLES: &[&str] = &["admin", "manager"];
    pub const ALLOWED_OPERATIONS: &[&str] = &["SELECT", "INSERT", "UPDATE"];

    pub fn check_permission(ctx: &PermissionContext) -> DbResult<()>;
    pub fn check_operation(ctx: &PermissionContext, op: &PermissionAction) -> DbResult<()>;
}

// 使用
let session = pool.get_session("admin").await?;
User::find_all(&session).await?; // OK
let session = pool.get_session("guest").await?;
User::find_all(&session).await?; // 错误：权限被拒绝
```

### `#[db_cache]`

启用实体查询的缓存（需要 `cache` 特性）。

**生成的方法：**

```rust
impl MyEntity {
    pub async fn find_cached(session: &Session, id: i64) -> DbResult<Option<MyEntity>>;
    pub async fn invalidate_cache(session: &Session, id: i64) -> DbResult<()>;
}
```

### `#[db_audit]`

启用实体操作的审计日志（需要 `audit` 特性）。

**效果：**
- 所有 CRUD 操作都记录到审计跟踪中
- 包括操作类型、时间戳、用户角色和结果

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
