# 📐 TDD - Technical Design Document

| 版本 | 日期 | 作者 | 变更内容 |
| --- | --- | --- | --- |
| v1.0 | 2025-01-01 | Architect | 初始版本 |
| v1.1 | 2025-01-15 | Architect | 根据修正文档更新: 宏使用示例, 补充插件化权限特性 |

## 1. 系统架构设计

### 1.1 整体架构图

```
┌─────────────────────────────────────────────────────────────┐
│                      User Application                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  #[db_entity]│  │  #[db_crud]  │  │#[db_permission]     │
│  │    Macro     │  │    Macro     │  │    Macro     │      │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘      │
│         │                  │                  │              │
│         └──────────────────┴──────────────────┘              │
│                            │                                 │
└────────────────────────────┼─────────────────────────────────┘
                             ▼
┌─────────────────────────────────────────────────────────────┐
│                    DB Module (Crate)                         │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │              Session Layer (Public API)                 │ │
│ │  • Session (RAII wrapper)                              │ │
│ │  • Transaction management                              │ │
│ │  • Write-after-read tracking                           │ │
│ └────────┬──────────────────────────────────────┬─────────┘ │
│          │                                       │           │
│ ┌────────▼─────────┐                  ┌────────▼─────────┐ │
│ │ Permission Guard │                  │  Metrics Collector│ │
│ │ • Role validation│                  │  • Query latency │ │
│ │ • Table ACL check│                  │  • Pool status   │ │
│ └────────┬─────────┘                  └──────────────────┘ │
│          │                                                  │
│ ┌────────▼──────────────────────────────────────────────┐  │
│ │           Connection Pool Manager                     │  │
│ │  • Dynamic config correction                         │  │
│ │  • Health check                                      │  │
│ │  • Auto reconnection                                 │  │
│ └────────┬──────────────────────────────────────────────┘  │
│          │                                                  │
│ ┌────────▼──────────────────────────────────────────────┐  │
│ │              Sea-ORM Adapter                          │  │
│ │  ┌──────────┐  ┌──────────┐  ┌──────────┐           │  │
│ │  │ SQLite   │  │PostgreSQL│  │  MySQL   │           │  │
│ │  │ Driver   │  │ Driver   │  │ Driver   │           │  │
│ │  └──────────┘  └──────────┘  └──────────┘           │  │
│ └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                             │
        ┌────────────────────┼────────────────────┐
        ▼                    ▼                    ▼
   ┌─────────┐         ┌─────────┐         ┌─────────┐
   │ SQLite  │         │PostgreSQL│        │  MySQL  │
   │   DB    │         │   DB    │         │   DB    │
   └─────────┘         └─────────┘         └─────────┘
```

### 1.2 模块分层设计

```
src/
├── lib.rs                    # 公共API导出 ✅ 已实现 - 模块结构和公共API定义完整
├── config/                   # 配置管理模块 ⚠️ 部分实现 - 只有mod.rs,缺少loader.rs和validator.rs
│   ├── mod.rs                ✅ 已实现 - DbConfig和DbError定义完整
│   ├── loader.rs             ❌ 未实现 - 无YAML/TOML解析逻辑
│   ├── validator.rs          ❌ 未实现 - 无配置验证与修正逻辑
│   └── permission.rs         ✅ 已实现 - 权限配置结构在permission模块
├── pool/                     # 连接池管理 ⚠️ 部分实现 - 只有mod.rs,缺少健康检查和指标
│   ├── mod.rs                ✅ 已实现 - DbPool基本结构和get_session
│   ├── manager.rs            ⚠️ 部分实现 - 缺少健康检查和自动重连
│   ├── health.rs             ❌ 未实现 - 无连接池健康检查
│   └── metrics.rs            ❌ 未实现 - 无池级别指标收集
├── session/                  # Session抽象层 ⚠️ 部分实现 - 基本结构完整,事务逻辑缺失
│   ├── mod.rs                ✅ 已实现 - Session核心结构和生命周期管理
│   ├── session.rs            ⚠️ 部分实现 - CRUD方法未实现
│   ├── transaction.rs        ❌ 未实现 - 事务逻辑为TODO
│   └── write_tracker.rs      ⚠️ 部分实现 - should_use_master存在但未正确追踪
├── permission/               # 权限控制 ⚠️ 部分实现 - 结构完整,集成缺失
│   ├── mod.rs                ✅ 已实现 - 权限结构和检查逻辑
│   ├── guard.rs              ⚠️ 部分实现 - 无PermissionGuard守卫实现
│   ├── policy.rs             ✅ 已实现 - RolePolicy策略定义
│   └── error.rs              ✅ 已实现 - 权限错误类型在DbError中
├── macros/                   # 宏定义(proc-macro crate) ✅ 已实现
│   ├── entity.rs             ✅ 已实现 - #[db_entity]宏在lib.rs中
│   ├── crud.rs               ✅ 已实现 - #[db_crud]宏在lib.rs中
│   └── permission.rs         ✅ 已实现 - #[db_permission]宏在lib.rs中
├── migration/                # Migration工具 ⚠️ 部分实现
│   ├── mod.rs                ✅ 已实现 - Schema/Table/Column等核心结构
│   ├── generator.rs          ✅ 已实现 - SqlGenerator SQL生成器
│   ├── differ.rs             ✅ 已实现 - SchemaDiffer差异检测
│   ├── executor.rs           ❌ 未实现 - 无Migration执行逻辑
│   └── dialect/              ✅ 已实现 - 三种数据库方言支持
│       ├── sqlite.rs
│       ├── postgres.rs
│       └── mysql.rs
├── metrics/                  # 监控指标 ✅ 已实现
│   ├── mod.rs                ✅ 已实现 - MetricsCollector完整实现
│   ├── collector.rs          ✅ 已实现 - 功能在mod.rs中
│   ├── exporter.rs           ✅ 已实现 - export_prometheus()方法
│   └── histogram.rs          ⚠️ 部分实现 - 使用Duration统计,非直方图
└── adapter/                  # Sea-ORM适配层 ❌ 未实现
    ├── mod.rs                ❌ 未实现
    └── query_builder.rs      ❌ 未实现
```

------

## 2. 核心模块详细设计

### 2.1 Session层设计

#### 2.1.1 Session生命周期状态机

```
     ┌─────────┐
     │  Pool   │
     └────┬────┘
          │ get_session(role)
          ▼
     ┌─────────┐
     │ Active  │ ◄──────┐
     │ Session │        │
     └────┬────┘        │
          │             │
          ├─ query() ───┘ (可多次执行)
          │
          ├─ begin_transaction()
          │        │
          │        ▼
          │   ┌─────────────┐
          │   │ Transaction │
          │   │   Active    │
          │   └──────┬──────┘
          │          │
          │          ├─ commit() ──┐
          │          │              │
          │          ├─ rollback()──┤
          │          │              │
          │          └──────────────┘
          │
          ├─ Drop (auto)
          │
          ▼
     ┌─────────┐
     │Released │
     │ to Pool │
     └─────────┘
```

#### 2.1.2 Session核心结构

```rust
pub struct Session {
    // 内部连接(不对外暴露)
    inner: DatabaseConnection,
    
    // 权限上下文
    permission_ctx: PermissionContext,
    
    // 写操作追踪(用于读写分离优化)
    last_write: Option<Instant>,
    
    // Metrics上报
    metrics: Arc<MetricsCollector>,
    
    // 事务状态
    tx_state: TransactionState,
}

impl Session {
    // 查询执行(自动权限检查)
    pub async fn execute<T>(&self, query: Query<T>) -> Result<T> {
        // 1. 权限检查
        self.permission_ctx.check_query(&query)?;
        
        // 2. Metrics记录开始时间
        let start = Instant::now();
        
        // 3. 执行查询
        let result = query.execute(&self.inner).await?;
        
        // 4. 记录延迟
        self.metrics.record_query_duration(
            query.table_name(),
            query.operation(),
            start.elapsed()
        );
        
        Ok(result)
    }
    
    // 标记写操作
    fn mark_write(&mut self) {
        self.last_write = Some(Instant::now());
    }
    
    // 检查是否需要走主库(5秒窗口)
    fn should_use_master(&self) -> bool {
        self.last_write
            .map(|t| t.elapsed() < Duration::from_secs(5))
            .unwrap_or(false)
    }
}

// RAII自动回收
impl Drop for Session {
    fn drop(&mut self) {
        // 自动归还连接到池
        // 如果事务未提交,自动rollback
        if self.tx_state.is_active() {
            warn!("Transaction not committed, auto rollback");
        }
    }
}
```

#### 2.1.3 Session设计符合性检查

**实现文件**: [session/mod.rs](file:///home/project/dbnexus/dbnexus/src/session/mod.rs)

**符合性评估**: ⚠️ 部分符合

| 设计要求 | 实现状态 | 说明 |
|---------|---------|------|
| inner: DatabaseConnection | ⚠️ 部分 | 实际使用 `connection: Option<DatabaseConnection>` |
| permission_ctx: PermissionContext | ✅ 符合 | `permission_ctx: PermissionContext` 字段存在 |
| last_write: Option\<Instant\> | ⚠️ 部分 | 字段存在但未在 CRUD 操作中正确更新 |
| metrics: Arc\<MetricsCollector\> | ❌ 不符合 | 无 MetricsCollector 字段 |
| tx_state: TransactionState | ✅ 符合 | `tx_state: TransactionState` 字段存在 |

**核心方法符合性**:

| 方法 | 设计要求 | 实现状态 |
|-----|---------|---------|
| execute() | 自动权限检查 + Metrics记录 | ❌ 未实现 - 无 execute 方法 |
| mark_write() | 标记写操作 | ❌ 未实现 - 无此方法 |
| should_use_master() | 5秒窗口判断 | ✅ 符合 | 方法已实现 |
| Drop | 自动回收 + 自动回滚 | ⚠️ 部分 | 有回收逻辑,自动回滚为 TODO |

**事务方法符合性**:

| 方法 | 设计要求 | 实现状态 |
|-----|---------|---------|
| begin_transaction() | 开启事务 | ❌ 未实现 - 为 TODO |
| commit() | 提交事务 | ❌ 未实现 - 为 TODO |
| rollback() | 回滚事务 | ❌ 未实现 - 为 TODO |

**架构偏差**:
- execute() 方法未实现,无法进行自动权限检查和 Metrics 记录
- CRUD 操作与 Session 分离,未遵循 TDD 设计的统一执行入口
- 缺少 MetricsCollector 集成,无法收集查询延迟等指标

**下一步行动**:
- 实现 Session::execute() 方法,集成权限检查和指标收集
- 实现 CRUD 方法 mark_write() 更新 last_write
- 实现完整的事务 begin/commit/rollback 逻辑
- 集成 MetricsCollector 进行查询延迟记录

#### 2.2.1 权限配置结构

```yaml
# permissions.yaml
roles:
  admin:
    tables:
      - name: "*"
        operations: ["SELECT", "INSERT", "UPDATE", "DELETE"]
  
  user:
    tables:
      - name: "users"
        operations: ["SELECT", "UPDATE"]
      - name: "orders"
        operations: ["SELECT", "INSERT"]
  
  readonly:
    tables:
      - name: "users"
        operations: ["SELECT"]
      - name: "orders"
        operations: ["SELECT"]
```

#### 2.2.2 权限检查流程

```
Query Request
     │
     ▼
┌─────────────────┐
│Extract Metadata │
│ • table_name    │
│ • operation     │
└────────┬────────┘
         │
         ▼
┌─────────────────────┐
│ Load Role Policy    │
│ from PermissionCtx  │
└────────┬────────────┘
         │
         ▼
┌─────────────────────┐      YES    ┌──────────┐
│ Check Table Access  ├─────────────►│ Execute  │
│ • "*" wildcard?     │              │  Query   │
│ • exact match?      │              └──────────┘
└────────┬────────────┘
         │ NO
         ▼
┌─────────────────────┐
│ Return Permission   │
│   Denied Error      │
└─────────────────────┘
```

#### 2.2.3 编译时权限检查

```rust
// 宏展开时生成的代码
#[db_permission(roles = ["admin", "user"])]
struct User { ... }

// 展开为:
impl User {
    const ALLOWED_ROLES: &'static [&'static str] = &["admin", "user"];
    
    fn check_permission(ctx: &PermissionContext) -> Result<()> {
        if !Self::ALLOWED_ROLES.contains(&ctx.role()) {
            return Err(PermissionError::RoleNotAllowed {
                entity: "User",
                role: ctx.role().to_string(),
                allowed: Self::ALLOWED_ROLES.to_vec(),
            });
        }
        Ok(())
    }
}
```

#### 2.2.4 权限控制设计符合性检查

**实现文件**: [permission/mod.rs](file:///home/project/dbnexus/dbnexus/src/permission/mod.rs)

**符合性评估**: ⚠️ 部分符合

| 设计要求 | 实现状态 | 说明 |
|---------|---------|------|
| YAML权限配置文件解析 | ✅ 符合 | PermissionConfig::from_file() 实现 |
| 角色策略 RolePolicy | ✅ 符合 | RolePolicy 结构完整 |
| 表权限 TablePermission | ✅ 符合 | TablePermission 包含 name 和 operations |
| 通配符支持 ("*") | ✅ 符合 | allows() 中检查 `perm.name == "*"` |
| 权限检查方法 check_table_access() | ✅ 符合 | 方法已实现 |
| PermissionGuard 守卫 | ❌ 不符合 | 无独立 PermissionGuard 实现 |

**权限配置结构符合性**:

| 字段 | 设计要求 | 实现状态 |
|-----|---------|---------|
| roles: Map\<String, RolePolicy\> | ✅ 符合 | `roles: HashMap<String, RolePolicy>` |
| tables: Vec\<TablePermission\> | ✅ 符合 | `tables: Vec<TablePermission>` |
| operations: Vec\<Operation\> | ✅ 符合 | Operation 枚举完整 |

**方法符合性**:

| 方法 | 设计要求 | 实现状态 |
|-----|---------|---------|
| allows(table, operation) | 权限检查 | ✅ 符合 | allows() 方法实现完整 |
| check_table_access() | 表访问检查 | ✅ 符合 | check_table_access() 方法存在 |
| from_file(path) | 配置文件加载 | ✅ 符合 | from_file() 实现 YAML 解析 |

**架构偏差**:
- 无独立 PermissionGuard 守卫实现,权限检查直接在 Session 中调用
- 配置文件自动加载未集成到 Session 创建流程
- 无编译时权限检查 (需要宏系统支持)

**下一步行动**:
- 实现 PermissionGuard 守卫模式
- 集成配置文件自动加载到 DbPool
- 实现 #[db_permission] 宏进行编译时检查

------

### 2.3 连接池配置修正设计

#### 2.3.1 修正算法

```rust
pub struct ConfigCorrector {
    db_max_connections: u32,  // 从数据库查询获得
}

impl ConfigCorrector {
    pub async fn correct(&self, mut config: PoolConfig) -> (PoolConfig, Vec<Correction>) {
        let mut corrections = Vec::new();
        
        // 规则1: max_connections不超过数据库能力的80%
        let safe_max = (self.db_max_connections as f32 * 0.8) as u32;
        if config.max_connections > safe_max {
            corrections.push(Correction {
                field: "max_connections",
                original: config.max_connections,
                corrected: safe_max,
                reason: format!(
                    "Exceeds database capacity ({}), limited to 80%",
                    self.db_max_connections
                ),
            });
            config.max_connections = safe_max;
        }
        
        // 规则2: min_connections不超过max_connections
        if config.min_connections > config.max_connections {
            corrections.push(Correction {
                field: "min_connections",
                original: config.min_connections,
                corrected: config.max_connections / 2,
                reason: "Cannot exceed max_connections".into(),
            });
            config.min_connections = config.max_connections / 2;
        }
        
        // 规则3: idle_timeout合理范围(60-3600秒)
        if config.idle_timeout < 60 {
            corrections.push(Correction {
                field: "idle_timeout",
                original: config.idle_timeout,
                corrected: 60,
                reason: "Too short, may cause frequent reconnections".into(),
            });
            config.idle_timeout = 60;
        }
        
        (config, corrections)
    }
}
```

#### 2.3.2 启动日志示例

```
2025-01-15T10:30:45Z [WARN] Config auto-corrected:
  • max_connections: 500 -> 200
    Reason: Exceeds database capacity (250), limited to 80%
  • min_connections: 250 -> 100
    Reason: Cannot exceed max_connections
2025-01-15T10:30:45Z [INFO] Connection pool initialized:
  • Database: PostgreSQL 14.2
  • Max connections: 200
  • Min connections: 100
  • Idle timeout: 300s
```

------

### 2.4 宏系统设计

#### 2.4.1 宏展开示例

**用户代码:**

```rust
#[derive(DbEntity)]
#[db_entity]
#[table_name = "users"]
#[db_crud]
#[db_permission(roles = ["admin"])]
struct User {
    #[primary_key]
    id: i64,
    name: String,
    email: String,
}
```

**展开后代码(简化版):**

```rust
// 第1层: Entity映射
impl sea_orm::EntityTrait for User {
    fn table_name() -> &'static str { "users" }
}

// 第2层: CRUD生成
impl User {
    pub async fn insert(session: &Session, entity: Self) -> Result<Self> {
        // 权限检查
        Self::check_permission(session.permission_ctx())?;
        
        // 标记写操作
        session.mark_write();
        
        // 执行插入
        let result = session.execute(
            sea_orm::Insert::one(entity.into_active_model())
        ).await?;
        
        // Metrics记录
        session.metrics().record_operation("users", "INSERT");
        
        Ok(result)
    }
    
    pub async fn find_by_id(session: &Session, id: i64) -> Result<Option<Self>> {
        Self::check_permission(session.permission_ctx())?;
        
        session.execute(
            sea_orm::Entity::find_by_id(id)
        ).await
    }
    
    pub async fn update(session: &Session, entity: Self) -> Result<Self> {
        Self::check_permission(session.permission_ctx())?;
        session.mark_write();
        
        session.execute(
            sea_orm::Update::one(entity.into_active_model())
        ).await
    }
    
    pub async fn delete(session: &Session, id: i64) -> Result<()> {
        Self::check_permission(session.permission_ctx())?;
        session.mark_write();
        
        session.execute(
            sea_orm::Delete::by_id(id)
        ).await
    }
}

// 第3层: 权限检查
impl User {
    const ALLOWED_ROLES: &'static [&'static str] = &["admin"];
    
    fn check_permission(ctx: &PermissionContext) -> Result<()> {
        if !Self::ALLOWED_ROLES.contains(&ctx.role()) {
            return Err(PermissionError::RoleNotAllowed {
                entity: "User",
                role: ctx.role(),
                allowed: Self::ALLOWED_ROLES,
            });
        }
        Ok(())
    }
}
```

#### 2.4.2 编译时检查实现

```rust
// 在宏展开时进行检查
#[proc_macro_derive(DbEntity, attributes(db_entity, table_name, primary_key))]
pub fn derive_db_entity(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    
    // 检查1: 必须有primary_key
    let has_pk = ast.fields.iter().any(|f| {
        f.attrs.iter().any(|a| a.path.is_ident("primary_key"))
    });
    if !has_pk {
        return syn::Error::new(
            ast.ident.span(),
            "Entity must have exactly one field marked with #[primary_key]"
        ).to_compile_error().into();
    }
    
    // 检查2: table_name必须存在
    let has_table_name = ast.attrs.iter().any(|a| {
        a.path.is_ident("table_name")
    });
    if !has_table_name {
        return syn::Error::new(
            ast.ident.span(),
            "Missing #[table_name = \"...\"] attribute"
        ).to_compile_error().into();
    }
    
    // ... 生成代码
}
```

------

### 2.5 Migration工具设计

#### 2.5.1 Schema Diff检测算法

```rust
pub struct SchemaDiffer {
    pub fn diff(&self, old: &Schema, new: &Schema) -> Vec<Migration> {
        let mut migrations = Vec::new();
        
        // 检测新增表
        for table in &new.tables {
            if !old.tables.contains(table.name) {
                migrations.push(Migration::CreateTable(table.clone()));
            }
        }
        
        // 检测表结构变更
        for new_table in &new.tables {
            if let Some(old_table) = old.tables.find(new_table.name) {
                // 检测新增列
                for col in &new_table.columns {
                    if !old_table.columns.contains(col.name) {
                        migrations.push(Migration::AddColumn {
                            table: new_table.name,
                            column: col.clone(),
                        });
                    }
                }
                
                // 检测列类型变更
                for new_col in &new_table.columns {
                    if let Some(old_col) = old_table.columns.find(new_col.name) {
                        if old_col.data_type != new_col.data_type {
                            migrations.push(Migration::AlterColumn {
                                table: new_table.name,
                                column: new_col.name,
                                old_type: old_col.data_type,
                                new_type: new_col.data_type,
                            });
                        }
                    }
                }
                
                // 检测索引变更
                let diff_indexes = self.diff_indexes(old_table, new_table);
                migrations.extend(diff_indexes);
            }
        }
        
        migrations
    }
}
```

#### 2.5.2 SQL生成(多方言支持)

```rust
pub trait SqlDialect {
    fn create_table(&self, table: &Table) -> String;
    fn add_column(&self, table: &str, column: &Column) -> String;
    fn create_index(&self, index: &Index) -> String;
}

// PostgreSQL实现
impl SqlDialect for PostgresDialect {
    fn create_table(&self, table: &Table) -> String {
        let mut sql = format!("CREATE TABLE {} (\n", table.name);
        
        for col in &table.columns {
            sql.push_str(&format!("  {} {},\n",
                col.name,
                self.map_type(&col.data_type)
            ));
        }
        
        sql.push_str(&format!("  PRIMARY KEY ({})\n", table.primary_key));
        sql.push_str(")");
        sql
    }
    
    fn add_column(&self, table: &str, column: &Column) -> String {
        format!("ALTER TABLE {} ADD COLUMN {} {}",
            table,
            column.name,
            self.map_type(&column.data_type)
        )
    }
    
    fn create_index(&self, index: &Index) -> String {
        let index_type = if index.is_unique { "UNIQUE INDEX" } else { "INDEX" };
        format!("CREATE {} {} ON {} ({})",
            index_type,
            index.name,
            index.table,
            index.columns.join(", ")
        )
    }
}

// MySQL实现(处理方言差异)
impl SqlDialect for MySqlDialect {
    fn create_table(&self, table: &Table) -> String {
        // MySQL特定: 需要ENGINE和CHARSET
        let mut sql = format!("CREATE TABLE {} (\n", table.name);
        // ... 列定义
        sql.push_str(") ENGINE=InnoDB DEFAULT CHARSET=utf8mb4");
        sql
    }
}
```

#### 2.5.3 Migration历史记录

```sql
-- 自动创建的历史表
CREATE TABLE schema_migrations (
    version VARCHAR(255) PRIMARY KEY,
    applied_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    description TEXT,
    checksum VARCHAR(64)  -- migration文件的hash,防止篡改
);
```

---

### 2.6 Metrics系统设计

#### 2.6.1 指标收集架构

```
Query Execution
     │
     ├─► Start Timer
     │
     ├─► Execute
     │
     ├─► End Timer
     │
     └─► MetricsCollector
              │
              ├─► Histogram (query_duration)
              │    └─► Update quantiles (p50/p95/p99)
              │
              ├─► Counter (query_total)
              │
              └─► Gauge (pool_connections)
                   └─► Update current value
```

#### 2.6.2 核心数据结构

```rust
pub struct MetricsCollector {
    // 查询延迟直方图
    query_duration: Arc<RwLock<HashMap<(String, String), Histogram>>>,
    // key: (table_name, operation)
    
    // 连接池状态
    pool_connections: Arc<AtomicU32>,
    pool_active: Arc<AtomicU32>,
    pool_idle: Arc<AtomicU32>,
    
    // 错误计数
    connection_errors: Arc<AtomicU64>,
    query_errors: Arc<AtomicU64>,
    
    // 慢查询
    slow_queries: Arc<AtomicU64>,
    slow_threshold: Duration,
}

impl MetricsCollector {
    pub fn record_query_duration(&self, table: &str, op: &str, duration: Duration) {
        let key = (table.to_string(), op.to_string());
        
        // 更新直方图
        let mut histograms = self.query_duration.write().unwrap();
        let histogram = histograms.entry(key).or_insert_with(Histogram::new);
        histogram.record(duration.as_secs_f64());
        
        // 慢查询统计
        if duration > self.slow_threshold {
            self.slow_queries.fetch_add(1, Ordering::Relaxed);
        }
    }
    
    pub fn export_prometheus(&self) -> String {
        let mut output = String::new();
        
        // 连接池指标
        output.push_str(&format!(
            "# HELP db_pool_connections Current connection pool status\n\
             # TYPE db_pool_connections gauge\n\
             db_pool_connections{{state=\"total\"}} {}\n\
             db_pool_connections{{state=\"active\"}} {}\n\
             db_pool_connections{{state=\"idle\"}} {}\n",
            self.pool_connections.load(Ordering::Relaxed),
            self.pool_active.load(Ordering::Relaxed),
            self.pool_idle.load(Ordering::Relaxed),
        ));
        
        // 查询延迟
        let histograms = self.query_duration.read().unwrap();
        for ((table, op), hist) in histograms.iter() {
            output.push_str(&format!(
                "db_query_duration_seconds{{table=\"{}\",op=\"{}\",quantile=\"0.5\"}} {:.6}\n\
                 db_query_duration_seconds{{table=\"{}\",op=\"{}\",quantile=\"0.95\"}} {:.6}\n\
                 db_query_duration_seconds{{table=\"{}\",op=\"{}\",quantile=\"0.99\"}} {:.6}\n",
                table, op, hist.quantile(0.5),
                table, op, hist.quantile(0.95),
                table, op, hist.quantile(0.99),
            ));
        }
        
        // 错误计数
        output.push_str(&format!(
            "# HELP db_errors_total Total database errors\n\
             # TYPE db_errors_total counter\n\
             db_errors_total{{type=\"connection\"}} {}\n\
             db_errors_total{{type=\"query\"}} {}\n",
            self.connection_errors.load(Ordering::Relaxed),
            self.query_errors.load(Ordering::Relaxed),
        ));
        
        // 慢查询
        output.push_str(&format!(
            "# HELP db_slow_queries_total Queries exceeding threshold\n\
             # TYPE db_slow_queries_total counter\n\
             db_slow_queries_total{{threshold=\"{}ms\"}} {}\n",
            self.slow_threshold.as_millis(),
            self.slow_queries.load(Ordering::Relaxed),
        ));
        
        output
    }
}

// 对外公开API由DbPool封装:
// pub fn export_metrics(&self) -> String {
//     self.metrics_collector.export_prometheus()
// }
```

---

## 3. 数据流设计

### 3.1 查询执行流程

```
User Code: User::find_by_id(&session, 1)
     │
     ▼
┌──────────────────────────────────────┐
│  Generated CRUD Method               │
│  • Extract table name: "users"       │
│  • Extract operation: "SELECT"       │
└────────────┬─────────────────────────┘
             │
             ▼
┌──────────────────────────────────────┐
│  Session.execute()                   │
│  1. Permission check                 │
│  2. Start metrics timer              │
└────────────┬─────────────────────────┘
             │
             ▼
┌──────────────────────────────────────┐
│  Permission Guard                    │
│  • Load role policy                  │
│  • Check table="users" allowed?      │
│  • Check op="SELECT" allowed?        │
└────────────┬─────────────────────────┘
             │ ✓ Authorized
             ▼
┌──────────────────────────────────────┐
│  Sea-ORM Query Execution             │
│  • Build SQL                         │
│  • Execute via connection            │
└────────────┬─────────────────────────┘
             │
             ▼
┌──────────────────────────────────────┐
│  Metrics Recording                   │
│  • Stop timer                        │
│  • Record duration to histogram      │
│  • Increment query counter           │
└────────────┬─────────────────────────┘
             │
             ▼
        Return Result
```

### 3.2 Migration执行流程

```
cargo db-migrate up
     │
     ▼
┌──────────────────────────────────────┐
│  Load Current Schema                 │
│  • Parse Rust structs (via macro)    │
│  • Build in-memory Schema object     │
└────────────┬─────────────────────────┘
             │
             ▼
┌──────────────────────────────────────┐
│  Query Database Schema               │
│  • PostgreSQL: information_schema    │
│  • MySQL: SHOW TABLES/COLUMNS        │
│  • SQLite: sqlite_master             │
└────────────┬─────────────────────────┘
             │
             ▼
┌──────────────────────────────────────┐
│  Schema Differ                       │
│  • Compare in-memory vs database     │
│  • Generate Migration list           │
└────────────┬─────────────────────────┘
             │
             ▼
┌──────────────────────────────────────┐
│  SQL Generator                       │
│  • SELECT dialect (PG/MySQL/SQLite)  │
│  • Generate CREATE/ALTER/INDEX SQL   │
└────────────┬─────────────────────────┘
             │
             ▼
┌──────────────────────────────────────┐
│  Execute Migrations                  │
│  • BEGIN TRANSACTION                 │
│  • Execute each SQL                  │
│  • Insert to schema_migrations       │
│  • COMMIT                            │
└────────────┬─────────────────────────┘
             │
             ▼
      Migration Complete
```

------

## 4. 安全性设计

### 4.1 威胁模型分析

| 威胁类型     | 攻击场景                  | 防护措施                                                |
| ------------ | ------------------------- | ------------------------------------------------------- |
| **SQL注入**  | 用户输入拼接到SQL         | • 依赖Sea-ORM参数化查询 • 宏生成代码不拼接字符串        |
| **越权访问** | 低权限角色访问敏感表      | • Session绑定角色 • 每次查询前权限检查 • 编译时角色验证 |
| **连接泄漏** | Session未释放导致连接耗尽 | • RAII自动回收 • Drop时强制归还 • 连接池超时回收        |
| **配置泄漏** | 日志中输出密码            | • 连接字符串脱敏<br>• 密码从环境变量读取                |
| **DOS攻击**  | 恶意创建大量连接          | • 连接池max_connections限制<br>• 慢查询超时中断         |

### 4.2 权限检查性能优化

```rust
// 使用Arc<HashMap>缓存权限策略,避免每次查询都解析YAML
pub struct PermissionContext {
    role: String,
    policy_cache: Arc<HashMap<String, TablePolicy>>,
}

impl PermissionContext {
    pub fn check_table_access(&self, table: &str, op: Operation) -> Result<()> {
        // O(1)查找
        let policy = self.policy_cache
            .get(self.role.as_str())
            .ok_or(PermissionError::RoleNotFound)?;
        
        // O(n)检查,n为table数量(通常很小)
        if !policy.allows(table, op) {
            return Err(PermissionError::AccessDenied {
                role: self.role.clone(),
                table: table.to_string(),
                operation: op,
            });
        }
        
        Ok(())
    }
}
```

------

## 5. 性能优化策略

### 5.1 连接池预热

```rust
impl PoolManager {
    pub async fn initialize(&self) -> Result<()> {
        // 启动时预创建min_connections个连接
        let mut connections = Vec::new();
        for _ in 0..self.config.min_connections {
            connections.push(self.create_connection().await?);
        }
        
        // 预热查询(避免首次查询慢)
        for conn in &connections {
            conn.execute_raw("SELECT 1").await?;
        }
        
        // 放入池中
        for conn in connections {
            self.pool.push(conn);
        }
        
        Ok(())
    }
}
```

### 5.2 Metrics采样策略

```rust
// 高频查询不记录每次延迟,采用采样
pub struct MetricsCollector {
    sample_rate: f64,  // 0.1 = 10%采样率
    rng: Arc<Mutex<SmallRng>>,
}

impl MetricsCollector {
    pub fn record_query_duration(&self, table: &str, op: &str, duration: Duration) {
        // 采样决策
        let sample = {
            let mut rng = self.rng.lock().unwrap();
            rng.gen::<f64>() < self.sample_rate
        };
        
        if sample {
            // 记录到histogram
            self.histograms.write().unwrap()
                .entry((table.into(), op.into()))
                .or_insert_with(Histogram::new)
                .record(duration.as_secs_f64());
        }
        
        // 计数器始终更新
        self.query_total.fetch_add(1, Ordering::Relaxed);
    }
}
```

### 5.3 查询结果缓存(v2.0)

```rust
// 通过宏启用缓存
#[db_entity]
#[db_cache(ttl = 300, key = "user:{id}")]
struct User { ... }

// 展开为:
impl User {
    pub async fn find_by_id(session: &Session, id: i64) -> Result<Option<Self>> {
        // 1. 检查缓存
        let cache_key = format!("user:{}", id);
        if let Some(cached) = session.cache().get(&cache_key).await? {
            return Ok(Some(cached));
        }
        
        // 2. 查询数据库
        let result = session.execute(/* query */).await?;
        
        // 3. 写入缓存
        if let Some(ref user) = result {
            session.cache().set(&cache_key, user, Duration::from_secs(300)).await?;
        }
        
        Ok(result)
    }
}
```

------

## 6. 可扩展性设计

### 6.1 Feature Gate架构

```toml
[features]
default = ["sqlite"]

# 数据库驱动(互斥)
sqlite = ["sea-orm/sqlx-sqlite"]
postgres = ["sea-orm/sqlx-postgres"]
mysql = ["sea-orm/sqlx-mysql"]

# 高级特性(可选)
migration = ["sea-orm-migration"]
metrics = ["prometheus"]
cache = ["redis"]
tracing = ["opentelemetry"]

# v2.0特性
read-write-split = []
sharding = []
```

```rust
// 编译期互斥检查
#[cfg(all(feature = "sqlite", feature = "postgres"))]
compile_error!("Cannot enable both 'sqlite' and 'postgres' features");

#[cfg(all(feature = "sqlite", feature = "mysql"))]
compile_error!("Cannot enable both 'sqlite' and 'mysql' features");

#[cfg(all(feature = "postgres", feature = "mysql"))]
compile_error!("Cannot enable both 'postgres' and 'mysql' features");

#[cfg(not(any(feature = "sqlite", feature = "postgres", feature = "mysql")))]
compile_error!(
    "Must enable exactly one database feature: 'sqlite', 'postgres', or 'mysql'"
);
```

### 6.2 插件化权限引擎(v2.0预留特性)

```rust
// 预留trait,允许用户自定义权限策略
pub trait PermissionPolicy: Send + Sync {
    fn check(&self, ctx: &Context, table: &str, op: Operation) -> Result<()>;
}

// 默认实现
pub struct YamlPolicy { ... }

// 用户可以实现自定义策略
pub struct CasbinPolicy { ... }

impl DbConfig {
    pub fn with_permission_policy<P: PermissionPolicy>(self, policy: P) -> Self {
        // ...
    }
}
```

#### 6.2.1 外部权限加载机制

```rust
pub trait PolicyLoader: Send + Sync {
    fn load(&self) -> Result<Vec<PolicyRule>>;
    fn watch(&self) -> Receiver<PolicyUpdate>;
}
```


------

## 7. 错误处理设计

### 7.1 错误类型层次

```rust
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("Connection error: {0}")]
    Connection(#[from] sea_orm::DbErr),
    
    #[error("Permission denied: {0}")]
    Permission(#[from] PermissionError),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Migration error: {0}")]
    Migration(String),
    
    #[error("Transaction error: {0}")]
    Transaction(String),
}

#[derive(Debug, thiserror::Error)]
pub enum PermissionError {
    #[error("Role '{role}' not found")]
    RoleNotFound { role: String },
    
    #[error("Role '{role}' cannot access table '{table}' with operation {operation:?}")]
    AccessDenied {
        role: String,
        table: String,
        operation: Operation,
    },
    
    #[error("Role '{role}' not allowed for entity {entity}. Allowed roles: {allowed:?}")]
    RoleNotAllowed {
        entity: &'static str,
        role: String,
        allowed: Vec<&'static str>,
    },
}
```

### 7.2 错误恢复策略

```rust
// 连接失败自动重试
impl PoolManager {
    async fn get_connection(&self) -> Result<Connection> {
        let mut retries = 0;
        loop {
            match self.try_connect().await {
                Ok(conn) => return Ok(conn),
                Err(e) if retries < self.config.max_retries => {
                    warn!("Connection failed, retry {}/{}: {}", 
                          retries + 1, self.config.max_retries, e);
                    retries += 1;
                    tokio::time::sleep(Duration::from_millis(100 * retries)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
}
```

------

## 8. 关键技术决策记录

### ADR-001: 为什么选择Sea-ORM而非Diesel?

**背景**: 需要选择底层ORM框架
 **决策**: 使用Sea-ORM
 **理由**:

- 原生async支持,Diesel需要依赖diesel-async
- 动态查询构建更灵活
- 多数据库支持更统一
- 社区活跃度高

### ADR-002: 为什么Session不直接暴露Sea-ORM的DatabaseConnection?

**背景**: 安全性设计
 **决策**: 通过Session封装,不暴露原始连接
 **理由**:

- 防止用户绕过权限检查直接执行查询
- 强制RAII生命周期管理
- 统一Metrics采集点
- 为未来的连接池负载均衡预留空间

### ADR-003: 为什么权限检查在运行时而非编译时?

**背景**: 权限配置存储在YAML文件
 **决策**: 运行时检查 + 编译时角色验证
 **理由**:

- 配置文件在运行时可能更改,编译时无法获取
- 编译时验证角色名是否存在(防止typo)
- 运行时检查开销小(<0.1ms)
- 为动态权限策略(v2.0)预留空间

### ADR-004: 为什么Migration不支持自动数据迁移?

**背景**: v1.0功能范围
 **决策**: 只迁移schema,不迁移数据
 **理由**:

- 数据迁移逻辑复杂,容易出错
- 业务相关性强,难以自动化
- v1.0聚焦核心功能
- 用户可通过自定义SQL实现数据迁移
