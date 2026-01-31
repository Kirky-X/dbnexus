// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 缓存模块
//!
//! 基于 oxcache 的高性能缓存系统

#[cfg(feature = "oxcache")]
pub mod oxcache_adapter;

#[cfg(feature = "oxcache")]
pub use oxcache_adapter::{AsyncCache, CacheKey, CacheValue, OxcacheAdapter, create_memory_cache};

/// 缓存配置
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// 最大容量
    pub capacity: usize,
    /// TTL（秒）
    pub ttl: Option<u64>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            capacity: 1000,
            ttl: None,
        }
    }
}

impl CacheConfig {
    /// 创建新配置
    pub fn new(capacity: usize, ttl: Option<u64>) -> Self {
        Self { capacity, ttl }
    }

    /// 设置容量
    pub fn capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity;
        self
    }

    /// 设置TTL
    pub fn ttl(mut self, ttl: u64) -> Self {
        self.ttl = Some(ttl);
        self
    }
}
