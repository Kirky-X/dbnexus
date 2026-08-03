<div align="center">

<a name="dbnexus"></a>

<img src="docs/image/logo/logo.png" alt="DBNexus Logo" width="200" style="margin-bottom: 16px;">

<p>
  <a href="https://github.com/Kirky-X/dbnexus/actions/workflows/ci.yml"><img src="https://github.com/Kirky-X/dbnexus/actions/workflows/ci.yml/badge.svg" alt="CI 状态" style="display:inline;margin:0 4px;"></a> <a href="https://crates.io/crates/dbnexus"><img src="https://img.shields.io/crates/v/dbnexus.svg" alt="版本" style="display:inline;margin:0 4px;"></a> <a href="https://docs.rs/dbnexus"><img src="https://docs.rs/dbnexus/badge.svg" alt="文档" style="display:inline;margin:0 4px;"></a> <a href="https://crates.io/crates/dbnexus"><img src="https://img.shields.io/crates/d/dbnexus.svg" alt="下载量" style="display:inline;margin:0 4px;"></a> <a href="https://github.com/Kirky-X/dbnexus/blob/main/LICENSE"><img src="https://img.shields.io/crates/l/dbnexus.svg" alt="许可证" style="display:inline;margin:0 4px;"></a> <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/rust-1.85+-orange.svg" alt="Rust 1.85+" style="display:inline;margin:0 4px;"></a>
</p>

[English](./README_EN.md)

<p align="center">
  <strong>企业级 Rust 数据库抽象层</strong>
</p>

<p align="center">
  <a href="#features" style="color:#3B82F6;">✨ 功能特性</a> •
  <a href="#quick-start" style="color:#3B82F6;">🚀 快速开始</a> •
  <a href="#documentation" style="color:#3B82F6;">📚 文档</a> •
  <a href="#examples" style="color:#3B82F6;">💻 示例</a> •
  <a href="#contributing" style="color:#3B82F6;">🤝 贡献</a>
</p>

</div>

---

### 🎯 基于 Sea-ORM 构建的高性能、高安全性、功能丰富的数据库访问层

DBNexus 提供了一种**声明式**的数据库访问方法：

| ✨ 类型安全 | 🔒 权限控制 | 🏊 智能连接池 | 📊 企业级监控 |
|:---------:|:----------:|:--------------:|:--------:|
| 编译时检查 | 表级 RBAC | RAII 自动管理 | Prometheus 指标 |

```rust
use dbnexus::{DbPool, db_entity};
use sea_orm::entity::prelude::*;

#[db_entity(table_name = "users", primary_key = "id")]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub email: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("sqlite::memory:").await?;
    let session = pool.get_session("admin").await?;
    let user = Model { id: 1, name: "Alice".to_string(), email: "alice@example.com".to_string() };
    Model::insert(&session, user).await?;
    Ok(())
}
```

---

## 📋 目录

<details open style="border-radius:8px; padding:16px; border:1px solid #E2E8F0;">
<summary style="cursor:pointer; font-weight:600; color:#1E293B;">📑 目录（点击展开）</summary>

- [✨ 功能特性](#features)
- [🚀 快速开始](#quick-start)
  - [📦 安装](#installation)
  - [💡 基本用法](#basic-usage)
  - [🔒 权限控制](#permission-control)
- [🎨 特性标志](#feature-flags)
- [📚 文档](#documentation)
- [💻 示例](#examples)
- [🏗️ 架构](#architecture)
- [🔒 安全性](#security)
- [🧪 测试](#testing)
- [🤝 贡献](#contributing)
- [📋 更新日志](#changelog)
- [📄 许可证](#license)
- [🙏 致谢](#acknowledgments)

</details>

---

## <span id="features">✨ 功能特性</span>

<div align="center" style="margin: 24px 0;">

| 🎯 核心功能 | ⚡ 企业级功能 |
|:----------:|:----------:|
| 始终可用 | 按需启用 |

</div>

<table style="width:100%; border-collapse: collapse;">
<tr>
<td width="50%" style="vertical-align:top; padding: 16px; border-radius:8px; border:1px solid #E2E8F0;">

### 🎯 核心功能（始终可用）

| 状态 | 功能 | 描述 |
|:----:|------|------|
| ✅ | **连接池管理** | RAII 风格的自动连接生命周期管理 |
| ✅ | **权限控制** | 基于角色的表级访问控制（RBAC） |
| ✅ | **过程宏** | 自动生成 CRUD 方法和权限检查 |
| ✅ | **SQL 解析器** | 提取操作类型和目标表 |
| ✅ | **事务支持** | 完整的事务管理 |
| ✅ | **多数据库支持** | SQLite、PostgreSQL、MySQL、DuckDB、Ladybug、Neo4j |

</td>
<td width="50%" style="vertical-align:top; padding: 16px; border-radius:8px; border:1px solid #E2E8F0;">

### ⚡ 企业级功能

| 状态 | 功能 | 描述 |
|:----:|------|------|
| 🔍 | **指标监控** | Prometheus 指标导出（`metrics` 特性） |
| 📊 | **分布式追踪** | OpenTelemetry 集成（`tracing` 特性） |
| 📝 | **审计日志** | 所有操作的自动审计（`audit` 特性） |
| 🗄️ | **数据库迁移** | 自动迁移执行（`migration` 特性） |
| 🔀 | **数据分片** | 支持分片策略（`sharding` 特性） |
| 🌐 | **全局索引** | 跨分片查询（`global-index` 特性） |
| 💾 | **缓存** | oxcache 缓存（内部 moka L1 后端）（`cache` 特性） |
| 🔐 | **权限引擎** | 高级权限系统（`permission-engine` 特性） |
| 🛡️ | **JWT 认证** | JWT + 密码强度验证（`authentication` 特性，0.3.0 新增） |
| 🌍 | **国际化** | ICU4X locale 感知格式化（核心特性，始终可用） |

</td>
</tr>
</table>

### 📦 特性预设

| 预设 | 特性 | 使用场景 |
|------|------|----------|
| <span style="color:#166534; padding:4px 8px; border-radius:4px;">embedded</span> | `runtime-tokio-rustls`, `sqlite`, `config-env` | 嵌入式/边缘设备超最小配置 |
| <span style="color:#1E40AF; padding:4px 8px; border-radius:4px;">microservice</span> | `runtime-tokio-rustls`, `postgres`, `permission`, `sql-parser`, `config-env`, `observability` | 微服务部署 |
| <span style="color:#7C3AED; padding:4px 8px; border-radius:4px;">monolith</span> | `runtime-tokio-rustls`, `postgres`, `permission`, `sql-parser`, `yaml`, `data-management`, `security`, `observability` | 单体应用 |
| <span style="color:#991B1B; padding:4px 8px; border-radius:4px;">enterprise</span> | `postgres`, `monolith`, `permission-engine` | 完整企业功能 |
| <span style="color:#64748B; padding:4px 8px; border-radius:4px;">all-optional</span> | `cache`, `observability`, `data-management`, `security`, `migration` | 5 个聚合 feature（手动添加数据库驱动和其他特性） |

---

## <span id="quick-start">🚀 快速开始</span>

### <span id="installation">📦 安装</span>

在你的 `Cargo.toml` 中添加：

```toml
[dependencies]
dbnexus = { version = "0.4", default-features = false, features = ["runtime-tokio-rustls", "sqlite", "permission", "sql-parser", "macros", "config-env"] }
tokio = { version = "1.52", features = ["rt-multi-thread", "macros"] }
sea-orm = { version = "2.0.0-rc.42", features = ["macros"] }
```

### <span id="basic-usage">💡 基本用法</span>

<div align="center" style="margin: 24px 0;">

#### 🎬 5 分钟快速开始

</div>

<table style="width:100%; border-collapse: collapse;">
<tr>
<td width="50%" style="padding: 16px; vertical-align:top;">

**步骤 1：定义实体**

```rust
use dbnexus::{DbPool, db_entity};
use sea_orm::entity::prelude::*;

#[db_entity(table_name = "users", primary_key = "id")]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub email: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

</td>
<td width="50%" style="padding: 16px; vertical-align:top;">

**步骤 2：创建连接池**

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("sqlite::memory:").await?;
    let session = pool.get_session("admin").await?;
    Ok(())
}
```

</td>
</tr>
<tr>
<td width="50%" style="padding: 16px; vertical-align:top;">

**步骤 3：插入数据**

```rust
let user = Model {
    id: 1,
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
};
Model::insert(&session, user).await?;
```

</td>
<td width="50%" style="padding: 16px; vertical-align:top;">

**步骤 4：查询数据**

```rust
let users = Model::find_all(&session).await?;
println!("找到 {} 个用户", users.len());
```

</td>
</tr>
</table>

### <span id="permission-control">🔒 权限控制</span>

```rust
use dbnexus::{DbPool, db_entity};
use sea_orm::entity::prelude::*;

#[db_entity(table_name = "users", primary_key = "id")]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// 管理员可以访问
let session = pool.get_session("admin").await?;
Model::find_all(&session).await?;

// 普通用户会被拒绝
let session = pool.get_session("guest").await?;
Model::find_all(&session).await?; // 错误：权限被拒绝
```

---

## <span id="feature-flags">🎨 特性标志</span>

### ⚠️ v0.2.0 破坏性变更

**所有用户必须更新 Cargo.toml：**

**v0.1.x → v0.2.0 是破坏性变更。** `cache` feature 不再默认启用，多个 feature 现在显式要求 `cache` 必须启用。

## ⚠️ v0.4.0 破坏性变更

- `default` feature 现在为空数组 `[]`（之前包含 7 个特性）
- 用户必须显式启用 runtime + 数据库驱动 + 所需功能特性
- 推荐使用 `default-no-db` 聚合特性 + 显式数据库驱动，例如：
  `default-features = false, features = ["default-no-db", "sqlite"]`

### 数据库驱动（选择一个）

```toml
# SQLite（嵌入式）
dbnexus = { version = "0.4", default-features = false, features = ["runtime-tokio-rustls", "sqlite"] }

# PostgreSQL
dbnexus = { version = "0.4", features = ["postgres"] }

# MySQL
dbnexus = { version = "0.4", features = ["mysql"] }

# DuckDB（嵌入式分析型数据库，0.3.0 新增）
dbnexus = { version = "0.4", features = ["duckdb"] }

# Ladybug（嵌入式图数据库，0.4.0 新增）
dbnexus = { version = "0.4", features = ["ladybug"] }

# Neo4j（图数据库服务器，0.4.0 新增）
dbnexus = { version = "0.4", features = ["neo4j"] }
```

### 协议兼容数据库

DBNexus 通过标准协议支持以下兼容数据库（无需额外特性，使用对应协议驱动即可）：

| 数据库 | 兼容协议 | 说明 |
|--------|----------|------|
| CockroachDB | PostgreSQL | 分布式 SQL 数据库 |
| YugabyteDB | PostgreSQL | 分布式 PostgreSQL |
| TiDB | MySQL | 分布式 HTAP 数据库 |
| MariaDB | MySQL | MySQL 兼容分支 |
| Aurora | PostgreSQL/MySQL | AWS 云原生数据库 |

### 运行时

```toml
# Tokio with RustLS（默认）
dbnexus = { version = "0.4", features = ["runtime-tokio-rustls"] }

# Tokio with Native TLS
dbnexus = { version = "0.4", features = ["runtime-tokio-native-tls"] }

# AsyncStd
dbnexus = { version = "0.4", features = ["runtime-async-std"] }
```

### 可选功能

```toml
# 核心功能
# 权限控制（自动启用 sql-parser + yaml + cache 特性，强制依赖 sql-parser 防 SQL 注入）
dbnexus = { version = "0.4", features = ["permission"] }

# SQL 解析（自动启用 cache 特性）
dbnexus = { version = "0.4", features = ["sql-parser"] }

# 过程宏
dbnexus = { version = "0.4", features = ["macros"] }

# 企业级功能
dbnexus = { version = "0.4", features = [
    "metrics",          # Prometheus 指标
    "tracing",          # 分布式追踪
    "audit",            # 审计日志
    "migration",        # 数据库迁移
    "sharding",         # 数据分片
    "global-index",     # 跨分片全局索引
    "permission-engine", # 高级权限引擎
    "authentication"   # JWT 认证 + 密码强度验证（0.3.0 新增）
    # i18n 已为核心特性，始终可用，无需显式启用
] }

# 配置
dbnexus = { version = "0.4", features = [
    "yaml",            # YAML 配置支持
    "config-toml",     # TOML 配置支持
    "config-env",      # 环境变量（默认）
] }
```

---

## <span id="documentation">📚 文档</span>

<div align="center" style="margin: 24px 0;">

<table style="width:100%; max-width: 800px;">
<tr>
<td align="center" width="33%" style="padding: 16px;">
<a href="docs/USER_GUIDE.md" style="text-decoration:none;">
<div style="padding: 24px; border-radius:12px; transition: transform 0.2s;">
<b style="color:#1E293B;">📖 用户指南</b>
</div>
</a>
<br><span style="color:#64748B;">完整使用指南</span>
</td>
<td align="center" width="33%" style="padding: 16px;">
<a href="https://docs.rs/dbnexus" style="text-decoration:none;">
<div style="padding: 24px; border-radius:12px; transition: transform 0.2s;">
<b style="color:#1E293B;">📘 API 参考</b>
</div>
</a>
<br><span style="color:#64748B;">完整 API 文档</span>
</td>
<td align="center" width="33%" style="padding: 16px;">
<a href="examples/" style="text-decoration:none;">
<div style="padding: 24px; border-radius:12px; transition: transform 0.2s;">
<b style="color:#1E293B;">💻 示例代码</b>
</div>
</a>
<br><span style="color:#64748B;">代码示例</span>
</td>
</tr>
</table>

</div>

### 📖 补充资源

| 资源 | 描述 |
|------|------|
| 📖 [用户指南](docs/USER_GUIDE.md) | 使用 DBNexus 的全面指南 |
| 📘 [API 参考](docs/API_REFERENCE.md) | 完整 API 文档 |
| 🏗️ [架构文档](docs/ARCHITECTURE.md) | 系统架构和设计决策 |
| 📦 [示例](examples/) | 可运行的代码示例 |

---

## <span id="examples">💻 示例</span>

<div align="center" style="margin: 24px 0;">

### 💡 真实示例

</div>

<table style="width:100%; border-collapse: collapse;">
<tr>
<td width="50%" style="padding: 16px; border-radius:8px; border:1px solid #E2E8F0; vertical-align:top;">

#### 📝 高级配置

```rust
use dbnexus::{DbPool, DbConfig, PoolConfig};

let config = DbConfig {
    url: "postgresql://user:pass@localhost/db".to_string(),
    pool_config: PoolConfig {
        max_connections: 20,
        min_connections: 5,
        idle_timeout: 300,
        acquire_timeout: 5000,
    },
    ..Default::default()
};

let pool = DbPool::with_config(config).await?;
```

</td>
<td width="50%" style="padding: 16px; border-radius:8px; border:1px solid #E2E8F0; vertical-align:top;">

#### 🔧 环境变量

```bash
export DATABASE_URL="postgresql://user:pass@localhost/db"
export DB_MAX_CONNECTIONS=20
export DB_MIN_CONNECTIONS=5
export DB_ADMIN_ROLE=admin
```

```rust
let config = dbnexus::DbConfig::from_env()?;
let pool = dbnexus::DbPool::with_config(config).await?;
```

</td>
</tr>
<tr>
<td width="50%" style="padding: 16px; border-radius:8px; border:1px solid #E2E8F0; vertical-align:top;">

#### 🔄 事务处理

```rust
let session = pool.get_session("admin").await?;

// 开始事务
session.begin_transaction().await?;

// 多个操作
Model::insert(&session, user1).await?;
Model::insert(&session, user2).await?;

// 提交
session.commit().await?;
```

</td>
<td width="50%" style="padding: 16px; border-radius:8px; border:1px solid #E2E8F0; vertical-align:top;">

#### 📊 监控

```rust
use dbnexus::{DbPool, MetricsCollector};

let pool = DbPool::new("postgresql://localhost/db").await?;

// 获取连接池状态
let status = pool.status();
println!("活跃: {}, 空闲: {}", status.active, status.idle);

// 导出 Prometheus 指标
let metrics = MetricsCollector::new();
println!("{}", metrics.export_prometheus());
```

</td>
</tr>
</table>

<div align="center" style="margin: 24px 0;">

**[📂 查看所有示例 →](examples/)**

</div>

> **注意**：`dbnexus-examples` 已设为 `publish = false` 并纳入 workspace 管理。

---

## <span id="architecture">🏗️ 架构</span>

<div align="center" style="margin: 24px 0;">

### 🏗️ 系统架构

</div>

<div align="center">

</div>

```mermaid
graph TD
    A[应用层<br/>使用 DbPool 和 Session 的代码] --> B[DBNexus API 层<br/>DbPool, Session<br/>权限检查<br/>事务管理]
    B --> C[功能模块<br/>Config, Permission, Metrics<br/>Migration, Sharding, Audit]
    C --> D[连接池层<br/>连接生命周期管理<br/>健康检查<br/>RAII 保证]
    D --> E[Sea-ORM / SQLx<br/>数据库驱动<br/>查询构建器]
```

查看 [ARCHITECTURE.md](docs/ARCHITECTURE.md) 获取详细的架构文档。

---

## <span id="security">🔒 安全性</span>

<div align="center" style="margin: 24px 0;">

### 🛡️ 安全特性

</div>

DBNexus 在设计时就考虑了安全性：

- **无 unsafe 代码** - 所有库代码都使用 `#![forbid(unsafe_code)]`
- **权限强制执行** - 具有编译时验证的表级访问控制
- **SQL 注入防护** - 默认使用参数化查询
- **配置路径验证** - 防止路径遍历攻击
- **速率限制** - 权限检查速率限制以防止滥用

---

## <span id="testing">🧪 测试</span>

<div align="center" style="margin: 24px 0;">

### 🎯 运行测试

</div>

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

---

## <span id="contributing">🤝 贡献</span>

<div align="center" style="margin: 24px 0;">

欢迎贡献！请查看 [CONTRIBUTING.md](docs/CONTRIBUTING.md) 获取详细的贡献指南。

</div>

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

---

## <span id="changelog">📋 更新日志</span>

详见 [CHANGELOG.md](CHANGELOG.md)。

---

## <span id="license">📄 许可证</span>

<div align="center" style="margin: 24px 0;">

本项目采用 **MIT** 许可证：

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

</div>

---

## <span id="acknowledgments">🙏 致谢</span>

<div align="center" style="margin: 24px 0;">

### 🌟 基于优秀工具构建

</div>

- [Sea-ORM](https://www.sea-ql.org/SeaORM/) - 优秀的 ORM 框架，DBNexus 建立在其上
- [SQLx](https://github.com/launchbadge/sqlx) - 异步 SQL 工具包
- Rust 社区提供的优秀工具和库

---

## 📞 支持

<div align="center" style="margin: 24px 0;">

<table style="width:100%; max-width: 600px;">
<tr>
<td align="center" width="33%">
<a href="https://github.com/Kirky-X/dbnexus/issues">
<div style="padding: 16px; border-radius:8px;">
<b style="color:#991B1B;">📋 Issues</b>
</div>
</a>
<br><span style="color:#64748B;">报告 Bug 和问题</span>
</td>
<td align="center" width="33%">
<a href="https://github.com/Kirky-X/dbnexus/discussions">
<div style="padding: 16px; border-radius:8px;">
<b style="color:#1E40AF;">💬 Discussions</b>
</div>
</a>
<br><span style="color:#64748B;">提问和分享想法</span>
</td>
<td align="center" width="33%">
<a href="https://github.com/Kirky-X/dbnexus">
<div style="padding: 16px; border-radius:8px;">
<b style="color:#1E293B;">🐙 GitHub</b>
</div>
</a>
<br><span style="color:#64748B;">查看源代码</span>
</td>
</tr>
</table>

</div>

---

## ⭐ Star 历史

<div align="center">

[![Star History Chart](https://api.star-history.com/svg?repos=Kirky-X/dbnexus&type=Date)](https://star-history.com/#Kirky-X/dbnexus&Date)

</div>

---

<div align="center" style="margin: 32px 0; padding: 24px; border-radius: 12px;">

### 💝 支持本项目

如果您觉得这个项目有用，请考虑给它一个 ⭐️！

**由 Kirky.X 用 ❤️ 构建**

---

**[⬆ 返回顶部](#dbnexus)**

---

<sub>© 2026 Kirky.X. All rights reserved.</sub>

</div>
