# DBNexus Examples

本目录包含 DBNexus 的所有功能示例，帮助您快速了解和使用 DBNexus 的各项特性。

**注意**：这是一个独立的 Rust 项目（`dbnexus-examples`），通过 `examples/Cargo.toml` 管理。

## 📋 目录

- [快速开始](#快速开始)
- [基础模块](#基础模块-basic)
- [配置模块](#配置模块-config)
- [数据库模块](#数据库模块-database)
- [权限模块](#权限模块-permission)
- [安全模块](#安全模块-security)
- [认证与审计](#认证与审计-auth)
- [可观测性模块](#可观测性模块-observability)
- [宏模块](#宏模块-macros)
- [图数据库模块](#图数据库模块-graph)
- [国际化模块](#国际化模块-i18n)
- [集成适配器](#集成适配器-integrations)
- [缓存功能](#缓存功能-cache)
- [Kit 能力管理](#kit-能力管理-kit)
- [通用模块](#通用模块-common)
- [运行示例](#运行示例)

## 快速开始

### basic_connection
最基础的连接池示例，展示 DbPool 和 Session 的创建：

```bash
cargo run --bin basic_connection
```

### basic_crud
基础 CRUD 操作，展示 `#[db_entity]` 宏的使用：

```bash
cargo run --bin basic_crud
```

### basic_transaction
事务管理（begin/commit/rollback），包含转账场景：

```bash
cargo run --bin basic_transaction
```

## 基础模块 (basic/)

| 示例 | 文件 | 说明 |
|------|------|------|
| `basic_connection` | `basic/basic_connection.rs` | 连接池 + Session + PoolStatus |
| `basic_crud` | `basic/basic_crud.rs` | `#[db_entity]` 宏 + CRUD 操作 |
| `basic_transaction` | `basic/basic_transaction.rs` | 事务 begin/commit/rollback |

## 配置模块 (config/)

| 示例 | 文件 | 说明 |
|------|------|------|
| `config_env` | `config/config_env.rs` | 环境变量配置 |
| `config_yaml` | `config/config_yaml.rs` | YAML 配置文件 |
| `config_toml` | `config/config_toml.rs` | TOML 配置文件 |
| `config_presets` | `config/config_presets.rs` | 预设配置对比（embedded/microservice/monolith/enterprise） |

## 数据库模块 (database/)

| 示例 | 文件 | 说明 |
|------|------|------|
| `database_sqlite` | `database/database_sqlite.rs` | SQLite 内存 + 文件模式 |
| `database_postgres` | `database/database_postgres.rs` | PostgreSQL 连接 + 优雅降级 |
| `database_mysql` | `database/database_mysql.rs` | MySQL 连接 + 优雅降级 |
| `duckdb_query` | `database/duckdb_query.rs` | DuckDB 分析型查询 |
| `migration` | `database/migration.rs` | 迁移定义/应用/历史/回滚 |
| `sharding` | `database/sharding.rs` | 4 种分片策略 + 路由 |
| `global_index` | `database/global_index.rs` | 全局索引 CRUD + SyncEvent |
| `pool_management` | `database/pool_management.rs` | 连接池 warmup/health-check/auto-migrate |

## 权限模块 (permission/)

| 示例 | 文件 | 说明 |
|------|------|------|
| `permission_rbac` | `permission/permission_rbac.rs` | MemoryPermissionProvider + RBAC |
| `permission_yaml` | `permission/permission_yaml.rs` | YAML 策略加载/解析 |
| `permission_macro` | `permission/permission_macro.rs` | `#[db_entity(permissions(...))]` 宏 |
| `permission_engine` | `permission/permission_engine.rs` | PDP + RBAC + 速率限制 |

## 安全模块 (security/)

| 示例 | 文件 | 说明 |
|------|------|------|
| `sql_parser` | `security/sql_parser.rs` | SQL 解析 + 操作类型提取 |
| `sql_injection_detection` | `security/sql_injection_detection.rs` | 注入检测 + Unicode 防护 |
| `ddl_guard` | `security/ddl_guard.rs` | DDL AST 安全验证 |
| `sensitive_masker` | `security/sensitive_masker.rs` | 7 种脱敏类型 |
| `rate_limiter` | `security/rate_limiter.rs` | 令牌桶速率限制 |

## 认证与审计 (auth/)

| 示例 | 文件 | 说明 |
|------|------|------|
| `authentication_jwt` | `auth/authentication_jwt.rs` | JWT 签发/验证/刷新 |
| `authentication_password` | `auth/authentication_password.rs` | bcrypt 哈希 + 用户管理 |
| `audit_logging` | `auth/audit_logging.rs` | 审计日志完整流程 |

## 可观测性模块 (observability/)

| 示例 | 文件 | 说明 |
|------|------|------|
| `metrics_prometheus` | `observability/metrics_prometheus.rs` | Prometheus 指标导出 |
| `health_check` | `observability/health_check.rs` | HealthChecker + CircuitBreaker |
| `latency_histogram` | `observability/latency_histogram.rs` | 延迟直方图 + 慢查询 |
| `tracing_otlp` | `observability/tracing_otlp.rs` | OTLP 分布式追踪 |

## 宏模块 (macros/)

| 示例 | 文件 | 说明 |
|------|------|------|
| `macros_db_entity` | `macros/macros_db_entity.rs` | 多实体定义 + 关系 |
| `macros_db_crud` | `macros/macros_db_crud.rs` | 8 种 CRUD 方法 + 批量 + 分页 |
| `macros_db_audit` | `macros/macros_db_audit.rs` | audit 子参数集成 |
| `macros_db_cache` | `macros/macros_db_cache.rs` | cache 子参数集成 |
| `macros_soft_delete_unique` | `macros/macros_soft_delete_unique.rs` | 软删除 + 复合唯一约束 |
| `macros_db_entity_v2` | `macros/macros_db_entity_v2.rs` | timestamps + validate + hooks |
| `macros_advanced_query` | `macros/macros_advanced_query.rs` | schema/query/paginate/batch |

## 图数据库模块 (graph/)

| 示例 | 文件 | 说明 |
|------|------|------|
| `graph_ladybug` | `graph/graph_ladybug.rs` | Ladybug 嵌入式图数据库（DDL/节点/关系/事务） |
| `graph_neo4j` | `graph/graph_neo4j.rs` | Neo4j 服务器端图数据库（URL 解析/优雅降级/事务） |

```bash
# Ladybug（嵌入式，无需外部服务器）
cargo run --bin graph_ladybug

# Neo4j（需要 Neo4j 服务器，无服务器时演示优雅降级）
cargo run --bin graph_neo4j
NEO4J_URL=neo4j://user:pass@localhost:7687 cargo run --bin graph_neo4j  # pragma: allowlist secret
```

## 国际化模块 (i18n)

| 示例 | 文件 | 说明 |
|------|------|------|
| `i18n_formatting` | `common/i18n_formatting.rs` | ICU4X locale 感知格式化（数字/日期/复数/排序） |

```bash
cargo run --bin i18n_formatting
```

## 集成适配器 (integrations/)

| 示例 | 文件 | 说明 |
|------|------|------|
| `oxcache_adapter` | `integrations/oxcache_adapter.rs` | OxcacheDbCacheAdapter 适配 oxcache 到 DbCacheProvider |

```bash
cargo run --bin oxcache_adapter
```

## 缓存功能 (cache)

| 示例 | 文件 | 说明 |
|------|------|------|
| `cache_standalone` | `common/cache_standalone.rs` | 自定义 DbCacheProvider 实现 + DbPoolBuilder 集成 |

```bash
cargo run --bin cache_standalone
```

## Kit 能力管理 (kit/)

| 示例 | 文件 | 说明 |
|------|------|------|
| `kit_usage` | `kit/kit_usage.rs` | DbNexusKit 能力注册/发现/替换 |
| `kit_advanced` | `kit/kit_advanced.rs` | pool + permission + metrics 三能力组合 |

```bash
cargo run --bin kit_usage
cargo run --bin kit_advanced
```

## 通用模块 (common/)

| 示例 | 文件 | 说明 |
|------|------|------|
| `error_handling` | `common/error_handling.rs` | QueryErrorReport + ErrorCategory |

## 运行示例

### 基本运行

```bash
cd examples
```

#### 方式 1：直接运行特定示例

```bash
cargo run --bin basic_connection
cargo run --bin graph_ladybug
cargo run --bin i18n_formatting
```

#### 方式 2：编译所有示例

```bash
cargo build --all-targets
```

### Feature Flags

所有必要的 features 已在 `Cargo.toml` 中默认启用，包括：

#### 数据库驱动
- `sqlite` — SQLite 嵌入式数据库
- `duckdb` — DuckDB 分析型数据库
- `ladybug` — Ladybug 嵌入式图数据库
- `neo4j` — Neo4j 图数据库服务器

#### 核心特性
- `permission` / `permission-engine` — 权限控制
- `sql-parser` — SQL 解析器
- `macros` — 过程宏
- `cache` — 缓存功能

#### 企业特性
- `metrics` — Prometheus 指标
- `health-check` — 健康检查
- `tracing` — OpenTelemetry 追踪
- `audit` — 审计日志
- `authentication` — JWT 认证

#### 数据管理
- `migration` / `auto-migrate` — 数据库迁移
- `sharding` — 数据分片
- `global-index` — 全局索引

#### 集成
- `oxcache-integration` — Oxcache 缓存适配器
- `kit` — trait-kit AsyncKit 模块系统

## 示例文件结构

每个示例文件都遵循统一的结构：

```rust
// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 示例标题
//!
//! 示例描述
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --bin <name>
//! ```

use dbnexus::{...};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 示例代码
    Ok(())
}
```

## 获取帮助

1. 查看示例文件中的注释和文档
2. 查看 [用户指南](../docs/USER_GUIDE.md)
3. 查看 [API 文档](../docs/API_REFERENCE.md)
4. 在 [GitHub Issues](https://github.com/Kirky-X/dbnexus/issues) 提问

## 贡献

欢迎贡献新的示例！请确保：

1. 遵循现有的示例文件结构
2. 添加清晰的注释和文档
3. 包含运行说明
4. 确保示例可以正确编译和运行
