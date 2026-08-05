# DBNexus 用户指南

在应用程序中使用 DBNexus 的完整指南。

## 目录

- [入门指南](#入门指南)
- [安装](#安装)
- [配置](#配置)
- [定义实体](#定义实体)
- [使用连接](#使用连接)
- [CRUD 操作](#crud-操作)
- [权限控制](#权限控制)
- [事务](#事务)
- [高级特性](#高级特性)
- [最佳实践](#最佳实践)
- [故障排除](#故障排除)

---

## 入门指南

本指南将带您从零开始到生产就绪的数据库使用。

### 您将学到

- 如何在项目中设置 DBNexus
- 如何定义数据库实体
- 如何执行 CRUD 操作
- 如何实现基于角色的访问控制
- 如何配置和优化连接
- 如何使用缓存和指标等高级特性

### 先决条件

- Rust 1.91 或更高版本
- Rust 和 SQL 基础知识
- 数据库（PostgreSQL、MySQL 或 SQLite）

---

## 安装

### 1. 添加依赖

添加到您的 `Cargo.toml`：

```toml
[dependencies]
dbnexus = "0.5"
tokio = { version = "1.52", features = ["rt-multi-thread", "macros"] }
```

### 2. 选择特性

选择您需要的特性：

```toml
# 嵌入式设备最小配置
dbnexus = { version = "0.5", default-features = false, features = ["embedded"] }

# 带企业特性的 PostgreSQL
dbnexus = { version = "0.5", features = [
    "postgres",
    "permission",
    "metrics",
    "tracing",
    "audit"
] }

# 带基础特性的 SQLite
dbnexus = { version = "0.5", features = ["sqlite", "permission", "sql-parser"] }
```

完整特性列表请参见 [README.md](../README.md#feature-flags)。

### 3. 启用数据库驱动

选择一个数据库驱动：

```toml
# SQLite（默认）
dbnexus = { version = "0.5", features = ["sqlite"] }

# PostgreSQL
dbnexus = { version = "0.5", features = ["postgres"] }

# MySQL
dbnexus = { version = "0.5", features = ["mysql"] }

# DuckDB（嵌入式分析型数据库）
dbnexus = { version = "0.5", features = ["duckdb"] }

# Ladybug（嵌入式图数据库）
dbnexus = { version = "0.5", features = ["ladybug"] }

# Neo4j（图数据库服务器）
dbnexus = { version = "0.5", features = ["neo4j"] }
```

**重要：**关系型数据库驱动（SQLite/PostgreSQL/MySQL/DuckDB）之间一次只能启用一个。图数据库驱动（Ladybug/Neo4j）可与关系型驱动共存。

### 4. 验证安装

```rust
use dbnexus::DbPool;

fn main() {
    println!("DBNexus 已准备就绪！");
}
```

---

## 配置

### 使用环境变量快速开始

配置 DBNexus 最简单的方法是使用环境变量。

#### 步骤 1：设置环境变量

```bash
export DATABASE_URL="postgresql://user:password@localhost/mydb"
export DB_MAX_CONNECTIONS=20
export DB_MIN_CONNECTIONS=5
export DB_ADMIN_ROLE=admin
```

#### 步骤 2：加载配置

```rust
use dbnexus::DbPool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("postgresql://localhost/mydb").await?;
    println!("已连接！");
    Ok(())
}
```

### 使用配置文件

#### YAML 配置

创建 `dbnexus.yaml`：

```yaml
url: "postgresql://localhost/mydb"
max_connections: 20
min_connections: 5
idle_timeout: 300
acquire_timeout: 5000
auto_migrate: true
admin_role: admin
```

#### 加载 YAML 配置

```rust
use dbnexus::DbConfig;

// 从 YAML 字符串加载（需要 yaml feature）
#[cfg(feature = "yaml")]
let yaml_content = std::fs::read_to_string("dbnexus.yaml")?;
#[cfg(feature = "yaml")]
let config = DbConfig::from_yaml_str(&yaml_content)?;
let pool = DbPool::with_config(config).await?;
```

#### TOML 配置

创建 `dbnexus.toml`：

```toml
url = "postgresql://localhost/mydb"
max_connections = 20
min_connections = 5
idle_timeout = 300
acquire_timeout = 5000
auto_migrate = true
admin_role = "admin"
```

#### 加载 TOML 配置

```rust
#[cfg(feature = "config-toml")]
use dbnexus::DbConfig;

// 从 JSON 字符串加载（DbConfig 支持 serde_json 反序列化）
let json_content = std::fs::read_to_string("dbnexus.json")?;
let config = DbConfig::from_json_str(&json_content)?;
let pool = DbPool::with_config(config).await?;
```

### 使用程序化配置

对于程序化配置，直接构造 `DbConfig` 结构体：

```rust
use dbnexus::{DbPool, DbConfig, PoolConfig};

let config = DbConfig {
    url: "postgresql://localhost/mydb".to_string(),
    pool_config: PoolConfig {
        max_connections: 20,
        min_connections: 5,
        idle_timeout: 300,
        acquire_timeout: 5000,
    },
    admin_role: "admin".to_string(),
    auto_migrate: true,
    ..Default::default()
};

let pool = DbPool::with_config(config).await?;
```

### 配置参数

| 参数 | 类型 | 默认值 | 描述 |
|-----------|-------|----------|-------------|
| `url` | `String` | 必需 | 数据库连接 URL |
| `max_connections` | `u32` | 20 | 最大池大小 |
| `min_connections` | `u32` | 5 | 最小池大小 |
| `idle_timeout` | `u64` | 300 | 空闲连接超时（秒） |
| `acquire_timeout` | `u64` | 5000 | 连接获取超时（毫秒） |
| `permissions_path` | `Option<String>` | None | 权限配置路径 |
| `migrations_dir` | `Option<PathBuf>` | None | 迁移目录 |
| `auto_migrate` | `bool` | false | 自动运行迁移 |
| `migration_timeout` | `u64` | 60 | 迁移超时（秒） |
| `admin_role` | `String` | "admin" | 管理员角色名称 |
| `warmup_timeout` | `u64` | 30 | 连接池预热超时（秒） |
| `warmup_retries` | `u32` | 3 | 连接池预热重试次数 |

---

## 定义实体

### 基本实体定义

定义映射到数据库表的结构体：

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
    pub created_at: chrono::DateTime<chrono::Utc>,
}
```

**必需参数：**

- `#[db_entity(table_name = "...", primary_key = "...")]` - 统一的属性宏，生成 CRUD 方法
- `#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]` - Sea-ORM 实体特性
- `#[sea_orm(table_name = "...")]` - 指定表名
- `#[sea_orm(primary_key)]` - 标记主键字段

**可选参数：**

- `timestamps = true` - 自动管理 `created_at`/`updated_at` 字段
- `soft_delete = true` - 自动注入 `deleted_at` 字段
- `cache(...)` - 启用缓存（需要 `cache` 特性）
- `audit(...)` - 启用审计日志（需要 `audit` 特性）
- `hooks(...)` - 配置生命周期钩子函数

### 带权限控制的实体

添加基于角色的访问控制：

```rust
use dbnexus::db_entity;
use sea_orm::entity::prelude::*;

// 权限控制现在由 Session 根据权限配置文件强制执行，不再通过宏声明
#[db_entity(table_name = "users", primary_key = "id")]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct User {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
}
```

### 带操作级控制的实体

指定允许的操作：

```rust
use dbnexus::db_entity;
use sea_orm::entity::prelude::*;

// 操作级权限控制由 Session 根据权限配置强制执行
#[db_entity(table_name = "users", primary_key = "id")]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct User {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
}
```

### 带缓存的实体

为读操作启用缓存：

```rust
use dbnexus::db_entity;
use sea_orm::entity::prelude::*;

#[db_entity(
    table_name = "users",
    primary_key = "id",
    cache(ttl = 60, strategy = "lru", max_capacity = 5000)
)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct User {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
}
```

### 带审计日志的实体

为所有操作启用审计日志：

```rust
use dbnexus::db_entity;
use sea_orm::entity::prelude::*;

#[db_entity(
    table_name = "users",
    primary_key = "id",
    audit(table_name = "audit_log", operations = ["INSERT", "UPDATE", "DELETE"], log_values = true)
)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct User {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
}
```

### 复杂实体示例

```rust
use dbnexus::db_entity;
use sea_orm::entity::prelude::*;

#[db_entity(
    table_name = "orders",
    primary_key = "id",
    cache(ttl = 60, strategy = "lru", max_capacity = 5000),
    audit(table_name = "audit_log", operations = ["INSERT", "UPDATE", "DELETE"], log_values = true)
)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "orders")]
pub struct Order {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub user_id: i64,
    pub amount: f64,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
```

---

## 使用连接

### 获取会话

使用 `get_session()` 获取数据库连接：

```rust
use dbnexus::DbPool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("postgresql://localhost/mydb").await?;

    // 获取带角色的会话
    let session = pool.get_session("admin").await?;

    // 使用会话...
    // 连接在丢弃时自动释放

    Ok(())
}
```

### 会话生命周期

会话是 RAII 管理的：

```rust
{
    let session = pool.get_session("admin").await?;
    // 连接在此处活跃

    User::find_all(&session).await?;
    User::insert(&session, new_user).await?;

} // 连接在此处自动释放
```

### 检查池状态

监控连接池健康：

```rust
let status = pool.status();

println!("总连接数: {}", status.total);
println!("活跃连接数: {}", status.active);
println!("空闲连接数: {}", status.idle);
println!("等待计数: {}", status.wait_count);
println!("观察到的最大活跃数: {}", status.max_active);
```

### 手动健康检查

触发连接池健康检查：

```rust
let invalid_count = pool.clean_invalid_connections().await;
println!("移除了 {} 个无效连接", invalid_count);
```

---

## CRUD 操作

### 创建（插入）

插入新记录：

```rust
let user = User {
    id: 1,
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
    created_at: chrono::Utc::now(),
};

let inserted = User::insert(&session, user).await?;
println!("插入用户: {}", inserted.name);
```

### 读取（选择）

#### 按主键查找

```rust
let user = User::find_by_id(&session, 1).await?;
if let Some(user) = user {
    println!("找到用户: {}", user.name);
}
```

#### 查找所有记录

```rust
let users = User::find_all(&session).await?;
println!("找到 {} 个用户", users.len());
```

#### 按条件查找

```rust
use dbnexus::entity::*;

let condition = Condition::all()
    .add(Column::Name.like("%Alice%"))
    .add(Column::CreatedAt.gte(chrono::Utc::now() - chrono::Duration::days(7)));

let users = User::find_by_condition(&session, condition).await?;
```

#### 记录计数

```rust
let count = User::count(&session).await?;
println!("总用户数: {}", count);
```

### 更新

更新现有记录：

```rust
let mut user = User::find_by_id(&session, 1).await?.unwrap();
user.email = "alice_new@example.com".to_string();
user.updated_at = chrono::Utc::now();

let updated = User::update(&session, user).await?;
println!("更新用户: {}", updated.email);
```

### 删除

#### 按主键删除

```rust
User::delete(&session, 1).await?;
println!("删除了 ID 为 1 的用户");
```

#### 按条件删除

```rust
use dbnexus::entity::*;

let condition = Column::CreatedAt.lt(chrono::Utc::now() - chrono::Duration::days(365));
let deleted_count = User::delete_many(&session, condition).await?;
println!("删除了 {} 个旧用户", deleted_count);
```

---

## 权限控制

### 设置权限

创建 `permissions.yaml` 文件：

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
      - name: "orders"
        operations:
          - select

  user:
    tables:
      - name: "users"
        operations:
          - select
```

### 在实体上定义权限

```rust
use dbnexus::db_entity;
use sea_orm::entity::prelude::*;

// 权限控制现在由 Session 根据权限配置文件强制执行，不再通过宏声明
#[db_entity(table_name = "users", primary_key = "id")]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct User {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
}
```

### 使用权限

```rust
// 管理员可以做任何事
let admin_session = pool.get_session("admin").await?;
User::insert(&admin_session, user).await?;
User::delete(&admin_session, 1).await?;

// 经理只能对用户进行选择/插入/更新
let manager_session = pool.get_session("manager").await?;
User::find_all(&manager_session).await?; // OK
User::insert(&manager_session, user).await?; // OK
User::delete(&manager_session, 1).await?; // 错误：权限被拒绝

// 用户只能选择
let user_session = pool.get_session("user").await?;
User::find_all(&user_session).await?; // OK
User::insert(&user_session, user).await?; // 错误：权限被拒绝
```

### 通配符表

使用 `"*"` 授予对所有表的访问权限：

```yaml
roles:
  admin:
    tables:
      - name: "*"  # 所有表
        operations:
          - select
          - insert
          - update
          - delete
```

### 操作级控制

限制特定操作：

```yaml
roles:
  readonly:
    tables:
      - name: "reports"
        operations:
          - select  # 只允许 SELECT
```

---

## 事务

### 基本事务

开始、提交和回滚：

```rust
use dbnexus::DbPool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("postgresql://localhost/mydb").await?;
    let mut session = pool.get_session("admin").await?;

    // 开始事务
    session.begin_transaction().await?;

    // 执行操作
    User::insert(&session, user1).await?;
    User::insert(&session, user2).await?;

    // 提交事务
    session.commit().await?;

    Ok(())
}
```

### 带错误处理的事务

```rust
session.begin_transaction().await?;

match perform_operations(&session).await {
    Ok(_) => {
        session.commit().await?;
    }
    Err(e) => {
        eprintln!("错误: {}", e);
        session.rollback().await?;
    }
}
```

### RAII 事务守护

使用 RAII 模式进行自动回滚：

```rust
use dbnexus::DbPool;

struct TransactionGuard<'a> {
    session: &'a mut Session,
}

impl<'a> TransactionGuard<'a> {
    fn new(session: &'a mut Session) -> Self {
        session.begin_transaction().await.ok();
        Self { session }
    }

    pub async fn commit(self) {
        self.session.commit().await.ok();
    }
}

impl<'a> Drop for TransactionGuard<'a> {
    fn drop(&mut self) {
        if self.session.is_in_transaction() {
            let _ = self.session.rollback().now_or_never();
        }
    }
}

// 使用
{
    let tx = TransactionGuard::new(&mut session);
    User::insert(&session, user).await?;
    tx.commit().await; // 显式提交
} // 或在丢弃时隐式回滚
```

---

## 高级特性

### 缓存

为读密集型操作启用缓存：

```rust
use dbnexus::db_entity;
use sea_orm::entity::prelude::*;

#[db_entity(
    table_name = "products",
    primary_key = "id",
    cache(ttl = 60, strategy = "lru", max_capacity = 5000)
)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "products")]
pub struct Product {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub price: f64,
}

// 使用缓存读取
let product = Product::find_cached(&session, 1).await?;

// 写入时使缓存失效
Product::invalidate_cache(&session, 1).await?;
```

### 指标

启用 Prometheus 指标：

```toml
[dependencies.dbnexus]
version = "0.5"
features = ["metrics"]
```

收集和导出指标：

```rust
use dbnexus::metrics::MetricsCollector;

let collector = MetricsCollector::new();

// 获取池指标
let pool_metrics = collector.pool_status();
println!("活跃连接: {}", pool_metrics.active);

// 获取查询指标
if let Some(stats) = collector.get_query_stats("SELECT") {
    println!("P99 延迟: {}ns", stats.latency_percentiles.p99_ns);
}

// 导出 Prometheus 格式
let prometheus_metrics = collector.export_prometheus();
println!("{}", prometheus_metrics);
```

### 审计日志

启用审计日志：

```toml
[dependencies.dbnexus]
version = "0.5"
features = ["audit"]
```

使用 `#[db_entity(... audit(...))]` 自动进行审计日志：

```rust
use dbnexus::db_entity;
use sea_orm::entity::prelude::*;

#[db_entity(
    table_name = "sensitive_data",
    primary_key = "id",
    audit(table_name = "audit_log", operations = ["INSERT", "UPDATE", "DELETE"], log_values = true)
)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "sensitive_data")]
pub struct SensitiveData {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub data: String,
}

// 所有操作自动记录
SensitiveData::insert(&session, data).await?;
SensitiveData::find_by_id(&session, 1).await?;
```

### 分布式跟踪

启用 OpenTelemetry 跟踪：

```toml
[dependencies.dbnexus]
version = "0.5"
features = ["tracing"]
```

初始化跟踪：

```rust
use dbnexus::TracingGuard;

// 初始化 OTLP 导出（默认端点 http://localhost:4317）
let _guard = TracingGuard::init_with_otlp("http://localhost:4317")?;

// 所有 DB 操作自动跟踪
let session = pool.get_session("admin").await?;
User::find_all(&session).await?;
```

### DuckDB 嵌入式数据库

DuckDB 是嵌入式分析型数据库，适合 OLAP 场景。0.3.0 新增通过连接池模式支持真正并行查询。

启用 DuckDB：

```toml
[dependencies.dbnexus]
version = "0.5"
features = ["duckdb"]
```

URL 格式支持：

- `:memory:` 或 `duckdb::memory:` — 内存数据库
- `duckdb:path/to/file.db` — 文件数据库
- `duckdb://path/to/file.db` — 文件数据库（URL 格式）

API 说明：

- `DuckDbConnection::new(url: &str) -> Result<Self, DbError>` — 创建连接（默认连接池大小 4）
- `DuckDbConnection::with_pool_size(url: &str, pool_size: usize) -> Result<Self, DbError>` — 指定连接池大小
- `conn.execute(sql: &str) -> DbResult<DuckDbExecResult>` — 执行 DDL/DML，返回受影响行数
- `conn.query(sql: &str) -> DbResult<Vec<DuckDbRow>>` — 执行查询，返回行集合
- `conn.health_check() -> DbResult<()>` — 健康检查（执行 `SELECT 1`）

`DuckDbRow` 通过列名获取值：`row.get("column_name") -> Option<&DuckValue>`。

```rust
use dbnexus::DuckDbConnection;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建 DuckDB 连接（默认连接池大小 4）
    let conn = DuckDbConnection::new(":memory:")?;

    // 创建表
    conn.execute("CREATE TABLE sales (id INTEGER, amount DOUBLE)").await?;

    // 插入数据
    conn.execute("INSERT INTO sales VALUES (1, 100.0), (2, 200.0)").await?;

    // 查询
    let rows = conn.query("SELECT id, amount FROM sales").await?;
    for row in &rows {
        // 按列名获取值
        if let (Some(id), Some(amount)) = (row.get("id"), row.get("amount")) {
            println!("id={:?}, amount={:?}", id, amount);
        }
    }

    // 健康检查
    conn.health_check().await?;
    Ok(())
}
```

### JWT 认证

启用基于 JWT 的用户认证系统：

```toml
[dependencies.dbnexus]
version = "0.5"
features = ["authentication"]
```

API 说明：

- `AuthenticationManager::new(jwt_secret: &[u8]) -> Self` — 创建认证管理器（密钥为字节切片）
- `AuthenticationManager::with_config(jwt_secret, access_exp_secs, refresh_exp_secs) -> Self` — 自定义过期时间
- `manager.register_user(username, password, role) -> AuthResult<()>` — 注册用户（async，含密码强度校验和哈希）
- `manager.authenticate(credentials: AuthCredentials) -> AuthResult<String>` — 验证凭据并生成 JWT（async）
- `manager.verify_token(token: &str) -> AuthResult<JwtClaims>` — 验证 JWT（同步方法）
- `manager.refresh_token(refresh_token: &str) -> AuthResult<String>` — 刷新访问令牌

关联类型：`AuthCredentials`（字段：`username`、`password`）、`JwtClaims`（字段：`sub`、`username`、`role`、`exp`、`iat`、`token_type`）。

```rust
use dbnexus::{AuthenticationManager, AuthCredentials};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // JWT 密钥（建议从环境变量读取，至少 32 字节）
    let secret = b"my-secret-key-32-bytes-long-xxxxx";
    let manager = AuthenticationManager::new(secret);

    // 注册用户（密码需通过强度检查：≥8 字符 + 含字母 + 含数字）
    manager.register_user("alice", "password123", "admin").await?;

    // 认证
    let credentials = AuthCredentials {
        username: "alice".to_string(),
        password: "password123".to_string(),
    };
    let token = manager.authenticate(credentials).await?;
    println!("Token: {}", token);

    // 验证 token（同步方法，无需 await）
    let claims = manager.verify_token(&token)?;
    println!("User: {}, Role: {}", claims.sub, claims.role);

    Ok(())
}
```

### DDL 安全守卫

`DdlGuard` 使用 AST（抽象语法树）分析验证 DDL 语句安全性，相比字符串前缀匹配更可靠。需启用 `sql-parser` feature（默认已启用）。

```toml
[dependencies.dbnexus]
version = "0.5"
features = ["sql-parser"]
```

API 说明：

- `DdlGuard::new() -> Self` — 创建守卫实例
- `guard.validate(sql: &str) -> Result<DdlValidationResult, String>` — 验证 SQL，返回 `Result`（解析失败时返回 `Err`）

`DdlValidationResult` 三态变体：

- `Allowed` — 验证通过
- `Forbidden(String)` — SQL 被拦截，含原因
- `ParseError(String)` — SQL 解析失败

白名单允许的语句：`CreateTable`、`AlterTable`、`CreateIndex`、`CreateView`、`Truncate`、`Query`（SELECT）、`DropIndex`、`DropView`。禁止：`DROP TABLE`、`DROP DATABASE`、`INSERT`、`UPDATE`、`DELETE` 等。

注意：`Session::execute_raw_ddl` 和 `execute_duckdb_raw` 内部已自动调用 `DdlGuard`；只有 admin role 才能执行 DDL。

```rust
use dbnexus::{DdlGuard, DdlValidationResult};

fn main() {
    let guard = DdlGuard::new();

    // 允许的 DDL（CREATE TABLE 在白名单中）
    assert!(matches!(
        guard.validate("CREATE TABLE users (id INT)").unwrap(),
        DdlValidationResult::Allowed
    ));

    // 拒绝的 DDL（DROP TABLE 被禁止，仅允许 DROP INDEX/DROP VIEW）
    match guard.validate("DROP TABLE users").unwrap() {
        DdlValidationResult::Forbidden(reason) => {
            println!("拒绝执行: {}", reason);
        }
        _ => panic!("应该拒绝 DROP TABLE"),
    }
}
```

### 分片路由

启用数据分片，支持水平扩展：

```toml
[dependencies.dbnexus]
version = "0.5"
features = ["sharding"]
```

API 说明：

- `ShardRouter::new(strategy: S, total_shards: u32) -> Self` — 使用策略实例创建（`S: ShardingStrategy + 'static`）
- `ShardRouter::with_strategy(strategy: &str, total_shards: u32) -> Self` — 按策略名创建（如 `"hash"`、`"yearly"`，同步方法）
- `ShardRouter::with_config(config: &ShardConfig) -> Result<Self, DbError>` — 异步、并行初始化所有分片连接池
- `router.shard_id_for_key(shard_key: &str) -> u32` — 根据 key 计算分片 ID
- `router.calculate_shard(timestamp: DateTime<Utc>, key: &str) -> u32` — 按时间+key 计算分片
- `router.get_session_for_shard(shard_key, role) -> Result<Session, DbError>` — 获取分片对应的 Session（async）

默认策略为 `YearlyStrategy`。内置 `create_strategy(name)` 工厂函数支持 `"hash"`、`"yearly"` 等策略名。

```rust
use dbnexus::ShardRouter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 使用 hash 策略创建 4 分片路由器（同步方法，不创建连接池）
    let router = ShardRouter::with_strategy("hash", 4);

    // 根据 key 计算分片 ID
    let shard_id = router.shard_id_for_key("user_123");
    println!("Shard: {}", shard_id);

    Ok(())
}
```

### 数据验证

启用数据验证 feature 后，`DbError` 增加 `Validation(String)` 变体，并集成 [`validator`](https://docs.rs/validator) crate 提供声明式验证：

```toml
[dependencies.dbnexus]
version = "0.5"
features = ["validation"]
```

启用后可使用 `DbError::Validation` 作为统一错误类型，并将 `validator` 的错误转换为 `DbError`：

```rust
use dbnexus::DbError;
use validator::Validate;

#[derive(Validate)]
struct UserInput {
    #[validate(email)]
    email: String,
    #[validate(length(min = 8))]
    password: String,
}

fn process_input(input: &UserInput) -> Result<(), DbError> {
    // 调用 validator crate 的验证，错误转换为 DbError::Validation
    input.validate().map_err(|e| DbError::Validation(e.to_string()))?;
    // 处理输入...
    Ok(())
}

fn validate_email(email: &str) -> Result<(), DbError> {
    if email.is_empty() {
        return Err(DbError::Validation("邮箱不能为空".to_string()));
    }
    if !email.contains('@') {
        return Err(DbError::Validation("邮箱格式无效".to_string()));
    }
    Ok(())
}
```

### 图数据库（需要 `ladybug` 或 `neo4j` 特性）

DBNexus 支持图数据库，通过 `GraphConnection` trait 统一抽象。图数据库与关系型数据库不互斥，可混合使用。

```toml
[dependencies.dbnexus]
version = "0.5"
features = ["ladybug"]  # 或 "neo4j"
```

API 说明：

- `LadybugConnection::new(url: &str) -> Result<Self, DbError>` — 创建嵌入式图数据库连接
- `Neo4jConnection::new(url, user, pass) -> Result<Self, DbError>` — 创建 Neo4j 服务器连接（async）
- `conn.execute_cypher(cypher: &str) -> Result<GraphExecResult, DbError>` — 执行 Cypher 语句
- `conn.query_cypher(cypher: &str) -> Result<GraphQueryResult, DbError>` — 查询图数据
- `session.execute_cypher(cypher: &str) -> Result<GraphExecResult, DbError>` — 通过 Session 执行（支持图事务）

```rust
use dbnexus::{LadybugConnection, GraphConnection};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conn = LadybugConnection::new("ladybug::memory:")?;

    // 创建节点
    conn.execute_cypher("CREATE (n:User {name: 'Alice'})").await?;

    // 查询
    let result = conn.query_cypher("MATCH (n:User) RETURN n").await?;
    println!("节点数: {}", result.rows.len());

    Ok(())
}
```

### 国际化格式化（核心特性，始终可用）

基于 ICU4X 的 locale 感知数字/日期/复数/排序格式化。国际化是系统核心组件，无需额外启用任何特性。

```rust
use dbnexus::DbI18nFormatter;

let formatter = DbI18nFormatter::new("zh-CN")?;
// locale 感知的数字格式化
let formatted = formatter.format_number(1234567.89);
```

---

## 最佳实践

### 1. 对敏感数据使用环境变量

永远不要硬编码凭据：

```rust
// ❌ 不好
let url = "postgresql://user:password@localhost/db";

// ✅ 好
let url = std::env::var("DATABASE_URL")?;
```

### 2. 对多步操作始终使用事务

```rust
// ❌ 不好
User::insert(&session, user1).await?;
User::insert(&session, user2).await?;
// 如果 user2 失败，user1 仍然插入！

// ✅ 好
session.begin_transaction().await?;
User::insert(&session, user1).await?;
User::insert(&session, user2).await?;
session.commit().await?;
// 要么都成功，要么都失败
```

### 3. 使用 RAII 进行资源管理

```rust
// ❌ 不好
let session = pool.get_session("admin").await?;
User::find_all(&session).await?;
pool.release_connection(session); // 容易忘记

// ✅ 好
{
    let session = pool.get_session("admin").await?;
    User::find_all(&session).await?;
} // 连接自动释放
```

### 4. 配置适当的池大小

对于低流量应用程序：
```rust
.min_connections(1)
.max_connections(5)
```

对于高流量应用程序：
```rust
.min_connections(10)
.max_connections(100)
```

### 5. 使用权限控制

即使是内部工具：

```rust
// ❌ 不好
let session = pool.get_session("root").await?;
// 完全访问，无检查

// ✅ 好
let session = pool.get_session("read_only").await?;
// 有限访问，明确权限
```

### 6. 监控池状态

定期检查池健康：

```rust
let status = pool.status();
if status.wait_count > 1000 {
    eprintln!("警告：高连接等待计数");
}
```

### 7. 优雅处理错误

```rust
// ❌ 不好
let user = User::find_by_id(&session, 1).await?.unwrap();
// 错误时恐慌

// ✅ 好
match User::find_by_id(&session, 1).await {
    Ok(Some(user)) => { /* 使用用户 */ }
    Ok(None) => { /* 处理未找到 */ }
    Err(e) => { /* 处理错误 */ }
}
```

### 8. 使用类型安全操作

```rust
// ❌ 不好
session.execute("DELETE FROM users WHERE id = 1").await?;
// 无类型安全，容易出错

// ✅ 好
User::delete(&session, 1).await?;
// 类型安全，自动权限检查
```

---

## 故障排除

### 连接池耗尽

**症状：** `DbError::ConnectionPool("Connection pool exhausted")`

**解决方案：**

1. 增加池大小：
   ```rust
   .max_connections(50)
   ```

2. 检查连接泄漏：
   ```rust
   let status = pool.status();
   println!("活跃: {}, 总计: {}", status.active, status.total);
   ```

3. 确保会话被丢弃：
   ```rust
   // 使用 RAII 模式
   {
       let session = pool.get_session("admin").await?;
       // 使用会话...
   } // 连接释放
   ```

### 权限被拒绝错误

**症状：** `DbError::Permission("Permission denied...")`

**解决方案：**

1. 检查角色是否在权限配置中：
   ```yaml
   roles:
     my_role:  # 必须完全匹配
   ```

2. 验证操作是否被允许：
   ```yaml
   roles:
     my_role:
       tables:
         - name: "users"
           operations:
             - select  # 确保操作被列出
   ```

3. 检查角色大小写：
   ```rust
   // 必须与配置完全匹配
   let session = pool.get_session("My_Role").await?; // ❌ 错误
   let session = pool.get_session("my_role").await?; // ✅ 正确
   ```

### 数据库连接错误

**症状：** `DbError::Connection("Failed to connect...")`

**解决方案：**

1. 验证 URL 格式：
   ```bash
   # SQLite
   sqlite::memory:
   sqlite:///path/to/db

   # PostgreSQL
   postgresql://user:password@host:port/database

   # MySQL
   mysql://user:password@host:port/database
   ```

2. 检查网络连接：
   ```bash
   ping postgres-server
   telnet postgres-server 5432
   ```

3. 验证凭据和权限：
   ```bash
   psql -U username -h postgres-server -d database
   ```

### 慢查询性能

**症状：** 查询耗时过长

**解决方案：**

1. 启用并检查指标：
   ```rust
   let collector = MetricsCollector::new();
   if let Some(stats) = collector.get_query_stats("SELECT") {
       println!("P99 延迟: {}ns", stats.latency_percentiles.p99_ns);
   }
   ```

2. 使用索引：
   ```sql
   CREATE INDEX idx_users_email ON users(email);
   ```

3. 优化查询：
   ```rust
   // ❌ 不好：获取所有
   let users = User::find_all(&session).await?;
   let filtered: Vec<_> = users.into_iter()
       .filter(|u| u.name.contains("Alice"))
       .collect();

   // ✅ 好：在数据库中过滤
   let users = User::find_by_condition(&session, Column::Name.like("%Alice%")).await?;
   ```

### 高内存使用

**症状：** 应用程序使用过多内存

**解决方案：**

1. 减少池大小：
   ```rust
   .max_connections(10)
   ```

2. 启用连接空闲超时：
   ```rust
   .idle_timeout(60)  // 更快关闭空闲连接
   ```

3. 检查缓存大小：
   ```rust
   #[db_entity(..., cache(max_capacity = 100))]  // 限制缓存条目
   ```

---

## 示例：完整应用程序

```rust
use dbnexus::{DbPool, db_entity};
use sea_orm::entity::prelude::*;
use chrono::Utc;

#[db_entity(table_name = "users", primary_key = "id")]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct User {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化连接池
    let pool = DbPool::new("postgresql://localhost/mydb").await?;
    println!("✓ 已连接到数据库");

    // 管理员操作
    {
        let admin_session = pool.get_session("admin").await?;
        println!("✓ 已获取管理员会话");

        // 插入用户
        let user1 = User {
            id: 1,
            name: "Alice".to_string(),
            email: "alice@example.com".to_string(),
        };
        User::insert(&admin_session, user1).await?;
        println!("✓ 插入用户: Alice");

        let user2 = User {
            id: 2,
            name: "Bob".to_string(),
            email: "bob@example.com".to_string(),
        };
        User::insert(&admin_session, user2).await?;
        println!("✓ 插入用户: Bob");
    }

    // 经理操作
    {
        let manager_session = pool.get_session("manager").await?;
        println!("✓ 已获取经理会话");

        // 查询用户
        let users = User::find_all(&manager_session).await?;
        println!("✓ 找到 {} 个用户", users.len());

        // 更新用户
        if let Some(mut user) = User::find_by_id(&manager_session, 1).await? {
            user.email = "alice_new@example.com".to_string();
            User::update(&manager_session, user).await?;
            println!("✓ 更新用户邮箱");
        }
    }

    // 池状态
    let status = pool.status();
    println!("\n📊 池状态:");
    println!("  总计: {}", status.total);
    println!("  活跃: {}", status.active);
    println!("  空闲: {}", status.idle);

    println!("\n✨ 应用程序成功完成！");

    Ok(())
}
```

---

更多信息：
- [API 参考](API_REFERENCE.md)
- [架构](ARCHITECTURE.md)
- [Rust 文档](https://docs.rs/dbnexus)
