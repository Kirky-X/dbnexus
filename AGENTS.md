# Agents Guide

## Overview

dbnexus 是基于 Sea-ORM 构建的企业级 Rust 数据库抽象层，提供连接池管理、权限控制（RBAC）、过程宏、SQL 解析、迁移、分片、审计、监控等企业级功能。

- **版本**: 0.3.2
- **edition**: 2024
- **rust-version**: 1.85+
- **许可证**: MIT
- **仓库**: https://github.com/Kirky-X/dbnexus
- **文档**: https://docs.rs/dbnexus

## Project Structure

```
dbnexus/
├── src/
│   ├── lib.rs                    # crate 根模块，公共 API 导出
│   ├── common/                   # 公共类型（error）
│   ├── foundation/               # Foundation 层 - 基础模块（config, error）
│   ├── domain/                   # Domain 层 - 领域模块
│   │   ├── permission/           #   权限领域模型
│   │   ├── migration/            #   迁移领域模型
│   │   ├── audit/                #   审计领域模型
│   │   └── cache_provider.rs     #   缓存提供者抽象
│   ├── database/                 # Database 模块
│   │   ├── pool/                 #   连接池（DbPool, Session, DuckDb）
│   │   ├── migration/             #   数据库迁移执行
│   │   └── sharding.rs           #   数据分片
│   ├── access/                   # Access 模块 - 安全、权限、认证
│   │   ├── permission/           #   权限控制（RBAC, cache, rate_limiter）
│   │   ├── permission_engine.rs  #   高级权限引擎
│   │   ├── security/             #   安全（ddl_guard, sensitive）
│   │   ├── authentication/       #   JWT 认证
│   │   └── sql_parser.rs         #   SQL 解析器
│   ├── observability/            # Observability 层
│   │   ├── metrics.rs            #   Prometheus 指标
│   │   ├── health.rs             #   健康检查
│   │   └── tracing.rs            #   OpenTelemetry 追踪
│   ├── storage/                  # Storage 模块（global-index）
│   ├── i18n/                     # 国际化（ICU4X）
│   ├── integrations/             # 外部 crate 集成适配器
│   │   ├── oxcache_adapter.rs    #   oxcache 缓存适配器
│   │   └── kit/                  #   trait-kit AsyncKit 集成
│   ├── generated_roles.rs        # 生成的权限角色模块
│   └── tools/
│       └── cli/                  # CLI 工具（独立 crate）
├── macros/                       # 过程宏 crate（dbnexus-macros）
├── examples/                     # 示例 crate（dbnexus-examples）
├── tests/                        # 测试套件
├── benches/                      # 基准测试
└── .github/
    └── workflows/                # CI/CD 工作流
```

## Where to Look

| 功能 | 位置 |
|------|------|
| 核心迁移 | `src/database/migration/` |
| 连接池 / Session | `src/database/pool/` |
| CLI 工具 | `src/tools/cli/` |
| 权限控制 | `src/access/permission/` |
| SQL 解析 | `src/access/sql_parser.rs` |
| 过程宏 | `macros/`（`#[db_entity]`） |
| 指标监控 | `src/observability/metrics.rs` |
| 健康检查 | `src/observability/health.rs` |
| 分布式追踪 | `src/observability/tracing.rs` |
| 数据分片 | `src/database/sharding.rs` |
| 全局索引 | `src/storage/global_index.rs` |
| 审计日志 | `src/domain/audit/` |
| JWT 认证 | `src/access/authentication/` |

## Conventions

- **edition 2024**, rust 1.85+
- **MIT License**（workspace 统一）
- **依赖必须通过 feature 门控**：所有可选依赖在 `[features]` 中声明，在 `[dependencies]` 中标记 `optional = true`
- **TDD 开发流程**：Red → Green → Commit → Analyze → Next
- **禁止 `unsafe` 代码**：`#![forbid(unsafe_code)]`
- **必须有文档注释**：`#![deny(missing_docs)]`
- **命名规范**：`snake_case`（函数/变量/模块），`CamelCase`（类型/Trait）
- **错误处理**：使用 `Result<T, E>` 返回错误，严禁吞掉错误
- **Feature 互斥规则**：
  - 数据库驱动互斥：`sqlite`/`postgres`/`mysql`/`duckdb`（编译期检查）
  - 嵌入式（sqlite/duckdb）与服务器端（postgres/mysql）不能混合
  - `permission` 需要 `cache`
  - `sql-parser` 需要 `cache`
  - `permission-engine` 需要 `cache`

## Commands

### 构建

```bash
# 构建所有特性
cargo build --all-features

# 构建特定数据库后端
cargo build --no-default-features --features sqlite,default-no-db,all-optional
cargo build --no-default-features --features postgres,default-no-db,all-optional
```

### 测试

```bash
# 全部测试
cargo test --all-features --lib

# 特定数据库后端测试
cargo test --features sqlite
cargo test --features postgres
cargo test --features mysql

# 集成测试（需要 Docker）
cargo test --features postgres --test postgres_testcontainers
cargo test --features mysql --test mysql_testcontainers
```

### 代码质量

```bash
# 格式化
cargo fmt --all -- --check

# Clippy（必须零警告）
cargo clippy --all-features --all-targets -- -D warnings

# 文档构建
cargo doc --all-features --no-deps --workspace
```

### 安全

```bash
# 依赖安全审计
cargo deny check licenses advisories bans
```

## GitNexus

本项目已由 GitNexus 索引（4904 symbols, 12127 relationships, 300 execution flows）。

修改代码前：
1. 使用 `gitnexus_impact` 分析影响范围
2. HIGH/CRITICAL 风险必须先警告用户
3. 提交前使用 `gitnexus_detect_changes` 验证变更范围
