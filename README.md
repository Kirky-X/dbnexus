<div align="center">

<img src="docs/assets/dbnexus.png" alt="DBNexus Logo" width="180">

[![CI Status](https://github.com/Kirky-X/dbnexus/actions/workflows/ci.yml/badge.svg)](https://github.com/Kirky-X/dbnexus/actions/workflows/ci.yml) [![Version](https://img.shields.io/crates/v/dbnexus.svg)](https://crates.io/crates/dbnexus) [![Docs.rs](https://docs.rs/dbnexus/badge.svg)](https://docs.rs/dbnexus) [![Downloads](https://img.shields.io/crates/d/dbnexus.svg)](https://crates.io/crates/dbnexus) [![License](https://img.shields.io/crates/l/dbnexus.svg)](LICENSE) [![Rust](https://img.shields.io/badge/rust-1.97.1%2B-orange.svg)](https://www.rust-lang.org/) [![Coverage](https://codecov.io/gh/Kirky-X/dbnexus/branch/main/graph/badge.svg)](https://codecov.io/gh/Kirky-X/dbnexus)

**中文** | [English](README_EN.md)

**企业级 Rust 数据库抽象层**

[✨ 功能特性](#-功能特性) • [🚀 快速开始](#-快速开始) • [📚 文档](#-文档) • [💻 示例](#-示例) • [🤝 参与贡献](#-参与贡献)

</div>

---

<div align="center" style="padding: 32px; margin: 24px 0">

### 🗄️ 声明式多数据库访问

通过派生宏定义实体，连接池、权限、审计与缓存由框架内建：

<table style="width:100%; border-collapse: collapse">
<tr><td align="center" width="25%" style="padding: 12px">🛡️<br><b>安全内建</b><br><span style="color:#64748B">SQL 解析 逐表权限 注入防护</span></td><td align="center" width="25%" style="padding: 12px">🧩<br><b>声明式宏</b><br><span style="color:#64748B">派生宏生成实体与仓储样板</span></td><td align="center" width="25%" style="padding: 12px">🌐<br><b>多数据库</b><br><span style="color:#64748B">SQLite PostgreSQL MySQL DuckDB Ladybug Neo4j</span></td><td align="center" width="25%" style="padding: 12px">📊<br><b>可观测可靠</b><br><span style="color:#64748B">连接指标 熔断重试 慢查询追踪</span></td></tr>
</table>

</div>

---

## 📋 目录

<details open>
<summary>📑 目录</summary>

- [✨ 功能特性](#-功能特性)
- [🚀 快速开始](#-快速开始)
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

DBNexus 基于 Sea-ORM 构建，提供一种**声明式**的数据库访问方式：一个宏定义实体，一层权限守住每条 SQL，一组特性按需裁剪。

<div align="center">

<table>
<tr>
<td align="center" width="25%">🔒<br><b>安全内建</b><br>全库禁用 unsafe，表级 RBAC 覆盖 JOIN 与子查询</td>
<td align="center" width="25%">🧩<br><b>声明式宏</b><br><code>#[db_entity]</code> 生成带权限检查的 CRUD 方法</td>
<td align="center" width="25%">🗄️<br><b>多数据库</b><br>SQLite / PostgreSQL / MySQL / DuckDB / Ladybug / Neo4j</td>
<td align="center" width="25%">📊<br><b>可观测可靠</b><br>Prometheus 指标、健康检查、重试与熔断</td>
</tr>
</table>

</div>

### 🎯 核心基座（无可选特性依赖）

| 能力 | 说明 |
|------|------|
| **连接池管理** | RAII 风格连接生命周期；池状态由原子类型维护，热路径无锁 |
| **事务支持** | `begin_transaction` / `commit` / 回滚的完整事务管理，RAII 保证资源释放 |
| **统一错误体系** | `ErrorCode` 错误码表 + `QueryErrorReport` 结构化错误报告（0.6.0-rc.3 统一） |
| **配置管理** | `DbConfig` / `PoolConfig`，环境变量 / YAML / TOML 多配置源 |
| **国际化** | ICU4X + Fluent locale 感知格式化（核心特性，始终编译） |

### ⚙️ 核心可选特性（`default-no-db` 聚合）

| 能力 | 说明 |
|------|------|
| **权限控制**（`permission`） | 基于角色的表级访问控制（RBAC），强制依赖 `sql-parser` 防止注入绕过 |
| **SQL 解析**（`sql-parser`） | 操作类型与表名提取、注入检测，解析结果缓存 |
| **过程宏**（`macros`） | `#[db_entity]` / `#[db_repository]` 生成带权限检查的 CRUD |
| **环境变量配置**（`config-env`） | `DbConfig::from_env` 直接读取环境变量 |

### ⚡ 企业级特性（按需启用）

| 特性 | 说明 |
|------|------|
| `metrics` | Prometheus 格式指标导出，含慢查询检测 |
| `audit` | 审计日志，admin 绕过权限的操作同样记录 |
| `migration` / `auto-migrate` | 数据库迁移与自动迁移执行 |
| `sharding` | 数据分片：一致性哈希策略与会话级分片路由 |
| `global-index` | 跨分片全局索引 |
| `cache` | oxcache 缓存（moka 为 L1 后端），`ArcSwap` 无锁读取 |
| `permission-engine` | 高级权限引擎：策略决策点、角色继承链、缓存与限流 |
| `authentication` | JWT 认证（访问/刷新令牌区分校验）+ bcrypt 密码强度策略 |
| `data-protection` 🆕 | 字段级自动脱敏（mask/哈希/截断）与行级安全谓词注入 |
| `permission-facade` 🆕 | RBAC + 脱敏 + RLS 统一门面，一处配置全局生效 |
| `query-dsl` 🆕 | `q!` 类型安全查询片段宏，标识符与值注入免疫 |
| `repository` / `data-api` 🆕 | 泛型仓储 `Repository<T>`；实体到 JSON 的数据 API 网关 |
| `prepare-cache` 🆕 | 语句级 prepared statement LRU 缓存与命中率指标 |
| `copy` 🆕 | COPY FROM STDIN 批量写入语句构建（pg 协议路径按驱动门控） |
| `entity-events` 🆕 | 实体事件总线 + Outbox 持久化投递 |
| `otel` 🆕 | 健康快照指标导出 OTLP/HTTP（stdout fallback 兜底） |
| `kit` | trait-kit AsyncKit 集成，注册即获得池/缓存/审计/健康全能力 |
| `config-confers` 🆕 | confers 配置热重载（`ArcSwap` 原子换装） |
| `retry` | 运行时重试：幂等判断 + 指数退避 |
| `failover` | 连接故障转移：CircuitBreaker 状态机与健康检查协同 |
| `replica-routing` | 副本路由读写分离：按 weight 与延迟选择、半开恢复 |
| `scatter-gather` | 跨分片聚合查询，支持 SUM / COUNT / AVG 合并 |
| `saga` | Saga 分布式事务：持久化日志、启动恢复、补偿编排 |
| `distributed-id` | Snowflake 分布式 ID 生成 |

> 🆕 为 0.6.0-rc.3 新增能力，完整清单见 [CHANGELOG](docs/CHANGELOG.md)。

---

## 🚀 快速开始

### 📦 安装

要求：Rust **1.97.1+**（`rust-toolchain.toml` 锁定，edition 2024），并至少启用一个运行时与一个数据库驱动特性。

```bash
cargo add dbnexus --features runtime-tokio-rustls,sqlite,permission,macros
cargo add tokio --features rt-multi-thread,macros
```

```toml
[dependencies]
dbnexus = { version = "0.6.0-rc.3", features = ["runtime-tokio-rustls", "sqlite", "permission", "macros"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

> `permission` 会强制启用 `sql-parser` 与 `cache`（编译期校验，防 SQL 注入绕过权限检查）。`default = []`：所有特性均需显式启用。

### 💡 最小示例

定义实体并获得宏生成的 CRUD（改编自 [examples/src/basic/basic_crud.rs](examples/src/basic/basic_crud.rs)，`ActiveModelBehavior` 由宏自动实现）：

```rust
use dbnexus::{DbPool, db_entity};
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

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 连接池 + admin 会话（RAII：Session 丢弃时连接自动归还）
    let pool = DbPool::new("sqlite::memory:").await?;
    let session = pool.get_session("admin").await?;

    // 宏生成的 CRUD：每条语句都经过解析与表级权限检查
    let user = Model { id: 1, name: "Alice".to_string(), email: "alice@example.com".to_string() };
    Model::insert(&session, user).await?;

    let users = Model::find_all(&session).await?;
    println!("找到 {} 个用户", users.len());
    Ok(())
}
```

<details>
<summary>🎬 权限控制：未授权角色被拒绝</summary>

```rust
// admin 角色默认放行（无权限配置文件时的安全默认）
let session = pool.get_session("admin").await?;
Model::find_all(&session).await?;

// guest 角色未在策略中定义时被拒绝；策略中未授权的表同样拒绝
let session = pool.get_session("guest").await?;
Model::find_all(&session).await?; // 错误：权限被拒绝
```

</details>

### 🧭 核心概念

| 概念 | 一句话说明 |
|------|-----------|
| `DbPool` | 连接池入口，可从 URL 或 `DbConfig` 构建，管理连接生命周期 |
| `Session` | 按角色获取的会话句柄，RAII 归还连接，承载事务与执行通道 |
| `#[db_entity]` | 一个宏生成 Sea-ORM 实体模型 + 8 个带权限检查的 CRUD 方法 |
| 权限策略 | 角色 → 表 → 操作的 RBAC 策略（内存或 YAML），JOIN/子查询表同样受检 |
| 特性门控 | 驱动互斥在编译期 `compile_error!` 强制，未启用的能力零开销 |

---

## 🎨 特性标志

`default = []`：无任何默认特性，运行时、数据库驱动与功能特性均需显式启用。嵌入式（`sqlite`/`duckdb`）与服务器端（`postgres`/`mysql`）驱动严格互斥，混用直接编译失败。

### 运行时（互斥，三选一）

| 标志 | 说明 | 默认 |
|------|------|:----:|
| `runtime-tokio-rustls` | Tokio 运行时 + rustls TLS | 否 |
| `runtime-tokio-native-tls` | Tokio 运行时 + native-tls | 否 |
| `runtime-async-std` | async-std 运行时 | 否 |

### 数据库驱动

关系型驱动四选一（编译期互斥）；图数据库驱动可与关系型驱动共存。`sqlite` / `postgres` / `mysql` 基于 sea-orm/sqlx 对应驱动实现，`duckdb`（嵌入式分析型）、`ladybug`（原 Kuzu）与 `neo4j` 为各自的原生绑定；各驱动的数据库、类型与引入版本见[数据库支持](#-数据库支持)。

### 核心能力

| 标志 | 说明 | 默认 |
|------|------|:----:|
| `permission` | 表级 RBAC 权限控制，强制依赖 `sql-parser`，并启用 `yaml` 与 `cache` | 否 |
| `sql-parser` | SQL 解析、表名提取与注入检测，自动启用 `cache` | 否 |
| `macros` | `dbnexus-macros` 过程宏（`db_entity` / `db_repository`） | 否 |
| `default-no-db` | 无驱动的默认聚合（运行时 + permission + sql-parser + macros + config-env + with-time），供 CI 按驱动组合测试 | 否 |

### 数据访问与集成

| 标志 | 说明 | 默认 |
|------|------|:----:|
| `cache` | oxcache 缓存（moka L1 后端）+ `ArcSwap` 无锁读取 | 否 |
| `oxcache-integration` | OxcacheDbCacheAdapter 适配器 | 否 |
| `kit` | trait-kit AsyncKit 集成，隐含池/缓存/审计/健康全能力闭包 | 否 |
| `repository` | 泛型仓储 `Repository<T>` CRUD 端口 + `impl_json_repository!` 宏 | 否 |
| `data-api` | 数据 API 网关：实体到 JSON 查询端点（白名单 + 过滤 + 分页） | 否 |
| `prepare-cache` | 语句级 prepared statement LRU 缓存 | 否 |
| `query-dsl` | `q!` 类型安全查询片段宏 | 否 |
| `entity-events` | 实体事件总线 + Outbox | 否 |
| `copy` | COPY FROM STDIN 批量写入语句构建 | 否 |
| `data-protection` | 字段脱敏与行级安全谓词注入 | 否 |
| `permission-facade` | RBAC + 脱敏 + RLS 统一门面 | 否 |
| `config-confers` | confers 配置热重载 | 否 |

### 可观测性

| 标志 | 说明 | 默认 |
|------|------|:----:|
| `metrics` | Prometheus 格式指标导出（含慢查询检测） | 否 |
| `health-check` | 健康检查模块与 `health_snapshot` 结构化导出 | 否 |
| `observability` | `metrics` + `health-check` 聚合 | 否 |
| `otel` | OTLP/HTTP JSON 信封导出桥 | 否 |

### 数据管理

| 标志 | 说明 | 默认 |
|------|------|:----:|
| `migration` | 数据库迁移 | 否 |
| `auto-migrate` | 自动迁移执行 | 否 |
| `sharding` | 数据分片（策略与会话级路由） | 否 |
| `global-index` | 跨分片全局索引 | 否 |
| `data-management` | 上述四项的聚合 | 否 |

### 分布式能力

| 标志 | 说明 | 默认 |
|------|------|:----:|
| `retry` | 运行时重试 + 指数退避（幂等判断） | 否 |
| `failover` | 连接故障转移（CircuitBreaker + 健康检查） | 否 |
| `replica-routing` | 副本路由读写分离 | 否 |
| `scatter-gather` | 跨分片聚合查询执行器 | 否 |
| `shard-migration` | 分片迁移编排 | 否 |
| `saga` | Saga 分布式事务编排（持久化恢复） | 否 |
| `distributed-id` | Snowflake 分布式 ID | 否 |
| `distributed-capabilities` | 上述 7 项的聚合 | 否 |

### 安全与合规

| 标志 | 说明 | 默认 |
|------|------|:----:|
| `audit` | 审计日志（操作 + 用户上下文） | 否 |
| `permission-engine` | 高级权限引擎（依赖 `permission`） | 否 |
| `authentication` | JWT 认证 + bcrypt 密码强度策略 | 否 |
| `security` | `audit` + `permission-engine` 聚合 | 否 |

<details>
<summary>📦 类型、配置源、连接池增强与开发工具特性</summary>

| 标志 | 说明 | 默认 |
|------|------|:----:|
| `with-json` / `with-time` / `with-chrono` / `with-uuid` | sea-orm 类型桥接（JSON / time / chrono / UUID 字段） | 否 |
| `validation` | validator 数据验证 | 否 |
| `json` | 直接 serde_json 反序列化支持 | 否 |
| `yaml` | YAML 权限/配置文件解析 | 否 |
| `config-toml` | TOML 配置支持（无额外依赖） | 否 |
| `config-env` | 环境变量配置（无额外依赖） | 否 |
| `pool-health-check` | 连接池健康检查 | 否 |
| `pool-warmup` | 连接池预热 | 否 |
| `dev` / `dev-full` | 开发辅助聚合 | 否 |
| `bench` | criterion 基准依赖 | 否 |
| `test-utils` | 测试辅助工具（tempfile / assert_cmd） | 否 |
| `cli-tests` | CLI 集成测试门控 | 否 |

</details>

### 特性预设

| 预设 | 特性 | 使用场景 |
|------|------|----------|
| `embedded` | `runtime-tokio-rustls`, `sqlite`, `config-env` | 嵌入式/边缘设备超最小配置 |
| `microservice` | `runtime-tokio-rustls`, `postgres`, `permission`, `sql-parser`, `config-env`, `observability` | 微服务部署 |
| `monolith` | `runtime-tokio-rustls`, `postgres`, `permission`, `sql-parser`, `yaml`, `data-management`, `security`, `observability`, 全部 7 项分布式能力 | 单体应用 |
| `enterprise` | `postgres`, `monolith`, `permission-engine` | 完整企业功能 |
| `all-optional` | 除数据库驱动外的 15 项可选特性（cache / observability / data-management / security / migration / retry / failover / replica-routing / scatter-gather / shard-migration / saga / distributed-id / repository / data-api / prepare-cache） | 全功能验证（手动追加驱动） |

### 使用示例

```toml
# 嵌入式/边缘设备（最小配置）
dbnexus = { version = "0.6.0-rc.3", features = ["embedded"] }

# 微服务
dbnexus = { version = "0.6.0-rc.3", features = ["microservice"] }

# 单体应用
dbnexus = { version = "0.6.0-rc.3", features = ["monolith"] }

# 企业级（完整功能）
dbnexus = { version = "0.6.0-rc.3", features = ["enterprise"] }
```

---

## 📚 文档

| 文档 | 说明 |
|------|------|
| [📖 用户指南](docs/USER_GUIDE.md) | 从安装到进阶的完整使用教程 |
| [📘 API 参考](docs/API_REFERENCE.md) | 全部公开 API 的详细说明 |
| [🏗️ 架构文档](docs/ARCHITECTURE.md) | 设计理念、模块划分、数据流与安全/性能设计 |
| [📊 性能基线](docs/PERFORMANCE.md) | 端到端基准数据与复现命令 |
| [🔒 安全文档](docs/SECURITY.md) | 纵深防御设计、最佳实践与漏洞报告流程 |
| [📋 更新日志](docs/CHANGELOG.md) | 每个版本的变更记录 |
| [🤝 贡献指南](docs/CONTRIBUTING.md) | TDD 工作流、代码规范与提交/PR 流程 |
| [🧪 测试场景固化](docs/TEST_SCENARIOS.md) | 测试金字塔基线、驱动组矩阵与 E2E 场景定义 |
| [📦 在线 API 文档](https://docs.rs/dbnexus) | docs.rs 自动生成的最新文档 |
| [📦 crates.io](https://crates.io/crates/dbnexus) | 发布页面 |

---

## 💻 示例

全部示例位于 [examples/](examples/)（独立 crate `dbnexus-examples`，随 workspace 管理，`publish = false`），共 **51 个二进制目标**（截至 0.6.0-rc.3）：

```bash
cd examples

# 运行单个示例（每个示例是一个 bin 目标）
cargo run --bin basic_crud --features "sqlite,permission,macros"

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
| 图数据库 | `graph_ladybug`*、`graph_neo4j` | Ladybug 嵌入式图 DB / Neo4j 服务器 |
| 分布式能力 | `distributed_id`、`saga`、`scatter_gather`、`replica_routing`、`shard_migration` | Snowflake ID / Saga / 跨分片聚合 / 读写分离 / 分片迁移 |
| 可靠性 | `retry`、`failover` | 重试退避 / 熔断故障转移 |
| 国际化 | `i18n_formatting` | ICU4X locale 格式化 |
| 集成与缓存 | `oxcache_adapter`、`cache_standalone` | oxcache 适配 / 自定义缓存 Provider |
| Kit | `kit_usage`、`kit_advanced` | 能力注册 / 多能力组合 |
| 通用 | `error_handling` | 结构化错误报告 |

\* `graph_ladybug` 因与 `duckdb` 存在 mbedtls 链接冲突未注册为 bin，需单独编译：`cargo build --bin graph_ladybug --no-default-features --features "runtime-tokio-rustls,sqlite,cache,ladybug"`。

完整说明见 [examples/README.md](examples/README.md)。

### 📝 代码片段

高级配置与环境变量、事务与监控等更多代码片段见[用户指南](docs/USER_GUIDE.md)（配置、事务、指标章节）与 [API 参考](docs/API_REFERENCE.md)。

---

## 🏗️ 架构

DBNexus 采用分层模块设计：`foundation` 提供配置与错误基座，`database` 模块承载连接池、Session、迁移、分片、Saga 与 scatter-gather，`access` 模块集中 SQL 解析、权限引擎、认证与脱敏，`domain` 模块沉淀权限/审计/迁移的领域抽象，`observability` 与 `reliability` 分别提供指标健康与重试容错。所有可选能力经特性门控编译期裁剪，过程宏 `dbnexus-macros` 在编译期为实体生成带权限检查的 CRUD 代码。

分层模块设计、各层职责与模块全景图的详细设计（设计理念、模块划分、数据流与安全/性能设计）见[架构文档](docs/ARCHITECTURE.md#系统架构)。

### 🔗 核心执行路径

`Session::execute_raw` 在 `sql-parser` + `permission` 特性组合下的真实执行管道：`get_session` 校验角色后，语句经"拒绝 DDL → 解析 → 逐表权限检查 → 驱动执行"返回；解析失败时 admin 角色放行、非 admin 角色拒绝，JOIN / 子查询涉及的目标表逐一受检（rc.2 补全）；`retry` 幂等重试、`metrics` 慢查询观测与 `audit` 的 admin 绕过记录均挂接于此管道（源码见 [src/database/pool/session.rs](src/database/pool/session.rs)）。

完整时序图与路径要点见[架构文档 · 核心执行管道](docs/ARCHITECTURE.md#核心执行管道)。

### 🌐 数据库支持

| 驱动特性 | 数据库 | 类型 | 引入版本 |
|----------|--------|------|----------|
| `sqlite` | SQLite | 嵌入式关系型 | 初始 |
| `postgres` | PostgreSQL | 服务器关系型 | 初始 |
| `mysql` | MySQL | 服务器关系型 | 初始 |
| `duckdb` | DuckDB | 嵌入式分析型 | 0.3.0 |
| `ladybug` | Ladybug（原 Kuzu） | 嵌入式图数据库 | 0.4.0 |
| `neo4j` | Neo4j | 图数据库服务器 | 0.4.0 |

通过标准协议支持的兼容数据库（无需额外特性，使用对应协议驱动即可）：

| 数据库 | 兼容协议 | 说明 |
|--------|----------|------|
| CockroachDB | PostgreSQL | 分布式 SQL 数据库 |
| YugabyteDB | PostgreSQL | 分布式 PostgreSQL |
| TiDB | MySQL | 分布式 HTAP 数据库 |
| MariaDB | MySQL | MySQL 兼容分支 |
| Aurora | PostgreSQL/MySQL | AWS 云原生数据库 |

> 已知限制：`duckdb` 与 `ladybug` 同时启用存在 mbedtls 重复符号链接冲突，多驱动验证请采用分组特性组合（见[路线图](#️-开发路线图)）。

---

## 🧪 测试

### 测试策略矩阵

测试分六层承载：`src/**` 内 `#[cfg(test)]` 单元测试、`tests/**` 显式注册的集成测试目标（按 feature 门控）、`tests/e2e/` 端到端场景（按 `cfg(feature)` 隔离）、`postgres_testcontainers` / `mysql_testcontainers` 容器级测试（每测试独立容器隔离）、doc tests（CI 单独运行 `cargo test --doc`）与 [benches/](benches/) 基准测试（见[性能](#-性能)）。金字塔基线、驱动组矩阵与 E2E 场景定义详见 [docs/TEST_SCENARIOS.md](docs/TEST_SCENARIOS.md)。

### 测试规模（截至 0.6.0-rc.3）

| 指标 | 数值 | 来源 |
|------|------|------|
| 测试函数总数 | 2389 个 `#[test]` / `#[tokio::test]` | grep 统计（src 1066 + tests 1321 + macros 2） |
| 显式注册测试目标 | 79 个 `[[test]]` | `Cargo.toml` |
| 驱动组全量通过 | sqlite 1712 / postgres 1276 / mysql 1276 / duckdb 1300 | [docs/TEST_SCENARIOS.md](docs/TEST_SCENARIOS.md) |
| 覆盖率门禁 | ≥ 80% 行覆盖 | `.github/workflows/ci.yml`（llvm-cov） |

### 运行命令（与 CI 一致）

```bash
# 全量测试（CI 标准特性组合；sqlite/postgres/mysql/duckdb 驱动互斥，不使用 --all-features）
cargo test --no-default-features --features sqlite,default-no-db,all-optional --workspace --exclude dbnexus-examples --exclude dbnexus-macros

# 切换数据库后端运行集成测试（CI 以 services 容器提供 PostgreSQL 15 / MySQL 8.0）
cargo test --no-default-features --features postgres,default-no-db,all-optional --workspace --exclude dbnexus-examples --exclude dbnexus-macros

# 文档测试
cargo test --no-default-features --features sqlite,default-no-db,all-optional --doc --workspace --exclude dbnexus-examples --exclude dbnexus-macros
```

> 嵌入式（`sqlite`/`duckdb`）与服务器端（`postgres`/`mysql`）驱动在编译期严格互斥（`compile_error!`），验证多驱动时按分组特性组合运行。

---

## 📊 性能

DBNexus 遵循零成本抽象原则，性能相关能力均为设计层面保证：零成本特性门控（`#[cfg(feature = ...)]` 编译期裁剪）、无锁热路径（池状态原子维护、权限配置与缓存 Provider 经 `ArcSwap` 无锁读取、`AcqRel` 内存序）、异步优先（全部 I/O `async/await`，读多写少状态用 `RwLock`）、连接池策略（LRU 复用、最大连接限制、最小连接预热、健康检查剔除死连接），以及 0.5.1 热路径优化（注入检测模式表静态化、Saga 补偿查找预索引、`Session` 并发读优化、`DbConfig` Arc 共享）。设计细节见[架构文档 · 性能架构](docs/ARCHITECTURE.md#性能架构)。

### 端到端基准

仓库内置 5 个 criterion 基准：`permission_bench`、`permission_engine_bench`、`sharding_bench`、`metrics_bench`、`e2e_bench`（位于 [benches/](benches/)，`cargo bench` 运行）。端到端基线（2026-09-11 采样，非 SLA）：`DbPool::get_session` 句柄获取 ≈ 0.24 µs、`DbPool::query_rows` 单行完整管道 ≈ 480 µs、`Session::execute_raw` 64 行循环 INSERT ≈ 708 ms/迭代（≈ 11 ms/行）。测量环境、逐项解读与复现命令见[性能基线 · 端到端基线](docs/PERFORMANCE.md#端到端基线)。

### 历史优化对照

两轮平台优化的累计结果（6 项基准全部正向提升、无回退，平均累计提升约 5.5%）见 [benches/baseline-after.md](benches/baseline-after.md) 与[性能基线 · 历史优化对照](docs/PERFORMANCE.md#历史优化对照)。

---

## 🔒 安全

DBNexus 从设计之初就以内建安全为目标，纵深防御自下而上分为五层（完整设计见[安全文档](docs/SECURITY.md)）：

| 层级 | 机制 |
|------|------|
| 编译时保证 | 全库 `#![forbid(unsafe_code)]`；嵌入式与服务器端驱动混用直接 `compile_error!`；特性依赖缺失即编译失败，无静默降级 |
| 运行时权限 | 表级 RBAC 覆盖 JOIN/子查询跨表路径；带 TTL 的权限缓存 + singleflight 防击穿；令牌桶限流防滥用 |
| 注入防护 | 默认参数化查询；`SqlParser` 表名提取与 `InjectionEngine` 统一注入检测（含 Unicode 归一化）；`DdlGuard` AST 校验且 DDL 仅限 admin；图查询统一参数化通道 |
| 认证与配置 | JWT 访问/刷新令牌区分校验、撤销缓存 TTL 化；bcrypt 密码策略；路径遍历校验，URL 解析错误不回显凭据 |
| 审计与脱敏 | `audit` 完整操作与用户上下文日志（admin 绕过同样记录）；`SensitiveMasker` 多类型脱敏与行级安全 |

供应链安全：CI 常开 `cargo deny check`（许可证/公告/重复依赖，豁免留痕于 [deny.toml](deny.toml)）与 `cargo audit`（[audit.toml](audit.toml)），CodeQL 语义扫描与 Dependabot 自动更新，pre-commit 私钥扫描拦截。

**漏洞报告**：请勿通过公开 Issue 提交。请使用 GitHub [Security Advisories](https://github.com/Kirky-X/dbnexus/security/advisories/new) 私密通道（"Report a vulnerability"）。响应承诺：48 小时内确认，7 天内给出初步评估（见 [SECURITY.md](docs/SECURITY.md)）。

---

## 🗺️ 开发路线图

### 短期（0.6.0 正式发布）

- [ ] 发布 0.6.0 正式版：完成 rc 验证后按工作区传导表更新 trait-kit 0.5.0 / oxcache 0.5.0 依赖要求，`cargo publish --dry-run` 核对后打 tag 触发 release.yml 自动发布
- [x] 对齐 MSRV 声明与依赖实际要求 — 已按工作区 CONFIG_BASELINE 统一为 1.97.1（2026-09-06），覆盖传递依赖的 1.94 要求
- [ ] 恢复 MySQL 集成测试的常规运行（testcontainers 已就绪，当前受数据库服务依赖阻塞）

### 中期

- [ ] 解决 `duckdb` 与 `ladybug` 同时启用时的 mbedtls 重复符号链接冲突（当前多驱动验证采用分组特性组合）
- [ ] 跟进代码质量审查留档的 Medium 项

> 条目整理自 [CHANGELOG.md](docs/CHANGELOG.md) 与仓库验收记录。

---

## 🤝 参与贡献

欢迎贡献！请先阅读 [CONTRIBUTING.md](docs/CONTRIBUTING.md)，了解 TDD 工作流、开发环境要求（Rust 1.97.1 工具链、lefthook / pre-commit 钩子——安装脚本 `./scripts/install-pre-commit.sh`，禁止 `--no-verify` 绕过、Conventional Commits 提交信息）与质量门禁（`cargo fmt --check`、`cargo clippy -D warnings`、`cargo deny check`、`cargo audit`、行覆盖 ≥ 80%），以及提交/PR 流程。

---

## 📋 更新日志

完整版本历史见 [CHANGELOG.md](docs/CHANGELOG.md)。

| 版本 | 日期 | 要点 |
|------|------|------|
| 0.6.0-rc.3 | 2026-09-10 | 统一行查询 `query_rows`；Saga 持久化恢复；字段级脱敏与行级安全（`data-protection`）；权限统一门面与查询 DSL；COPY 批量写入与 OTel 导出桥；端到端基准 `e2e_bench`；运维 CLI `migrate`/`health`/`user` 子命令 |
| 0.6.0-rc.2 | 2026-09-03 | 移除 `tracing` 特性；四驱动互斥严格化（编译期 `compile_error!`）；补全 JOIN/子查询跨表权限检查；`h2` 安全升级 |
| 0.5.1 | 2026-08-06 | 热路径性能优化（注入检测静态表、Session 读写锁化、`DbConfig` Arc 共享等）；清理弃用 builder 方法 |

---

## 📄 许可证

本项目基于 **MIT + Commons Clause** 许可证发布：在 MIT 许可的基础上附加 Commons Clause 条件，未经单独书面授权不得销售本软件或将其用于商业用途。详见 [LICENSE](LICENSE)。

---

## 🙏 致谢

- [Sea-ORM](https://www.sea-ql.org/SeaORM/) - 优秀的 ORM 框架，DBNexus 建立在其上
- [SQLx](https://github.com/launchbadge/sqlx) - 异步 SQL 工具包
- [sqlparser](https://github.com/apache/datafusion-sqlparser-rs) - SQL 方言解析，支撑权限检查与注入防护
- [ICU4X](https://github.com/unicode-org/icu4x) - Unicode 国际化组件，支撑 locale 感知格式化
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
