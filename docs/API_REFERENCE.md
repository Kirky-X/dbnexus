# DB Nexus API 参考

## 目录

- [核心类型](#核心类型)
- [宏属性](#宏属性)
- [配置类型](#配置类型)
- [错误类型](#错误类型)
- [可选模块 API](#可选模块-api)

---

## 核心类型

### DbPool

数据库连接池管理器，负责创建和管理数据库连接。

```rust
pub struct DbPool;
```

#### 方法

```rust
impl DbPool {
    /// 从连接字符串创建连接池
    pub async fn new(url: &str) -> Result<Self, DbError>;

    /// 从环境变量加载配置并创建连接池
    pub async fn new() -> Result<Self, DbError>;

    /// 使用自定义配置创建连接池
    pub async fn with_config(config: DbConfig) -> Result<Self, DbError>;

    /// 获取指定角色的 Session
    pub async fn get_session(&self, role: &str) -> Result<Session, DbError>;

    /// 获取只读 Session
    pub async fn get_read_session(&self) -> Result<Session, DbError>;

    /// 获取连接池状态
    pub fn status(&self) -> PoolStatus;

    /// 关闭连接池
    pub async fn close(&self);
}
```

#### PoolStatus

```rust
pub struct PoolStatus {
    pub total: u32,      // 总连接数
    pub active: u32,     // 活跃连接数
    pub idle: u32,       // 空闲连接数
    pub waiters: u32,    // 等待获取连接的请求数
}
```

### Session

数据库会话包装器，提供 RAII 风格的连接管理。

```rust
pub struct Session {
    // 私有字段
}
```

#### 方法

```rust
impl Session {
    /// 获取连接
    pub fn connection(&self) -> &sea_orm::prelude::Connection;

    /// 开始事务
    pub async fn transaction<F, T, E, Fut>(
        &self,
        f: F,
    ) -> Result<T, DbError>
    where
        F: FnMut(&Session) -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: Into<DbError>;

    /// 提交事务
    pub async fn commit(&self) -> Result<(), DbError>;

    /// 回滚事务
    pub async fn rollback(&self) -> Result<(), DbError>;

    /// 检查是否在事务中
    pub fn in_transaction(&self) -> bool;

    /// 获取角色
    pub fn role(&self) -> &str;

    /// 执行原始 SQL
    pub async fn execute_raw(
        &self,
        sql: &str,
        params: Vec<sea_orm::prelude::Value>,
    ) -> Result<sea_orm::prelude::ExecResult, DbError>;

    /// 获取底层连接
    pub fn conn(&self) -> &(dyn sea_orm::prelude::Connection + Send + Sync);
}
```

### DbEntity 派生宏生成的类型

当使用 `#[derive(DbEntity)]` 时，会自动生成以下类型：

```rust
/// Sea-ORM Entity
pub struct Entity;

/// ActiveModel - 用于插入和更新
#[derive(Clone, Default)]
pub struct ActiveModel {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub email: Option<String>,
    // ...
}

/// Model - 数据库记录类型
#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub id: i64,
    pub name: String,
    pub email: String,
    // ...
}
```

---

## 宏属性

### #[derive(DbEntity)]

将 Rust struct 映射为 Sea-ORM Entity。

```rust
use dbnexus::DbEntity;

#[derive(DbEntity)]
#[db_entity]
#[table_name = "users")]
struct User {
    #[primary_key]
    id: i64,
    name: String,
    email: String,
}
```

#### 可用属性

| 属性 | 描述 |
|------|------|
| `#[db_entity]` | 标记为数据库实体 |
| `#[table_name = "..."]` | 指定数据库表名 |
| `#[primary_key]` | 标记主键字段 |
| `#[column_name = "..."]` | 指定列名 |

### #[db_crud]

自动生成 CRUD 方法（每次操作前自动检查权限）。

```rust
use dbnexus::{DbEntity, db_crud};

#[derive(DbEntity)]
#[db_entity]
#[table_name = "users")]
#[db_crud]
struct User {
    #[primary_key]
    id: i64,
    name: String,
    email: String,
}
```

#### 生成的方法

```rust
impl User {
    /// 插入新记录
    pub async fn insert(
        session: &Session,
        entity: Model,
    ) -> Result<Model, DbError>;

    /// 根据 ID 查询
    pub async fn find_by_id(
        session: &Session,
        id: i64,
    ) -> Result<Model, DbError>;

    /// 更新记录
    pub async fn update(
        session: &Session,
        entity: Model,
    ) -> Result<Model, DbError>;

    /// 根据 ID 删除
    pub async fn delete(
        session: &Session,
        id: i64,
    ) -> Result<u64, DbError>;

    /// 查询所有记录
    pub async fn find_all(
        session: &Session,
    ) -> Result<Vec<Model>, DbError>;

    /// 批量删除
    pub async fn delete_many(
        session: &Session,
        filter: String,
    ) -> Result<u64, DbError>;

    /// 统计数量
    pub async fn count(
        session: &Session,
    ) -> Result<u64, DbError>;
}
```

### #[db_permission]

声明允许访问的角色和操作。

```rust
use dbnexus::{DbEntity, db_crud, db_permission};

#[derive(DbEntity)]
#[db_entity]
#[table_name = "users")]
#[db_crud]
#[db_permission(role = "admin", actions = ["read", "write", "delete"])]
#[db_permission(role = "user", actions = ["read"])]
struct User {
    #[primary_key]
    id: i64,
    name: String,
    email: String,
}
```

### #[db_audit]

启用审计日志记录。

```rust
use dbnexus::{DbEntity, db_audit};

#[derive(DbEntity)]
#[db_entity]
#[table_name = "users")]
#[db_audit(operations = ["CREATE", "UPDATE", "DELETE"])]
struct User {
    #[primary_key]
    id: i64,
    name: String,
    email: String,
}
```

### #[db_cache]

启用实体缓存。

```rust
use dbnexus::{DbEntity, db_cache};

#[derive(DbEntity)]
#[db_entity]
#[table_name = "users")]
#[db_cache(ttl = 300, capacity = 1000)]
struct User {
    #[primary_key]
    id: i64,
    name: String,
    email: String,
}
```

### #[global_index]

启用全局索引。

```rust
use dbnexus::DbEntity;

#[derive(DbEntity)]
#[db_entity]
#[table_name = "orders")]
#[global_index(fields = ["user_id", "product_id"])]
struct Order {
    #[primary_key]
    id: i64,
    user_id: i64,
    product_id: i64,
    amount: Decimal,
}
```

---

## 配置类型

### DbConfig

数据库连接配置。

```rust
use dbnexus::DbConfig;

pub struct DbConfig {
    pub url: String,
    pub database_type: DatabaseType,
    pub max_connections: u32,
    pub min_connections: u32,
    pub idle_timeout: u64,
    pub acquire_timeout: u64,
}
```

#### 方法

```rust
impl DbConfig {
    /// 从环境变量加载配置
    pub fn from_env() -> Result<Self, DbError>;

    /// 从连接字符串创建配置
    pub fn new(url: &str) -> Self;

    /// 验证配置
    pub fn validate(&self) -> Result<(), DbError>;
}
```

### PoolConfig

连接池配置。

```rust
use dbnexus::PoolConfig;

pub struct PoolConfig {
    pub max_connections: u32,
    pub min_connections: u32,
    pub idle_timeout: u64,
    pub acquire_timeout: u64,
}
```

#### 方法

```rust
impl PoolConfig {
    pub fn new() -> Self;

    pub fn max_connections(mut self, n: u32) -> Self;

    pub fn min_connections(mut self, n: u32) -> Self;

    pub fn idle_timeout(mut self, seconds: u64) -> Self;

    pub fn acquire_timeout(mut self, millis: u64) -> Self;
}
```

### DatabaseType

数据库类型枚举。

```rust
use dbnexus::DatabaseType;

pub enum DatabaseType {
    SQLite,
    PostgreSQL,
    MySQL,
}

impl DatabaseType {
    pub fn as_str(&self) -> &str;

    pub fn from_str(s: &str) -> Option<Self>;
}
```

---

## 错误类型

### DbError

数据库错误类型。

```rust
use dbnexus::DbError;

pub enum DbError {
    /// 连接失败
    ConnectionFailed(String),

    /// 连接池耗尽
    PoolExhausted,

    /// 连接超时
    ConnectionTimeout,

    /// 权限被拒绝
    PermissionDenied(String),

    /// 配置错误
    ConfigError(String),

    /// 迁移错误
    MigrationError(String),

    /// 事务错误
    TransactionError(String),

    /// 缓存错误
    CacheError(String),

    /// 审计错误
    AuditError(String),

    /// 未知错误
    Unknown(String),
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result;
}

impl std::error::Error for DbError {}
```

### DbResult

结果类型的别名。

```rust
pub type DbResult<T> = Result<T, DbError>;
```

---

## 可选模块 API

### 缓存模块 (cache)

#### CacheConfig

```rust
use dbnexus::cache::CacheConfig;

pub struct CacheConfig {
    pub ttl: u64,           // 过期时间（秒）
    pub capacity: usize,    // 最大容量
    pub strategy: LruStrategy,
}
```

#### CacheManager

```rust
use dbnexus::cache::CacheManager;

pub trait CacheManager {
    async fn get(&self, key: &str) -> Option<Vec<u8>>;

    async fn set(&self, key: &str, value: &[u8], ttl: u64) -> Result<(), DbError>;

    async fn delete(&self, key: &str) -> Result<(), DbError>;

    async fn clear(&self) -> Result<(), DbError>;

    async fn exists(&self, key: &str) -> bool;
}
```

### 审计模块 (audit)

#### AuditLog

```rust
use dbnexus::audit::AuditLog;

pub struct AuditLog {
    pub id: i64,
    pub user_id: Option<i64>,
    pub operation: String,
    pub table_name: String,
    pub record_id: Option<String>,
    pub old_value: Option<Json>,
    pub new_value: Option<Json>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub ip_address: Option<String>,
}

impl AuditLog {
    pub async fn query() -> AuditLogQuery;

    pub async fn create(
        session: &Session,
        log: &CreateAuditLog,
    ) -> Result<Self, DbError>;
}
```

### 分片模块 (sharding)

#### ShardConfig

```rust
use dbnexus::sharding::ShardConfig;

pub struct ShardConfig {
    pub name: String,
    pub num_shards: u32,
    pub table_name: String,
    pub connection_template: String,
    pub strategy: ShardingStrategy,
}
```

#### ShardRouter

```rust
use dbnexus::sharding::ShardRouter;

pub struct ShardRouter {
    // 私有字段
}

impl ShardRouter {
    pub fn with_config(config: &ShardConfig) -> Self;

    pub fn route_by_hash(&self, key: &[u8]) -> u32;

    pub fn route_by_time(&self, time: chrono::DateTime<chrono::Utc>) -> u32;

    pub fn get_shard_connection(&self, shard_id: u32) -> Result<DbPool, DbError>;
}
```

#### ShardingStrategy

```rust
use dbnexus::sharding::ShardingStrategy;

pub enum ShardingStrategy {
    /// 时间分片
    Yearly {
        current_year: i32,
    },

    /// 哈希分片
    Hash {
        algorithm: HashAlgorithm,
        num_shards: u32,
    },

    /// 范围分片
    Range {
        start_id: i64,
        shard_size: i64,
    },

    /// 地理位置分片
    Geo {
        region: String,
    },
}
```

### 全局索引模块 (global-index)

#### GlobalIndex

```rust
use dbnexus::global_index::GlobalIndex;

pub struct GlobalIndex {
    pub name: String,
    pub fields: Vec<String>,
    pub unique: bool,
}

impl GlobalIndex {
    pub fn new(name: &str, fields: Vec<&str>) -> Self;

    pub async fn create(&self, session: &Session) -> Result<(), DbError>;

    pub async fn lookup(
        &self,
        session: &Session,
        values: &[sea_orm::prelude::Value],
    ) -> Result<Option<i64>, DbError>;

    pub async fn insert(
        &self,
        session: &Session,
        record_id: i64,
        values: &[sea_orm::prelude::Value],
    ) -> Result<(), DbError>;

    pub async fn delete(
        &self,
        session: &Session,
        record_id: i64,
    ) -> Result<(), DbError>;
}
```

### 指标模块 (metrics)

#### MetricsConfig

```rust
use dbnexus::metrics::MetricsConfig;

pub struct MetricsConfig {
    pub enabled: bool,
    pub port: u16,
    pub path: String,
}
```

#### Metrics

```rust
use dbnexus::metrics;

pub fn register_histogram(
    name: &str,
    help: &str,
) -> Result<metrics::Histogram, DbError>;

pub fn register_counter(
    name: &str,
    help: &str,
) -> Result<metrics::Counter, DbError>;

pub fn register_gauge(
    name: &str,
    help: &str,
) -> Result<metrics::Gauge, DbError>;
```

### 权限引擎模块 (permission-engine)

#### PolicyDecisionPoint

```rust
use dbnexus::permission_engine::{PolicyDecisionPoint, PermissionContext};

pub struct PolicyDecisionPoint {
    // 私有字段
}

impl PolicyDecisionPoint {
    pub fn new(provider: Arc<dyn PermissionProvider>) -> Self;

    pub async fn check_permission(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> Result<PermissionDecision, DbError>;

    pub async fn check_permission_with_context(
        &self,
        role: &str,
        resource: &str,
        action: &str,
        context: &PermissionContext,
    ) -> Result<PermissionDecision, DbError>;
}
```

#### PermissionProvider

```rust
use dbnexus::permission_engine::PermissionProvider;

pub trait PermissionProvider: Send + Sync {
    fn get_role_permissions(&self, role: &str) -> Option<Vec<TablePermission>>;

    fn get_all_roles(&self) -> Vec<String>;

    fn has_role(&self, role: &str) -> bool;
}
```

#### YamlPermissionProvider

```rust
use dbnexus::permission_engine::YamlPermissionProvider;

pub struct YamlPermissionProvider {
    // 私有字段
}

impl YamlPermissionProvider {
    pub fn new(path: &str) -> Result<Self, DbError>;

    pub fn with_content(content: &str) -> Result<Self, DbError>;
}
```

#### RbacPermissionProvider

```rust
use dbnexus::permission_engine::RbacPermissionProvider;

pub struct RbacPermissionProvider {
    // 私有字段
}

impl RbacPermissionProvider {
    pub fn new(roles: Vec<Role>, permissions: Vec<Permission>) -> Self;
}
```

### 追踪模块 (tracing)

#### TracingConfig

```rust
use dbnexus::tracing::TracingConfig;

pub struct TracingConfig {
    pub enabled: bool,
    pub service_name: String,
    pub exporter: ExporterType,
    pub sample_rate: f64,
}
```

#### ExporterType

```rust
use dbnexus::tracing::ExporterType;

pub enum ExporterType {
    /// OpenTelemetry OTLP 导出器
    OTLP,

    /// Jaeger 导出器
    Jaeger,

    /// Zipkin 导出器
    Zipkin,

    /// 控制台导出器（开发用）
    Console,
}
```

---

## 类型别名

### Operation

`PermissionAction` 的别名，用于简化使用。

```rust
use dbnexus::Operation;

pub type Operation = PermissionAction;
```

### PermissionAction

权限操作类型。

```rust
use dbnexus::PermissionAction;

pub enum PermissionAction {
    Select,
    Insert,
    Update,
    Delete,
    All,
}
```

### RolePolicy

角色策略类型。

```rust
use dbnexus::RolePolicy;

pub struct RolePolicy {
    pub name: String,
    pub inherits: Vec<String>,
    pub permissions: Vec<TablePermission>,
}
```

### PermissionConfig

权限配置类型。

```rust
use dbnexus::PermissionConfig;

pub struct PermissionConfig {
    pub roles: HashMap<String, RolePolicy>,
    pub resources: HashMap<String, ResourcePermission>,
}
```
