# DBNexus 真实数据库测试指南

本文档说明如何使用 Docker 部署真实的 PostgreSQL 和 MySQL 数据库进行测试，而不是使用 mock 或内存数据库。

## 前提条件

- Docker 已安装并运行
- Docker Compose v2 可用

## 快速开始

### 1. 启动数据库容器

```bash
make docker-up
```

或者使用脚本：

```bash
./scripts/test-databases.sh
```

### 2. 运行测试

#### SQLite 测试（内存数据库）

```bash
make test-sqlite
```

#### PostgreSQL 测试（真实数据库）

```bash
make test-postgres
```

#### MySQL 测试（真实数据库）

```bash
make test-mysql
```

#### 运行所有数据库测试

```bash
make test-all
```

### 3. 停止数据库容器

```bash
make docker-down
```

## 数据库连接信息

| 数据库 | 连接字符串 | 端口 |
|--------|-----------|------|
| PostgreSQL | `postgres://dbnexus:dbnexus_password@localhost:15432/dbnexus_test` | 15432 |
| MySQL | `mysql://dbnexus:dbnexus_password@localhost:13306/dbnexus_test` | 13306 |
| SQLite | `sqlite::memory:` | - |

## 测试架构说明

### 测试辅助模块 (`tests/common/mod.rs`)

测试辅助模块提供了统一的数据库配置获取接口：

```rust
pub fn get_test_config() -> DbConfig {
    // 根据环境变量 TEST_DB_TYPE 返回对应的数据库配置
    // 支持的值: "sqlite", "postgres", "mysql"
}
```

### 环境变量

- `TEST_DB_TYPE`: 指定测试数据库类型（sqlite/postgres/mysql）
- `DATABASE_URL`: 指定数据库连接字符串（可选，有默认值）

### 测试文件修改

所有集成测试文件都已修改为使用测试辅助模块：

- `tests/session_transaction.rs`
- `tests/pool_integration.rs`
- `tests/permission_integration.rs`

## Docker Compose 服务

### PostgreSQL

- 镜像: `postgres:16-alpine`
- 端口: `15432:5432`
- 数据库名: `dbnexus_test`
- 用户名: `dbnexus`
- 密码: `dbnexus_password`

### MySQL

- 镜像: `mysql:8.0`
- 端口: `13306:3306`
- 数据库名: `dbnexus_test`
- 用户名: `dbnexus`
- 密码: `dbnexus_password`

### Adminer

- 镜像: `adminer:latest`
- 端口: `8080:8080`
- 用途: 数据库管理界面

## 数据库初始化

### PostgreSQL 初始化脚本

`scripts/init-postgres.sql` 创建以下表：

- `test_users`: 用户表
- `test_accounts`: 账户表（外键关联 test_users）
- `test_orders`: 订单表（外键关联 test_users）

### MySQL 初始化脚本

`scripts/init-mysql.sql` 创建相同的表结构。

## CI/CD 集成

在 CI/CD 环境中，可以使用以下命令：

```bash
# 启动数据库
docker compose up -d

# 等待数据库就绪
sleep 15

# 运行测试
make test-all

# 停止数据库
docker compose down
```

## 故障排查

### 端口冲突

如果遇到端口冲突，可以修改 `docker-compose.yml` 中的端口映射：

```yaml
ports:
  - "新端口:容器端口"
```

### 数据库连接失败

1. 检查容器状态：`docker compose ps`
2. 查看日志：`docker compose logs`
3. 验证连接：`docker exec dbnexus-postgres pg_isready -U dbnexus`

### Docker 权限问题

如果遇到权限问题，可能需要使用 sudo：

```bash
sudo docker compose up -d
```

## 测试覆盖率

当前测试覆盖以下功能：

- Session 管理
- 事务操作（begin/commit/rollback）
- 连接池管理
- 权限检查
- 跨数据库兼容性

## 下一步

- 添加更多集成测试用例
- 实现性能基准测试
- 添加数据迁移测试