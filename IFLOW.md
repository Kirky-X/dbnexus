<!-- OPENSPEC:START -->
# OpenSpec Instructions

These instructions are for AI assistants working in this project.

Always open `@/openspec/AGENTS.md` when the request:
- Mentions planning or proposals (words like proposal, spec, change, plan)
- Introduces new capabilities, breaking changes, architecture shifts, or big performance/security work
- Sounds ambiguous and you need the authoritative spec before coding

Use `@/openspec/AGENTS.md` to learn:
- How to create and apply change proposals
- Spec format and conventions
- Project structure and guidelines

Keep this managed block so 'openspec update' can refresh the instructions.

<!-- OPENSPEC:END -->

# IFLOW.md - DBNexus 项目交互指南

## 项目概述

DBNexus 是一个基于 Sea-ORM 构建的企业级 Rust 数据库抽象层，为应用提供高性能、高安全性、可扩展的数据访问能力。

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

---

## 项目结构

```
dbnexus/
├── dbnexus/                      # 主 crate
│   ├── src/
│   │   ├── lib.rs               # 主入口，公共 API 导出
│   │   ├── audit.rs             # 审计日志模块
│   │   ├── cache.rs             # 实体缓存模块
│   │   ├── config.rs            # 配置管理 (DbConfig, DbError)
│   │   ├── entity.rs            # 实体转换工具
│   │   ├── generated_roles.rs   # 自动生成的权限角色
│   │   ├── global_index.rs      # 全局索引模块
│   │   ├── metrics.rs           # Prometheus 指标 (可选)
│   │   ├── migration.rs         # 数据库迁移 (可选)
│   │   ├── permission.rs        # 权限控制
│   │   ├── permission_engine.rs # 可插拔权限引擎
│   │   ├── pool.rs              # 连接池管理
│   │   ├── sharding.rs          # 分片管理
│   │   └── tracing.rs           # 分布式追踪
│   └── tests/                   # 集成测试
│       ├── common/mod.rs        # 测试辅助函数
│       ├── audit_integration.rs
│       ├── auto_migrate_integration.rs
│       ├── cache_integration.rs
│       ├── cli_integration.rs
│       ├── concurrency_integration.rs
│       ├── global_index_integration.rs
│       ├── metrics_integration.rs
│       ├── migration_integration.rs
│       ├── multi_db_integration.rs
│       ├── permission_engine_integration.rs
│       ├── permission_integration.rs
│       ├── pool_integration.rs
│       ├── session_transaction.rs
│       └── sharding_integration.rs
├── dbnexus-cli/                 # CLI 工具
│   └── src/main.rs
├── dbnexus-macros/              # 过程宏 crate
│   └── src/lib.rs
├── examples/                    # 示例代码
│   ├── audit.rs                 # 审计日志示例
│   ├── cache.rs                 # 缓存示例
│   ├── permissions.rs           # 权限控制示例
│   ├── quickstart.rs            # 快速开始示例
│   ├── sharding.rs              # 分片示例
│   └── transactions.rs          # 事务示例
├── docs/                        # 文档
│   ├── API_REFERENCE.md
│   ├── ARCHITECTURE.md
│   ├── CONTRIBUTING.md
│   ├── FAQ.md
│   └── USER_GUIDE.md
├── scripts/                     # 脚本
│   ├── generate-sql.sh
│   ├── init-mysql.sql
│   ├── init-postgres.sql
│   ├── init-sqlite.sql
│   └── test-databases.sh
├── .github/workflows/           # CI/CD 配置
│   ├── ci.yml                   # 持续集成
│   ├── release.yml              # 发布流程
│   └── tag-deleted.yml          # 标签删除处理
└── Makefile                     # 构建和测试命令
```

---

## 构建与运行

### 环境要求

- **Rust 版本**: 1.85+
- **数据库**: SQLite 3.35+ / PostgreSQL 12+ / MySQL 8.0+
- **Docker**: 用于启动开发数据库容器（可选）

### 编译命令

```bash
# 检查编译（所有特性）
cargo check --all-features --all

# 格式化代码
cargo fmt --all

# 检查格式化
cargo fmt --check --all

# Clippy 静态检查（严格模式）
cargo clippy --all-features --all -- -D warnings

# 构建发布版本（选择一种数据库）
cargo build --release --features sqlite
cargo build --release --features postgres
cargo build --release --features mysql

# 构建包含所有可选功能的版本
cargo build --release --features "sqlite,all-optional"

# 生成文档
cargo doc --no-deps --all-features
```

### 测试命令

```bash
# 运行所有测试（选择一种数据库）
cargo test --features sqlite --all
cargo test --features postgres --all
cargo test --features mysql --all

# 运行单个测试
cargo test --features sqlite -p dbnexus test_name

# 运行特定集成测试文件
cargo test --features sqlite -p dbnexus --test pool_integration

# 使用 Makefile（推荐）
make test-sqlite      # SQLite 测试
make test-postgres    # PostgreSQL 测试
make test-mysql       # MySQL 测试
make test-all         # 所有数据库测试

# Clippy 检查
make clippy-sqlite    # SQLite clippy
make clippy-postgres  # PostgreSQL clippy
make clippy-mysql     # MySQL clippy
make clippy-all       # 所有数据库 clippy
```

### 数据库容器管理

```bash
# 启动数据库容器（PostgreSQL + MySQL）
make docker-up

# 停止数据库容器
make docker-down

# 查看数据库日志
make docker-logs
```

**注意**: 数据库容器启动后需要等待约 15 秒才能正常使用。

---

## 代码风格与规范

### 编码规范

- **缩进**: 4 空格
- **行宽**: 最大 120 字符
- **Rust Edition**: 2024
- **不安全代码**: 禁止使用 (`#![forbid(unsafe_code)]`)
- **文档要求**: 所有公开 API 必须有文档注释
- **Clippy**: 所有警告视为错误 (`-D warnings`)

### 测试规范

- 测试模块允许使用 `#[allow(clippy::unwrap_used)]`
- 测试文件放在 `tests/` 目录
- 使用 `mod common` 导入测试辅助模块
- 集成测试以 `_integration.rs` 格式命名

### 命名约定

| 类型 | 约定 | 示例 |
|------|------|------|
| 结构体 | PascalCase | `DbPool`, `Session` |
| 枚举 | PascalCase | `Operation`, `DatabaseType` |
| 函数 | snake_case | `check_connection_health` |
| 常量 | SCREAMING_SNAKE_CASE | `ALLOWED_ROLES` |
| 宏属性 | snake_case | `#[db_crud]`, `#[primary_key]` |
| 字段 | snake_case | `max_connections` |

---

## 架构设计

### 核心类型

```
┌─────────────────────────────────────────────────────────────┐
│                        DbPool                                │
│  - 管理数据库连接池                                           │
│  - 提供 get_session(role) 获取 Session                       │
│  - 配置验证与修正                                             │
│  - 连接健康检查                                               │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                        Session                               │
│  - RAII 包装数据库连接                                        │
│  - 自动返回连接到池（Drop 实现）                              │
│  - 事务管理 (begin/commit/rollback)                          │
│  - 写操作追踪 (should_use_master)                            │
│  - 权限上下文绑定                                             │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                      Entity + CRUD                           │
│  - 由 #[derive(DbEntity)] 生成 EntityTrait 实现              │
│  - 由 #[db_crud] 生成 CRUD 方法                              │
│  - 由 #[db_permission] 生成权限检查                          │
└─────────────────────────────────────────────────────────────┘
```

### 模块职责

| 模块 | 文件 | 职责 |
|------|------|------|
| **config** | `config.rs` | 配置加载、环境变量、配置文件解析 |
| **pool** | `pool.rs` | 连接池管理、Session 生命周期 |
| **permission** | `permission.rs` | 角色权限检查、Operation 枚举 |
| **permission_engine** | `permission_engine.rs` | 可插拔权限引擎、RBAC 支持 |
| **entity** | `entity.rs` | ActiveModel 转换、EntityTrait 扩展 |
| **metrics** | `metrics.rs` | Prometheus 指标收集与导出 |
| **migration** | `migration.rs` | Schema 差异检测、SQL 生成 |
| **audit** | `audit.rs` | 审计日志、操作追踪 |
| **cache** | `cache.rs` | LRU 缓存、TTL 过期 |
| **sharding** | `sharding.rs` | 分片策略、路由 |
| **global_index** | `global_index.rs` | 全局唯一索引 |
| **tracing** | `tracing.rs` | 分布式追踪 |

---

## 功能特性详解

### 编译特性

```toml
[features]
default = ["runtime-tokio-rustls"]

# Async runtime (互斥)
runtime-tokio-rustls = ["sea-orm/runtime-tokio-rustls", "tokio/rt-multi-thread"]
runtime-tokio-native-tls = ["sea-orm/runtime-tokio-native-tls", "tokio/rt-multi-thread"]
runtime-async-std = ["sea-orm/runtime-async-std"]

# Database drivers (互斥 - 必须且只能选一个)
sqlite = ["sea-orm/sqlx-sqlite", "sea-orm/macros"]
postgres = ["sea-orm/sqlx-postgres", "sea-orm/macros"]
mysql = ["sea-orm/sqlx-mysql", "sea-orm/macros"]

# Optional features
metrics = ["dep:prometheus"]           # Prometheus 指标
migration = []                          # 迁移支持
auto-migrate = ["migration"]           # 自动迁移
sharding = ["dep:twox-hash", "dep:chrono"]  # 分片
global-index = ["dep:sha2", "dep:async-trait", "dep:chrono"]  # 全局索引
cache = ["dep:async-trait", "dep:uuid", "dep:indexmap"]  # 缓存
audit = ["dep:chrono", "dep:uuid", "dep:async-trait"]    # 审计日志
permission-engine = ["dep:async-trait"]  # 可插拔权限引擎
tracing = [
    "dep:tracing-subscriber",
    "dep:tracing-opentelemetry",
    "dep:opentelemetry-otlp",
    "dep:opentelemetry-jaeger",
    "dep:opentelemetry",
    "dep:opentelemetry_sdk",
    "dep:chrono",
    "dep:http",
    "dep:tower",
    "dep:once_cell",
]

# 启用所有可选功能（不包括数据库特性）
all-optional = ["metrics", "migration", "auto-migrate", "tracing", "sharding", "global-index", "cache", "audit", "permission-engine"]
```

### 审计日志

```rust
use dbnexus::db_audit;

#[derive(DbEntity)]
#[db_entity]
#[table_name = "users")]
#[db_audit(operations = ["CREATE", "UPDATE", "DELETE"])]
struct User {
    #[primary_key]
    id: i64,
    name: String,
}
```

### 实体缓存

```rust
use dbnexus::db_cache;

#[derive(DbEntity)]
#[db_entity]
#[table_name = "users")]
#[db_cache(ttl = 300, capacity = 1000)]
struct User {
    #[primary_key]
    id: i64,
    name: String,
}
```

### 分片管理

```rust
use dbnexus::sharding::{ShardRouter, ShardConfig, YearlyStrategy};

let config = ShardConfig::new("yearly", 12, "orders", "postgresql://localhost/{shard}");
let router = ShardRouter::with_config(&config);
```

### 权限引擎

```rust
use dbnexus::permission_engine::{PolicyDecisionPoint, YamlPermissionProvider};

let provider = YamlPermissionProvider::new("permissions.yaml")?;
let pdp = PolicyDecisionPoint::new(Arc::new(provider));

let result = pdp.check_permission("admin", "users", "SELECT").await;
```

---

## 宏系统详解

### 第 1 层: #[derive(DbEntity)]

将 Rust struct 映射为 Sea-ORM Entity。

```rust
use dbnexus::{DbEntity, db_entity};

#[derive(DbEntity)]
#[db_entity]
#[table_name = "users")]
struct User {
    #[primary_key]
    id: i64,
    name: String,
    email: String,
}
```

**生成代码**:
- `EntityTrait` 实现（表名、主键）
- `ActiveModel` 结构体
- `IntoActiveModel` 实现

### 第 2 层: #[db_crud]

自动生成 CRUD 方法（每次操作前自动检查权限）。

```rust
#[derive(DbEntity)]
#[db_entity]
#[table_name = "users")]
#[db_crud]
struct User {
    #[primary_key]
    id: i64,
    name: String,
}
```

**生成方法**:
- `insert(session, entity)` - 插入并返回带 ID 的实体
- `find_by_id(session, id)` - 根据 ID 查询
- `update(session, entity)` - 更新记录
- `delete(session, id)` - 删除记录
- `find_all(session)` - 查询所有
- `delete_many(session, filter)` - 批量删除
- `count(session)` - 统计数量

### 第 3 层: #[db_permission]

声明允许访问的角色和操作。

```rust
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
}
```

**注意**: `#[db_crud]` 和 `#[db_permission]` 不要同时使用在同一个 struct 上，会导致重复实现错误。

---

## 权限配置

### 配置文件格式 (`permissions.yaml`)

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

### 环境变量

| 变量 | 描述 | 默认值 |
|------|------|--------|
| `DATABASE_URL` | 数据库连接字符串 | 必需 |
| `DB_MAX_CONNECTIONS` | 最大连接数 | 20 |
| `DB_MIN_CONNECTIONS` | 最小连接数 | 5 |
| `DB_IDLE_TIMEOUT` | 空闲超时（秒） | 300 |
| `DB_ACQUIRE_TIMEOUT` | 获取连接超时（毫秒） | 5000 |
| `DB_PERMISSIONS_PATH` | 权限配置文件路径 | `permissions.yaml` |
| `TEST_DB_TYPE` | 测试数据库类型 | `sqlite` |
| `TEST_TIMEOUT_MS` | 测试超时时间（毫秒） | 30000 |

---

## 测试指南

### 测试配置

测试通过环境变量选择数据库类型：

```bash
# SQLite（默认）
export TEST_DB_TYPE=sqlite

# PostgreSQL
export TEST_DB_TYPE=postgres
export DATABASE_URL=postgres://dbnexus:dbnexus_password@localhost:15432/dbnexus_test

# MySQL
export TEST_DB_TYPE=mysql
export DATABASE_URL=mysql://dbnexus:dbnexus_password@localhost:13306/dbnexus_test
```

### 测试辅助函数 (`common/mod.rs`)

```rust
// 获取测试配置
let config = get_test_config();

// 获取 SQLite 内存数据库配置
let config = get_sqlite_memory_config();

// 创建 SQLite 文件数据库配置（推荐用于迁移测试）
let (config, _temp_dir) = get_sqlite_file_config();

// 创建测试夹具（池 + 迁移目录）
let (pool, migrations_dir, _temp_dir) = create_test_fixture().await;

// 获取当前数据库类型
let db_type = get_current_db_type();

// 生成唯一测试表名（避免测试冲突）
let table_name = generate_test_table_name("test_table");

// 创建小容量连接池配置（测试连接耗尽场景）
let config = get_small_pool_config();

// 创建大容量连接池配置（测试高并发场景）
let config = get_large_pool_config();
```

### 集成测试示例

```rust
use dbnexus::DbPool;
mod common;

#[tokio::test]
async fn test_pool_creation() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");
    let status = pool.status();

    assert!(status.total >= 1);
    assert_eq!(status.total, status.active + status.idle);
}
```

---

## 开发流程

### 添加新功能

1. **修改 dbnexus/src/** 添加核心逻辑
2. **修改 dbnexus-macros/src/** 如果需要新宏
3. **更新权限配置** (如适用)
4. **添加测试** 在 `dbnexus/tests/`
5. **更新文档** (如需要)
6. **运行测试**: `cargo test --features <db> -p dbnexus`

### 发布新版本

```bash
# 1. 更新版本号 (Cargo.toml)
# 2. 运行完整测试
make test-all
# 3. 运行 clippy
make clippy-all
# 4. 构建发布版本
cargo build --release --features sqlite
cargo build --release --features postgres
cargo build --release --features mysql
```

---

## 常见问题

### Q: 编译错误 "Cannot enable both 'sqlite' and 'postgres'"

**A**: 数据库特性互斥，只能选择一种：
```toml
# 正确：选择一种
dbnexus = { features = ["sqlite"] }

# 错误：不能同时启用
dbnexus = { features = ["sqlite", "postgres"] }
```

### Q: Session 无法获取连接

**A**: 检查连接池配置和数据库连接字符串：
```rust
// 验证配置
let config = DbConfig::from_env();
println!("URL: {}", config.url);
println!("Max connections: {}", config.max_connections);

// 检查池状态
let status = pool.status();
println!("Total: {}, Active: {}, Idle: {}", status.total, status.active, status.idle);
```

### Q: 权限检查失败

**A**: 确认角色在 `permissions.yaml` 中定义：
```yaml
roles:
  my_role:  # 确保角色名正确
    tables:
      - name: "my_table"
        operations: ["SELECT"]
```

### Q: 测试超时

**A**: 增加测试超时时间：
```bash
export TEST_TIMEOUT_MS=60000
```

---

## 参考资源

- [API 文档](https://docs.rs/dbnexus)
- [Sea-ORM 文档](https://www.sea-ql.org/SeaORM/)
- [用户指南](docs/USER_GUIDE.md)
- [架构文档](docs/ARCHITECTURE.md)
- [API 参考](docs/API_REFERENCE.md)
- [GitHub 仓库](https://github.com/Kirky-X/dbnexus)