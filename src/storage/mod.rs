// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Storage 模块
//!
//! 提供全局索引等存储功能

#[cfg(feature = "global-index")]
pub mod global_index;

#[cfg(feature = "global-index")]
pub use global_index::{
    GlobalIndex, IndexEntry, SYNC_STATUS_FAILED, SYNC_STATUS_PENDING, SYNC_STATUS_SYNCED, SyncEvent, SyncResult,
};
