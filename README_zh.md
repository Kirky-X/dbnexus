# DBNexus

<div align="center">

**企业级 Rust 数据库抽象层**

[![Crates.io](https://img.shields.io/crates/v/dbnexus)](https://crates.io/crates/dbnexus)
[![文档](https://docs.rs/dbnexus/badge.svg)](https://docs.rs/dbnexus)
[![许可证](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

**基于 Sea-ORM 构建的高性能、高安全性、功能丰富的数据库访问层**

[快速开始](#快速开始) • [功能特性](#功能特性) • [文档](https://docs.rs/dbnexus) • [示例](#示例)

</div>

---

## 📖 概述

DBNexus 是一个为 Rust 构建的企业级数据库抽象层，提供：

- **基于会话的连接管理**：RAII 风格的自动连接生命周期管理
- **声明式权限控制**：通过过程宏实现编译时权限检查
- **智能连接池**：动态配置修正和健康检查
- **企业级功能**：指标、分布式追踪、审计日志等

建立在 [Sea-ORM](https://www.sea-ql.org/SeaORM/) 之上，DBNexus 在保持简单和易用性的同时，增加了生产就绪的功能。

## 🚀 快速开始

### 安装

在你的 `Cargo.toml` 中添加：

```toml
[dependencies]
dbnexus = "0.1"
```

### 基础用法

```rust
use dbnexus::{DbPool, DbEntity, db_crud};

// 定义你的实体
#[derive(DbEntity)]
#[db_entity]
#[table_name = "users"]
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

    // 获取基于角色访问的会话
    let session = pool.get_session("admin").await?;

    // 插入用户
    let user = User {
        id: 1,
        name: "张三".to_string(),
        email: "zhangsan@example.com".to_string(),
    };
    User::insert(&session, user).await?;

    // 查询用户
    let users = User::find_all(&session).await?;
    println!("找到 {} 个用户", users.len());

    Ok(())
}
```

### 使用权限控制

```rust
use dbnexus::{DbPool, DbEntity, db_crud, db_permission};

#[derive(DbEntity)]
#[db_entity]
#[table_name = "users"]
#[db_crud]
#[db_permission(roles = ["admin", "manager"], operations = ["SELECT", "INSERT"])]
struct User {
    #[primary_key]
    id: i64,
    name: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("sqlite::memory:").await?;

    // 管理员可以访问
    let session = pool.get_session("admin").await?;
    User::find_all(&session).await?;

    // 普通用户会被拒绝
    let session = pool.get_session("guest").await?;
    User::find_all(&session).await?; // 错误：权限被拒绝

    Ok(())
}
```

## ✨ 功能特性

### 核心功能

- **🔒 权限控制**
  - 基于角色的表级访问控制
  - 编译时权限验证
  - 支持 YAML 和 RBAC 策略提供者
  - 权限策略的 LRU 缓存

- **🏊 智能连接池**
  - 基于 RAII 的自动连接管理
  - 动态配置修正
  - 健康检查和自动连接重建
  - 连接预热支持

- **⚡ 高性能**
  - 零成本抽象
  - 频繁访问数据的 LRU 缓存
  - 使用原子操作的无锁计数器
  - 面向 Tokio 的异步设计

### 企业级功能

- **📊 监控**
  - Prometheus 指标导出
  - 连接池状态监控
  - 查询性能跟踪

- **🔍 分布式追踪**
  - OpenTelemetry 集成
  - Jaeger 支持
  - 自动追踪传播

- **📝 审计日志**
  - 所有数据库操作的自动审计
  - 操作类型和时间戳跟踪
  - 用户上下文捕获

- **🗄️ 高级数据库功能**
  - 数据库迁移支持
  - 自动迁移执行
  - 数据分片支持
  - 跨数据库查询的全局索引

### 开发者体验

- **🎯 过程宏**
  - `#[db_entity]` - 实体定义
  - `#[db_crud]` - 自动生成 CRUD 方法
  - `#[db_permission]` - 权限声明
  - `#[db_cache]` - 缓存注解
  - `#[db_audit]` - 审计注解

- **🔧 灵活配置**
  - 环境变量
  - YAML 配置文件
  - TOML 配置文件
  - 构建器模式 API

## 🎨 特性标志

DBNexus 使用 Cargo 特性让你精确选择所需功能：

### 数据库驱动（选择一个）

```toml
# SQLite（默认）
dbnexus = { version = "0.1", features = ["sqlite"] }

# PostgreSQL
dbnexus = { version = "0.1", features = ["postgres"] }

# MySQL
dbnexus = { version = "0.1", features = ["mysql"] }
```

### 运行时

```toml
# Tokio with RustLS（默认）
dbnexus = { version = "0.1", features = ["runtime-tokio-rustls"] }

# Tokio with Native TLS
dbnexus = { version = "0.1", features = ["runtime-tokio-native-tls"] }

# AsyncStd
dbnexus = { version = "0.1", features = ["runtime-async-std"] }
```

### 可选功能

```toml
# 核心功能
dbnexus = { version = "0.1", features = [
    "permission",      # 权限控制
    "sql-parser",      # SQL 解析
    "macros",          # 过程宏
] }

# 企业级功能
dbnexus = { version = "0.1", features = [
    "metrics",         # Prometheus 指标
    "tracing",         # 分布式追踪
    "audit",           # 审计日志
    "migration",       # 数据库迁移
    "sharding",        # 数据分片
] }

# 配置
dbnexus = { version = "0.1", features = [
    "config-yaml",     # YAML 配置支持
    "config-toml",     # TOML 配置支持
    "config-env",       # 环境变量（默认）
] }
```

### 预设配置

```toml
# 嵌入式设备最小配置
dbnexus = { version = "0.1", default-features = false, features = ["minimal"] }

# 微服务配置
dbnexus = { version = "0.1", default-features = false, features = ["microservice"] }

# 完整企业功能
dbnexus = { version = "0.1", default-features = false, features = ["all-optional"] }
```

查看 [FEATURES.md](FEATURES.md) 了解所有特性及其组合的完整列表。

## 📚 文档

- **[用户指南](USER_GUIDE.md)** - 使用 DBNexus 的全面指南
- **[API 参考](API_REFERENCE.md)** - 完整 API 文档
- **[架构文档](ARCHITECTURE.md)** - 系统架构和设计决策
- **[示例](examples/)** - 可运行的代码示例
- **[Rust 文档](https://docs.rs/dbnexus)** - docs.rs 上的 API 文档

## 💡 示例

### 配置

```rust
use dbnexus::{DbPool, config::DbConfigBuilder};

let config = DbConfigBuilder::new()
    .url("postgresql://user:pass@localhost/db")
    .max_connections(20)
    .min_connections(5)
    .idle_timeout(300)
    .acquire_timeout(5000)
    .build()?;

let pool = DbPool::try_from_config(config).await?;
```

### 环境变量

```bash
export DATABASE_URL="postgresql://user:pass@localhost/db"
export DB_MAX_CONNECTIONS=20
export DB_MIN_CONNECTIONS=5
export DB_ADMIN_ROLE=admin
```

```rust
let pool = DbPool::new().await?;
```

### 事务

```rust
use dbnexus::DbPool;

let pool = DbPool::new("sqlite::memory:").await?;
let mut session = pool.get_session("admin").await?;

// 开始事务
session.begin_transaction().await?;

// 多个操作
User::insert(&session, user1).await?;
User::insert(&session, user2).await?;

// 提交
session.commit_transaction().await?;
```

### 监控

```rust
use dbnexus::{DbPool, metrics::MetricsCollector};

let pool = DbPool::new("postgresql://localhost/db").await?;

// 获取连接池状态
let status = pool.status();
println!("活跃: {}, 空闲: {}", status.active, status.idle);

// 导出 Prometheus 指标
let metrics = MetricsCollector::new(&pool);
println!("{}", metrics.export_prometheus());
```

查看 [examples/](examples/) 目录获取更全面的示例。

## 🏗️ 架构

DBNexus 采用分层架构：

```
┌─────────────────────────────────────────────────┐
│              应用层                              │
│     (使用 DbPool 和 Session 的代码)              │
└────────────────┬────────────────────────────┘
                 │
┌────────────────▼────────────────────────────┐
│           DBNexus API 层                     │
│   - DbPool, Session                          │
│   - 权限检查                                │
│   - 事务管理                                 │
└────────────────┬────────────────────────────┘
                 │
┌────────────────▼────────────────────────────┐
│           功能模块                             │
│   - Config, Permission, Metrics            │
│   - Migration, Sharding, Audit             │
└────────────────┬────────────────────────────┘
                 │
┌────────────────▼────────────────────────────┐
│           连接池层                             │
│   - 连接生命周期管理                          │
│   - 健康检查                                 │
│   - RAII 保证                                  │
└────────────────┬────────────────────────────┘
                 │
┌────────────────▼────────────────────────────┐
│         Sea-ORM / SQLx                        │
│   - 数据库驱动                                │
│   - 查询构建器                                │
└───────────────────────────────────────────┘
```

查看 [ARCHITECTURE.md](ARCHITECTURE.md) 获取详细的架构文档。

## 🔒 安全性

DBNexus 在设计时就考虑了安全性：

- **无 unsafe 代码** - 所有库代码都使用 `#![forbid(unsafe_code)]`
- **权限强制执行** - 具有编译时验证的表级访问控制
- **SQL 注入防护** - 默认使用参数化查询
- **配置路径验证** - 防止路径遍历攻击
- **速率限制** - 权限检查速率限制以防止滥用

## 🧪 测试

### 运行测试

```bash
# SQLite 测试
cargo test --features sqlite

# PostgreSQL 测试
cargo test --features postgres

# MySQL 测试
cargo test --features mysql

# 所有测试（需要 Docker）
make test-all
```

### 使用 Docker

```bash
# 启动数据库
make docker-up

# 运行所有测试
make test-all

# 停止数据库
make docker-down
```

## 🤝 贡献

欢迎贡献！请参阅 [CONTRIBUTING.md](CONTRIBUTING.md) 了解指南。

### 开发设置

```bash
# 克隆仓库
git clone https://github.com/Kirky-X/dbnexus.git
cd dbnexus

# 安装 pre-commit 钩子
./scripts/install-pre-commit.sh

# 运行测试
cargo test --all-features

# 运行 linter
cargo clippy --all-features
```

## 📝 许可证

以以下任一许可方式授权：

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) 或 http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) 或 http://opensource.org/licenses/MIT)

由你选择。

## 🙏 致谢

- [Sea-ORM](https://www.sea-ql.org/SeaORM/) - 优秀的 ORM 框架，DBNexus 建立在其上
- [SQLx](https://github.com/launchbadge/sqlx) - 异步 SQL 工具包
- Rust 社区提供的优秀工具和库

## 📞 支持

- **文档**: https://docs.rs/dbnexus
- **问题**: https://github.com/Kirky-X/dbnexus/issues
- **讨论**: https://github.com/Kirky-X/dbnexus/discussions

## 🌟 获取星标

如果你觉得 DBNexus 有用，请考虑在 [GitHub](https://github.com/Kirky-X/dbnexus) 上给它一个星标 ⭐！

---

<div align="center">

**由 DBNexus 团队用 ❤️ 构建**

[GitHub](https://github.com/Kirky-X/dbnexus) • [Rust](https://www.rust-lang.org) • [Sea-ORM](https://www.sea-ql.org/SeaORM/)

</div>
