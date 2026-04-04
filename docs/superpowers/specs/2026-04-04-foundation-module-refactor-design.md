# DBNexus 基础模块架构重构设计

> 日期：2026-04-04
> 状态：待批准
> 依据：`temp/BrickArchitecture/foundation_module.md`

## 1. 概述

将 DBNexus 从单体模块结构重构为符合积木架构规范的分层基础模块架构。

### 1.1 设计目标

- 基础模块零内部依赖
- 通过 mod + feature 隔离模块
- 遵循 ISP 原则拆分接口
- 支持依赖注入组装

### 1.2 约束

- 单 crate，不拆分为多个 crate
- 不需要向后兼容
- 缓存由 oxcache 提供，通过依赖注入传入
- 内存实现按需提供

## 2. 目录结构

```
dbnexus/
├── src/
│   ├── foundation/              # 基础模块层（零内部依赖）
│   │   ├── pool/               # 连接池基础模块
│   │   │   ├── mod.rs          # pub use + 工厂函数
│   │   │   ├── config.rs       # PoolConfig + validate
│   │   │   ├── error.rs        # PoolError + PoolConfigError
│   │   │   ├── interface.rs    # PoolConnector trait
│   │   │   ├── types.rs        # PoolStatus, Connection 等
│   │   │   ├── impl_/              # 实现目录（避免 Rust 关键字冲突）
│   │   │   │   ├── mod.rs
│   │   │   │   ├── default.rs  # DbPool 实现
│   │   │   │   └── memory.rs   # MemoryPool（按需）
│   │   └── mod.rs
│   │
│   ├── domain/                  # 领域模块层（可依赖 foundation + 第三方）
│   │   ├── permission/          # 权限领域模块
│   │   │   ├── mod.rs
│   │   │   ├── config.rs
│   │   │   ├── error.rs
│   │   │   ├── interface.rs    # PermissionProvider trait
│   │   │   ├── types.rs
│   │   │   └── impl/
│   │   │       ├── mod.rs
│   │   │       ├── default.rs  # YamlPermissionProvider
│   │   │       └── memory.rs   # MemoryPermissionProvider
│   │   ├── migration/           # 迁移领域模块
│   │   ├── audit/               # 审计领域模块
│   │   ├── auth/                # 认证领域模块
│   │   └── sql_parser/          # SQL 解析领域模块
│   │
│   ├── observability/           # 可观测模块层
│   │   ├── metrics/
│   │   ├── health/
│   │   └── mod.rs
│   │
│   ├── common/                  # 共享类型（非模块）
│   │   ├── error.rs            # DbNexusError 统一错误
│   │   └── types.rs
│   │
│   └── lib.rs                   # crate 入口 + feature 控制
│
├── macros/                      # 过程宏 crate（保持独立）
└── Cargo.toml
```

## 3. 依赖方向

```
┌─────────────────────────────────────────────────┐
│                    lib.rs                        │
│              (应用层组装 + API)                   │
└──────────────────────┬──────────────────────────┘
                       │
         ┌─────────────┼─────────────┐
         ▼             ▼             ▼
┌─────────────┐ ┌───────────┐ ┌──────────────┐
│   domain/   │ │observability│ │  foundation/ │
│ permission  │ │   metrics   │ │    pool      │
│ migration   │ │   health    │ │              │
│   audit     │ │             │ │              │
│   auth      │ │             │ │              │
└──────┬──────┘ └──────┬──────┘ └──────────────┘
       │               │               │
       │      ┌────────┴────────┐      │
       │      │  oxcache (外部)  │      │
       │      │  sea-orm (外部)  │      │
       │      └─────────────────┘      │
       │               │               │
       └───────────────┼───────────────┘
                       ▼
              ┌────────────────┐
              │    common/     │
              │  (共享类型)     │
              └────────────────┘
```

**规则**：

- foundation 层：零内部依赖，只依赖第三方库和标准库
- domain 层：可依赖 foundation 层和第三方库
- observability 层：相对独立
- common：共享类型，非模块概念

## 4. 模块删除

删除 `storage/cache/` 目录。缓存由 oxcache 库提供，通过依赖注入传入各模块。

## 5. Trait 设计（接口隔离）

### 5.1 Foundation 层 - Pool 模块

```rust
// foundation/pool/interface.rs

use async_trait::async_trait;
use crate::foundation::pool::types::{PoolStatus, Connection};
use crate::foundation::pool::error::PoolError;

/// 连接池读取能力
#[async_trait]
pub trait PoolReader: Send + Sync {
    /// 获取连接池状态
    fn status(&self) -> PoolStatus;

    /// 获取当前连接数
    fn connection_count(&self) -> u32;
}

/// 连接池写入能力
#[async_trait]
pub trait PoolWriter: Send + Sync {
    /// 获取连接
    async fn acquire(&self) -> Result<Connection, PoolError>;

    /// 释放连接
    async fn release(&self, conn: Connection);
}

/// 连接池生命周期管理
#[async_trait]
pub trait PoolLifecycle: Send + Sync {
    /// 健康检查
    async fn health_check(&self) -> anyhow::Result<()>;

    /// 优雅关闭
    async fn shutdown(&self);
}

/// 连接池组合 trait
pub trait PoolConnector: PoolReader + PoolWriter + PoolLifecycle + Send + Sync {}
```

### 5.2 Domain 层 - Permission 模块

```rust
// domain/permission/interface.rs

use async_trait::async_trait;
use crate::domain::permission::types::{PermissionAction, RolePolicy};
use crate::domain::permission::error::PermissionError;

/// 权限检查能力
#[async_trait]
pub trait PermissionChecker: Send + Sync {
    /// 检查权限
    async fn check(&self, role: &str, table: &str, action: PermissionAction)
        -> Result<bool, PermissionError>;
}

/// 权限策略管理能力
#[async_trait]
pub trait PolicyManager: Send + Sync {
    /// 获取角色策略
    async fn get_policy(&self, role: &str) -> Result<Option<RolePolicy>, PermissionError>;

    /// 刷新策略缓存
    async fn refresh(&self) -> Result<(), PermissionError>;
}

/// 权限生命周期
#[async_trait]
pub trait PermissionLifecycle: Send + Sync {
    async fn health_check(&self) -> anyhow::Result<()>;
    async fn shutdown(&self);
}

/// 权限提供者组合 trait
pub trait PermissionProvider: PermissionChecker + PolicyManager + PermissionLifecycle {}
```

### 5.3 ISP 原则应用

- `PoolReader` - 监控场景只需读取
- `PoolWriter` - 连接管理场景
- `PermissionChecker` - 只需检查权限的场景
- `PolicyManager` - 管理策略的场景

## 6. 配置系统

### 6.1 Pool 模块配置

```rust
// foundation/pool/config.rs

use serde::Deserialize;
use crate::foundation::pool::error::PoolConfigError;

/// 连接池配置
#[derive(Debug, Clone, Deserialize)]
pub struct PoolConfig {
    /// 数据库连接 URL（必填）
    pub url: String,

    /// 最大连接数
    #[serde(default = "PoolConfig::default_max_connections")]
    pub max_connections: u32,

    /// 最小连接数
    #[serde(default = "PoolConfig::default_min_connections")]
    pub min_connections: u32,

    /// 空闲超时（秒）
    #[serde(default = "PoolConfig::default_idle_timeout")]
    pub idle_timeout: u64,

    /// 获取连接超时（毫秒）
    #[serde(default = "PoolConfig::default_acquire_timeout")]
    pub acquire_timeout: u64,
}

impl PoolConfig {
    pub fn validate(&self) -> Result<(), PoolConfigError> {
        if self.url.is_empty() {
            return Err(PoolConfigError::MissingField("url".into()));
        }
        if self.max_connections == 0 {
            return Err(PoolConfigError::InvalidValue {
                field: "max_connections".into(),
                reason: "must be greater than 0".into(),
            });
        }
        if self.min_connections > self.max_connections {
            return Err(PoolConfigError::InvalidValue {
                field: "min_connections".into(),
                reason: "cannot exceed max_connections".into(),
            });
        }
        Ok(())
    }

    fn default_max_connections() -> u32 { 20 }
    fn default_min_connections() -> u32 { 5 }
    fn default_idle_timeout() -> u64 { 300 }
    fn default_acquire_timeout() -> u64 { 5000 }
}
```

### 6.2 Permission 模块配置

```rust
// domain/permission/config.rs

use serde::Deserialize;
use crate::domain::permission::error::PermissionConfigError;

/// 权限模块配置
#[derive(Debug, Clone, Deserialize)]
pub struct PermissionConfig {
    /// 权限策略文件路径
    pub policy_path: Option<String>,

    /// 默认策略（allow_all / deny_all）
    #[serde(default)]
    pub default_policy: DefaultPolicy,

    /// 管理员角色名称
    #[serde(default = "PermissionConfig::default_admin_role")]
    pub admin_role: String,

    /// 是否启用速率限制
    #[serde(default)]
    pub rate_limit_enabled: bool,

    /// 速率限制：最大请求数
    #[serde(default = "PermissionConfig::default_rate_limit_max")]
    pub rate_limit_max_requests: u32,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub enum DefaultPolicy {
    #[default]
    DenyAll,
    AllowAll,
}

impl PermissionConfig {
    pub fn validate(&self) -> Result<(), PermissionConfigError> {
        if self.admin_role.is_empty() {
            return Err(PermissionConfigError::MissingField("admin_role".into()));
        }
        if self.rate_limit_enabled && self.rate_limit_max_requests == 0 {
            return Err(PermissionConfigError::InvalidValue {
                field: "rate_limit_max_requests".into(),
                reason: "must be greater than 0 when rate limiting enabled".into(),
            });
        }
        Ok(())
    }

    fn default_admin_role() -> String { "admin".into() }
    fn default_rate_limit_max() -> u32 { 100 }
}
```

## 7. 错误处理

### 7.1 Foundation 层错误

```rust
// foundation/pool/error.rs

use thiserror::Error;

/// 连接池配置错误
#[derive(Debug, Error)]
pub enum PoolConfigError {
    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("invalid value for field '{field}': {reason}")]
    InvalidValue { field: String, reason: String },
}

/// 连接池运行时错误
#[derive(Debug, Error)]
pub enum PoolError {
    #[error("failed to acquire connection within timeout")]
    AcquireTimeout,

    #[error("connection pool exhausted")]
    PoolExhausted,

    #[error("connection failed: {0}")]
    ConnectionFailed(String),

    #[error("health check failed: {0}")]
    HealthCheckFailed(String),

    #[error("database error: {0}")]
    Database(String),
}
```

### 7.2 Domain 层错误

```rust
// domain/permission/error.rs

use thiserror::Error;

/// 权限配置错误
#[derive(Debug, Error)]
pub enum PermissionConfigError {
    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("invalid value for field '{field}': {reason}")]
    InvalidValue { field: String, reason: String },

    #[error("policy file not found: {0}")]
    PolicyFileNotFound(String),
}

/// 权限运行时错误
#[derive(Debug, Error)]
pub enum PermissionError {
    #[error("permission denied for {operation} on {resource}")]
    Denied { resource: String, operation: String },

    #[error("role not found: {0}")]
    RoleNotFound(String),

    #[error("invalid policy configuration: {0}")]
    InvalidPolicy(String),

    #[error("rate limit exceeded")]
    RateLimited,

    #[error("policy parse error: {0}")]
    ParseError(String),
}
```

### 7.3 统一错误类型

```rust
// common/error.rs

use thiserror::Error;
use crate::foundation::pool::error::{PoolError, PoolConfigError};
use crate::domain::permission::error::{PermissionError, PermissionConfigError};

/// DBNexus 顶层统一错误
#[derive(Debug, Error)]
pub enum DbNexusError {
    #[error(transparent)]
    Pool(#[from] PoolError),

    #[error(transparent)]
    PoolConfig(#[from] PoolConfigError),

    #[error(transparent)]
    Permission(#[from] PermissionError),

    #[error(transparent)]
    PermissionConfig(#[from] PermissionConfigError),
    // ... 其他模块错误
}

pub type DbNexusResult<T> = Result<T, DbNexusError>;
```

**核心原则**：

- 配置错误与运行时错误分离
- 不向上泄漏第三方库错误
- 所有第三方错误在 `impl/` 内部转换

## 8. 工厂函数与依赖注入

### 8.1 Foundation 层 - Pool 模块

```rust
// foundation/pool/mod.rs

mod config;
mod error;
mod interface;
mod types;
mod impl_;

pub use config::{PoolConfig, PoolConfigError};
pub use error::{PoolError, PoolConfigError};
pub use interface::{PoolConnector, PoolReader, PoolWriter, PoolLifecycle};
pub use types::{PoolStatus, Connection};

/// 标准工厂函数
pub async fn new(config: PoolConfig) -> Result<impl PoolConnector, PoolConfigError> {
    config.validate()?;
    impl_::default::DbPool::connect(config).await
}

/// 内存实现工厂函数（测试用）
pub fn new_in_memory() -> impl PoolConnector {
    impl_::memory::MemoryPool::new()
}
```

### 8.2 Domain 层 - Permission 模块

```rust
// domain/permission/mod.rs

mod config;
mod error;
mod interface;
mod types;
mod impl_;

pub use config::{PermissionConfig, PermissionConfigError, DefaultPolicy};
pub use error::{PermissionError, PermissionConfigError};
pub use interface::{PermissionProvider, PermissionChecker, PolicyManager, PermissionLifecycle};
pub use types::{PermissionAction, RolePolicy, TablePermission};

use oxcache::Cache;

/// 标准工厂函数
pub async fn new(config: PermissionConfig) -> Result<impl PermissionProvider, PermissionConfigError> {
    config.validate()?;
    impl_::default::YamlPermissionProvider::new(config).await
}

/// 带缓存注入的工厂函数
pub async fn with_cache(
    config: PermissionConfig,
    cache: std::sync::Arc<Cache<String, RolePolicy>>
) -> Result<impl PermissionProvider, PermissionConfigError> {
    config.validate()?;
    impl_::default::YamlPermissionProvider::with_cache(config, cache).await
}

/// 内存实现工厂函数
pub fn new_in_memory() -> impl PermissionProvider {
    impl_::memory::MemoryPermissionProvider::new()
}
```

### 8.3 应用层组装

```rust
// lib.rs 或应用层

use dbnexus::foundation::pool::{self as pool_module, PoolConfig};
use dbnexus::domain::permission::{self as perm_module, PermissionConfig};
use oxcache::Cache;

pub struct AppDependencies {
    pub pool: Box<dyn pool_module::PoolConnector>,
    pub permission: Box<dyn perm_module::PermissionProvider>,
}

pub async fn assemble(config: AppConfig) -> Result<AppDependencies, DbNexusError> {
    // 创建缓存
    let policy_cache = Cache::builder()
        .capacity(4096)
        .build()
        .await?;

    // 创建权限模块（注入缓存）
    let permission = perm_module::with_cache(
        config.permission,
        std::sync::Arc::new(policy_cache)
    ).await?;

    // 创建连接池
    let pool = pool_module::new(config.pool).await?;

    Ok(AppDependencies {
        pool: Box::new(pool),
        permission: Box::new(permission),
    })
}
```

## 9. 生命周期管理

### 9.1 实现示例

```rust
// foundation/pool/impl/default.rs

use async_trait::async_trait;
use crate::foundation::pool::{PoolConnector, PoolReader, PoolWriter, PoolLifecycle};
use crate::foundation::pool::{PoolConfig, PoolError, PoolStatus, Connection};

pub struct DbPool {
    config: PoolConfig,
    inner: sea_orm::DatabaseConnection,
}

impl DbPool {
    pub async fn connect(config: PoolConfig) -> Result<Self, PoolConfigError> {
        let inner = sea_orm::Database::connect(&config.url)
            .await
            .map_err(|e| PoolConfigError::InvalidValue {
                field: "url".into(),
                reason: e.to_string(),
            })?;
        Ok(Self { config, inner })
    }
}

#[async_trait]
impl PoolLifecycle for DbPool {
    async fn health_check(&self) -> anyhow::Result<()> {
        self.inner
            .ping()
            .await
            .map_err(|e| anyhow::anyhow!("pool health check failed: {}", e))
    }

    async fn shutdown(&self) {
        self.inner.close().await;
    }
}

impl PoolConnector for DbPool {}
```

### 9.2 内部可变性示例

```rust
// domain/permission/impl/memory.rs

use std::collections::HashMap;
use std::sync::RwLock;
use async_trait::async_trait;

pub struct MemoryPermissionProvider {
    /// 角色策略存储（RwLock 保护内部可变性）
    policies: RwLock<HashMap<String, RolePolicy>>,
}

impl MemoryPermissionProvider {
    pub fn new() -> Self {
        Self {
            policies: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl PermissionChecker for MemoryPermissionProvider {
    async fn check(&self, role: &str, table: &str, action: PermissionAction)
        -> Result<bool, PermissionError>
    {
        // 读锁：允许并发读
        let guard = self.policies.read().unwrap();
        let policy = guard.get(role).ok_or(PermissionError::RoleNotFound(role.into()))?;
        Ok(policy.allows(table, &action))
        // guard 自动释放
    }
}

impl PermissionProvider for MemoryPermissionProvider {}
```

**关键设计**：

- 所有 trait 方法使用 `&self`
- 内部状态通过 `RwLock`/`Mutex` 保护
- 读多写少用 `RwLock`，读写均衡用 `Mutex`
- 避免在热路径中持有锁超过必要时间

## 10. 实施计划

### 10.1 阶段一：目录结构调整

1. 创建新的目录结构
2. 迁移现有代码到对应模块
3. 删除 `storage/cache/` 模块

### 10.2 阶段二：Trait 设计

1. 定义各模块 interface.rs
2. 按 ISP 原则拆分接口
3. 实现组合 trait

### 10.3 阶段三：配置与错误

1. 定义各模块 config.rs
2. 实现 validate 方法
3. 定义各模块 error.rs
4. 创建统一错误类型

### 10.4 阶段四：实现迁移

1. 迁移现有实现到 impl/ 目录
2. 实现生命周期方法
3. 添加内存实现（按需）

### 10.5 阶段五：工厂函数

1. 实现 new() 工厂函数
2. 实现 with_xxx() 依赖注入
3. 实现 new_in_memory()（按需）

### 10.6 阶段六：集成测试

1. 更新现有测试
2. 添加模块间集成测试
3. 验证依赖注入组装

## 11. 风险与缓解

| 风险                 | 缓解措施                        |
| -------------------- | ------------------------------- |
| 大规模重构影响稳定性 | 分阶段实施，每阶段验证          |
| 依赖解耦复杂度高     | 先解耦 pool，再逐步解耦其他模块 |
| 测试覆盖不足         | 每个阶段添加对应测试            |
| API 变更影响用户     | 提供迁移指南                    |

## 12. Feature Flag 策略

### 12.1 层级 Feature

```toml
[features]
# 基础模块（互不依赖）
pool = []
cache = []  # oxcache 集成

# 领域模块（可依赖基础模块）
permission = ["oxcache"]
migration = []
audit = []
auth = []

# 可观测模块
metrics = []
health-check = []

# 组合 feature
observability = ["metrics", "health-check"]
security = ["permission", "audit", "auth"]
full = ["pool", "permission", "migration", "audit", "auth", "observability"]
```

### 12.2 Feature 隔离规则

| Feature      | 依赖      | 说明                   |
| ------------ | --------- | ---------------------- |
| `pool`       | 无        | 纯基础模块，零内部依赖 |
| `permission` | `oxcache` | 注入缓存依赖           |
| `migration`  | 无        | 独立领域模块           |
| `audit`      | 无        | 独立领域模块           |
| `auth`       | 无        | 独立领域模块           |

### 12.3 条件编译示例

```rust
// lib.rs

#[cfg(feature = "pool")]
pub mod foundation;

#[cfg(feature = "permission")]
pub mod domain;

#[cfg(feature = "metrics")]
pub mod observability;
```

## 13. 验收标准

- [ ] 所有基础模块零内部依赖
- [ ] 每个模块有完整的 config.rs、error.rs、interface.rs、types.rs
- [ ] trait 按 ISP 原则拆分
- [ ] 配置有 validate 方法
- [ ] 错误区分配置错误和运行时错误
- [ ] 工厂函数返回 Result
- [ ] 生命周期方法正确实现
- [ ] 所有测试通过
