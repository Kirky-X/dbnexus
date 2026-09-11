// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 分布式事务 Saga 编排器
//!
//! 每分片独立事务 + 补偿操作，应用层协调，无跨分片锁。




// ============================================================================
// T425 大文件拆分：以下内容按职责纯移动至 types/store/orchestrator 子模块，
// pub use 保持既有 `crate::database::saga::*` 路径兼容。
// ============================================================================

mod orchestrator;
mod store;
mod types;

pub use orchestrator::*;
pub use store::*;
pub use types::*;
