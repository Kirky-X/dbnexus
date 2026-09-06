<div align="center">

<img src="docs/assets/dbnexus.png" alt="DBNexus Logo" width="200">

[![CI Status](https://github.com/Kirky-X/dbnexus/actions/workflows/ci.yml/badge.svg)](https://github.com/Kirky-X/dbnexus/actions/workflows/ci.yml) [![Version](https://img.shields.io/crates/v/dbnexus.svg)](https://crates.io/crates/dbnexus) [![Docs.rs](https://docs.rs/dbnexus/badge.svg)](https://docs.rs/dbnexus) [![Downloads](https://img.shields.io/crates/d/dbnexus.svg)](https://crates.io/crates/dbnexus) [![License](https://img.shields.io/crates/l/dbnexus.svg)](LICENSE) [![Rust](https://img.shields.io/badge/rust-1.97.1%2B-orange.svg)](https://www.rust-lang.org/) [![Coverage](https://codecov.io/gh/Kirky-X/dbnexus/branch/main/graph/badge.svg)](https://codecov.io/gh/Kirky-X/dbnexus)

**中文** | [English](README_EN.md)

**企业级 Rust 数据库抽象层**

[✨ 功能特性](#-功能特性) • [🚀 快速开始](#-快速开始) • [📚 文档](#-文档) • [💻 示例](#-示例) • [🤝 参与贡献](#-参与贡献)

</div>

---

## 📋 目录

<details open>
<summary>📑 目录（点击展开）</summary>

- [✨ 功能特性](#-功能特性)
- [🚀 快速开始](#-快速开始)
  - [📦 安装](#-安装)
  - [💡 基本用法](#-基本用法)
  - [🔒 权限控制](#-权限控制)
- [🎨 特性标志](#-特性标志)
- [📚 文档](#-文档)
- [💻 示例](#-示例)
- [🏗️ 架构](#️-架构)
- [🧪 测试](#-测试)
- [📊 性能](#-性能)
- [🔒 安全](#-安全)
- [🗺️ 开发路线图](#️-开发路线图)
- [🤝 参与贡献](#-参与贡献)
- [📋 更新日志](#-更新日志)
- [📄 许可证](#-许可证)
- [🙏 致谢](#-致谢)
- [📞 联系与支持](#-联系与支持)
- [⭐ Star 历史](#-star-历史)

</details>

---

## ✨ 功能特性

基于 Sea-ORM 构建的高性能、高安全性、功能丰富的数据库访问层。DBNexus 提供一种**声明式**的数据库访问方法：

| ✨ 类型安全 | 🔒 权限控制 | 🏊 智能连接池 | 📊 企业级监控 |
|:---------:|:----------:|:--------------:|:--------:|
| 编译时检查 | 表级 RBAC | RAII 自动管理 | Prometheus 指标 |

### 🎯 核心功能（始终可用）

| 状态 | 功能 | 描述 |
|:----:|------|------|
| ✅ | **连接池管理** | RAII 风格的自动连接生命周期管理 |
| ✅ | **权限控制** | 基于角色的表级访问控制（RBAC） |
| ✅ | **过程宏** | 自动生成 CRUD 方法和权限检查 |
| ✅ | **SQL 解析器** | 提取操作类型和目标表 |
| ✅ | **事务支持** | 完整的事务管理 |
| ✅ | **多数据库支持** | SQLite、PostgreSQL、MySQL、DuckDB、Ladybug、Neo4j |

### ⚡ 企业级功能（按需启用）

| 状态 | 功能 | 描述 |
|:----:|------|------|
| 🔍 | **指标监控** | Prometheus 指标导出（`metrics` 特性） |
| 📝 | **审计日志** | 所有操作的自动审计（`audit` 特性） |
| 🗄️ | **数据库迁移** | 自动迁移执行（`migration` 特性） |
| 🔀 | **数据分片** | 支持分片策略（`sharding` 特性） |
| 🌐 | **全局索引** | 跨分片查询（`global-index` 特性） |
| 💾 | **缓存** | oxcache 缓存（内部 moka L1 后端）（`cache` 特性） |
| 🩺 | **权限健康检查** | 内存 provider 校验策略表容量、YAML provider 校验策略文件可读（`permission` 特性） |
| 🔐 | **权限引擎** | 高级权限系统（`permission-engine` 特性） |
| 🛡️ | **JWT 认证** | JWT + 密码强度验证（`authentication` 特性） |
| 🌍 | **国际化** | ICU4X locale 感知格式化（核心特性，始终可用） |
| 🔁 | **重试机制** | 指数退避 + 幂等判断（`retry` 特性） |
| 🔄 | **故障转移** | CircuitBreaker 状态机（`failover` 特性） |
| 🌐 | **副本路由** | 读写分离（`replica-routing` 特性） |
| 📡 | **Scatter-Gather** | 跨分片聚合查询（`scatter-gather` 特性） |
| 🧩 | **Saga 事务** | 分布式事务编排（`saga` 特性） |
| 🔢 | **分布式 ID** | Snowflake ID 生成（`distributed-id` 特性） |

### 📦 特性预设

| 预设 | 特性 | 使用场景 |
|------|------|----------|
| `embedded` | `runtime-tokio-rustls`, `sqlite`, `config-env` | 嵌入式/边缘设备超最小配置 |
| `microservice` | `runtime-tokio-rustls`, `postgres`, `permission`, `sql-parser`, `config-env`, `observability` | 微服务部署 |
| `monolith` | `runtime-tokio-rustls`, `postgres`, `permission`, `sql-parser`, `yaml`, `data-management`, `security`, `observability`, `distributed-capabilities` | 单体应用（含全部 7 项分布式能力） |
| `enterprise` | `postgres`, `monolith`, `permission-engine` | 完整企业功能 |
| `all-optional` | `cache`, `observability`, `data-management`, `security`, `migration`, `retry`, `failover`, `replica-routing`, `scatter-gather`, `shard-migration`, `saga`, `distributed-id` | 12 个独立 feature（手动添加数据库驱动和其他特性） |

---

## 🚀 快速开始

### 📦 安装

在你的 `Cargo.toml` 中添加：

```toml
[dependencies]
dbnexus = { version = "0.6.0-rc.2", default-features = false, features = ["runtime-tokio-rustls", "sqlite", "permission", "sql-parser", "macros", "config-env"] }
tokio = { version = "1.52", features = ["rt-multi-thread", "macros"] }
sea-orm = { version = "2.0.0-rc.42", features = ["macros"] }
```

### 💡 基本用法

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

**步骤 2：创建连接池**

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::new("sqlite::memory:").await?;
    let session = pool.get_session("admin").await?;
    Ok(())
}
```

**步骤 3：插入数据**

```rust
let user = Model {
    id: 1,
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
};
Model::insert(&session, user).await?;
```

**步骤 4：查询数据**

```rust
let users = Model::find_all(&session).await?;
println!("找到 {} 个用户", users.len());
```

<details>
<summary>🎬 完整示例（可直接运行）</summary>

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

</details>

### 🔒 权限控制

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

## 🎨 特性标志

### 数据库驱动（选择一个）

```toml
# SQLite（嵌入式）
dbnexus = { version = "0.6.0-rc.2", default-features = false, features = ["runtime-tokio-rustls", "sqlite"] }

# PostgreSQL
dbnexus = { version = "0.6.0-rc.2", features = ["postgres"] }

# MySQL
dbnexus = { version = "0.6.0-rc.2", features = ["mysql"] }

# DuckDB（嵌入式分析型数据库，0.3.0 新增）
dbnexus = { version = "0.6.0-rc.2", features = ["duckdb"] }

# Ladybug（嵌入式图数据库，0.4.0 新增）
dbnexus = { version = "0.6.0-rc.2", features = ["ladybug"] }

# Neo4j（图数据库服务器，0.4.0 新增）
dbnexus = { version = "0.6.0-rc.2", features = ["neo4j"] }
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
dbnexus = { version = "0.6.0-rc.2", features = ["runtime-tokio-rustls"] }

# Tokio with Native TLS
dbnexus = { version = "0.6.0-rc.2", features = ["runtime-tokio-native-tls"] }

# AsyncStd
dbnexus = { version = "0.6.0-rc.2", features = ["runtime-async-std"] }
```

### 核心功能

```toml
# 权限控制（自动启用 sql-parser + yaml + cache 特性，强制依赖 sql-parser 防 SQL 注入）
dbnexus = { version = "0.6.0-rc.2", features = ["permission"] }

# SQL 解析（自动启用 cache 特性）
dbnexus = { version = "0.6.0-rc.2", features = ["sql-parser"] }

# 过程宏
dbnexus = { version = "0.6.0-rc.2", features = ["macros"] }
```

### 使用预设（推荐）

```toml
# 嵌入式/边缘设备（最小配置）
dbnexus = { version = "0.6.0-rc.2", features = ["embedded"] }

# 微服务
dbnexus = { version = "0.6.0-rc.2", features = ["microservice"] }

# 单体应用
dbnexus = { version = "0.6.0-rc.2", features = ["monolith"] }

# 企业级（完整功能）
dbnexus = { version = "0.6.0-rc.2", features = ["enterprise"] }
```

### 可选功能

```toml
# 可观测性（metrics + health-check）
dbnexus = { version = "0.6.0-rc.2", features = ["observability"] }

# 数据管理（migration + sharding + global-index）
dbnexus = { version = "0.6.0-rc.2", features = ["data-management"] }

# 安全（audit + permission-engine）
dbnexus = { version = "0.6.0-rc.2", features = ["security"] }

# 独立特性
dbnexus = { version = "0.6.0-rc.2", features = [
    "metrics",          # Prometheus 指标
    "audit",            # 审计日志
    "migration",        # 数据库迁移
    "sharding",         # 数据分片
    "global-index",     # 跨分片全局索引
    "permission-engine", # 高级权限引擎
    "authentication",   # JWT 认证 + 密码强度验证
    "distributed-capabilities" # 分布式能力聚合（retry/failover/replica-routing/scatter-gather/shard-migration/saga/distributed-id）
    # i18n 已为核心特性，始终可用，无需显式启用
] }
```

### 配置

```toml
dbnexus = { version = "0.6.0-rc.2", features = [
    "yaml",            # YAML 配置支持
    "config-toml",     # TOML 配置支持
    "config-env",      # 环境变量（默认）
] }
```

---

## 📚 文档

| 文档 | 说明 |
|------|------|
| [📖 用户指南](docs/USER_GUIDE.md) | 从安装到进阶的完整使用教程 |
| [📘 API 参考](docs/API_REFERENCE.md) | 全部公开 API 的详细说明 |
| [🏗️ 架构文档](docs/ARCHITECTURE.md) | 设计理念与内部实现 |
| [🔒 安全文档](docs/SECURITY.md) | 安全设计与最佳实践 |
| [📋 更新日志](docs/CHANGELOG.md) | 每个版本的变更记录 |
| [🤝 贡献指南](docs/CONTRIBUTING.md) | 如何参与项目开发 |
| [📦 在线 API 文档](https://docs.rs/dbnexus) | docs.rs 自动生成的最新文档 |

---

## 💻 示例

全部示例位于 [examples/](examples/)（独立 crate `dbnexus-examples`，随 workspace 管理）：

```bash
cd examples

# 运行单个示例（每个示例是一个 bin 目标）
cargo run --bin basic_connection

# 编译全部示例
cargo build --all-targets
```

| 模块 | 示例 | 说明 |
|------|------|------|
| 基础 | `basic_connection`、`basic_crud`、`basic_transaction` | 连接池 / `#[db_entity]` CRUD / 事务 |
| 配置 | `config_env`、`config_yaml`、`config_toml`、`config_presets` | 环境变量 / YAML / TOML / 预设对比 |
| 数据库 | `database_sqlite`、`database_postgres`、`database_mysql`、`duckdb_query`、`migration`、`sharding`、`global_index`、`pool_management` | 驱动连接 / OLAP 查询 / 迁移 / 分片 / 全局索引 / 池管理 |
| 权限 | `permission_rbac`、`permission_yaml`、`permission_macro`、`permission_engine` | RBAC / YAML 策略 / 宏权限 / 权限引擎 |
| 安全 | `sql_parser`、`sql_injection_detection`、`ddl_guard`、`sensitive_masker`、`rate_limiter` | SQL 解析 / 注入检测 / DDL 守卫 / 脱敏 / 限流 |
| 认证与审计 | `authentication_jwt`、`authentication_password`、`audit_logging` | JWT / 密码哈希 / 审计日志 |
| 可观测性 | `metrics_prometheus`、`health_check`、`latency_histogram` | 指标 / 健康检查与熔断 / 延迟直方图 |
| 宏 | `macros_db_entity`、`macros_db_crud`、`macros_db_audit`、`macros_db_cache`、`macros_soft_delete_unique`、`macros_db_entity_v2`、`macros_advanced_query` | 宏全量能力（CRUD/审计/缓存/软删除/hooks/分页） |
| 图数据库 | `graph_ladybug`*、`graph_neo4j` | Ladybug 嵌入式图 DB / Neo4j 服务器（*`graph_ladybug` 因与 `duckdb` 存在 mbedtls 链接冲突未注册为 bin，需单独编译：`cargo build --bin graph_ladybug --no-default-features --features "runtime-tokio-rustls,sqlite,cache,ladybug"`） |
| 分布式能力 | `distributed_id`、`saga`、`scatter_gather`、`replica_routing`、`shard_migration` | Snowflake ID / Saga / 跨分片聚合 / 读写分离 / 分片迁移 |
| 可靠性 | `retry`、`failover` | 重试退避 / 熔断故障转移 |
| 国际化 | `i18n_formatting` | ICU4X locale 格式化 |
| 集成与缓存 | `oxcache_adapter`、`cache_standalone` | oxcache 适配 / 自定义缓存 Provider |
| Kit | `kit_usage`、`kit_advanced` | 能力注册 / 多能力组合 |
| 通用 | `error_handling` | 结构化错误报告 |

完整说明见 [examples/README.md](examples/README.md)。

> **注意**：`dbnexus-examples` 已设为 `publish = false` 并纳入 workspace 管理。

### 📝 代码片段

#### 高级配置

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

#### 环境变量

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

#### 事务处理

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

#### 监控

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

---

## 🏗️ 架构

```mermaid
graph TD
    A[应用层<br/>使用 DbPool 和 Session 的代码] --> B[DBNexus API 层<br/>DbPool, Session<br/>权限检查<br/>事务管理]
    B --> C[功能模块<br/>Config, Permission, Metrics<br/>Migration, Sharding, Audit]
    C --> D[连接池层<br/>连接生命周期管理<br/>健康检查<br/>RAII 保证]
    D --> E[Sea-ORM / SQLx<br/>数据库驱动<br/>查询构建器]
```

详细的设计理念、模块划分、数据流与安全/性能设计见 [架构文档](docs/ARCHITECTURE.md)。

---

## 🧪 测试

```bash
# 全量测试（CI 标准特性组合；sqlite/postgres/mysql/duckdb 驱动互斥，不使用 --all-features）
cargo test --no-default-features --features sqlite,default-no-db,all-optional --workspace --exclude dbnexus-examples --exclude dbnexus-macros

# 切换数据库后端运行集成测试（PostgreSQL/MySQL 需要 Docker 环境）
cargo test --no-default-features --features postgres,default-no-db,all-optional --workspace --exclude dbnexus-examples --exclude dbnexus-macros
```

> 嵌入式（`sqlite`/`duckdb`）与服务器端（`postgres`/`mysql`）驱动在编译期严格互斥（`compile_error!`），验证多驱动时按分组特性组合运行。

---

## 📊 性能

DBNexus 遵循零成本抽象原则，性能相关能力均为设计层面保证：

- **零成本特性门控**：`metrics` 等可选功能通过 `#[cfg(feature = ...)]` 编译期裁剪，未启用时为零开销空实现
- **无锁计数**：连接池状态（`PoolStatus`）使用原子类型维护，热路径无锁
- **异步优先**：全部 I/O 使用 `async/await`；读多写少状态使用 `RwLock`；原子操作采用 `AcqRel` 内存序，减少不必要的全局同步
- **连接池策略**：池 + LRU 复用连接（避免握手开销）、限制最大连接、预热最小连接、健康检查剔除死连接
- **热路径优化**（0.5.1）：SQL 注入检测模式表静态化、Saga 补偿查找预索引、`Session` 并发读优化、`DbConfig` Arc 共享等

### 基准测试

仓库内置 4 个 criterion 基准：`permission_bench`、`permission_engine_bench`、`sharding_bench`、`metrics_bench`（位于 [benches/](benches/)）：

```bash
cargo bench
```

以下为 2026-08-14 在 Linux x86_64（release profile，lto=thin）上两轮平台优化的累计结果（完整报告见 [benches/baseline-after.md](benches/baseline-after.md)）：

| 基准项 | 原始基线 | 优化后 | 累计变化 |
|--------|----------|--------|---------|
| shard_id_for_key | 2.9725 – 3.0496 µs | 2.8871 – 2.9188 µs | -3.9% |
| enforce_shard_binding_conflict | 5.8179 – 5.9490 µs | 5.4019 – 5.5934 µs | -5.5% |
| prometheus_export | 4.2057 – 4.3566 µs | 4.0485 – 4.1789 µs | -3.1% |
| histogram_record | 882.21 – 888.70 ns | 796.62 – 804.15 ns | -9.4% |
| permission_cache_hit | 8.8730 – 8.9906 µs | 8.1415 – 8.1825 µs | -8.3% |
| permission_cache_miss | 3.6778 – 3.7570 µs | 3.5827 – 3.6179 µs | -2.8% |

6 项基准全部正向提升、无回退，平均累计提升约 5.5%。

---

## 🔒 安全

DBNexus 从设计之初就以内建安全为目标：

- **无 unsafe 代码** — 所有库代码使用 `#![forbid(unsafe_code)]`
- **权限强制执行** — 基于角色的表级访问控制（RBAC），覆盖 JOIN/子查询跨表路径
- **SQL 注入防护** — 默认参数化查询，`SqlParser` 表名提取与注入检测，`DdlGuard` AST 校验
- **配置路径校验** — 防止路径遍历攻击；连接 URL 解析错误不回显凭据
- **速率限制** — 权限检查令牌桶限流，防止滥用

完整的纵深防御设计、漏洞报告流程与安全最佳实践见 [安全文档](docs/SECURITY.md)。

---

## 🗺️ 开发路线图

### 短期（0.6.0 正式发布）

- [ ] 发布 0.6.0 正式版：完成 rc 验证后按工作区传导表更新 trait-kit 0.5.0 / oxcache 0.5.0 依赖要求，`cargo publish --dry-run` 核对后打 tag 触发 release.yml 自动发布
- [x] 对齐 MSRV 声明与依赖实际要求 — 已按工作区 CONFIG_BASELINE 统一为 1.97.1（2026-09-06），覆盖传递依赖的 1.94 要求
- [ ] 恢复 MySQL 集成测试的常规运行（testcontainers 已就绪，当前受数据库服务依赖阻塞）

### 中期

- [ ] 解决 `duckdb` 与 `ladybug` 同时启用时的 mbedtls 重复符号链接冲突（当前多驱动验证采用分组特性组合）
- [ ] 跟进代码质量审查留档的 Medium 项

> 条目整理自工作区验收计划与 [CHANGELOG.md](docs/CHANGELOG.md)。

---

## 🤝 参与贡献

欢迎贡献！请先阅读 [CONTRIBUTING.md](docs/CONTRIBUTING.md)，了解 TDD 工作流、代码规范与提交/PR 流程。

### 开发设置

```bash
# 克隆仓库
git clone https://github.com/Kirky-X/dbnexus.git
cd dbnexus

# 安装 pre-commit 钩子
./scripts/install-pre-commit.sh

# 运行测试（CI 特性组合）
cargo test --no-default-features --features sqlite,default-no-db,all-optional

# 运行 linter
cargo clippy --no-default-features --features sqlite,default-no-db,all-optional --all-targets -- -D warnings
```

---

## 📋 更新日志

完整版本历史见 [CHANGELOG.md](docs/CHANGELOG.md)。

| 版本 | 日期 | 要点 |
|------|------|------|
| 0.6.0-rc.2 | 2026-09-03 | 移除 `tracing` 特性；四驱动互斥严格化（编译期 `compile_error!`）；补全 JOIN/子查询跨表权限检查；`h2` 安全升级 |
| 0.5.1 | 2026-08-06 | 热路径性能优化（注入检测静态表、Session 读写锁化、`DbConfig` Arc 共享等）；清理弃用 builder 方法 |
| 0.5.0 | 2026-08-04 | 补全 7 个分布式能力示例（`saga`/`scatter_gather`/`replica_routing` 等）；API 与架构文档同步 |

---

## 📄 许可证

本项目基于 MIT + Commons Clause 许可证发布，商业使用需单独授权。详见 [LICENSE](LICENSE)。

---

## 🙏 致谢

- [Sea-ORM](https://www.sea-ql.org/SeaORM/) - 优秀的 ORM 框架，DBNexus 建立在其上
- [SQLx](https://github.com/launchbadge/sqlx) - 异步 SQL 工具包
- Rust 社区提供的优秀工具和库

---

## 📞 联系与支持

| 渠道 | 用途 |
|------|------|
| [📋 Issues](https://github.com/Kirky-X/dbnexus/issues) | 报告 Bug 和问题 |
| [💬 Discussions](https://github.com/Kirky-X/dbnexus/discussions) | 提问和分享想法 |
| [🐙 GitHub](https://github.com/Kirky-X/dbnexus) | 查看源代码 |

安全漏洞请勿通过公开 Issue 提交，参见[安全文档](docs/SECURITY.md)的漏洞报告流程。

---

## ⭐ Star 历史

[![Star History Chart](https://api.star-history.com/svg?repos=Kirky-X/dbnexus&type=Date)](https://star-history.com/#Kirky-X/dbnexus&Date)

### 💝 支持本项目

如果您觉得这个项目有用，请考虑给它一个 ⭐️！

---

<div align="center">

**由 Kirky.X 用 ❤️ 构建**

[⬆ 返回顶部](#readme)

<sub>© 2026 Kirky.X. All rights reserved.</sub>

</div>
