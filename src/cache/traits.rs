// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 缓存 trait 定义
//!
//! 定义统一的缓存后端接口，支持依赖注入

use async_trait::async_trait;
use std::time::Duration;
use thiserror::Error;

/// 缓存错误类型
#[derive(Error, Debug)]
pub enum CacheError {
    /// 缓存操作失败
    #[error("Cache operation failed: {0}")]
    Operation(String),
    /// 序列化错误
    #[error("Serialization error: {0}")]
    Serialization(String),
    /// 缓存不可用
    #[error("Cache not available: {0}")]
    NotAvailable(String),
}

/// 缓存操作结果
pub type CacheResult<T> = Result<T, CacheError>;

/// 缓存后端 trait
///
/// 定义统一的缓存操作接口，支持依赖注入。
/// 所有缓存实现必须实现此 trait。
#[async_trait]
pub trait CacheBackend: Send + Sync {
    /// 异步获取缓存值
    ///
    /// # Arguments
    /// * `key` - 缓存键
    ///
    /// # Returns
    /// * `Some(String)` - 找到的缓存值
    /// * `None` - 缓存不存在
    async fn get(&self, key: &str) -> Option<String>;

    /// 异步设置缓存值
    ///
    /// # Arguments
    /// * `key` - 缓存键
    /// * `value` - 缓存值
    /// * `ttl` - 可选的过期时间
    async fn set(&self, key: &str, value: String, ttl: Option<Duration>) -> CacheResult<()>;

    /// 异步删除缓存值
    ///
    /// # Arguments
    /// * `key` - 缓存键
    async fn delete(&self, key: &str) -> CacheResult<()>;

    /// 检查缓存键是否存在
    ///
    /// # Arguments
    /// * `key` - 缓存键
    ///
    /// # Returns
    /// * `true` - 键存在
    /// * `false` - 键不存在
    async fn exists(&self, key: &str) -> bool;

    /// 获取缓存统计信息
    ///
    /// 返回缓存的统计信息（如果实现支持）。
    fn stats(&self) -> Option<CacheBackendStats> {
        let _ = self;
        None
    }
}

/// 缓存后端统计信息
#[derive(Debug, Clone, Default)]
pub struct CacheBackendStats {
    /// 命中次数
    pub hits: u64,
    /// 未命中次数
    pub misses: u64,
    /// 当前条目数
    pub entry_count: u64,
    /// 命中率
    pub hit_rate: f64,
}
