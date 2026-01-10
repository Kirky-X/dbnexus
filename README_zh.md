# DBNexus

<div align="center">

**企业级 Rust 数据库抽象层，支持权限控制与连接池管理**

[![Rust 版本](https://img.shields.io/badge/rust-1.85+-blue.svg)](https://www.rust-lang.org)
[![许可证](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-yellow.svg)](https://opensource.org/licenses/MIT)
[![版本](https://img.shields.io/badge/version-0.1.0-green.svg)](https://github.com/Kirky-X/dbnexus)

</div>

## 概述

DBNexus 是基于 Sea-ORM 构建的 Rust 数据库抽象库，提供企业级功能，包括连接池管理、基于角色的访问控制、审计日志、缓存和数据库分片。

## 核心特性

- **连接池管理**: 高效的数据库连接管理，支持配置最小/最大连接数
- **权限引擎**: 基于角色的表级访问控制，支持 YAML 配置文件
- **审计日志**: 完整的审计追踪，支持数据脱敏
- **缓存支持**: LRU 缓存用于权限策略和频繁访问的数据
- **数据库分片**: 水平扩展支持，适用于大规模数据集
- **性能指标**: Prometheus 兼容的指标收集
- **数据库迁移**: 内置迁移管理，支持自动执行
- **分布式追踪**: OpenTelemetry 支持

## 支持的数据库

- SQLite
- PostgreSQL
- MySQL

## 快速开始

### 安装

```toml
[dependencies]
dbnexus = "0.1"
# 选择数据库驱动
dbnexus = { version = "0.1", features = ["sqlite"] }
dbnexus = { version = "0.1", features = ["postgres"] }
dbnexus = { version = "0.1", features = ["mysql"] }
```

### 基本用法

```rust
use dbnexus::{DbPool, DbConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = DbConfig {
        url: "sqlite://example.db".to_string(),
        max_connections: 10,
        min_connections: 2,
        idle_timeout: 300,
        acquire_timeout: 5000,
        permissions_path: None,
        migrations_dir: None,
        auto_migrate: false,
        migration_timeout: 60,
        admin_role: "admin".to_string(),
    };

    let pool = DbPool::with_config(config).await?;
    let session = pool.get_session("admin").await?;

    // 执行查询
    let result = session.execute_raw("SELECT 1").await?;
    println!("查询执行成功");

    Ok(())
}
```

## 配置说明

### 环境变量

| 变量 | 描述 | 默认值 |
|------|------|--------|
| `DB_URL` | 数据库连接 URL | - |
| `DB_MAX_CONNECTIONS` | 连接池最大大小 | 10 |
| `DB_MIN_CONNECTIONS` | 连接池最小大小 | 2 |
| `DB_IDLE_TIMEOUT` | 空闲连接超时（秒） | 300 |
| `DB_ACQUIRE_TIMEOUT` | 获取连接超时（毫秒） | 3000 |
| `DB_ADMIN_ROLE` | DDL 操作的管理员角色 | "admin" |

### 权限配置

创建 `permissions.yaml` 文件：

```yaml
roles:
  admin:
    - table: "*"
      operations: ["SELECT", "INSERT", "UPDATE", "DELETE"]
  user:
    - table: "users"
      operations: ["SELECT", "INSERT"]
    - table: "orders"
      operations: ["SELECT"]
  reader:
    - table: "*"
      operations: ["SELECT"]
```

## 可选功能

| 功能 | 描述 |
|------|------|
| `metrics` | Prometheus 指标收集 |
| `migration` | 数据库迁移支持 |
| `auto-migrate` | 启动时自动迁移 |
| `sharding` | 水平分片支持 |
| `global-index` | 分布式查询全局索引 |
| `cache` | 高级缓存功能 |
| `audit` | 完整审计日志 |
| `tracing` | OpenTelemetry 分布式追踪 |

启用所有可选功能：

```toml
dbnexus = { version = "0.1", features = ["all-optional", "postgres"] }
```

## 项目结构

```
dbnexus/
├── dbnexus/           # 核心库
├── dbnexus-cli/       # CLI 工具
├── dbnexus-macros/    # 过程宏
├── examples/          # 示例代码
├── scripts/           # 构建和实用脚本
└── docs/              # 文档
```

## 示例

查看 [examples](examples/) 目录获取完整示例：

- 基础数据库操作
- 权限配置
- 分片设置
- 审计日志

## 文档

- [API 参考](https://docs.rs/dbnexus)
- [架构指南](docs/ARCHITECTURE.md)
- [用户指南](docs/USER_GUIDE.md)
- [API 参考](docs/API_REFERENCE.md)

## 测试

```bash
# 运行所有测试
cargo test --all

# 运行覆盖率测试
cargo tarpaulin --output-dir ./target/tarpaulin

# 运行特定测试套件
cargo test --package dbnexus --lib
cargo test --package dbnexus-cli
```

## 性能测试

```bash
cargo bench
```

## 贡献

1. Fork 本仓库
2. 创建功能分支
3. 提交更改
4. 推送到分支
5. 发起 Pull Request

## 许可证

本项目基于 MIT 或 Apache-2.0 许可证。

## 作者

DBNexus Team

## 版本

0.1.0

## 联系方式

- 仓库地址: https://github.com/Kirky-X/dbnexus
- 问题反馈: https://github.com/Kirky-X/dbnexus/issues
