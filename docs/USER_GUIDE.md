# 📖 Dbnexus 用户指南

在应用程序中使用 DBNexus 的完整指南，覆盖安装、配置、实体定义、CRUD、权限控制、事务与高级特性。

> 周边文档：[📘 API 参考](API_REFERENCE.md) ｜ [🏗️ 架构文档](ARCHITECTURE.md) ｜ [🔒 安全文档](SECURITY.md) ｜ [📊 性能基线](PERFORMANCE.md)。特性全表见 [README](../README.md#-特性标志)。

## 📋 目录

<details open>
<summary>📑 目录</summary>

- [🚀 快速上手](#-快速上手)
  - [先决条件](#先决条件)
  - [安装](#安装)
  - [选择数据库驱动](#选择数据库驱动)
  - [验证安装](#验证安装)
- [🔧 配置](#-配置)
  - [环境变量](#环境变量)
  - [YAML 配置文件](#yaml-配置文件)
  - [JSON 配置](#json-配置)
  - [TOML 配置](#toml-配置)
  - [程序化配置](#程序化配置)
  - [配置参数](#配置参数)
- [🧩 定义实体](#-定义实体)
- [📦 连接池与会话](#-连接池与会话)
- [📝 CRUD 操作](#-crud-操作)
- [🔐 权限控制](#-权限控制)
- [🔄 事务](#-事务)
- [🧱 高级特性](#-高级特性)
- [✅ 最佳实践](#-最佳实践)
- [🧯 故障排除](#-故障排除)
- [📦 完整示例](#-完整示例)
- [📚 相关文档](#-相关文档)

</details>

---

## 🚀 快速上手

本节带您从零开始走通"连接数据库 → 定义实体 → 执行 CRUD"的最小闭环。

### 先决条件

- Rust **1.97.1** 或更高版本（`rust-toolchain.toml` 锁定，edition 2024）
- Rust 与 SQL 基础知识
- 数据库之一：PostgreSQL、MySQL、SQLite、DuckDB（图数据库 Ladybug / Neo4j 可选）

### 安装

添加依赖到 `Cargo.toml`：

```toml
[dependencies]
dbnexus = { version = "0.6.0-rc.3", features = ["runtime-tokio-rustls", "sqlite", "permission", "macros"] }
tokio = { version = "1.53", features = ["rt-multi-thread", "macros"] }
```

> DBNexus 的 `default` 特性为空：运行时、数据库驱动与功能特性均需显式启用。
> `permission` 会强制启用 `sql-parser` 与 `cache`（编译期校验，防止注入绕过权限检查）。

### 选择数据库驱动

| 驱动特性 | 数据库 | 类型 | 约束 |
|----------|--------|------|------|
| `sqlite` | SQLite | 嵌入式关系型 | 与其他关系型驱动互斥 |
| `postgres` | PostgreSQL | 服务器关系型 | 与其他关系型驱动互斥 |
| `mysql` | MySQL | 服务器关系型 | 与其他关系型驱动互斥 |
| `duckdb` | DuckDB | 嵌入式分析型 | 与其他关系型驱动互斥 |
| `ladybug` | Ladybug | 嵌入式图数据库 | 可与关系型驱动共存 |
| `neo4j` | Neo4j | 图数据库服务器 | 可与关系型驱动共存 |

```toml
# 嵌入式设备最小配置
dbnexus = { version = "0.6.0-rc.3", default-features = false, features = ["runtime-tokio-rustls", "sqlite", "config-env"] }

# 带企业特性的 PostgreSQL
dbnexus = { version = "0.6.0-rc.3", features = ["runtime-tokio-rustls", "postgres", "permission", "metrics", "audit"] }

# 带基础特性的 SQLite
dbnexus = { version = "0.6.0-rc.3", features = ["runtime-tokio-rustls", "sqlite", "permission"] }
```

**重要**：关系型驱动（SQLite / PostgreSQL / MySQL / DuckDB）之间一次只能启用一个，混用在编译期直接报错。图数据库驱动（Ladybug / Neo4j）可与关系型驱动共存。

### 验证安装

```rust
use dbnexus::DbPool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("sqlite::memory:").await?;
    println!("DBNexus 已就绪");
    Ok(())
}
```

从安装到首次查询的整体路径如下图所示：

```mermaid
flowchart TD
    A["安装 dbnexus"] --> B["选择运行时与数据库驱动"]
    B --> C["配置连接池<br/>环境变量 / YAML / JSON"]
    C --> D["定义实体<br/>db_entity 宏"]
    D --> E["获取会话<br/>get_session 指定角色"]
    E --> F["CRUD 与事务<br/>每条语句经过权限检查"]
    F --> G{需要企业能力？}
    G -->|是| H["按需启用特性<br/>metrics / audit / sharding 等"]
    G -->|否| I["上线"]
    H --> I
```

---

## 🔧 配置

### 环境变量

最快的配置方式是环境变量（需要 `config-env` 特性）：

```bash
export DATABASE_URL="postgresql://user:password@localhost/mydb"
export DB_MAX_CONNECTIONS=20
export DB_MIN_CONNECTIONS=5
export DB_ADMIN_ROLE=admin
```

```rust
use dbnexus::{DbConfig, DbPool};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = DbConfig::from_env()?;
    let pool = DbPool::with_config(config).await?;
    println!("已连接");
    Ok(())
}
```

### YAML 配置文件

创建 `dbnexus.yaml`（需要 `yaml` 特性）：

```yaml
url: "postgresql://localhost/mydb"
max_connections: 20
min_connections: 5
idle_timeout: 300
acquire_timeout: 5000
auto_migrate: true
admin_role: admin
```

```rust
use dbnexus::DbConfig;

let yaml_content = std::fs::read_to_string("dbnexus.yaml")?;
let config = DbConfig::from_yaml_str(&yaml_content)?;
let pool = DbPool::with_config(config).await?;
```

### JSON 配置

`DbConfig` 支持 serde JSON 直接反序列化（无需额外特性）：

```rust
use dbnexus::DbConfig;

let json_content = std::fs::read_to_string("dbnexus.json")?;
let config = DbConfig::from_json_str(&json_content)?;
let pool = DbPool::with_config(config).await?;
```

### TOML 配置

启用 `config-toml` 特性表示采纳 TOML 配置约定；DBNexus 不强制捆绑 `toml` 解析器，由应用自行引入 `toml` crate 反序列化（`DbConfig` 实现了 `serde::Deserialize`）：

```toml
[dependencies]
toml = "0.9"
```

```rust
use dbnexus::DbConfig;

let toml_content = std::fs::read_to_string("dbnexus.toml")?;
let config: DbConfig = toml::from_str(&toml_content)?;
let pool = DbPool::with_config(config).await?;
```

### 程序化配置

直接构造 `DbConfig` 结构体：

```rust
use dbnexus::{DbConfig, DbPool, PoolConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    Ok(())
}
```

### 配置参数

| 参数 | 类型 | 默认值 | 描述 |
|------|------|--------|------|
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

> `PoolConfig` 的字段经 `#[serde(flatten)]` 扁平化，YAML/JSON 中直接写 `max_connections` 等键即可；Rust 代码中通过 `config.pool_config.max_connections` 访问。

---

## 🧩 定义实体

### 基本实体定义

用 `#[db_entity]` 宏把结构体映射到数据库表，并获得宏生成的带权限检查 CRUD 方法：

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
```

**必需参数：**

| 参数 | 描述 |
|------|------|
| `table_name = "..."` | 数据库表名 |
| `primary_key = "..."` | 主键字段名 |
| `#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]` | Sea-ORM 实体派生 |
| `#[sea_orm(table_name = "...")]` | Sea-ORM 表名声明 |
| `#[sea_orm(primary_key)]` | 标记主键字段 |

**可选参数：**

| 参数 | 描述 | 特性要求 |
|------|------|----------|
| `timestamps = true` | 自动管理 `created_at` / `updated_at` 字段 | `with-time` |
| `soft_delete = true` | 自动注入 `deleted_at` 字段并改写删除语义 | 无 |
| `validate` | 集成 `validator` crate 声明式验证 | `validation` |
| `cache(...)` | 生成实体级缓存配置 | `cache` |
| `audit(...)` | 生成审计配置 | `audit` |
| `hooks(...)` | 配置生命周期钩子（校验 → 时间戳 → 用户钩子） | 无 |

> 使用 `chrono::DateTime` / `uuid::Uuid` 等字段类型时，需启用对应的 `with-chrono` / `with-uuid` 类型桥接特性。

### 缓存与审计子参数

`cache(...)` 子参数：

| 参数 | 描述 | 默认值 |
|------|------|--------|
| `ttl` | 缓存存活时间（秒） | 60 |
| `strategy` | 缓存策略 | `"lru"` |
| `max_capacity` | 最大缓存容量 | 5000 |

`audit(...)` 子参数：

| 参数 | 描述 | 默认值 |
|------|------|--------|
| `table_name` | 审计日志表名 | `"audit_log"` |
| `operations` | 审计的操作列表 | `["INSERT", "UPDATE", "DELETE"]` |
| `roles` | 允许审计的角色列表 | 无 |
| `log_values` | 是否记录字段值 | `true` |

### 复杂实体示例

```rust
use dbnexus::db_entity;
use dbnexus::sea_orm::entity::prelude::*;

#[db_entity(
    table_name = "orders",
    primary_key = "id",
    cache(ttl = 60, strategy = "lru", max_capacity = 5000),
    audit(table_name = "audit_log", operations = ["INSERT", "UPDATE", "DELETE"], log_values = true)
)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "orders")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub user_id: i64,
    pub amount: f64,
    pub status: String,
}
```

---

## 📦 连接池与会话

### 获取会话

通过 `get_session()` 以指定角色获取数据库会话：

```rust
use dbnexus::DbPool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("sqlite::memory:").await?;

    // 获取带角色的会话
    let session = pool.get_session("admin").await?;
    println!("角色: {}", session.role());

    // 会话在丢弃时自动归还连接
    Ok(())
}
```

### 会话生命周期（RAII）

会话由 RAII 管理，作用域结束即归还连接，无需手动释放：

```rust
{
    let session = pool.get_session("admin").await?;
    // 连接在此处活跃
    Model::find_all(&session).await?;
    Model::insert(&session, new_user).await?;
} // 连接在此处自动归还连接池
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

触发连接健康检查与清理（需要 `pool-health-check` 特性）：

```rust
let invalid_count = pool.clean_invalid_connections().await;
println!("移除了 {} 个无效连接", invalid_count);
```

---

## 📝 CRUD 操作

以下示例假设已定义 `Model` 实体并获取了 `session`。完整方法签名见 [API 参考 · 过程宏](API_REFERENCE.md#️-过程宏)。

### 创建（插入）

```rust
let user = Model {
    id: 1,
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
};

let inserted = Model::insert(&session, user).await?;
println!("插入用户: {}", inserted.name);
```

### 读取（查询）

按主键查找：

```rust
let user = Model::find_by_id(&session, 1).await?;
if let Some(user) = user {
    println!("找到用户: {}", user.name);
}
```

查找所有记录：

```rust
let users = Model::find_all(&session).await?;
println!("找到 {} 个用户", users.len());
```

按条件查找：

```rust
use dbnexus::sea_orm::entity::prelude::*;

let condition = Condition::all()
    .add(Column::Name.like("%Alice%"))
    .add(Column::Id.gte(1));

let users = Model::find_by_condition(&session, condition).await?;
```

记录计数：

```rust
let count = Model::count(&session).await?;
println!("总用户数: {}", count);
```

### 更新

```rust
let mut user = Model::find_by_id(&session, 1).await?.unwrap();
user.email = "alice_new@example.com".to_string();

let updated = Model::update(&session, user).await?;
println!("更新用户: {}", updated.email);
```

### 删除

按主键删除：

```rust
Model::delete(&session, 1).await?;
println!("删除了 ID 为 1 的用户");
```

按条件批量删除：

```rust
use dbnexus::sea_orm::entity::prelude::*;

let condition = Condition::all().add(Column::Id.lt(100));
let deleted_count = Model::delete_many(&session, condition).await?;
println!("删除了 {} 条记录", deleted_count);
```

---

## 🔐 权限控制

权限控制由 `Session` 在运行时根据权限配置（YAML/JSON）强制执行，每条语句在执行前都会经过"解析 → 逐表权限检查"管道；JOIN / 子查询路径上的目标表同样受检。完整设计见[安全文档](SECURITY.md#-安全设计概览)。

### 定义权限策略

创建 `permissions.yaml`：

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

### 使用权限

```rust
// admin 对所有表拥有全部操作
let admin_session = pool.get_session("admin").await?;
Model::insert(&admin_session, user).await?;
Model::delete(&admin_session, 1).await?;

// manager 只能对 users 执行选择/插入/更新
let manager_session = pool.get_session("manager").await?;
Model::find_all(&manager_session).await?;      // OK
Model::insert(&manager_session, user).await?;  // OK
Model::delete(&manager_session, 1).await?;     // 错误：权限被拒绝

// user 只能选择
let user_session = pool.get_session("user").await?;
Model::find_all(&user_session).await?;         // OK
Model::insert(&user_session, user).await?;     // 错误：权限被拒绝
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

## 🔄 事务

### 基本事务

```rust
use dbnexus::DbPool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("postgresql://localhost/mydb").await?;
    let session = pool.get_session("admin").await?;

    // 开始事务
    session.begin_transaction().await?;

    // 执行操作
    Model::insert(&session, user1).await?;
    Model::insert(&session, user2).await?;

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

用守护结构体封装"丢弃即回滚"语义（`Drop` 中不能 `await`，回滚需在异步上下文中显式执行或使用 `now_or_never` 立即求值）：

```rust
use dbnexus::{DbResult, Session};
use futures::FutureExt;

struct TransactionGuard<'a> {
    session: &'a Session,
    active: bool,
}

impl<'a> TransactionGuard<'a> {
    pub async fn begin(session: &'a Session) -> DbResult<Self> {
        session.begin_transaction().await?;
        Ok(Self { session, active: true })
    }

    pub async fn commit(mut self) -> DbResult<()> {
        self.active = false;
        self.session.commit().await
    }
}

impl Drop for TransactionGuard<'_> {
    fn drop(&mut self) {
        if self.active {
            // 丢弃时立即回滚（同步上下文中求值）
            let _ = self.session.rollback().now_or_never();
        }
    }
}

// 使用
let tx = TransactionGuard::begin(&session).await?;
Model::insert(&session, user).await?;
tx.commit().await?; // 显式提交；若此前发生错误且守护被丢弃，则自动回滚
```

---

## 🧱 高级特性

### 查询结果缓存

启用 `cache` 特性后，可通过 `DbCacheProvider` 注入缓存后端，驱动只读查询结果缓存（写入路径自动失效）：

```toml
[dependencies.dbnexus]
version = "0.6.0-rc.3"
features = ["cache"]
```

实现 `DbCacheProvider` trait 或使用内置适配器（`oxcache-integration` 特性提供 `OxcacheDbCacheAdapter`，适配 oxcache）：

```rust
use dbnexus::{DbCacheProvider, DbPoolBuilder};

// 自定义缓存实现（完整示例见 examples/cache_standalone）
let custom_cache: std::sync::Arc<dyn DbCacheProvider + Send + Sync> =
    std::sync::Arc::new(my_cache);

let pool = DbPoolBuilder::new()
    .config(config)
    .cache_provider(custom_cache)
    .build()
    .await?;
```

`#[db_entity(... cache(...))]` 宏参数则为实体生成缓存配置常量（`CACHE_TTL` / `CACHE_STRATEGY` / `CACHE_MAX_CAPACITY`）与 `cache_key()` 辅助方法，供缓存层使用：

```rust
#[db_entity(
    table_name = "products",
    primary_key = "id",
    cache(ttl = 60, strategy = "lru", max_capacity = 5000)
)]
// ... 实体定义

// 宏生成的缓存配置
assert_eq!(Product::CACHE_TTL, 60);
assert_eq!(Product::CACHE_STRATEGY, "lru");
assert_eq!(Product::CACHE_MAX_CAPACITY, 5000);
assert_eq!(Product::cache_key(1), "products:1");
```

### 指标

启用 Prometheus 指标：

```toml
[dependencies.dbnexus]
version = "0.6.0-rc.3"
features = ["metrics"]
```

收集和导出指标：

```rust
use dbnexus::MetricsCollector;

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

```toml
[dependencies.dbnexus]
version = "0.6.0-rc.3"
features = ["audit"]
```

用 `#[db_entity(... audit(...))]` 为实体声明审计配置，操作即自动记录；admin 绕过权限的操作同样会被审计：

```rust
#[db_entity(
    table_name = "sensitive_data",
    primary_key = "id",
    audit(table_name = "audit_log", operations = ["INSERT", "UPDATE", "DELETE"], log_values = true)
)]
// ... 实体定义

// 所有操作自动记录审计日志
SensitiveData::insert(&session, data).await?;
SensitiveData::find_by_id(&session, 1).await?;
```

### DuckDB 嵌入式数据库

DuckDB 是嵌入式分析型数据库，适合 OLAP 场景，以分析只读旁路接入（绕过 sea-orm）。0.3.0 起通过连接池模式支持真正并行查询。

```toml
[dependencies.dbnexus]
version = "0.6.0-rc.3"
features = ["duckdb"]
```

URL 格式支持：

| 格式 | 含义 |
|------|------|
| `:memory:` 或 `duckdb::memory:` | 内存数据库 |
| `duckdb:path/to/file.db` | 文件数据库 |
| `duckdb://path/to/file.db` | 文件数据库（URL 格式） |

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

    // 创建表并插入数据
    conn.execute("CREATE TABLE sales (id INTEGER, amount DOUBLE)").await?;
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
version = "0.6.0-rc.3"
features = ["authentication"]
```

API 说明：

- `AuthenticationManager::new(jwt_secret: &[u8]) -> AuthResult<Self>` — 创建认证管理器（密钥为字节切片，不少于 32 字节）
- `AuthenticationManager::with_config(jwt_secret, access_exp_secs, refresh_exp_secs) -> AuthResult<Self>` — 自定义过期时间
- `manager.register_user(username, password, role) -> AuthResult<()>` — 注册用户（async，含密码强度校验和 bcrypt 哈希）
- `manager.authenticate(credentials: AuthCredentials) -> AuthResult<String>` — 验证凭据并生成 JWT（async）
- `manager.verify_token(token: &str) -> AuthResult<JwtClaims>` — 验证 JWT（同步方法）
- `manager.refresh_token(refresh_token: &str) -> AuthResult<String>` — 刷新访问令牌（async）

关联类型：`AuthCredentials`（字段：`username`、`password`）、`JwtClaims`（字段：`sub`、`username`、`role`、`exp`、`iat`、`token_type`）。

```rust
use dbnexus::{AuthenticationManager, AuthCredentials};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // JWT 密钥建议从环境变量读取，至少 32 字节
    let secret = b"my-secret-key-32-bytes-long-xxxxx";
    let manager = AuthenticationManager::new(secret)?;

    // 注册用户（密码需通过强度检查：至少 12 字符，且包含大写字母、小写字母、数字、特殊字符）
    manager.register_user("alice", "Str0ng!Passw0rd", "admin").await?;

    // 认证
    let credentials = AuthCredentials {
        username: "alice".to_string(),
        password: "Str0ng!Passw0rd".to_string(),
    };
    let token = manager.authenticate(credentials).await?;
    println!("Token: {}", token);

    // 验证 token（同步方法，无需 await）
    let claims = manager.verify_token(&token)?;
    println!("User: {}, Role: {}", claims.sub, claims.role);

    Ok(())
}
```

> 访问令牌与刷新令牌区分校验：`verify_access_token` / `verify_refresh_token` 会额外校验 `token_type`，防止刷新令牌冒用为访问令牌。

### DDL 安全守卫

`DdlGuard` 基于 AST（抽象语法树）分析验证 DDL 语句，配合统一注入检测引擎拦截危险模式。需要 `sql-parser` 特性。

```toml
[dependencies.dbnexus]
version = "0.6.0-rc.3"
features = ["sql-parser"]
```

API 说明：

- `DdlGuard::new() -> Self` — 创建守卫实例
- `guard.validate(sql: &str) -> Result<DdlValidationResult, String>` — 验证 SQL，返回 `Result`（解析失败时返回 `Err`）

`DdlValidationResult` 三态变体：

| 变体 | 含义 |
|------|------|
| `Allowed` | 验证通过 |
| `Forbidden(String)` | SQL 被拦截，含原因 |
| `ParseError(String)` | SQL 解析失败 |

白名单允许的语句类型：`CreateTable`、`AlterTable`、`DropTable`、`CreateIndex`、`DropIndex`、`CreateView`、`DropView`、`Truncate`、`Query`（SELECT）以及迁移事务所需的 DML（`Insert` / `Update` / `Delete`）。`DROP DATABASE` 等危险模式由统一注入检测引擎硬拦截。

注意：`Session::execute_raw_ddl` 与 DuckDB 裸执行通道内部已自动调用 `DdlGuard`；只有 admin 角色才能执行 DDL。

```rust
use dbnexus::{DdlGuard, DdlValidationResult};

fn main() {
    let guard = DdlGuard::new();

    // 允许的 DDL（CREATE TABLE 在白名单中）
    assert!(matches!(
        guard.validate("CREATE TABLE users (id INT)").unwrap(),
        DdlValidationResult::Allowed
    ));

    // 拒绝的危险模式（DROP DATABASE 命中注入检测引擎）
    match guard.validate("DROP DATABASE production").unwrap() {
        DdlValidationResult::Forbidden(reason) => {
            println!("拒绝执行: {}", reason);
        }
        _ => panic!("应该拒绝 DROP DATABASE"),
    }
}
```

### 分片路由

启用数据分片，支持水平扩展：

```toml
[dependencies.dbnexus]
version = "0.6.0-rc.3"
features = ["sharding"]
```

API 说明：

- `ShardRouter::new(strategy: S, total_shards: u32) -> Self` — 使用策略实例创建（`S: ShardingStrategy + 'static`）
- `ShardRouter::with_strategy(strategy: &str, total_shards: u32) -> Self` — 按策略名创建（同步方法）
- `ShardRouter::with_config(config: &ShardConfig) -> Result<Self, DbError>` — 异步、并行初始化所有分片连接池
- `router.shard_id_for_key(shard_key: &str) -> u32` — 根据 key 计算分片 ID
- `router.calculate_shard(timestamp: DateTime<Utc>, key: &str) -> u32` — 按时间 + key 计算分片
- `router.get_session_for_shard(shard_key, role) -> Result<Session, DbError>` — 获取分片对应的 Session（async）

`create_strategy(name)` 工厂支持 `"yearly"`、`"monthly"`、`"daily"`、`"hash"`、`"consistent-hash"` 策略名，未知名称回退到默认的 `YearlyStrategy`。

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

启用 `validation` 特性后，`DbError` 增加 `Validation(String)` 变体，并集成 [`validator`](https://docs.rs/validator) crate 提供声明式验证：

```toml
[dependencies.dbnexus]
version = "0.6.0-rc.3"
features = ["validation"]
```

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
```

### 图数据库（需要 `ladybug` 或 `neo4j` 特性）

DBNexus 通过 `GraphConnection` trait 统一抽象图数据库。图数据库与关系型数据库不互斥，可混合使用。

```toml
[dependencies.dbnexus]
version = "0.6.0-rc.3"
features = ["ladybug"]  # 或 "neo4j"
```

API 说明：

- `LadybugConnection::new(url: &str) -> Result<Self, DbError>` — 创建嵌入式图数据库连接
- `Neo4jConnection::new(url, user, pass) -> Result<Self, DbError>` — 创建 Neo4j 服务器连接（async）
- `conn.execute_cypher(cypher: &str) -> Result<GraphExecResult, DbError>` — 执行 Cypher 语句
- `conn.query_cypher(cypher: &str) -> Result<GraphQueryResult, DbError>` — 查询图数据
- `session.execute_cypher(cypher: &str) -> Result<GraphExecResult, DbError>` — 通过 Session 执行（支持图事务）

```rust
use dbnexus::{GraphConnection, LadybugConnection};

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

> 裸 Cypher 执行已废弃：统一使用 `execute_cypher_with_params` 参数化通道（带注入防护）。

### 国际化格式化（核心特性，始终可用）

基于 ICU4X 的 locale 感知数字/日期/复数/排序格式化。国际化是系统核心组件，无需额外启用任何特性：

```rust
use dbnexus::DbI18nFormatter;

let formatter = DbI18nFormatter::new("zh-CN")?;
// locale 感知的数字格式化
let formatted = formatter.format_number(1234567.89)?;
```

---

## ✅ 最佳实践

### 1. 对敏感数据使用环境变量

永远不要硬编码凭据：

```rust
// 不好
let url = "postgresql://user:password@localhost/db";

// 好
let url = std::env::var("DATABASE_URL")?;
```

### 2. 对多步操作始终使用事务

```rust
// 不好：user2 失败时 user1 已插入
Model::insert(&session, user1).await?;
Model::insert(&session, user2).await?;

// 好：要么都成功，要么都失败
session.begin_transaction().await?;
Model::insert(&session, user1).await?;
Model::insert(&session, user2).await?;
session.commit().await?;
```

### 3. 依赖 RAII 管理会话

会话丢弃时连接自动归还连接池，不需要也不应手动干预连接释放：

```rust
// 好：作用域结束即归还
{
    let session = pool.get_session("admin").await?;
    Model::find_all(&session).await?;
} // 连接自动归还
```

### 4. 配置适当的池大小

低流量应用使用小池（`min_connections: 1`、`max_connections: 5`），高流量应用按并发水位扩容（`min_connections: 10`、`max_connections: 100`），并以 `pool.status()` 的等待计数验证容量是否匹配。

### 5. 使用权限控制

即使是内部工具，也按角色收敛权限：

```rust
// 不好：日常业务使用 admin，完全访问
let session = pool.get_session("admin").await?;

// 好：有限访问，明确权限
let session = pool.get_session("read_only").await?;
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
// 不好：错误时恐慌
let user = Model::find_by_id(&session, 1).await?.unwrap();

// 好：分支处理
match Model::find_by_id(&session, 1).await {
    Ok(Some(user)) => { /* 使用用户 */ }
    Ok(None) => { /* 处理未找到 */ }
    Err(e) => { /* 处理错误 */ }
}
```

### 8. 使用类型安全操作

```rust
// 不好：裸 SQL，无类型安全，容易出错
session.execute("DELETE FROM users WHERE id = 1").await?;

// 好：类型安全，自动权限检查
Model::delete(&session, 1).await?;
```

---

## 🧯 故障排除

### 连接池耗尽

**症状**：`DbError::ConnectionPool("Connection pool exhausted")`

**解决方案**：

1. 增加池大小（调大 `PoolConfig::max_connections`）；
2. 检查连接泄漏：

   ```rust
   let status = pool.status();
   println!("活跃: {}, 总计: {}", status.active, status.total);
   ```

3. 确保会话按 RAII 模式在作用域内丢弃。

### 权限被拒绝错误

**症状**：`DbError::Permission("Permission denied...")`

**解决方案**：

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

3. 检查角色大小写（必须与配置完全匹配）：

   ```rust
   let session = pool.get_session("My_Role").await?; // 错误：大小写不匹配
   let session = pool.get_session("my_role").await?; // 正确
   ```

### 数据库连接错误

**症状**：`DbError::Connection("Failed to connect...")`

**解决方案**：

1. 验证 URL 格式：

   ```text
   # SQLite
   sqlite::memory:
   sqlite:///path/to/db

   # PostgreSQL
   postgresql://user:password@host:port/database

   # MySQL
   mysql://user:password@host:port/database
   ```

2. 检查网络连通性：

   ```bash
   ping postgres-server
   telnet postgres-server 5432
   ```

3. 验证凭据和权限：

   ```bash
   psql -U username -h postgres-server -d database
   ```

### 慢查询性能

**症状**：查询耗时过长

**解决方案**：

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

3. 在数据库中过滤而不是拉取全量后内存过滤：

   ```rust
   // 不好：获取所有再内存过滤
   let users = Model::find_all(&session).await?;

   // 好：在数据库中过滤
   let condition = Condition::all().add(Column::Name.like("%Alice%"));
   let users = Model::find_by_condition(&session, condition).await?;
   ```

### 高内存使用

**症状**：应用程序使用过多内存

**解决方案**：

1. 减少池大小（调低 `PoolConfig::max_connections`）；
2. 启用连接空闲超时（调低 `PoolConfig::idle_timeout`）；
3. 检查缓存容量：调低实体 `cache(max_capacity = ...)` 参数或缓存 Provider 容量。

---

## 📦 完整示例

改编自 [examples/basic/basic_crud.rs](../examples/src/basic/basic_crud.rs)（共 51 个可运行示例，见 [examples/README.md](../examples/README.md)）：

```rust
use dbnexus::{db_entity, DbPool};
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化连接池（嵌入式 SQLite，无需外部数据库）
    let pool = DbPool::new("sqlite::memory:").await?;

    // admin 会话（RAII：丢弃时连接自动归还）
    let session = pool.get_session("admin").await?;

    // 建表（DDL 仅限 admin 角色，经 DdlGuard 校验）
    session
        .execute_raw_ddl("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, email TEXT)")
        .await?;

    // 插入用户
    let user = Model {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    Model::insert(&session, user).await?;

    // 查询
    let users = Model::find_all(&session).await?;
    println!("找到 {} 个用户", users.len());

    // 池状态
    let status = pool.status();
    println!("池状态: 总计 {}, 活跃 {}, 空闲 {}", status.total, status.active, status.idle);

    Ok(())
}
```

---

## 📚 相关文档

| 文档 | 说明 |
|------|------|
| [📘 API 参考](API_REFERENCE.md) | 全部公开 API 的详细说明 |
| [🏗️ 架构文档](ARCHITECTURE.md) | 设计理念、模块划分、数据流与安全/性能设计 |
| [🔒 安全文档](SECURITY.md) | 纵深防御设计、最佳实践与漏洞报告流程 |
| [📊 性能基线](PERFORMANCE.md) | 端到端基准数据与复现命令 |
| [🧪 测试场景固化](TEST_SCENARIOS.md) | 测试金字塔基线、驱动组矩阵与 E2E 场景定义 |
| [📦 在线 API 文档](https://docs.rs/dbnexus) | docs.rs 自动生成的最新文档 |
