// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Storage 模块
//!
//! 提供缓存系统和全局索引等存储功能

pub mod cache;

// 单文件模块
#[cfg(feature = "global-index")]
pub mod global_index;

// Re-exports
#[cfg(feature = "cache")]
pub use cache::{CacheBackend, CacheError, CacheKey, CacheResult, OxcacheBackend};
#[cfg(all(feature = "global-index", feature = "with-json"))]
pub use global_index::{GlobalIndex, IndexEntry, SyncEvent};
