// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 分布式事务 Saga 编排器
//!
//! 每分片独立事务 + 补偿操作，应用层协调，无跨分片锁。
//!
//! # 日志存储选择
//!
//! [`SagaOrchestrator::new`] 默认使用 `InMemorySagaLog`——**仅适用于测试**，
//! 进程重启后日志即丢失，中断的 saga 无法恢复。生产环境必须经
//! [`SagaOrchestrator::new_with_log_store`] 注入持久化存储（如
//! [`DbSagaLog`]）。执行结果中的 `persist_failures > 0` 表示日志持久化
//! 出现失败（恢复链不完整），调用方应告警或重试。

// ============================================================================
// 大文件拆分：以下内容按职责纯移动至 types/store/orchestrator 子模块，
// pub use 保持既有 `crate::database::saga::*` 路径兼容。
// ============================================================================

mod orchestrator;
mod store;
mod types;

pub use orchestrator::*;
pub use store::*;
pub use types::*;
