# DB Nexus 用户指南

## 目录

- [产品概述](#产品概述)
- [快速开始](#快速开始)
- [安装与配置](#安装与配置)
- [基本 CRUD 操作](#基本-crud-操作)
- [权限控制](#权限控制)
- [高级特性](#高级特性)
- [最佳实践](#最佳实践)
- [故障排除](#故障排除)

---

## 产品概述

### 什么是 DB Nexus

DB Nexus 是一个基于 Sea-ORM 构建的**企业级 Rust 数据库抽象层**，为应用提供高性能、高安全性、可扩展的数据访问能力。

### 核心特性

| 特性 | 描述 |
|------|------|
| **Session 机制** | RAII 自动管理数据库连接生命周期，防止连接泄漏 |
| **权限控制** | 声明式宏自动生成权限检查代码，支持 YAML 配置和 RBAC |
| **连接池管理** | 动态配置修正、健康检查、自动重连 |
| **三层宏系统** | `#[derive(DbEntity)]`、`#[db_crud]`、`#[db_permission]` |
| **多数据库支持** | SQLite、PostgreSQL、MySQL（编译时互斥选择） |
| **监控指标** | Prometheus 指标导出，查询延迟统计 |
| **审计日志** | CRUD 操作审计、用户追踪、敏感操作告警 |
| **实体缓存** | LRU 缓存策略、TTL 过期、缓存穿透/击穿防护 |
| **分片管理** | 时间/哈希分片策略，自动路由 |
| **分布式追踪** | OpenTelemetry 集成，全链路追踪 |
| **全局索引** | 跨分片的全局唯一索引支持 |

### 技术栈

- **语言**: Rust 2024 Edition
- **ORM**: Sea-ORM 2.0.0-rc.22
- **异步运行时**: Tokio 1.42
- **数据库驱动**: sqlx (sqlite/postgres/mysql)
- **宏系统**: dbnexus-macros (过程宏)

### 使用场景

- **企业级应用**: 需要严格权限控制的大型系统
- **微服务架构**: 多数据库、多租户场景
- **高并发系统**: 需要连接池和缓存优化
- **审计要求**: 需要完整操作日志的系统
- **数据敏感**: 需要细粒度权限控制的应用

---

## 快速开始

### 环境要求

- **Rust 版本**: 1.85+
- **数据库**: SQLite 3.35+ / PostgreSQL 12+ / MySQL 8.0+
- **Cargo**: 最新版本

### 步骤 1：添加依赖

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
dbnexus = { version = "0.1.0", features = ["sqlite"] }
```

选择数据库特性（互斥）：

```toml
# SQLite
dbnexus = { version = "0.1.0", features = ["sqlite"] }

# PostgreSQL
dbnexus = { version = "0.1.0", features = ["postgres"] }

# MySQL
dbnexus = { version = "0.1.0", features = ["mysql"] }
```

### 步骤 2：定义实体

```rust
use dbnexus::{DbPool, DbEntity, db_crud};

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

### 步骤 3：创建连接池

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建连接池
    let pool = DbPool::new("sqlite::memory:").await?;

    // 或者从环境变量读取配置
    // let pool = DbPool::new().await?;

    println!("连接池创建成功！");
    Ok(())
}
```

### 步骤 4：执行 CRUD 操作

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("sqlite::memory:").await?;
    let session = pool.get_session("admin").await?;

    // 创建用户
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    User::insert(&session, user).await?;
    println!("用户创建成功！");

    // 查询用户
    let found = User::find_by_id(&session, 1).await?;
    println!("查询到用户: {}", found.name);

    // 更新用户
    let updated = User {
        id: 1,
        name: "Alice Smith".to_string(),
        email: "alice@example.com".to_string(),
    };
    User::update(&session, updated).await?;
    println!("用户更新成功！");

    // 删除用户
    User::delete(&session, 1).await?;
    println!("用户删除成功！");

    Ok(())
}
```

### 完整示例

```rust
use dbnexus::{DbPool, DbEntity, db_crud};

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建连接池
    let pool = DbPool::new("sqlite::memory:").await?;
    let session = pool.get_session("admin").await?;

    // 创建用户
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    User::insert(&session, user).await?;
    println!("用户创建成功！");

    // 查询用户
    let found = User::find_by_id(&session, 1).await?;
    println!("查询到用户: {}", found.name);

    Ok(())
}
```

### 运行示例

```bash
# 运行快速开始示例
cargo run --example quickstart --features sqlite

# 运行权限控制示例
cargo run --example permissions --features sqlite

# 运行事务示例
cargo run --example transactions --features sqlite
```

---

## 安装与配置

### 安装方式

#### 方式一：Cargo 依赖（推荐）

```toml
[dependencies]
dbnexus = "0.1.0"
```

#### 方式二：Git 源码

```toml
[dependencies]
dbnexus = { git = "https://github.com/Kirky-X/dbnexus", tag = "v0.1.0" }
```

### 编译特性

DB Nexus 采用模块化设计，所有特性都是可选的：

```toml
[dependencies]
dbnexus = { version = "0.1.0", features = ["sqlite", "cache", "audit"] }
```

#### 数据库特性（互斥）

| 特性 | 描述 |
|------|------|
| `sqlite` | SQLite 数据库支持 |
| `postgres` | PostgreSQL 数据库支持 |
| `mysql` | MySQL 数据库支持 |

#### 可选特性

| 特性 | 描述 | 默认 |
|------|------|------|
| `cache` | 实体缓存支持 | false |
| `audit` | 审计日志支持 | false |
| `sharding` | 分片支持 | false |
| `global-index` | 全局索引支持 | false |
| `metrics` | Prometheus 指标导出 | false |
| `migration` | Migration 工具 | false |
| `auto-migrate` | 自动迁移支持 | false |
| `permission-engine` | 可插拔权限引擎 | false |
| `tracing` | 分布式追踪支持 | false |
| `runtime-tokio-rustls` | Tokio 运行时 + RustLS | true |
| `all-optional` | 启用所有可选功能 | false |

### 环境变量配置

DB Nexus 支持以下环境变量：

| 变量 | 描述 | 默认值 | 必需 |
|------|------|--------|------|
| `DATABASE_URL` | 数据库连接字符串 | - | 是 |
| `DB_MAX_CONNECTIONS` | 最大连接数 | 20 | 否 |
| `DB_MIN_CONNECTIONS` | 最小连接数 | 5 | 否 |
| `DB_IDLE_TIMEOUT` | 空闲超时（秒） | 300 | 否 |
| `DB_ACQUIRE_TIMEOUT` | 获取连接超时（毫秒） | 5000 | 否 |
| `DB_PERMISSIONS_PATH` | 权限配置文件路径 | `permissions.yaml` | 否 |
| `TEST_DB_TYPE` | 测试数据库类型 | `sqlite` | 否 |
| `TEST_TIMEOUT_MS` | 测试超时时间（毫秒） | 30000 | 否 |

### 配置文件

#### 数据库配置

```rust
use dbnexus::DbConfig;

let config = DbConfig::from_env()?;
println!("Database URL: {}", config.url);
println!("Max connections: {}", config.max_connections);
```

#### 连接池配置

```rust
use dbnexus::{DbPool, PoolConfig};

let pool_config = PoolConfig::new()
    .max_connections(50)
    .min_connections(10)
    .idle_timeout(600)
    .acquire_timeout(10000);

let pool = DbPool::with_config(pool_config).await?;
```

### 数据库连接字符串格式

#### SQLite

```bash
# 内存数据库
DATABASE_URL=sqlite::memory:

# 文件数据库
DATABASE_URL=sqlite:./data.db

# 带 URI 参数
DATABASE_URL=sqlite:./data.db?mode=ro
```

#### PostgreSQL

```bash
# 基本连接
DATABASE_URL=postgres://user:password@localhost:5432/dbname

# SSL 连接
DATABASE_URL=postgres://user:password@localhost:5432/dbname?sslmode=require

# Unix Socket
DATABASE_URL=postgres://user@/dbname?host=/var/run/postgresql
```

#### MySQL

```bash
# 基本连接
DATABASE_URL=mysql://user:password@localhost:3306/dbname

# SSL 连接
DATABASE_URL=mysql://user:password@localhost:3306/dbname?ssl-mode=REQUIRED

# Unix Socket
DATABASE_URL=mysql://user@/dbname?unix_socket=/var/run/mysqld/mysqld.sock
```

---

## 基本 CRUD 操作

### 宏 vs API 选择指南

DB Nexus 提供两种操作方式：宏和 API。

#### 宏的使用场景

- 简单的 CRUD 操作（insert、delete）
- 需要自动生成 SQL 语句
- 代码简洁性优先

#### API 的使用场景

- 复杂的查询操作（JOIN、子查询、聚合）
- 需要获取查询结果
- 事务操作
- 需要完整的灵活性

### 插入操作

#### 使用宏

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

// 插入单个用户
let user = User {
    id: 1,
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
};
let inserted = User::insert(&session, user).await?;
println!("插入的用户 ID: {}", inserted.id);
```

#### 使用 API

```rust
use dbnexus::Session;

// 执行原始 SQL
session.execute_raw(
    "INSERT INTO users (name, email) VALUES ($1, $2)",
    vec!["Alice".into(), "alice@example.com".into()],
).await?;
```

### 查询操作

#### 使用宏

```rust
// 根据 ID 查询
let user = User::find_by_id(&session, 1).await?;

// 查询所有
let all_users = User::find_all(&session).await?;

// 统计数量
let count = User::count(&session).await?;
```

#### 使用 API

```rust
// 执行查询
let results = session.execute_raw(
    "SELECT * FROM users WHERE name LIKE $1",
    vec!["%Alice%".into()],
).await?;

// 事务中的查询
session.transaction(|s| {
    let users = s.execute_raw("SELECT * FROM users", vec![])?;
    Ok(users)
}).await?;
```

### 更新操作

#### 使用宏

```rust
let user = User {
    id: 1,
    name: "Alice Smith".to_string(),
    email: "alice.smith@example.com".to_string(),
};
User::update(&session, user).await?;
```

#### 使用 API

```rust
session.execute_raw(
    "UPDATE users SET name = $1, email = $2 WHERE id = $3",
    vec!["Alice Smith".into(), "alice.smith@example.com".into(), 1.into()],
).await?;
```

### 删除操作

#### 使用宏

```rust
// 删除单个用户
User::delete(&session, 1).await?;

// 批量删除
User::delete_many(&session, "name = 'Alice'".to_string()).await?;
```

#### 使用 API

```rust
session.execute_raw(
    "DELETE FROM users WHERE id = $1",
    vec![1.into()],
).await?;
```

### 事务操作

```rust
use dbnexus::Session;

let result = session.transaction(|s| -> Result<(), Box<dyn std::error::Error>> {
    // 扣减库存
    s.execute_raw(
        "UPDATE inventory SET count = count - 1 WHERE id = $1",
        vec![1.into()],
    )?;

    // 创建订单
    s.execute_raw(
        "INSERT INTO orders (user_id, product_id) VALUES ($1, $2)",
        vec![1.into(), 1.into()],
    )?;

    Ok(())
}).await?;

match result {
    Ok(_) => println!("事务提交成功！"),
    Err(e) => println!("事务回滚: {}", e),
}
```

---

## 权限控制

### 声明式权限

DB Nexus 提供声明式权限配置，通过宏属性简化权限检查：

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

### 权限配置说明

#### `#[db_permission]` 属性

| 参数 | 描述 |
|------|------|
| `role` | 角色名称 |
| `actions` | 允许的操作列表 |

#### 支持的操作

| 操作 | 描述 |
|------|------|
| `read` | 读取/查询操作 |
| `write` | 写入/插入操作 |
| `delete` | 删除操作 |

### 权限配置文件

#### permissions.yaml 格式

```yaml
roles:
  admin:
    tables:
      - name: "*"
        operations:
          - SELECT
          - INSERT
          - UPDATE
          - DELETE

  user:
    tables:
      - name: "users"
        operations:
          - SELECT
          - INSERT
      - name: "products"
        operations:
          - SELECT

  guest:
    tables:
      - name: "products"
        operations:
          - SELECT
```

#### 使用权限引擎

```rust
use dbnexus::permission_engine::{PolicyDecisionPoint, YamlPermissionProvider};

let provider = YamlPermissionProvider::new("permissions.yaml")?;
let pdp = PolicyDecisionPoint::new(Arc::new(provider));

// 检查权限
let result = pdp.check_permission("admin", "users", "SELECT").await;

match result {
    Ok(decision) => {
        if decision.allowed {
            println!("权限检查通过");
        } else {
            println!("权限被拒绝: {}", decision.reason);
        }
    }
    Err(e) => println!("权限检查错误: {}", e),
}
```

### 角色继承

```rust
use dbnexus::{RolePolicy, PermissionConfig, PermissionContext, TablePermission};

let role_policy = RolePolicy {
    name: "super_admin".to_string(),
    inherits: vec!["admin".to_string(), "moderator".to_string()],
    permissions: vec![],
};
```

### 权限上下文

```rust
use dbnexus::{PermissionContext, PermissionAction};

let ctx = PermissionContext::new()
    .with_user_id(123)
    .with_tenant_id("tenant_abc")
    .with_ip_address("192.168.1.1");
```

---

## 高级特性

### 审计日志

启用审计日志功能：

```rust
use dbnexus::{DbEntity, db_audit};

#[derive(DbEntity)]
#[db_entity]
#[table_name = "users")]
#[table_name = "users")]
#[db_audit(operations = ["CREATE", "UPDATE", "DELETE"])]
struct User {
    #[primary_key]
    id: i64,
    name: String,
    email: String,
}
```

#### 审计操作类型

| 操作 | 描述 |
|------|------|
| `CREATE` | 创建操作 |
| `READ` | 读取操作 |
| `UPDATE` | 更新操作 |
| `DELETE` | 删除操作 |

#### 查询审计日志

```rust
use dbnexus::audit::AuditLog;

let logs = AuditLog::query()
    .user_id(123)
    .operation("CREATE")
    .table_name("users")
    .execute(&session)
    .await?;
```

### 实体缓存

启用缓存功能：

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

#### 缓存配置

| 参数 | 描述 | 默认值 |
|------|------|--------|
| `ttl` | 缓存过期时间（秒） | 300 |
| `capacity` | 缓存最大容量 | 1000 |

#### 缓存策略

- **LRU**: 最近最少使用的条目优先被淘汰
- **TTL**: 基于时间的过期策略
- **穿透防护**: 防止缓存击穿
- **击穿防护**: 防止热点 key 过期时的并发冲击

### 分片管理

#### 时间分片

```rust
use dbnexus::sharding::{ShardRouter, ShardConfig, YearlyStrategy};

let config = ShardConfig::new("yearly", 12, "orders", "postgresql://localhost/{shard}");
let router = ShardRouter::with_config(&config);

// 路由到特定分片
let shard = router.route_by_time(chrono::Utc::now());
```

#### 哈希分片

```rust
use dbnexus::sharding::{ShardRouter, ShardConfig, HashStrategy};

let config = ShardConfig::new("hash", 4, "users", "postgresql://localhost/shard_{shard}");
let router = ShardRouter::with_config(&config);

// 根据用户 ID 路由
let shard = router.route_by_hash(&12345i64.to_le_bytes());
```

### 全局索引

```rust
use dbnexus::{DbEntity, global_index};

#[derive(DbEntity)]
#[db_entity]
#[table_name = "orders")]
#[global_index]
struct Order {
    #[primary_key]
    id: i64,
    user_id: i64,
    product_id: i64,
    amount: Decimal,
}

// 创建全局唯一索引
let index = global_index::GlobalIndex::new("order_user_product", vec!["user_id", "product_id"]);
```

### 监控指标

启用 Prometheus 指标：

```rust
use dbnexus::metrics;

let metrics_config = metrics::MetricsConfig::new()
    .enabled(true)
    .port(9090)
    .path("/metrics");

// 注册指标
let query_duration = metrics::register_histogram!(
    "dbnexus_query_duration_seconds",
    "Query duration in seconds"
)?;
```

#### 内置指标

| 指标名称 | 类型 | 描述 |
|----------|------|------|
| `dbnexus_pool_connections` | Gauge | 连接池连接数 |
| `dbnexus_query_duration_seconds` | Histogram | 查询耗时 |
| `dbnexus_queries_total` | Counter | 查询总数 |
| `dbnexus_cache_hits_total` | Counter | 缓存命中次数 |
| `dbnexus_cache_misses_total` | Counter | 缓存未命中次数 |

### 分布式追踪

启用 OpenTelemetry 追踪：

```rust
use dbnexus::tracing;

let tracing_config = tracing::TracingConfig::new()
    .enabled(true)
    .service_name("dbnexus")
    .exporter(tracing::ExporterType::OTLP);

tracing::init(tracing_config)?;
```

---

## 最佳实践

### 1. 连接池配置

```rust
use dbnexus::PoolConfig;

let pool_config = PoolConfig::new()
    .max_connections(50)           // 根据并发需求调整
    .min_connections(5)            // 保持最小连接数
    .idle_timeout(600)             // 空闲超时 10 分钟
    .acquire_timeout(10000);       // 获取连接超时 10 秒
```

### 2. 错误处理

```rust
use dbnexus::{DbPool, DbError};

match DbPool::new().await {
    Ok(pool) => {
        let session = pool.get_session("admin").await?;
        // 业务逻辑
    }
    Err(DbError::ConnectionFailed(e)) => {
        eprintln!("数据库连接失败: {}", e);
        // 重试逻辑
    }
    Err(DbError::PoolExhausted) => {
        eprintln!("连接池已耗尽");
        // 排队或拒绝
    }
    Err(e) => {
        eprintln!("数据库错误: {}", e);
    }
}
```

### 3. 事务使用

```rust
let result = session.transaction(|s| {
    // 在事务中执行多个操作

    // 1. 创建订单
    let order = create_order(s, &user_id, &items)?;

    // 2. 扣减库存
    update_inventory(s, &items)?;

    // 3. 更新用户积分
    update_points(s, &user_id, &points)?;

    Ok(order)
}).await;

match result {
    Ok(order) => println!("订单创建成功: {}", order.id),
    Err(e) => println!("订单创建失败: {}", e),
}
```

### 4. 权限最小化

```rust
// 不推荐：过度宽松的权限
#[db_permission(role = "admin", actions = ["read", "write", "delete"])]

// 推荐：精确的权限配置
#[db_permission(role = "admin", actions = ["read", "write"])]
#[db_permission(role = "admin", actions = ["delete"], condition = "is_owner()")]
struct UserData {
    // ...
}
```

### 5. 缓存策略

```rust
// 高频读取、低频修改的数据适合缓存
#[db_cache(ttl = 3600, capacity = 5000)]
struct Product {
    #[primary_key]
    id: i64,
    name: String,
    price: Decimal,
}

// 低频读取或高频修改的数据不适合缓存
struct AuditLog {
    #[primary_key]
    id: i64,
    // ... 不启用缓存
}
```

### 6. 分片策略选择

| 分片类型 | 适用场景 |
|----------|----------|
| **时间分片** | 日志、订单、事件等按时间增长的数据 |
| **哈希分片** | 用户数据、需要均匀分布的数据 |
| **范围分片** | ID 范围明确、需要范围查询的数据 |
| **地理位置** | 需要按地区查询的数据 |

### 7. 性能优化

```rust
// 使用批量操作减少数据库往返
let users = vec![
    User { id: 1, name: "Alice".into(), email: "alice@example.com".into() },
    User { id: 2, name: "Bob".into(), email: "bob@example.com".into() },
    User { id: 3, name: "Charlie".into(), email: "charlie@example.com".into() },
];

for user in users {
    User::insert(&session, user).await?;
}

// 使用只读副本处理查询（如果配置了）
let read_session = pool.get_session("reader").await?;
```

---

## 故障排除

### 常见错误

#### 1. 连接池耗尽

**错误信息**: `DbError: PoolExhausted`

**解决方案**:
```rust
// 增加连接池大小
let pool_config = PoolConfig::new()
    .max_connections(100);

// 使用连接超时
let session = pool
    .get_session_with_timeout("admin", Duration::from_secs(30))
    .await?;
```

#### 2. 数据库连接失败

**错误信息**: `DbError::ConnectionFailed`

**解决方案**:
```bash
# 检查数据库服务状态
pg_isready -h localhost -p 5432

# 验证连接字符串
export DATABASE_URL="postgres://user:pass@localhost:5432/db"
```

#### 3. 权限检查失败

**错误信息**: `PermissionDenied`

**解决方案**:
```rust
// 确认角色在配置中定义
let provider = YamlPermissionProvider::new("permissions.yaml")?;

// 打印权限配置用于调试
println!("{:?}", provider.get_role_permissions("admin"));
```

#### 4. 编译特性冲突

**错误信息**: `Cannot enable both 'sqlite' and 'postgres' features`

**解决方案**:
```toml
# 只能选择一种数据库
dbnexus = { version = "0.1.0", features = ["sqlite"] }
```

#### 5. 宏冲突

**错误信息**: 重复实现错误

**解决方案**:
```rust
// 不要同时使用 #[db_crud] 和 #[db_permission]
// 使用 #[db_crud] 时，权限在宏内部自动生成

// 正确的用法
#[derive(DbEntity)]
#[db_entity]
#[table_name = "users")]
#[db_crud]  // 包含权限检查
struct User {
    #[primary_key]
    id: i64,
}

// 正确的用法（自定义权限）
#[derive(DbEntity)]
#[db_entity]
#[table_name = "users")]
#[db_permission(role = "admin", actions = ["read", "write", "delete"])]
struct UserData {
    #[primary_key]
    id: i64,
    // 手动实现 CRUD 或使用 API
}
```

### 调试技巧

#### 1. 启用日志

```rust
use env_logger;

env_logger::init();

let pool = DbPool::new().await?;
```

#### 2. 连接池状态监控

```rust
let status = pool.status();
println!("总连接数: {}", status.total);
println!("活跃连接: {}", status.active);
println!("空闲连接: {}", status.idle);
println!("等待任务: {}", status.waiters);
```

#### 3. 慢查询日志

```rust
use dbnexus::metrics;

let slow_query_duration = std::time::Duration::from_millis(1000);

// 配置慢查询阈值
```

### 获取帮助

- **GitHub Issues**: [报告问题](https://github.com/Kirky-X/dbnexus/issues)
- **文档**: [docs.rs/dbnexus](https://docs.rs/dbnexus)
- **API 参考**: [docs/API_REFERENCE.md](./API_REFERENCE.md)
- **架构文档**: [docs/ARCHITECTURE.md](./ARCHITECTURE.md)
