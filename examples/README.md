# DBNexus Examples

本目录包含 DBNexus 的所有功能示例，帮助您快速了解和使用 DBNexus 的各项特性。

**注意**：这是一个独立的 Rust 项目，需要进入 `examples/` 目录后运行。

## 📋 目录

- [快速开始](#快速开始)
- [核心功能](#核心功能)
- [企业功能](#企业功能)
- [高级功能](#高级功能)
- [运行示例](#运行示例)

## 快速开始

### [quickstart.rs](quickstart.rs)
DBNexus 基础入门示例，展示最核心的功能：
- 定义 Entity 并自动生成 CRUD 方法
- 创建数据库连接池
- 获取 Session 执行数据库操作

```bash
cargo run --example quickstart --features sqlite
```

## 核心功能

### [config.rs](config.rs)
配置管理示例，展示多种配置初始化方式：
- 使用 DbConfigBuilder 创建配置
- 使用配置结构体创建配置
- 从配置文件加载配置
- 使用环境变量配置

```bash
cargo run --example config --features sqlite
```

### [permissions.rs](permissions.rs)
权限控制示例，展示基于角色的访问控制：
- 定义带权限控制的 Entity
- 使用 Session 执行权限检查
- 测试不同角色的访问权限
- 编译时角色验证

```bash
cargo run --example permissions --features sqlite
```

### [transactions.rs](transactions.rs)
事务管理示例，展示事务的完整使用流程：
- 使用 begin/commit/rollback 管理事务
- 验证事务的原子性
- 处理事务失败和回滚
- 转账场景演示

```bash
cargo run --example transactions --features sqlite
```

### [sql_parser.rs](sql_parser.rs)
SQL 解析器示例，展示 SQL 语句解析能力：
- 提取 SQL 操作类型
- 提取目标表名
- 解析 SQL 参数
- 支持多种 SQL 语句类型

```bash
cargo run --example sql_parser --features "sqlite,sql-parser"
```

### [permission_engine.rs](permission_engine.rs)
权限引擎示例，展示高级权限管理功能：
- 使用权限引擎进行权限检查
- 定义自定义权限规则
- 权限缓存优化
- 复杂权限场景处理

```bash
cargo run --example permission_engine --features "sqlite,permission-engine"
```

### [entity_basic.rs](entity_basic.rs)
实体操作基础示例，展示 DbEntity 和 db_crud 宏的使用：
- 使用 #[derive(DbEntity, DeriveEntityModel)] 定义实体
- 使用 #[db_crud] 自动生成 CRUD 方法
- 执行 insert / find_by_id / update / delete / find_all 操作

```bash
cargo run --example entity_basic --features sqlite,macros
```

### [security.rs](security.rs)
安全功能示例，展示 DDL 安全守卫和敏感数据脱敏：
- DdlGuard: DDL 语句安全验证，防止 DROP DATABASE/TRUNCATE 等危险操作
- SensitiveMasker: 敏感数据脱敏，支持手机号、邮箱、身份证、银行卡、姓名、地址
- SQL 注入防护测试
- 错误处理和边界情况

```bash
cargo run --example security --features sqlite
```

## 企业功能

### [metrics.rs](metrics.rs)
Prometheus 指标监控示例：
- 配置 Prometheus 指标收集器
- 收集数据库操作指标
- 导出 Prometheus 格式指标
- 查询和监控指标数据

```bash
cargo run --example metrics --features "sqlite,metrics"
```

### [tracing.rs](tracing.rs)
OpenTelemetry 分布式追踪示例：
- 配置 OpenTelemetry 追踪
- 追踪数据库操作
- 导出追踪数据到 Jaeger
- 分析性能瓶颈

```bash
cargo run --example tracing --features "sqlite,tracing"
```

### [audit.rs](audit.rs)
审计日志示例：
- 配置审计日志记录器
- 记录 CRUD 操作审计
- 使用 #[db_audit] 宏自动审计
- 查询和导出审计日志

```bash
cargo run --example audit --features "sqlite,audit"
```

## 高级功能

### [cache.rs](cache.rs)
缓存使用示例：
- 创建和管理缓存管理器
- 使用 LRU 缓存策略
- 配置 TTL 过期时间
- 防止缓存穿透和击穿
- 使用 #[db_cache] 宏自动缓存

```bash
cargo run --example cache --features "sqlite,cache"
```

### [sharding.rs](sharding.rs)
分片管理示例：
- 配置分片路由器
- 使用不同的分片策略（年、月、日、哈希）
- 管理多个数据库分片
- 跨分片查询
- 分片数据迁移和均衡

```bash
cargo run --example sharding --features "sqlite,sharding"
```

### [migration.rs](migration.rs)
数据库迁移示例：
- 创建迁移文件
- 应用数据库迁移
- 回滚迁移
- 自动迁移功能

```bash
cargo run --example migration --features "sqlite,migration"
```

### [global_index.rs](global_index.rs)
全局索引示例：
- 创建跨分片全局唯一索引
- 使用全局索引查询
- 全局索引约束管理
- 性能优化

```bash
cargo run --example global_index --features "sqlite,global-index"
```

## 运行示例

### 基本运行

**首先进入 examples 目录**：

```bash
cd examples
```

然后可以使用以下方式运行示例：

#### 方式 1：交互式菜单

```bash
cargo run
```

这会显示一个交互式菜单，让您选择要运行的示例。

#### 方式 2：直接运行特定示例

```bash
cargo run --bin quickstart
cargo run --bin permissions
cargo run --bin metrics
```

#### 方式 3：通过命令行参数

```bash
cargo run -- quickstart
cargo run -- permissions
cargo run -- metrics
```

### Feature Flags

本项目已默认启用所有必要的 features，包括：

#### 数据库驱动
- `sqlite` - SQLite 数据库（默认）

#### 核心特性
- `permission` - 权限控制
- `sql-parser` - SQL 解析器

#### 企业特性
- `metrics` - Prometheus 指标
- `tracing` - OpenTelemetry 追踪
- `audit` - 审计日志
- `permission-engine` - 权限引擎

#### 高级特性
- `cache` - 缓存
- `sharding` - 数据分片
- `migration` - 数据库迁移
- `global-index` - 全局索引

### 快速测试所有示例

```bash
# 检查所有示例编译
cargo build --all-targets

# 运行特定示例
cargo run --bin <example_name>
```

### 使用不同的数据库

如需使用 PostgreSQL 或 MySQL，请修改 `Cargo.toml` 中的 features：

```toml
[dependencies]
dbnexus = { path = "..", features = [
    "postgres",  # 或 "mysql"
    # ... 其他 features
] }
```

## 示例文件结构

每个示例文件都遵循统一的结构：

```rust
// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 示例标题
//!
//! 示例描述
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example <name> --features <features>
//! ```

use dbnexus::{...};

// 示例代码
```

## 获取帮助

如果您在使用示例时遇到问题：

1. 查看示例文件中的注释和文档
2. 查看 [用户指南](../docs/USER_GUIDE.md)
3. 查看 [API 文档](../docs/API_REFERENCE.md)
4. 在 [GitHub Issues](https://github.com/Kirky-X/dbnexus/issues) 提问

## 贡献

欢迎贡献新的示例！请确保：

1. 遵循现有的示例文件结构
2. 添加清晰的注释和文档
3. 包含运行说明和所需的 feature flags
4. 确保示例可以正确编译和运行
