# 🧪 TEST - Testing Document

| 版本 | 日期 | 作者 | 变更内容 |
| --- | --- | --- | --- |
| v1.0 | 2025-01-01 | QA Engineer | 初始版本 |
| v1.1 | 2025-01-15 | QA Engineer | 根据修正文档更新: 增强泄漏测试, 补充方言/环境变量测试 |

## 1. 测试策略

### 1.1 测试金字塔

```
        ┌─────────────┐
        │   E2E (5%)  │  - 完整流程测试
        └─────────────┘
       ┌───────────────┐
       │Integration(20%)│ - 模块间交互测试
       └───────────────┘
     ┌──────────────────┐
     │  Unit Tests (75%) │ - 单元功能测试
     └──────────────────┘
```

### 1.2 测试原则

- **真实数据库**: 禁止mock,所有测试使用真实数据库(SQLite内存模式用于快速测试)
- **隔离性**: 每个测试独立数据库实例,互不影响
- **可重复性**: 测试结果确定,不依赖外部状态
- **性能基准**: 关键路径包含性能断言

### 1.3 测试环境配置

```toml
# tests/test_config.toml
[test.sqlite]
url = ":memory:"
max_connections = 10

[test.postgres]
url = "postgresql://test:test@localhost:5432/test_db"
max_connections = 20

[test.mysql]
url = "mysql://test:test@localhost:3306/test_db"
max_connections = 20
```

------

## 2. 单元测试(Unit Tests)

### 2.0 测试编号规划说明

测试用例编号采用分段规划,便于扩展和维护:

| 编号范围 | 模块 | 说明 |
|----------|------|------|
| TEST-U-001~009 | 连接池管理 | PoolConfig、PoolManager、配置修正 |
| TEST-U-010~019 | 权限控制 | 配置加载、会话检查、编译时验证 |
| TEST-U-020~029 | 宏展开 | Entity、CRUD、Permission宏 |
| TEST-U-030~039 | Migration | Schema Diff、SQL生成、版本管理 |
| TEST-U-040~049 | 预留 | 扩展功能测试 |
| TEST-U-050~059 | Feature Gate | 编译期互斥检查 |

### 2.1 连接池管理测试

#### TEST-U-001: 连接池初始化

**测试目标**: 验证连接池按配置正确初始化

```rust
#[tokio::test]
async fn test_pool_initialization() {
    let config = PoolConfig {
        max_connections: 20,
        min_connections: 5,
        idle_timeout: 300,
    };
    
    let pool = PoolManager::new(config).await.unwrap();
    
    // 断言: 初始连接数等于min_connections
    assert_eq!(pool.total_connections(), 5);
    assert_eq!(pool.active_connections(), 0);
    assert_eq!(pool.idle_connections(), 5);
}
```

**预期结果**:

- ✓ total = 5
- ✓ active = 0
- ✓ idle = 5

#### TEST-U-002: 连接获取与归还(压力测试)

**测试目标**: 验证在高并发场景下的RAII生命周期管理,确保无连接泄漏

```rust
#[tokio::test]
async fn test_connection_lifecycle_stress() {
    let pool = create_test_pool().await;
    
    // 压力测试: 创建1000个Session,依赖RAII自动归还连接
    for _ in 0..1000 {
        let _session = pool.get_session("admin").await.unwrap();
        // Session自动Drop
    }
    
    // 等待回收
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // 断言: 所有连接已归还
    assert_eq!(pool.active_connections(), 0);
    assert!(pool.idle_connections() > 0);
}
```

**预期结果**:

- ✓ 创建1000个Session后无连接泄漏
- ✓ active/idle统计恢复到稳定状态

#### TEST-U-003: 连接池耗尽处理

**测试目标**: 验证连接池满时的等待机制

```rust
#[tokio::test]
async fn test_pool_exhaustion() {
    let config = PoolConfig {
        max_connections: 2,
        acquire_timeout: Duration::from_millis(100),
        ..Default::default()
    };
    let pool = PoolManager::new(config).await.unwrap();
    
    // 占满连接池
    let _s1 = pool.get_session("admin").await.unwrap();
    let _s2 = pool.get_session("admin").await.unwrap();
    
    // 第3个连接应该超时
    let start = Instant::now();
    let result = pool.get_session("admin").await;
    let elapsed = start.elapsed();
    
    assert!(result.is_err());
    assert!(elapsed >= Duration::from_millis(100));
    assert!(elapsed < Duration::from_millis(150)); // 允许10ms误差
}
```

**预期结果**:

- ✓ 返回超时错误
- ✓ 等待时间约为100ms

#### TEST-U-004: 配置自动修正

**测试目标**: 验证不合理配置被正确修正

```rust
#[tokio::test]
async fn test_config_auto_correction() {
    // 模拟数据库max_connections=100
    let db_capacity = 100;
    
    let config = PoolConfig {
        max_connections: 500,  // 超出数据库能力
        min_connections: 600,  // 超出max_connections
        ..Default::default()
    };
    
    let corrector = ConfigCorrector::new(db_capacity);
    let (corrected, warnings) = corrector.correct(config).await;
    
    // 断言: max_connections被修正为80(数据库能力的80%)
    assert_eq!(corrected.max_connections, 80);
    
    // 断言: min_connections被修正为40(max的50%)
    assert_eq!(corrected.min_connections, 40);
    
    // 断言: 生成了2条警告
    assert_eq!(warnings.len(), 2);
    assert!(warnings[0].field == "max_connections");
    assert!(warnings[1].field == "min_connections");
}
```

**预期结果**:

- ✓ max_connections: 500 -> 80
- ✓ min_connections: 600 -> 40
- ✓ 警告日志记录

#### TEST-U-005: 环境变量覆盖配置

**测试目标**: 验证环境变量可以覆盖配置文件中的默认值

```rust
#[tokio::test]
async fn test_config_env_override() {
    // 设置环境变量
    std::env::set_var("DB_MAX_CONNECTIONS", "50");
    std::env::set_var("DB_URL", "postgresql://override:pass@localhost/db");
    
    // 从文件加载配置(文件中max_connections=20)
    let config = DbConfig::from_file("config.yaml").await.unwrap();
    
    // 断言: 环境变量优先级更高
    assert_eq!(config.max_connections, 50);
    assert!(config.url.contains("override"));
    
    // 清理
    std::env::remove_var("DB_MAX_CONNECTIONS");
    std::env::remove_var("DB_URL");
}
```

**预期结果**:

- ✓ 环境变量成功覆盖配置文件
- ✓ 清理环境变量后不影响其他测试

------

### 2.2 权限控制测试

#### TEST-U-010: 权限配置加载

**测试目标**: 验证YAML权限配置正确解析

```rust
#[tokio::test]
async fn test_permission_config_loading() {
    let yaml = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations: ["SELECT", "INSERT", "UPDATE", "DELETE"]
  readonly:
    tables:
      - name: "users"
        operations: ["SELECT"]
"#;
    
    let config = PermissionConfig::from_yaml(yaml).unwrap();
    
    // 断言: admin角色可以访问所有表
    assert!(config.check_access("admin", "*", Operation::SELECT).is_ok());
    assert!(config.check_access("admin", "orders", Operation::DELETE).is_ok());
    
    // 断言: readonly角色只能SELECT users表
    assert!(config.check_access("readonly", "users", Operation::SELECT).is_ok());
    assert!(config.check_access("readonly", "users", Operation::INSERT).is_err());
    assert!(config.check_access("readonly", "orders", Operation::SELECT).is_err());
}
```

**预期结果**:

- ✓ admin通配符权限生效
- ✓ readonly权限限制生效

#### TEST-U-011: Session权限检查

**测试目标**: 验证Session执行查询前的权限验证

```rust
#[tokio::test]
async fn test_session_permission_check() {
    let pool = create_test_pool_with_permissions().await;
    let session = pool.get_session("readonly").await.unwrap();
    
    // 断言: SELECT允许
    let result = User::find_by_id(&session, 1).await;
    assert!(result.is_ok());
    
    // 断言: INSERT被拒绝
    let user = User { id: 2, name: "test".into() };
    let result = User::insert(&session, user).await;
    assert!(matches!(result, Err(DbError::Permission(_))));
    
    // 验证错误信息
    if let Err(DbError::Permission(e)) = result {
        assert!(e.to_string().contains("readonly"));
        assert!(e.to_string().contains("users"));
        assert!(e.to_string().contains("INSERT"));
    }
}
```

**预期结果**:

- ✓ SELECT成功
- ✓ INSERT返回PermissionError
- ✓ 错误信息包含角色、表名、操作

#### TEST-U-012: 编译时角色验证

**测试目标**: 验证宏在编译时检查角色是否存在

```rust
// 这个测试通过编译失败来验证
// 需要在CI中检查编译错误信息

// test_compile_fail/invalid_role.rs
#[db_entity]
#[db_permission(roles = ["non_existent_role"])]
struct User {
    #[primary_key]
    id: i64,
}

// 预期编译错误:
// error: Role 'non_existent_role' not found in permissions.yaml
//        Available roles: admin, readonly, user
```

**预期结果**:

- ✓ 编译失败
- ✓ 错误信息提示可用角色列表

------

### 2.3 宏展开测试

#### TEST-U-020: Entity宏展开

**测试目标**: 验证#[db_entity]正确生成代码

```rust
#[tokio::test]
async fn test_entity_macro_expansion() {
    #[db_entity]
    #[table_name = "test_users"]
    struct TestUser {
        #[primary_key]
        id: i64,
        name: String,
    }
    
    // 断言: 生成了table_name方法
    assert_eq!(TestUser::table_name(), "test_users");
    
    // 断言: 实现了必要的trait
    fn assert_entity<T: sea_orm::EntityTrait>() {}
    assert_entity::<TestUser>();
}
```

**预期结果**:

- ✓ table_name()返回正确值
- ✓ 实现EntityTrait

#### TEST-U-021: CRUD宏展开

**测试目标**: 验证#[db_crud]生成完整CRUD方法

```rust
#[tokio::test]
async fn test_crud_macro_expansion() {
    #[db_entity]
    #[db_crud]
    struct TestEntity {
        #[primary_key]
        id: i64,
        value: String,
    }
    
    let session = create_test_session().await;
    
    // 断言: insert方法存在
    let entity = TestEntity { id: 1, value: "test".into() };
    let inserted = TestEntity::insert(&session, entity).await.unwrap();
    assert_eq!(inserted.id, 1);
    
    // 断言: find_by_id方法存在
    let found = TestEntity::find_by_id(&session, 1).await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().value, "test");
    
    // 断言: update方法存在
    let mut updated = found.unwrap();
    updated.value = "updated".into();
    TestEntity::update(&session, updated).await.unwrap();
    
    // 断言: delete方法存在
    TestEntity::delete(&session, 1).await.unwrap();
    let deleted = TestEntity::find_by_id(&session, 1).await.unwrap();
    assert!(deleted.is_none());
}
```

**预期结果**:

- ✓ insert/find/update/delete全部生成
- ✓ 方法签名正确
- ✓ 功能正常

#### TEST-U-022: 主键缺失编译错误

**测试目标**: 验证缺少#[primary_key]时编译失败

```rust
// test_compile_fail/missing_primary_key.rs
#[db_entity]
struct InvalidEntity {
    id: i64,  // 缺少#[primary_key]标记
    name: String,
}

// 预期编译错误:
// error: Entity must have exactly one field marked with #[primary_key]
```

**预期结果**:

- ✓ 编译失败
- ✓ 错误信息清晰指出问题

------

### 2.6 Feature Gate编译失败测试

#### TEST-U-050: 多数据库特性同时启用时编译失败

**测试目标**: 验证同时启用多个数据库feature时触发编译期错误

```rust
// test_compile_fail/multiple_db_features.rs
// 该用例应在启用sqlite和postgres两个feature时编译失败

// Cargo.toml中:
// [features]
// sqlite = ["sea-orm/sqlx-sqlite"]
// postgres = ["sea-orm/sqlx-postgres"]
//
// 预期编译错误:
// error: Cannot enable both 'sqlite' and 'postgres' features
```

**预期结果**:

- ✓ 启用多个数据库特性时编译失败
- ✓ 错误信息提示互斥规则

------

### 2.4 Migration测试

#### TEST-U-030: Schema Diff检测

**测试目标**: 验证正确检测schema变更

```rust
#[tokio::test]
async fn test_schema_diff_detection() {
    // 旧schema
    let old_schema = Schema {
        tables: vec![
            Table {
                name: "users",
                columns: vec![
                    Column { name: "id", data_type: DataType::BigInt },
                    Column { name: "name", data_type: DataType::String },
                ],
            }
        ],
    };
    
    // 新schema(新增email字段)
    let new_schema = Schema {
        tables: vec![
            Table {
                name: "users",
                columns: vec![
                    Column { name: "id", data_type: DataType::BigInt },
                    Column { name: "name", data_type: DataType::String },
                    Column { name: "email", data_type: DataType::String },
                ],
            }
        ],
    };
    
    let differ = SchemaDiffer::new();
    let migrations = differ.diff(&old_schema, &new_schema);
    
    // 断言: 检测到1个migration
    assert_eq!(migrations.len(), 1);
    
    // 断言: 是AddColumn类型
    match &migrations[0] {
        Migration::AddColumn { table, column } => {
            assert_eq!(table, "users");
            assert_eq!(column.name, "email");
        }
        _ => panic!("Expected AddColumn migration"),
    }
}
```

**预期结果**:

- ✓ 正确检测到新增列
- ✓ Migration类型正确

#### TEST-U-031: SQL生成(多方言)

**测试目标**: 验证PostgreSQL/MySQL/SQLite方言的SQL生成

```rust
#[test]
fn test_sql_dialect_generation() {
    let table = Table {
        name: "users",
        columns: vec![
            Column { name: "id", data_type: DataType::BigInt, nullable: false },
            Column { name: "name", data_type: DataType::String, nullable: false },
            Column { name: "email", data_type: DataType::String, nullable: true },
        ],
        primary_key: "id",
    };
    
    // PostgreSQL
    let pg_dialect = PostgresDialect::new();
    let pg_sql = pg_dialect.create_table(&table);
    assert!(pg_sql.contains("CREATE TABLE users"));
    assert!(pg_sql.contains("id BIGINT NOT NULL"));
    assert!(pg_sql.contains("name VARCHAR(255) NOT NULL"));
    assert!(pg_sql.contains("email VARCHAR(255)"));
    assert!(pg_sql.contains("PRIMARY KEY (id)"));
    
    // MySQL
    let mysql_dialect = MySqlDialect::new();
    let mysql_sql = mysql_dialect.create_table(&table);
    assert!(mysql_sql.contains("CREATE TABLE users"));
    assert!(mysql_sql.contains("BIGINT"));
    assert!(mysql_sql.contains("ENGINE=InnoDB"));
    assert!(mysql_sql.contains("CHARSET=utf8mb4"));
    
    // SQLite
    let sqlite_dialect = SqliteDialect::new();
    let sqlite_sql = sqlite_dialect.create_table(&table);
    assert!(sqlite_sql.contains("CREATE TABLE users"));
    assert!(sqlite_sql.contains("INTEGER"));  // SQLite的BIGINT映射
    assert!(sqlite_sql.contains("TEXT"));
    assert!(sqlite_sql.contains("PRIMARY KEY (id)"));
}
```

**预期结果**:

- ✓ PostgreSQL/MySQL/SQLite CREATE TABLE语法正确
- ✓ 数据类型/方言差异正确处理

#### TEST-U-032: Migration执行与回滚

**测试目标**: 验证migration的执行和历史记录

```rust
#[tokio::test]
async fn test_migration_execution() {
    let db = create_test_db().await;
    let executor = MigrationExecutor::new(&db);
    
    // 执行migration
    let migration = Migration::CreateTable(/* ... */);
    executor.execute(migration).await.unwrap();
    
    // 断言: 表已创建
    let tables = db.query_raw("SELECT name FROM sqlite_master WHERE type='table'")
        .await.unwrap();
    assert!(tables.contains(&"users"));
    
    // 断言: 历史记录已写入
    let history = db.query_raw(
        "SELECT version FROM schema_migrations ORDER BY applied_at"
    ).await.unwrap();
    assert_eq!(history.len(), 1);
}
```

**预期结果**:

- ✓ 表创建成功
- ✓ 历史记录正确

------

### 2.5 Metrics测试

#### TEST-U-040: 查询延迟统计

**测试目标**: 验证延迟histogram正确记录

```rust
#[tokio::test]
async fn test_query_duration_metrics() {
    let collector = MetricsCollector::new();
    
    // 模拟10次查询
    for i in 0..10 {
        let duration = Duration::from_millis(10 * (i + 1)); // 10ms, 20ms, ..., 100ms
        collector.record_query_duration("users", "SELECT", duration);
    }
    
    let metrics = collector.export_prometheus();
    
    // 断言: P50约为50ms
    assert!(metrics.contains("quantile=\"0.5\"} 0.05"));
    
    // 断言: P95约为95ms
    assert!(metrics.contains("quantile=\"0.95\"} 0.095"));
    
    // 断言: P99约为100ms
    assert!(metrics.contains("quantile=\"0.99\"} 0.1"));
}
```

**预期结果**:

- ✓ 分位数计算正确
- ✓ Prometheus格式符合规范

#### TEST-U-041: 慢查询统计

**测试目标**: 验证慢查询阈值判断

```rust
#[tokio::test]
async fn test_slow_query_detection() {
    let collector = MetricsCollector::with_threshold(Duration::from_millis(100));
    
    // 快速查询
    collector.record_query_duration("users", "SELECT", Duration::from_millis(50));
    collector.record_query_duration("users", "SELECT", Duration::from_millis(80));
    
    // 慢查询
    collector.record_query_duration("orders", "SELECT", Duration::from_millis(150));
    collector.record_query_duration("orders", "SELECT", Duration::from_millis(200));
    
    let metrics = collector.export_prometheus();
    
    // 断言: 慢查询计数为2
    assert!(metrics.contains(r#"db_slow_queries_total{threshold="100ms"} 2"#));
}
```

**预期结果**:

- ✓ 慢查询正确统计
- ✓ 阈值判断准确

#### TEST-U-042: 连接池状态监控

**测试目标**: 验证连接池指标实时更新

```rust
#[tokio::test]
async fn test_pool_metrics() {
    let pool = create_test_pool().await;
    let collector = pool.metrics();
    
    // 初始状态
    let metrics = collector.export_prometheus();
    assert!(metrics.contains(r#"db_pool_connections{state="total"} 5"#));
    assert!(metrics.contains(r#"db_pool_connections{state="active"} 0"#));
    
    // 获取2个连接
    let _s1 = pool.get_session("admin").await.unwrap();
    let _s2 = pool.get_session("admin").await.unwrap();
    
    let metrics = collector.export_prometheus();
    assert!(metrics.contains(r#"db_pool_connections{state="active"} 2"#));
    assert!(metrics.contains(r#"db_pool_connections{state="idle"} 3"#));
}
```

**预期结果**:

- ✓ 指标实时更新
- ✓ active/idle统计正确

------

## 3. 集成测试(Integration Tests)

### 3.1 完整CRUD流程测试

#### TEST-I-001: 用户管理完整流程

**测试目标**: 验证从连接池到数据库的完整链路

```rust
#[tokio::test]
async fn test_full_user_crud_workflow() {
    // 1. 初始化
    let config = load_test_config("sqlite");
    let pool = DbPool::initialize(config).await.unwrap();
    
    // 2. 创建admin session
    let admin_session = pool.get_session("admin").await.unwrap();
    
    // 3. 插入用户
    let user = User {
        id: 1,
        name: "Alice".into(),
        email: "alice@example.com".into(),
    };
    let inserted = User::insert(&admin_session, user).await.unwrap();
    assert_eq!(inserted.name, "Alice");
    
    // 4. 查询用户
    let found = User::find_by_id(&admin_session, 1).await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().email, "alice@example.com");
    
    // 5. 更新用户
    let mut user = found.unwrap();
    user.email = "alice_new@example.com".into();
    User::update(&admin_session, user).await.unwrap();
    
    // 6. 验证更新
    let updated = User::find_by_id(&admin_session, 1).await.unwrap().unwrap();
    assert_eq!(updated.email, "alice_new@example.com");
    
    // 7. 删除用户
    User::delete(&admin_session, 1).await.unwrap();
    
    // 8. 验证删除
    let deleted = User::find_by_id(&admin_session, 1).await.unwrap();
    assert!(deleted.is_none());
    
    // 9. 验证metrics
    let metrics = pool.export_metrics();
    assert!(metrics.contains("db_query_duration_seconds"));
    assert!(metrics.contains(r#"table="users""#));
}
```

**预期结果**:

- ✓ 所有CRUD操作成功
- ✓ Metrics正确记录

------

### 3.2 权限集成测试

#### TEST-I-010: 跨角色访问控制

**测试目标**: 验证不同角色的权限隔离

```rust
#[tokio::test]
async fn test_multi_role_permission() {
    let pool = create_test_pool_with_permissions().await;
    
    // Admin session: 所有操作允许
    let admin = pool.get_session("admin").await.unwrap();
    let user = User { id: 1, name: "test".into() };
    User::insert(&admin, user).await.unwrap();
    User::delete(&admin, 1).await.unwrap();
    
    // User session: 只能读写自己的数据
    let user_session = pool.get_session("user").await.unwrap();
    let user = User { id: 2, name: "test".into() };
    User::insert(&user_session, user).await.unwrap();  // 允许
    
    let result = User::delete(&user_session, 2).await;
    assert!(result.is_err());  // 不允许删除
    
    // Readonly session: 只能读
    let readonly = pool.get_session("readonly").await.unwrap();
    let found = User::find_by_id(&readonly, 2).await.unwrap();  // 允许
    assert!(found.is_some());
    
    let result = User::insert(&readonly, User { id: 3, name: "test".into() }).await;
    assert!(matches!(result, Err(DbError::Permission(_))));  // 拒绝
}
```

**预期结果**:

- ✓ admin全部操作成功
- ✓ user部分操作被拒绝
- ✓ readonly写操作全部被拒绝

------

### 3.3 事务测试

#### TEST-I-020: 事务提交与回滚

**测试目标**: 验证事务的ACID特性

```rust
#[tokio::test]
async fn test_transaction_commit_and_rollback() {
    let pool = create_test_pool().await;
    let session = pool.get_session("admin").await.unwrap();
    
    // 测试提交
    {
        let tx = session.begin_transaction().await.unwrap();
        User::insert(&tx, User { id: 1, name: "Alice".into() }).await.unwrap();
        User::insert(&tx, User { id: 2, name: "Bob".into() }).await.unwrap();
        tx.commit().await.unwrap();
    }
    
    // 验证数据已提交
    let count = User::count(&session).await.unwrap();
    assert_eq!(count, 2);
    
    // 测试回滚
    {
        let tx = session.begin_transaction().await.unwrap();
        User::insert(&tx, User { id: 3, name: "Charlie".into() }).await.unwrap();
        tx.rollback().await.unwrap();
    }
    
    // 验证数据未提交
    let count = User::count(&session).await.unwrap();
    assert_eq!(count, 2);
}
```

**预期结果**:

- ✓ commit后数据持久化
- ✓ rollback后数据未写入

------

### 3.4 多数据库兼容性测试

#### TEST-I-030: SQLite/PostgreSQL/MySQL一致性

**测试目标**: 验证相同操作在不同数据库上结果一致

```rust
#[tokio::test]
async fn test_cross_database_compatibility() {
    for db_type in &["sqlite", "postgres", "mysql"] {
        let pool = create_test_pool_for(*db_type).await;
        let session = pool.get_session("admin").await.unwrap();
        
        // 执行相同的CRUD操作
        let user = User { id: 1, name: "Test".into(), email: "test@example.com".into() };
        User::insert(&session, user).await.unwrap();
        
        let found = User::find_by_id(&session, 1).await.unwrap().unwrap();
        assert_eq!(found.name, "Test");
        
        User::delete(&session, 1).await.unwrap();
        
        let deleted = User::find_by_id(&session, 1).await.unwrap();
        assert!(deleted.is_none());
    }
}
```

**预期结果**:

- ✓ 三种数据库行为一致
- ✓ 无数据库特定错误

------

## 4. 性能测试(Performance Tests)

### 4.1 连接池性能测试

#### TEST-P-001: 并发连接获取

**测试目标**: 验证100并发下连接池性能

```rust
#[tokio::test]
async fn test_concurrent_connection_acquisition() {
    let pool = create_test_pool().await;
    let start = Instant::now();
    
    // 100个并发任务
    let tasks: Vec<_> = (0..100).map(|i| {
        let pool = pool.clone();
        tokio::spawn(async move {
            let session = pool.get_session("admin").await.unwrap();
            User::find_by_id(&session, i).await.unwrap();
        })
    }).collect();
    
    for task in tasks {
        task.await.unwrap();
    }
    
    let elapsed = start.elapsed();
    
    // 断言: 100个查询在1秒内完成
    assert!(elapsed < Duration::from_secs(1), 
            "Took {:?}, expected < 1s", elapsed);
    
    // 断言: P99延迟 < 50ms
    let metrics = pool.export_metrics();
    let p99_line = metrics.lines()
        .find(|l| l.contains("quantile=\"0.99\""))
        .unwrap();
    let p99_value: f64 = p99_line.split_whitespace().last().unwrap().parse().unwrap();
    assert!(p99_value < 0.05, "P99 latency {}s exceeds 50ms", p99_value);
}
```

**预期结果**:

- ✓ 总耗时 < 1秒
- ✓ P99延迟 < 50ms

#### TEST-P-002: 连接池扩展性能

**测试目标**: 验证连接池从min到max的动态扩展

```rust
#[tokio::test]
async fn test_pool_scaling_performance() {
    let config = PoolConfig {
        min_connections: 5,
        max_connections: 20,
        ..Default::default()
    };
    let pool = PoolManager::new(config).await.unwrap();
    
    // 初始5个连接
    assert_eq!(pool.total_connections(), 5);
    
    // 并发20个请求,触发扩展
    let start = Instant::now();
    let tasks: Vec<_> = (0..20).map(|_| {
        let pool = pool.clone();
        tokio::spawn(async move {
            let _session = pool.get_session("admin").await.unwrap();
            tokio::time::sleep(Duration::from_millis(100)).await;
        })
    }).collect();
    
    // 等待扩展完成
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    // 断言: 连接数已扩展
    assert!(pool.total_connections() > 5);
    assert!(pool.total_connections() <= 20);
    
    for task in tasks {
        task.await.unwrap();
    }
    
    let elapsed = start.elapsed();
    
    // 断言: 动态扩展不显著影响性能
    assert!(elapsed < Duration::from_millis(150));
}
```

**预期结果**:

- ✓ 连接数动态扩展
- ✓ 扩展延迟可接受

------

### 4.2 查询性能测试

#### TEST-P-010: 权限检查开销

**测试目标**: 测量权限检查的性能开销

```rust
#[tokio::test]
async fn test_permission_check_overhead() {
    let pool_with_permission = create_test_pool_with_permissions().await;
    let pool_without_permission = create_test_pool_no_permissions().await;
    
    // 有权限检查的查询
    let start = Instant::now();
    let session = pool_with_permission.get_session("admin").await.unwrap();
    for _ in 0..1000 {
        User::find_by_id(&session, 1).await.unwrap();
    }
    let with_permission = start.elapsed();
    
    // 无权限检查的查询(直接Sea-ORM)
    let start = Instant::now();
    let session = pool_without_permission.get_session("admin").await.unwrap();
    for _ in 0..1000 {
        // 直接查询,绕过权限
        session.raw_query("SELECT * FROM users WHERE id = 1").await.unwrap();
    }
    let without_permission = start.elapsed();
    
    let overhead = with_permission - without_permission;
    let per_query_overhead = overhead / 1000;
    
    // 断言: 单次权限检查开销 < 0.1ms
    assert!(per_query_overhead < Duration::from_micros(100),
            "Permission check overhead {:?} exceeds 0.1ms", per_query_overhead);
}
```

**预期结果**:

- ✓ 权限检查开销 < 0.1ms/次

------

## 5. 压力测试(Stress Tests)

### TEST-S-001: 长时间运行稳定性

**测试目标**: 验证24小时运行无内存泄漏

```rust
#[tokio::test]
#[ignore]  // 标记为长时间测试
async fn test_long_running_stability() {
    let pool = create_test_pool().await;
    let start_memory = get_process_memory();
    
    // 运行24小时
    let end_time = Instant::now() + Duration::from_secs(24 * 3600);
    let mut iteration = 0;
    
    while Instant::now() < end_time {
        let session = pool.get_session("admin").await.unwrap();
        
        // 模拟真实工作负载
        User::insert(&session, User { id: iteration, name: format!("user_{}", iteration) }).await.unwrap();
        User::find_by_id(&session, iteration).await.unwrap();
        User::delete(&session, iteration).await.unwrap();
        
        iteration += 1;
        
        // 每小时检查一次内存
        if iteration % 36000 == 0 {
            let current_memory = get_process_memory();
            let growth = current_memory - start_memory;
            
            // 断言: 内存增长 < 100MB
            assert!(growth < 100 * 1024 * 1024,
                    "Memory leak detected: grew {}MB", growth / 1024 / 1024);
        }
        
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
```

**预期结果**:

- ✓ 无panic/crash
- ✓ 内存稳定
- ✓ 连接池无泄漏

------

## 6. 测试覆盖率目标

| 模块      | 目标覆盖率 | 当前覆盖率 |
| --------- | ---------- | ---------- |
| Session层 | 90%        | -          |
| 连接池    | 85%        | -          |
| 权限控制  | 95%        | -          |
| 宏系统    | 80%        | -          |
| Migration | 85%        | -          |
| Metrics   | 90%        | -          |
| **总体**  | **85%**    | -          |
