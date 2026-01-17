//! 测试套件根模块
//!
//! 测试组织结构:
//! - `common/`: 公共测试辅助函数
//! - `core/`: 核心功能测试
//! - `pool/`: 连接池测试
//! - `migration/`: 迁移测试
//! - `permission/`: 权限测试
//! - `audit/`: 审计测试
//! - `metrics/`: 指标测试
//! - `cache/`: 缓存测试
//! - `sharding/`: 分片测试
//! - `global_index/`: 全局索引测试
//! - `tracing/`: 追踪测试
//! - `cross_cutting/`: 横切关注点测试

pub mod common;
pub mod core;
pub mod pool;
pub mod migration;
pub mod permission;
pub mod audit;
pub mod metrics;
pub mod cache;
pub mod sharding;
pub mod global_index;
pub mod tracing;
pub mod cross_cutting;
