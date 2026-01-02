<div align="center">

# DB Nexus

<p>
  <a href="https://github.com/Kirky-X/dbnexus/actions/workflows/ci.yml"><img src="https://github.com/Kirky-X/dbnexus/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-yellow.svg" alt="License"></a>
  <a href="https://crates.io/crates/dbnexus"><img src="https://img.shields.io/crates/v/dbnexus.svg" alt="Crates.io"></a>
  <a href="https://docs.rs/dbnexus"><img src="https://docs.rs/dbnexus/badge.svg" alt="Documentation"></a>
  <a href="https://crates.io/crates/dbnexus"><img src="https://img.shields.io/crates/d/dbnexus.svg" alt="Downloads"></a>
</p>

<p align="center">
  <strong>DB Nexus 是一个企业级数据库抽象层，基于 Sea-ORM 构建，提供高性能、高安全性的 Rust 数据库访问解决方案。</strong>
</p>

<p align="center">
  <a href="#-features">特性</a> •
  <a href="#-quick-start">快速开始</a> •
  <a href="#-documentation">文档</a> •
  <a href="#-examples">示例</a> •
  <a href="#-contributing">贡献</a>
</p>

</div>

---

## ✨ 特性

### 核心特性

- **多数据库支持**: 通过 feature gate 支持 SQLite、PostgreSQL、MySQL
- **Session 机制**: RAII 自动管理数据库连接生命周期
- **权限控制**: 声明式宏自动生成权限检查代码
- **连接池管理**: 动态配置修正与健康检查
- **监控指标**: Prometheus 指标导出
- **Migration 工具**: 自动化 Schema 变更管理
- **分片支持**: 支持水平分片和全局索引
- **缓存层**: 可插拔的缓存抽象
- **审计日志**: 完整的操作审计追踪
- **可插拔权限引擎**: 支持自定义权限策略

---

## 🎯 使用场景

- **企业级应用**: 需要严格权限控制的大型系统
- **微服务架构**: 多数据库、多租户场景
- **高并发系统**: 需要连接池和缓存优化
- **审计要求**: 需要完整操作日志的系统
- **数据敏感**: 需要细粒度权限控制的应用

---

## 🚀 快速开始

### 安装

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
dbnexus = { version = "0.1.0", features = ["sqlite"] }
```

### 基本使用

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
    let pool = DbPool::new("sqlite::memory:").await?;
    let session = pool.get_session("admin").await?;
    
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    
    User::insert(&session, user).await?;
    Ok(())
}
```

### 运行示例

```bash
# 快速开始示例
cargo run --example quickstart --features sqlite

# 权限控制示例
cargo run --example permissions --features sqlite

# 事务示例
cargo run --example transactions --features sqlite
```

---

## 📚 文档

- [用户指南](docs/USER_GUIDE.md) - 详细的使用说明和最佳实践
- [API 文档](docs/API_REFERENCE.md) - 完整的 API 参考
- [架构文档](docs/ARCHITECTURE.md) - 系统架构和设计决策
- [常见问题](docs/FAQ.md) - 常见问题解答
- [贡献指南](docs/CONTRIBUTING.md) - 如何参与项目贡献

---

## 🎨 宏 vs API 使用指南

### 宏的使用场景

**适用场景**：
- 简单的 CRUD 操作（insert、delete）
- 需要自动生成 SQL 语句
- 代码简洁性优先

**示例**：
```rust
use dbnexus::{DbPool, DbEntity, db_crud};

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
    let pool = DbPool::new("sqlite::memory:").await?;
    let session = pool.get_session("admin").await?;
    
    // 使用宏生成的 insert 方法
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    User::insert(&session, user).await?;
    
    // 使用宏生成的 delete 方法
    User::delete(&session, 1).await?;
    
    Ok(())
}
```

**宏的限制**：
- 无法解析查询结果返回实体
- 不支持复杂查询（JOIN、子查询等）
- 不支持事务操作

### API 的使用场景

**适用场景**：
- 复杂的查询操作（JOIN、子查询、聚合）
- 需要获取查询结果
- 事务操作
- 需要完整的灵活性

**示例**：
```rust
use dbnexus::{DbPool, DbEntity, db_crud};

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
    let pool = DbPool::new("sqlite::memory:").await?;
    let session = pool.get_session("admin").await?;
    
    // 使用 API 执行复杂查询
    session.execute_raw("SELECT u.*, COUNT(o.id) as order_count FROM users u LEFT JOIN orders o ON u.id = o.user_id GROUP BY u.id").await?;
    
    // 使用 API 执行事务
    session.transaction(|s| {
        s.execute_raw("INSERT INTO users ...")?;
        s.execute_raw("UPDATE orders ...")?;
        Ok(())
    }).await?;
    
    Ok(())
}
```

### 推荐使用方式

**1. 简单 CRUD**：使用宏 + API
```rust
// 使用宏的 insert/delete 方法
User::insert(&session, user).await?;
User::delete(&session, 1).await?;

// 使用 API 的查询方法
session.execute_raw("SELECT * FROM users WHERE id = 1").await?;
```

**2. 复杂查询**：直接使用 API
```rust
session.execute_raw("SELECT u.*, COUNT(o.id) FROM users u LEFT JOIN orders o ON u.id = o.user_id GROUP BY u.id").await?;
```

**3. 事务操作**：使用 API
```rust
session.transaction(|s| {
    s.execute_raw("INSERT INTO users ...")?;
    s.execute_raw("UPDATE inventory SET count = count - 1 WHERE id = ?")?;
    s.execute_raw("INSERT INTO orders ...")?;
    Ok(())
}).await?;
```

### 功能对比

| 功能 | 宏 | API |
|-----|-----|-----|
| **权限检查** | ✅ 自动 | ✅ 自动 |
| **数据库操作** | ✅ 自动 | ✅ 手动 |
| **SQL 生成** | ✅ 自动 | ❌ 手动 |
| **结果解析** | ❌ 不支持 | ❌ 不支持 |
| **事务管理** | ❌ 不支持 | ✅ 支持 |
| **复杂查询** | ❌ 不支持 | ✅ 支持 |

---

## 🎨 示例

### 快速开始示例

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
    let pool = DbPool::new("sqlite::memory:").await?;
    let session = pool.get_session("admin").await?;
    
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    
    let inserted = User::insert(&session, user).await?;
    println!("插入用户: {}", inserted.name);
    
    Ok(())
}
```

### 权限控制示例

```rust
use dbnexus::{DbEntity, db_permission, db_crud};

#[derive(DbEntity)]
#[db_entity]
#[table_name = "users")]
#[db_permission(role = "admin", actions = ["read", "write", "delete"])]
#[db_permission(role = "user", actions = ["read"])]
#[db_crud]
struct User {
    #[primary_key]
    id: i64,
    name: String,
    email: String,
}
```

<div align="center">

**[查看更多示例 →](examples/)**

</div>

---

## 🏗️ 项目结构

```
dbnexus/
├── dbnexus/              # 核心库
│   ├── src/
│   │   ├── lib.rs       # 库入口
│   │   ├── pool.rs      # 连接池管理
│   │   ├── session.rs   # Session 机制
│   │   ├── permission.rs # 权限控制
│   │   ├── config.rs    # 配置管理
│   │   ├── cache.rs     # 缓存层
│   │   ├── audit.rs     # 审计日志
│   │   ├── sharding.rs  # 分片支持
│   │   ├── global_index.rs # 全局索引
│   │   ├── metrics.rs   # 监控指标
│   │   ├── migration.rs # Migration 工具
│   │   ├── tracing.rs   # 分布式追踪
│   │   ├── permission_engine.rs # 可插拔权限引擎
│   │   ├── entity.rs    # 实体转换
│   │   └── generated_roles.rs # 生成的权限角色
│   └── tests/           # 集成测试
│       ├── pool_integration.rs
│       ├── permission_integration.rs
│       ├── cache_integration.rs
│       ├── audit_integration.rs
│       ├── sharding_integration.rs
│       ├── migration_integration.rs
│       ├── multi_db_integration.rs
│       ├── session_transaction.rs
│       ├── cli_integration.rs
│       └── concurrency_integration.rs
├── dbnexus-macros/      # 过程宏
│   └── src/
│       └── lib.rs       # 宏定义
├── dbnexus-cli/         # CLI 工具
│   └── src/
│       └── main.rs      # CLI 入口
├── examples/            # 示例代码
│   ├── quickstart.rs    # 快速开始
│   ├── permissions.rs   # 权限控制
│   └── transactions.rs  # 事务处理
├── docs/                # 文档
│   ├── USER_GUIDE.md    # 用户指南
│   ├── API_REFERENCE.md  # API 参考
│   ├── ARCHITECTURE.md  # 架构文档
│   ├── FAQ.md           # 常见问题
│   ├── CONTRIBUTING.md  # 贡献指南
│   ├── prd.md           # 产品需求文档
│   ├── task.md          # 任务文档
│   ├── tdd.md           # TDD 指南
│   ├── test.md          # 测试文档
│   └── uat.md           # 用户验收测试
├── scripts/             # 脚本工具
│   ├── init-sqlite.sql
│   ├── init-mysql.sql
│   ├── init-postgres.sql
│   ├── generate-sql.sh
│   └── test-databases.sh
├── Cargo.toml           # Workspace 配置
├── Cargo.lock           # 依赖锁定
├── Makefile             # 构建脚本
├── rustfmt.toml         # 代码格式化配置
├── deny.toml            # 依赖审计配置
└── tarpaulin.toml       # 测试覆盖率配置
```

---

## ⚙️ 配置

### 基本配置

```toml
[dependencies]
dbnexus = { version = "0.1.0", features = ["sqlite", "cache", "audit"] }
```

### Feature 选项

| Feature | 描述 | 默认 |
|---------|------|------|
| `sqlite` | SQLite 数据库支持 | - |
| `postgres` | PostgreSQL 数据库支持 | - |
| `mysql` | MySQL 数据库支持 | - |
| `cache` | 缓存层支持 | false |
| `audit` | 审计日志支持 | false |
| `sharding` | 分片支持 | false |
| `global-index` | 全局索引支持 | false |
| `metrics` | Prometheus 指标导出 | false |
| `migration` | Migration 工具 | false |
| `permission-engine` | 可插拔权限引擎 | false |
| `tracing` | 分布式追踪支持 | false |

**注意**: 数据库特性（sqlite、postgres、mysql）互斥，只能选择一个。

---

## 🧪 测试

```bash
# 运行所有测试
cargo test --all-features

# 运行特定测试
cargo test pool_integration --features sqlite

# 运行测试并生成覆盖率报告
cargo tarpaulin --out Html --all-features

# 运行集成测试
cargo test --test '*' --all-features
```

### 测试覆盖

| 测试类型 | 测试文件 | 覆盖内容 |
|---------|---------|---------|
| 连接池测试 | pool_integration.rs | 连接池创建、获取、健康检查 |
| 权限测试 | permission_integration.rs | 权限检查、角色管理 |
| 缓存测试 | cache_integration.rs | 缓存读写、失效策略 |
| 审计测试 | audit_integration.rs | 审计日志记录、查询 |
| 分片测试 | sharding_integration.rs | 分片路由、全局索引 |
| Migration 测试 | migration_integration.rs | Schema 变更、版本管理 |
| 多数据库测试 | multi_db_integration.rs | 多数据库连接、事务 |
| Session 测试 | session_transaction.rs | Session 生命周期、事务 |
| CLI 测试 | cli_integration.rs | 命令行工具功能 |
| 并发测试 | concurrency_integration.rs | 并发安全、锁竞争 |

---

## 📊 性能

### 基准测试

```bash
# 运行基准测试
cargo bench
```

### 性能特性

- **零拷贝**: 使用 Rust 的所有权系统避免不必要的拷贝
- **异步 I/O**: 基于 Tokio 的异步运行时
- **连接池**: 高效的连接复用和管理
- **缓存**: LRU 缓存减少数据库访问
- **批量操作**: 支持批量插入和更新

---

## 🔒 安全

### 安全特性

- **编译时安全**: Rust 的类型系统和借用检查器
- **权限控制**: 基于角色的表级权限控制
- **审计日志**: 完整的操作审计追踪
- **SQL 注入防护**: 使用参数化查询
- **连接安全**: 支持 TLS 加密连接

### 安全最佳实践

1. 始终使用参数化查询
2. 启用审计日志记录关键操作
3. 使用最小权限原则配置角色
4. 定期更新依赖版本
5. 在生产环境使用 TLS 加密

---

## 🗺️ 路线图

### v0.1.0 (当前版本)

- [x] 多数据库支持
- [x] Session 机制
- [x] 权限控制
- [x] 连接池管理
- [x] 基础缓存
- [x] 审计日志
- [x] Migration 工具
- [x] 基础文档和示例

### v0.2.0 (计划中)

- [ ] 高级分片策略
- [ ] 分布式事务支持
- [ ] 更多数据库驱动
- [ ] 性能优化
- [ ] 更多示例和教程

### v1.0.0 (未来)

- [ ] 完整的插件系统
- [ ] 多语言绑定
- [ ] 企业级特性
- [ ] 云原生支持

---

## 🤝 贡献

我们欢迎所有形式的贡献！

### 如何贡献

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

### 开发指南

- 遵循 Rust 代码规范
- 编写单元测试和集成测试
- 更新相关文档
- 确保 CI 通过

详见 [贡献指南](docs/CONTRIBUTING.md)

---

## 📄 许可证

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

本项目采用 MIT 许可证。

---

## 🙏 致谢

本项目基于以下优秀的开源项目：

- [Sea-ORM](https://github.com/SeaQL/sea-orm) - 异步 ORM 框架
- [Tokio](https://tokio.rs/) - 异步运行时
- [Serde](https://serde.rs/) - 序列化/反序列化框架
- [Prometheus](https://prometheus.io/) - 监控指标系统

感谢所有贡献者的支持！

---

## 📞 联系方式

- **GitHub Issues**: [报告问题](https://github.com/dbnexus/dbnexus/issues)
- **GitHub Discussions**: [参与讨论](https://github.com/dbnexus/dbnexus/discussions)
- **文档**: [docs.rs/dbnexus](https://docs.rs/dbnexus)

---

<div align="center">

### 如果这个项目对您有帮助，请给我们一个 ⭐️！

**Built with ❤️ by DB Nexus Team**

[⬆ 返回顶部](#db-nexus)

---

<sub>© 2025 DB Nexus Team. All rights reserved.</sub>

</div>
