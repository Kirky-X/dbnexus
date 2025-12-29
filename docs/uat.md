# ✅ UAT - User Acceptance Testing Document

| 版本 | 日期 | 作者 | 变更内容 |
| --- | --- | --- | --- |
| v1.0 | 2025-01-01 | QA Manager | 初始版本 |
| v1.1 | 2025-01-15 | QA Manager | 根据修正文档更新: 统一权限配置格式, 宏使用示例 |

## 1. 验收测试概述

### 1.1 测试目标

验证DB模块在真实业务场景下满足所有功能和性能需求,确保系统可以安全上线生产环境。

### 1.2 测试环境

| 环境类型        | 配置                                       |
| --------------- | ------------------------------------------ |
| **数据库**      | PostgreSQL 14.2, MySQL 8.0.32, SQLite 3.40 |
| **硬件**        | 4核CPU, 8GB RAM                            |
| **网络**        | 本地网络延迟 < 1ms                         |
| **Rust版本**    | 1.75+                                      |
| **Sea-ORM版本** | 1.0+                                       |

### 1.3 验收标准

- ✅ 所有测试用例通过率 100%
- ✅ 性能指标满足预期
- ✅ 无阻塞性Bug
- ✅ 文档完整且可用

------

## 2. 功能验收测试

### 2.1 快速上手场景(UAT-F-001)

**业务场景**: 新开发者5分钟内完成首次集成

**前置条件**:

- 已安装Rust 1.75+
- 已安装PostgreSQL

**测试步骤**:

```bash
# Step 1: 创建新项目
cargo new my_app
cd my_app

# Step 2: 添加依赖
cat >> Cargo.toml << EOF
[dependencies]
your-db = { version = "0.1", features = ["postgres"] }
tokio = { version = "1", features = ["full"] }
EOF

# Step 3: 创建配置文件
cat > config.yaml << EOF
database:
  url: "postgresql://user:pass@localhost/testdb"
  max_connections: 10
  
roles:
  admin:
    tables:
      - name: "*"
        operations: ["SELECT", "INSERT", "UPDATE", "DELETE"]
EOF

# Step 4: 编写代码
cat > src/main.rs << 'EOF'
use your_db::prelude::*;

#[derive(DbEntity)]
#[db_entity]
#[table_name = "users"]
#[db_crud]
struct User {
    #[primary_key]
    id: i64,
    name: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化连接池
    let pool = DbPool::from_config("config.yaml").await?;
    
    // 获取session
    let session = pool.get_session("admin").await?;
    
    // CRUD操作
    let user = User { id: 1, name: "Alice".into() };
    User::insert(&session, user).await?;
    
    let found = User::find_by_id(&session, 1).await?;
    println!("Found user: {:?}", found);
    
    Ok(())
}
EOF

# Step 5: 运行
cargo run
```

**预期结果**:

- ✅ 编译成功,无警告
- ✅ 运行输出: `Found user: Some(User { id: 1, name: "Alice" })`
- ✅ 总耗时 < 5分钟

**验收标准**:

-  新手按文档操作无障碍
-  编译错误信息清晰
-  示例代码可直接运行

------

### 2.2 权限控制场景(UAT-F-002)

**业务场景**: 限制只读账号不能修改生产数据

**测试步骤**:

```yaml
# permissions.yaml
roles:
  admin:
    tables:
      - name: "*"
        operations: ["SELECT", "INSERT", "UPDATE", "DELETE"]
  
  readonly:
    tables:
      - name: "users"
        operations: ["SELECT"]
      - name: "orders"
        operations: ["SELECT"]
```

```rust
#[tokio::test]
async fn uat_permission_enforcement() {
    let pool = DbPool::from_config("config.yaml").await.unwrap();
    
    // 场景1: readonly角色查询数据 - 应该成功
    let readonly_session = pool.get_session("readonly").await.unwrap();
    let result = User::find_by_id(&readonly_session, 1).await;
    assert!(result.is_ok(), "Readonly should be able to SELECT");
    
    // 场景2: readonly角色插入数据 - 应该失败
    let user = User { id: 2, name: "Bob".into() };
    let result = User::insert(&readonly_session, user).await;
    assert!(result.is_err(), "Readonly should NOT be able to INSERT");
    
    // 场景3: 错误信息应该明确
    if let Err(DbError::Permission(e)) = result {
        let error_msg = e.to_string();
        assert!(error_msg.contains("readonly"));
        assert!(error_msg.contains("INSERT"));
        assert!(error_msg.contains("users"));
    }
    
    // 场景4: readonly角色访问未授权表 - 应该失败
    let result = Product::find_all(&readonly_session).await;
    assert!(result.is_err(), "Readonly should NOT access Product table");
}
```

**预期结果**:

- ✅ readonly可以SELECT授权表
- ✅ readonly无法INSERT/UPDATE/DELETE
- ✅ readonly无法访问未授权表
- ✅ 错误信息包含角色、表名、操作类型

**验收标准**:

-  权限配置100%生效
-  错误信息便于调试
-  无绕过权限的方法

------

### 2.3 配置自动修正场景(UAT-F-003)

**业务场景**: 运维人员配置不当时系统自动修正并告警

**测试步骤**:

```yaml
# config.yaml (错误配置)
database:
  url: "postgresql://user:pass@localhost/testdb"
  max_connections: 1000  # 超过数据库能力(假设数据库max=100)
  min_connections: 1500  # 超过max_connections
  idle_timeout: 10       # 过短
```

```rust
#[tokio::test]
async fn uat_config_auto_correction() {
    // 初始化时应自动修正
    let pool = DbPool::from_config("config.yaml").await.unwrap();
    
    // 验证修正后的值
    let actual_config = pool.get_actual_config();
    
    // 数据库max_connections=100, 80%=80
    assert_eq!(actual_config.max_connections, 80);
    
    // min不能超过max, 修正为max的50%=40
    assert_eq!(actual_config.min_connections, 40);
    
    // idle_timeout最小60秒
    assert_eq!(actual_config.idle_timeout, 60);
}
```

```rust
// 环境变量覆盖配置示例
std::env::set_var("DB_MAX_CONNECTIONS", "80");
std::env::set_var("DB_IDLE_TIMEOUT", "60");

let pool = DbPool::from_config("config.yaml").await.unwrap();
let actual_config = pool.get_actual_config();

assert_eq!(actual_config.max_connections, 80);
assert_eq!(actual_config.idle_timeout, 60);

std::env::remove_var("DB_MAX_CONNECTIONS");
std::env::remove_var("DB_IDLE_TIMEOUT");
```

**预期日志输出**:

```
[2025-01-15T10:00:00Z WARN] Configuration auto-corrected:
  ✓ max_connections: 1000 -> 80
    Reason: Exceeds database capacity (100), limited to 80%
  ✓ min_connections: 1500 -> 40
    Reason: Cannot exceed max_connections, set to 50% of max
  ✓ idle_timeout: 10 -> 60
    Reason: Too short, minimum is 60 seconds
[2025-01-15T10:00:00Z INFO] Connection pool initialized successfully
  • Database: PostgreSQL 14.2
  • Max connections: 80
  • Min connections: 40
```

**验收标准**:

-  启动日志清晰显示修正项
-  修正后系统正常运行
-  不会因配置错误而启动失败

------

### 2.4 Schema Migration场景(UAT-F-004)

**业务场景**: 开发人员修改数据模型后自动生成迁移脚本

**测试步骤**:

```rust
// 原始版本
#[derive(DbEntity)]
#[db_entity]
#[table_name = "users"]
struct User {
    #[primary_key]
    id: i64,
    name: String,
}

// 修改后版本(新增email字段)
#[derive(DbEntity)]
#[db_entity]
#[table_name = "users"]
struct User {
    #[primary_key]
    id: i64,
    name: String,
    email: String,  // 新增
}
```

```bash
# 生成migration
cargo db-migrate generate "add_email_to_users"

# 自动生成的文件: migrations/20250115_000001_add_email_to_users.sql
# ALTER TABLE users ADD COLUMN email VARCHAR(255);

# 执行migration
cargo db-migrate up

# 验证
psql -c "SELECT column_name FROM information_schema.columns WHERE table_name='users';"
# 输出应包含: id, name, email
```

**预期结果**:

- ✅ 自动检测到字段新增
- ✅ 生成正确的ALTER TABLE语句
- ✅ 执行成功,无数据丢失
- ✅ schema_migrations表记录版本

**验收标准**:

-  检测90%+的常见schema变更
-  SQL语法适配目标数据库
-  历史版本可追溯

------

### 2.5 Metrics监控场景(UAT-F-005)

**业务场景**: SRE通过Prometheus监控数据库健康状况

**测试步骤**:

```rust
// 在应用中暴露metrics endpoint
use axum::{Router, routing::get};

#[tokio::main]
async fn main() {
    let pool = DbPool::from_config("config.yaml").await.unwrap();
    
    let app = Router::new()
        .route("/metrics", get(|| async move {
            pool.export_metrics()
        }));
    
    axum::Server::bind(&"0.0.0.0:9090".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
```

```bash
# 执行一些数据库操作后
curl http://localhost:9090/metrics | grep db_

# 预期输出:
# db_pool_connections{state="total"} 10
# db_pool_connections{state="active"} 2
# db_pool_connections{state="idle"} 8
# db_query_duration_seconds{table="users",op="SELECT",quantile="0.5"} 0.005
# db_query_duration_seconds{table="users",op="SELECT",quantile="0.95"} 0.020
# db_query_duration_seconds{table="users",op="SELECT",quantile="0.99"} 0.050
# db_errors_total{type="connection"} 0
# db_errors_total{type="query"} 0
# db_slow_queries_total{threshold="100ms"} 0
```

**在Grafana中验证**:

1. 导入提供的dashboard模板
2. 查看连接池使用率图表
3. 查看查询延迟分布图表
4. 设置慢查询告警(阈值100ms)

**验收标准**:

-  Metrics格式符合Prometheus规范
-  所有关键指标都暴露
-  延迟分位数计算准确
-  可在Grafana正常展示

------

## 3. 性能验收测试

### 3.1 并发性能测试(UAT-P-001)

**业务场景**: 支持100 QPS的并发查询

**测试脚本**:

```rust
#[tokio::test]
async fn uat_concurrent_query_performance() {
    let pool = DbPool::from_config("config.yaml").await.unwrap();
    
    // 预热
    for i in 0..100 {
        let session = pool.get_session("admin").await.unwrap();
        User::insert(&session, User { id: i, name: format!("user_{}", i) }).await.unwrap();
    }
    
    // 压力测试: 10秒内发送1000个请求(100 QPS)
    let start = Instant::now();
    let mut tasks = Vec::new();
    
    for i in 0..1000 {
        let pool = pool.clone();
        let task = tokio::spawn(async move {
            let session = pool.get_session("admin").await.unwrap();
            let user_id = i % 100;
            User::find_by_id(&session, user_id).await.unwrap()
        });
        tasks.push(task);
        
        // 控制发送速率: 100 QPS = 每10ms一个请求
        if i % 10 == 9 {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
    
    // 等待所有请求完成
    for task in tasks {
        task.await.unwrap();
    }
    
    let elapsed = start.elapsed();
    let qps = 1000.0 / elapsed.as_secs_f64();
    
    // 验证指标
    let metrics = pool.export_metrics();
    let p99_latency = extract_p99_latency(&metrics);
    
    println!("Actual QPS: {:.2}", qps);
    println!("P99 Latency: {:.2}ms", p99_latency * 1000.0);
}
```

**验收标准**:

- [ ] 实际QPS ≥ 100
- [ ] P99延迟 < 10ms
- [ ] 无连接池耗尽
- [ ] 错误率 = 0%

**预期性能指标** (实际测试后更新):

```
预期指标:
  • QPS: ≥ 100
  • P99延迟: < 10ms
  • 连接池利用率: < 90%
  • 错误率: 0%

示例测试结果 (仅供参考):
  Actual QPS: 125.3
  P99 Latency: 8.7ms
  Connection Pool Max Usage: 18/20
  Error Rate: 0%
```

------

### 3.2 内存稳定性测试(UAT-P-002)

**业务场景**: 长时间运行无内存泄漏

**测试脚本**:

```bash
# 使用压测工具运行1小时
./stress_test.sh --duration=1h --qps=50

# 监控内存
while true; do
  ps aux | grep my_app | awk '{print $6}'
  sleep 60
done
```

**监控数据采集**:

- 初始内存: 50MB
- 30分钟后: 52MB
- 60分钟后: 53MB

**验收标准**:

-  内存增长 < 10%
-  无突发内存峰值
-  连接池稳定

------

## 4. 安全验收测试

### 4.1 SQL注入防护(UAT-S-001)

**业务场景**: 防止恶意用户通过输入注入SQL

**测试步骤**:

```rust
#[tokio::test]
async fn uat_sql_injection_protection() {
    let pool = DbPool::from_config("config.yaml").await.unwrap();
    let session = pool.get_session("admin").await.unwrap();
    
    // 恶意输入
    let malicious_inputs = vec![
        "1' OR '1'='1",
        "1; DROP TABLE users--",
        "1' UNION SELECT * FROM passwords--",
        "'; DELETE FROM users WHERE '1'='1",
    ];
    
    for input in malicious_inputs {
        // 尝试使用恶意输入查询
        let result = User::find_by_name(&session, input).await;
        
        // 应该返回空结果或错误,而不是执行恶意SQL
        match result {
            Ok(users) => assert!(users.is_empty(), "SQL injection detected!"),
            Err(_) => { /* 参数化查询阻止了注入 */ }
        }
    }
    
    // 验证数据库完整性
    let count = User::count(&session).await.unwrap();
    assert!(count > 0, "Table was dropped!");
}
```

**验收标准**:

-  所有恶意输入被安全处理
-  数据库结构完整
-  无数据泄露

------

### 4.2 连接泄漏防护(UAT-S-002)

**业务场景**: 即使代码有bug,也不会耗尽连接池

**测试步骤**:

```rust
#[tokio::test]
async fn uat_connection_leak_protection() {
    let pool = DbPool::from_config("config.yaml").await.unwrap();
    
    // 模拟忘记释放session的代码(放大到1000次以匹配PRD要求)
    for _ in 0..1000 {
        let _session = pool.get_session("admin").await.unwrap();
        // 故意不使用session,让它在作用域结束时自动Drop
    }
    
    // 等待RAII回收
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // 验证连接已归还
    assert_eq!(pool.active_connections(), 0);
    assert!(pool.idle_connections() > 0);
    
    // 验证仍可获取新连接
    let session = pool.get_session("admin").await;
    assert!(session.is_ok());
}
```

**验收标准**:

-  RAII机制100%生效
-  无连接泄漏
-  池资源正常回收

------

## 5. 易用性验收测试

### 5.1 错误信息可读性(UAT-U-001)

**业务场景**: 编译错误和运行时错误易于理解

**测试用例**:

```rust
// Case 1: 缺少primary_key
#[db_entity]
struct BadEntity1 {
    id: i64,  // 忘记标记#[primary_key]
}

// 预期编译错误:
// error: Entity must have exactly one field marked with #[primary_key]
//  --> src/main.rs:3:1
//   |
// 3 | struct BadEntity1 {
//   | ^^^^^^^^^^^^^^^^^
```

```rust
// Case 2: 权限角色不存在
#[db_entity]
#[db_permission(roles = ["super_admin"])]  // 配置中不存在
struct BadEntity2 {
    #[primary_key]
    id: i64,
}

// 预期编译错误:
// error: Role 'super_admin' not found in permissions.yaml
//        Available roles: admin, readonly, user
//  --> src/main.rs:2:25
//   |
// 2 | #[db_permission(roles = ["super_admin"])]
//   |                         ^^^^^^^^^^^^^^^
```

```rust
// Case 3: 运行时权限错误
let session = pool.get_session("readonly").await.unwrap();
User::delete(&session, 1).await.unwrap();

// 预期运行时错误:
// Error: Permission denied
//   Role 'readonly' cannot perform DELETE on table 'users'
//   Allowed operations: SELECT
//   
//   Hint: Check permissions.yaml for role configuration
```

**验收标准**:

-  编译错误指向具体代码位置
-  错误信息包含修复建议
-  运行时错误包含上下文信息

------

### 5.2 文档完整性(UAT-U-002)

**检查清单**:

- **README.md**: 包含快速开始、安装说明

- **API文档**: 所有公开函数有rustdoc注释

- 示例代码

  : 覆盖核心使用场景

  -  基础CRUD示例
  -  权限配置示例
  -  Migration使用示例
  -  Metrics集成示例

- **配置说明**: 所有配置项有注释

- **错误处理指南**: 常见错误及解决方案

- **迁移指南**: 从其他ORM迁移的步骤

**验收标准**:

-  新手按文档5分钟完成集成
-  API文档生成无警告: `cargo doc --no-deps`
-  示例代码可编译运行

------

## 6. 多数据库兼容性验收

### 6.1 跨数据库一致性(UAT-C-001)

**测试矩阵**:

| 测试用例      | SQLite | PostgreSQL | MySQL |
| ------------- | ------ | ---------- | ----- |
| 基础CRUD      | ✅      | ✅          | ✅     |
| 事务提交      | ✅      | ✅          | ✅     |
| 事务回滚      | ✅      | ✅          | ✅     |
| 联合索引      | ✅      | ✅          | ✅     |
| 外键约束      | ✅      | ✅          | ✅     |
| 并发查询      | ✅      | ✅          | ✅     |
| Migration生成 | ✅      | ✅          | ✅     |

**验收标准**:

-  所有测试在三种数据库上通过
-  行为一致,无数据库特定bug
-  SQL方言自动适配

------

## 7. 验收结论

### 7.1 验收结果汇总

| 测试类别   | 用例总数 | 通过数 | 失败数 | 通过率 |
| ---------- | -------- | ------ | ------ | ------ |
| 功能测试   | 5        | -      | -      | -      |
| 性能测试   | 2        | -      | -      | -      |
| 安全测试   | 2        | -      | -      | -      |
| 易用性测试 | 2        | -      | -      | -      |
| 兼容性测试 | 1        | -      | -      | -      |
| **总计**   | **12**   | **-**  | **-**  | **-%** |

### 7.2 遗留问题

| 问题ID | 严重级别 | 描述 | 计划修复版本 |
| ------ | -------- | ---- | ------------ |
| -      | -        | -    | -            |

### 7.3 上线建议

**满足以下条件可以上线**:

-  所有阻塞性问题已修复
-  测试通过率 ≥ 95%
-  性能指标满足预期
-  文档完整且准确
-  已完成灰度测试

**风险评估**:

- 低风险: 核心功能稳定,测试覆盖充分
- 中风险: v2.0高级特性(分片/缓存)需持续验证
- 高风险: 无
