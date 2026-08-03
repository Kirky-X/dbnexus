// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 公共类型模块
//!
//! 提供跨模块共享的类型定义

/// 分布式 ID 生成器（distributed-id feature）
#[cfg(feature = "distributed-id")]
pub mod distributed_id;

#[cfg(feature = "distributed-id")]
pub use distributed_id::{DistributedIdGenerator, IdComponents, SnowflakeError, SnowflakeIdGenerator};
