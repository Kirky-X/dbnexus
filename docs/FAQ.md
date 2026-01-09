# DB Nexus 常见问题解答 (FAQ)

## 目录

- [安装问题](#安装问题)
- [配置问题](#配置问题)
- [功能使用问题](#功能使用问题)
- [性能问题](#性能问题)
- [故障排除](#故障排除)

---

## 安装问题

### Q1: 编译错误 "Cannot enable both 'sqlite' and 'postgres' features"

**问题**:
```error
error: Cannot enable both 'sqlite' and 'postgres' features
```

**原因**: 数据库特性互斥，只能选择一种。

**解决方案**:
```toml
# 只能选择一种数据库
[dependencies]
dbnexus = { version = "0.1.0", features = ["sqlite"] }

# PostgreSQL
# dbnexus = { version = "0.1.0", features = ["postgres"] }

# MySQL
# dbnexus = { version = "0.1.0", features = ["mysql"] }
```

### Q2: 找不到 `dbnexus` crate

**问题**:
```error
error: could not find `dbnexus` in `crates.io`
```

**原因**: crate 名称错误或未发布。

**解决方案**:
```toml
# 检查 crate 名称
[dependencies]
dbnexus = "0.1.0"

# 或者从 Git 安装
[dependencies]
dbnexus = { git = "https://github.com/Kirky-X/dbnexus", tag = "v0.1.0" }
```

### Q3: Rust 版本不兼容

**问题**:
```error
error: package `dbnexus v0.1.0` requires Rust version 1.85.0
```

**原因**: 当前 Rust 版本低于项目要求。

**解决方案**:
```bash
# 检查当前版本
rustc --version

# 更新 Rust
rustup update stable

# 或者安装特定版本
rustup install 1.85.0
rustup default 1.85.0
```

### Q4: 依赖下载失败

**问题**:
```error
error: failed to fetch `https://github.com/...`
```

**原因**: 网络问题或 Git 配置问题。

**解决方案**:
```bash
# 配置 Git
git config --global url."https://".insteadOf git://

# 清除缓存重新下载
cargo clean
cargo fetch
```

---

## 配置问题

### Q5: DATABASE_URL 格式错误

**问题**:
```error
DbError: ConfigError("Invalid DATABASE_URL format")
```

**原因**: 连接字符串格式不正确。

**解决方案**:

```bash
# SQLite
export DATABASE_URL=sqlite:./data.db
# 或
export DATABASE_URL=sqlite::memory:

# PostgreSQL
export DATABASE_URL=postgres://user:password@localhost:5432/dbname

# MySQL
export DATABASE_URL=mysql://user:password@localhost:3306/dbname
```

### Q6: 连接池配置不生效

**问题**: 修改 `DB_MAX_CONNECTIONS` 后没有效果。

**解决方案**:
```rust
use dbnexus::{DbPool, PoolConfig};

let pool_config = PoolConfig::new()
    .max_connections(100)  // 显式设置
    .min_connections(10)
    .idle_timeout(600)
    .acquire_timeout(10000);

let pool = DbPool::with_config(pool_config).await?;
```

### Q7: 环境变量不读取

**问题**: 设置的环境变量没有被读取。

**解决方案**:
```rust
use dbnexus::DbConfig;

let config = DbConfig::from_env()?;

println!("URL: {}", config.url);
println!("Max connections: {}", config.max_connections);
```

检查环境变量是否设置：
```bash
echo $DATABASE_URL
```

### Q8: 权限配置文件路径

**问题**: 无法找到权限配置文件。

**解决方案**:
```bash
# 设置权限配置文件路径
export DB_PERMISSIONS_PATH=/path/to/permissions.yaml

# 或使用绝对路径
```

```rust
let provider = YamlPermissionProvider::new("/absolute/path/permissions.yaml")?;
```

---

## 功能使用问题

### Q9: #[db_crud] 和 #[db_permission] 冲突

**问题**:
```error
error[E0119]: conflicting implementations
```

**原因**: 同时使用两个宏导致重复实现。

**解决方案**:
```rust
// 使用 #[db_crud]（包含内置权限检查）
#[derive(DbEntity)]
#[db_entity]
#[table_name = "users")]
#[db_crud]  // 包含权限检查
struct User {
    #[primary_key]
    id: i64,
    name: String,
}

// 或自定义权限（不使用 #[db_crud]）
#[derive(DbEntity)]
#[db_entity]
#[table_name = "users")]
#[db_permission(role = "admin", actions = ["read", "write", "delete"])]
struct UserData {
    #[primary_key]
    id: i64,
    // 手动实现 CRUD 或使用 API
}
```

### Q10: 宏不生成代码

**问题**: 宏没有生成预期的方法。

**解决方案**:
1. 确保启用了必要的特性：
```toml
[dependencies]
dbnexus = { version = "0.1.0", features = ["sqlite"] }
```

2. 检查宏导入：
```rust
use dbnexus::{DbEntity, db_crud, db_permission};
```

3. 确保 struct 定义正确：
```rust
#[derive(DbEntity)]
#[db_entity]
#[table_name = "users")]
#[db_crud]
struct User {
    #[primary_key]
    id: i64,
}
```

### Q11: 事务不生效

**问题**: 事务回滚没有生效。

**解决方案**:
```rust
let result = session.transaction(|s| {
    // 操作1
    s.execute_raw("INSERT INTO ...![])?;

    // 操作2 - 模拟错误", vec
    if condition {
        return Err(DbError::TransactionError("custom error".into()));
    }

    Ok(())
}).await;

match result {
    Ok(_) => println!("事务提交"),
    Err(e) => {
        println!("事务自动回滚: {}", e);
    }
}
```

### Q12: 缓存没有生效

**问题**: 启用缓存但没有看到缓存效果。

**解决方案**:
1. 确保启用了 `cache` 特性：
```toml
[dependencies]
dbnexus = { version = "0.1.0", features = ["sqlite", "cache"] }
```

2. 为实体添加缓存属性：
```rust
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

3. 检查缓存命中情况：
```rust
let status = pool.status();
```

### Q13: 审计日志不记录

**问题**: 启用审计但没有日志。

**解决方案**:
1. 确保启用了 `audit` 特性：
```toml
[dependencies]
dbnexus = { version = "0.1.0", features = ["sqlite", "audit"] }
```

2. 为实体添加审计属性：
```rust
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

3. 检查操作类型是否匹配：
```rust
// CREATE, READ, UPDATE, DELETE
#[db_audit(operations = ["CREATE", "UPDATE", "DELETE"])]
```

### Q14: 分片路由不正确

**问题**: 分片路由返回错误的分片。

**解决方案**:
```rust
use dbnexus::sharding::{ShardRouter, ShardConfig, HashStrategy};

// 配置哈希分片
let config = ShardConfig::new(
    "hash",                    // 分片名称
    4,                         // 分片数量
    "orders",                  // 表名
    "postgresql://localhost/shard_{shard}"  // 连接模板
);

// 使用哈希策略
let router = ShardRouter::with_config(&config);

// 根据用户 ID 路由
let shard_id = router.route_by_hash(&user_id.to_le_bytes());
```

---

## 性能问题

### Q15: 查询速度慢

**问题**: 查询响应时间长。

**解决方案**:
1. **使用缓存**：
```rust
#[db_cache(ttl = 3600, capacity = 10000)]
struct Product { /* ... */ }
```

2. **添加索引**：
```sql
CREATE INDEX idx_user_email ON users(email);
```

3. **优化连接池**：
```rust
let pool_config = PoolConfig::new()
    .max_connections(100)
    .min_connections(10);
```

4. **使用只读副本**：
```rust
let read_session = pool.get_session("reader").await?;
```

### Q16: 连接池耗尽

**问题**: `PoolExhausted` 错误。

**解决方案**:
1. **增加连接池大小**：
```rust
let pool_config = PoolConfig::new()
    .max_connections(200);
```

2. **减少连接占用时间**：
```rust
// 快速释放连接
let _ = session; // 使用后让 Session 释放
```

3. **增加获取超时**：
```rust
let pool_config = PoolConfig::new()
    .acquire_timeout(30000);  // 30秒
```

4. **检查连接泄漏**：
```rust
let status = pool.status();
println!("等待任务: {}", status.waiters);
```

### Q17: 内存使用过高

**问题**: 内存使用持续增长。

**解决方案**:
1. **减少缓存容量**：
```rust
#[db_cache(ttl = 300, capacity = 100)]  // 减小容量
struct User { /* ... */ }
```

2. **设置连接池上限**：
```rust
let pool_config = PoolConfig::new()
    .max_connections(50);
```

3. **监控内存使用**：
```rust
let status = pool.status();
println!("总连接数: {}", status.total);
```

---

## 故障排除

### Q18: 测试超时

**问题**: 测试运行超时。

**解决方案**:
```bash
# 增加测试超时
export TEST_TIMEOUT_MS=60000

# 或在测试中
#[tokio::test(timeout = 60)]
async fn test_name() { /* ... */ }
```

### Q19: 数据库迁移失败

**问题**: `MigrationError` 错误。

**解决方案**:
```rust
// 检查迁移目录
let migrations_dir = "./migrations";

// 使用测试夹具
let (pool, migrations_dir, _temp_dir) = create_test_fixture().await;

// 查看详细错误
println!("{:?}", error);
```

### Q20: SSL/TLS 连接问题

**问题**: 数据库 SSL 连接失败。

**解决方案**:
```bash
# PostgreSQL SSL
export DATABASE_URL=postgres://user:pass@host/db?sslmode=require

# MySQL SSL
export DATABASE_URL=mysql://user:pass@host/db?ssl-mode=REQUIRED
```

### Q21: 权限检查失败

**问题**: `PermissionDenied` 错误。

**解决方案**:
1. **检查角色配置**：
```yaml
# permissions.yaml
roles:
  admin:
    tables:
      - name: "users"
        operations:
          - SELECT
```

2. **验证角色名称**：
```rust
let session = pool.get_session("admin").await?;
// 确认 "admin" 角色存在
```

3. **打印权限调试**：
```rust
let provider = YamlPermissionProvider::new("permissions.yaml")?;
println!("{:?}", provider.get_role_permissions("admin"));
```

### Q22: 编译警告视为错误

**问题**: 所有警告导致编译失败。

**解决方案**:
```bash
# 仅检查错误（开发环境）
cargo check

# 或临时禁用警告
cargo clippy --all-features --all -- -A warnings
```

### Q23: 找不到测试文件

**问题**: 测试文件无法运行。

**解决方案**:
```bash
# 运行特定测试文件
cargo test --test pool_integration --features sqlite

# 列出所有测试
cargo test --features sqlite -- --list
```

### Q24: Docker 数据库连接失败

**问题**: 无法连接到 Docker 中的数据库。

**解决方案**:
```bash
# 检查容器状态
docker ps

# 查看容器日志
docker logs container_name

# 检查网络
docker network ls

# 重新启动容器
docker-compose restart
```

### Q25: 与其他 crate 冲突

**问题**: 与其他依赖冲突。

**解决方案**:
```toml
# 使用 Cargo.lock 解决
cargo update -p sea-orm --precise x.x.x

# 或在 Cargo.toml 中指定版本
[dependencies]
sea-orm = { version = "=0.12.0" }
```

---

## 获取更多帮助

### 报告问题

- **GitHub Issues**: [报告新问题](https://github.com/Kirky-X/dbnexus/issues/new)
- **请包含**:
  - 错误信息
  - 复现步骤
  - 环境信息（Rust 版本、操作系统等）

### 社区支持

- **GitHub Discussions**: [提问和讨论](https://github.com/Kirky-X/dbnexus/discussions)
- **Gitter**: 实时聊天室（如果有）

### 文档资源

- [API 文档](https://docs.rs/dbnexus)
- [用户指南](USER_GUIDE.md)
- [架构文档](ARCHITECTURE.md)
- [Sea-ORM 文档](https://www.sea-ql.org/SeaORM/)

---

## 快速参考

### 常用命令

```bash
# 检查编译
cargo check --all-features

# 运行测试
cargo test --features sqlite --all

# 格式化代码
cargo fmt --all

# Clippy 检查
cargo clippy --all-features --all -- -D warnings

# 构建文档
cargo doc --no-deps --all-features
```

### 环境变量

| 变量 | 描述 | 默认值 |
|------|------|--------|
| `DATABASE_URL` | 数据库连接字符串 | - |
| `DB_MAX_CONNECTIONS` | 最大连接数 | 20 |
| `DB_MIN_CONNECTIONS` | 最小连接数 | 5 |
| `DB_IDLE_TIMEOUT` | 空闲超时（秒） | 300 |
| `DB_ACQUIRE_TIMEOUT` | 获取超时（毫秒） | 5000 |
| `DB_PERMISSIONS_PATH` | 权限文件路径 | `permissions.yaml` |
| `TEST_DB_TYPE` | 测试数据库类型 | `sqlite` |
| `TEST_TIMEOUT_MS` | 测试超时（毫秒） | 30000 |
