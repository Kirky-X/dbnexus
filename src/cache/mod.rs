// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 缓存模块
//!
//! 基于 oxcache 的高性能缓存系统
//!
//! # Feature Requirements
//!
//! 此模块需要启用 `cache` feature。

#[cfg(feature = "cache")]
pub mod traits;
#[cfg(feature = "cache")]
pub use traits::{CacheBackend, CacheError, CacheResult};

#[cfg(feature = "cache")]
pub mod oxcache_backend;
#[cfg(feature = "cache")]
pub use oxcache_backend::OxcacheBackend;

#[cfg(feature = "cache")]
pub mod mock_backend;
#[cfg(feature = "cache")]
pub use mock_backend::MockCacheBackend;

/// 缓存键特征
pub trait CacheKey: Send + Sync + 'static {
    /// 将键转换为字符串形式用于缓存存储
    fn to_cache_key(&self) -> String;
}

impl CacheKey for String {
    fn to_cache_key(&self) -> String {
        self.clone()
    }
}

/// 缓存统计信息
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// 缓存命中次数
    pub hits: u64,
    /// 缓存未命中次数
    pub misses: u64,
    /// 缓存条目数
    pub entry_count: u64,
}

impl CacheStats {
    /// 计算缓存命中率
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}
