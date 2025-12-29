# DBNexus

[![Crates.io](https://img.shields.io/crates/v/dbnexus)](https://crates.io/crates/dbnexus)
[![Docs](https://docs.rs/dbnexus/badge.svg)](https://docs.rs/dbnexus)
[![License](https://img.shields.io/crates/l/dbnexus)](https://crates.io/crates/dbnexus)

**企业级 Rust 数据库抽象层 | 基于 Sea-ORM**

DBNexus 是一个基于 Sea-ORM 构建的高性能、高安全性 Rust 数据库抽象层，为应用提供企业级的数据访问能力。

## ✨ 核心特性

- 🔒 **内置权限控制** - 声明式宏自动生成权限检查代码，表级访问控制
- 🔄 **连接池管理** - 动态配置、健康检查、自动重连
- 📊 **监控指标** - Prometheus 指标导出，查询延迟统计
- 🚀 **声明式宏** - 三层宏系统，自动生成 CRUD 代码
- 🛡️ **RAII 生命周期** - 自动管理数据库连接，防止泄漏
- 🌍 **多数据库支持** - SQLite、PostgreSQL、MySQL

## 📦 安装

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
dbnexus = { version = "0.1", features = ["sqlite"] }  # 选择一种数据库
# 或
dbnexus = { version = "0.1", features = ["postgres"] }
# 或
dbnexus = { version = "0.1", features = ["mysql"] }
```

## 🚀 快速开始

### 1. 定义 Entity

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

### 2. 创建连接池

```rust
use dbnexus::DbPool;

let pool = DbPool::new("sqlite::memory:").await?;
```

### 3. 执行 CRUD 操作

```rust
// 获取 Session
let session = pool.get_session("admin").await?;

// 插入
let user = User {
    id: 1,
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
};
let inserted = User::insert(&session, user).await?;
println!("Inserted: {}", inserted.name);

// 查询
let found = User::find_by_id(&session, 1).await?;
if let Some(user) = found {
    println!("Found: {}", user.name);
}

// 更新
let mut user = found.unwrap();
user.email = "new@example.com";
User::update(&session, user).await?;

// 删除
User::delete(&session, 1).await?;
```

## 🔐 权限控制

### 定义权限

```rust
use dbnexus::{DbEntity, db_crud, db_permission};

#[derive(DbEntity)]
#[db_entity]
#[table_name = "users")]
#[db_crud]
#[db_permission(roles = ["admin", "manager"], operations = ["SELECT", "INSERT", "UPDATE"])]
struct User {
    #[primary_key]
    id: i64,
    name: String,
}
```

### 使用权限配置

创建 `permissions.yaml`:

```yaml
roles:
  admin:
    tables:
      - name: "*"
        operations: ["SELECT", "INSERT", "UPDATE", "DELETE"]
  readonly:
    tables:
      - name: "users"
        operations: ["SELECT"]
```

不同角色访问会被自动拒绝：

```rust
let admin_session = pool.get_session("admin").await?;
User::insert(&admin_session, user).await?; // ✅ 允许

let readonly_session = pool.get_session("readonly").await?;
User::insert(&readonly_session, user).await?; // ❌ 返回 PermissionDenied
```

## 📊 事务支持

```rust
let mut session = pool.get_session("admin").await?;

// 方式 1: 手动管理
session.begin_transaction().await?;
// ... 执行操作
session.commit().await?;

// 方式 2: 使用闭包（推荐）
let result = session.transaction(|session| async move {
    // 在事务中执行操作
    let user = User::find_by_id(session, 1).await?;
    Ok(user)
}).await?;
```

## 📁 项目结构

```
dbnexus/
├── src/
│   ├── lib.rs          # 主入口，公共 API 导出
│   ├── config/         # 配置管理 (DbConfig, DbError)
│   ├── pool/           # 连接池管理 (DbPool, Session)
│   ├── permission/     # 权限控制 (PermissionContext, RolePolicy)
│   └── entity/         # 实体转换工具
├── dbnexus-macros/     # 过程宏定义
│   └── src/lib.rs      # #[derive(DbEntity)], #[db_crud], #[db_permission]
└── examples/           # 示例代码
    ├── quickstart.rs   # 基础 CRUD 示例
    ├── permissions.rs  # 权限控制示例
    └── transactions.rs # 事务示例
```

## ⚙️ 配置

### 环境变量

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `DATABASE_URL` | 数据库连接字符串 | - |
| `DB_MAX_CONNECTIONS` | 最大连接数 | 20 |
| `DB_MIN_CONNECTIONS` | 最小连接数 | 5 |
| `DB_IDLE_TIMEOUT` | 空闲超时（秒） | 300 |
| `DB_ACQUIRE_TIMEOUT` | 获取连接超时（毫秒） | 5000 |
| `DB_PERMISSIONS_PATH` | 权限配置文件路径 | `permissions.yaml` |

### 配置文件

支持 YAML 和 TOML 格式：

```yaml
# dbnexus.yaml
database:
  url: "sqlite::memory:"
  max_connections: 20
  min_connections: 5
  idle_timeout: 300
  acquire_timeout: 5000
```

```toml
# dbnexus.toml
[database]
url = "sqlite::memory:"
max_connections = 20
min_connections = 5
idle_timeout = 300
acquire_timeout = 5000
```

## 🧪 测试

```bash
# 运行所有测试
cargo test --features sqlite --all

# 运行集成测试
cargo test --features sqlite -p dbnexus --tests

# 运行特定测试
cargo test --features sqlite -p dbnexus test_pool_creation
```

## 📚 文档

- [API 文档](https://docs.rs/dbnexus)
- [快速开始指南](examples/quickstart.rs)
- [权限控制示例](examples/permissions.rs)
- [事务示例](examples/transactions.rs)

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

## 📄 许可证

本项目采用 MIT 或 Apache-2.0 许可证。
