// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

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
//! - `health/`: 健康检查测试

#[cfg(feature = "audit")]
pub mod audit;
pub mod cache;
pub mod common;
pub mod core;
pub mod cross_cutting;
#[cfg(feature = "global-index")]
pub mod global_index;
#[cfg(feature = "health-check")]
pub mod health;
#[cfg(feature = "metrics")]
pub mod metrics;
#[cfg(feature = "migration")]
pub mod migration;
#[cfg(feature = "permission")]
pub mod permission;
pub mod pool;
#[cfg(feature = "sql-parser")]
pub mod security;
#[cfg(feature = "sharding")]
pub mod sharding;
#[cfg(feature = "tracing")]
pub mod tracing;
