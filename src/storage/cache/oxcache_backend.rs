// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! oxcache 后端适配器
//!
//! 将 oxcache 适配到统一的 CacheBackend trait 接口

use crate::storage::cache::traits::{CacheBackend, CacheError, CacheResult};
use async_trait::async_trait;
use std::time::Duration;
use thiserror::Error;

/// oxcache 缓存错误
#[derive(Error, Debug)]
pub enum OxcacheError {
    /// oxcache 操作错误
    #[error("oxcache error: {0}")]
    Oxcache(String),
}

/// oxcache 缓存后端适配器
///
/// 包装 oxcache 的 Cache 实现以满足 CacheBackend trait 接口
pub struct OxcacheBackend {
    inner: oxcache::Cache<String, String>,
}

impl OxcacheBackend {
    /// 创建新的 oxcache 后端（使用默认配置）
    pub async fn new() -> CacheResult<Self> {
        Self::with_capacity(1000).await
    }

    /// 创建指定容量的 oxcache 后端
    pub async fn with_capacity(capacity: u64) -> CacheResult<Self> {
        let cache = oxcache::Cache::builder()
            .capacity(capacity)
            .build()
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;
        Ok(Self { inner: cache })
    }

    /// 创建带 TTL 的 oxcache 后端
    pub async fn with_ttl(capacity: u64, _ttl: Duration) -> CacheResult<Self> {
        let cache = oxcache::Cache::builder()
            .capacity(capacity)
            .build()
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;
        Ok(Self { inner: cache })
    }
}

#[async_trait]
impl CacheBackend for OxcacheBackend {
    async fn get(&self, key: &str) -> Option<String> {
        self.inner.get(&key.to_string()).await.ok().flatten()
    }

    async fn set(&self, key: &str, value: String, _ttl: Option<Duration>) -> CacheResult<()> {
        self.inner
            .set(&key.to_string(), &value)
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> CacheResult<()> {
        self.inner
            .delete(&key.to_string())
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;
        Ok(())
    }

    async fn exists(&self, key: &str) -> bool {
        self.inner.get(&key.to_string()).await.ok().flatten().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_oxcache_backend_basic() {
        let backend = OxcacheBackend::with_capacity(100).await.unwrap();

        // Test set and get
        backend.set("key1", "value1".to_string(), None).await.unwrap();
        let result = backend.get("key1").await;
        assert_eq!(result, Some("value1".to_string()));

        // Test exists
        assert!(backend.exists("key1").await);
        assert!(!backend.exists("nonexistent").await);

        // Test delete
        backend.delete("key1").await.unwrap();
        assert!(backend.get("key1").await.is_none());
    }
}
