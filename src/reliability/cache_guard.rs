// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 缓存使用三防 — 穿透 / 击穿 / 雪崩
//!
//! 在 [`crate::domain::DbCacheProvider`] 之上提供策略层封装：
//!
//! | 防护 | 问题 | 手段 |
//! |------|------|------|
//! | 防雪崩 | 大量 key 同时过期，请求齐涌数据库 | [`jittered_ttl`]：TTL 随机抖动 |
//! | 防穿透 | 查询不存在的 key，缓存永不命中 | 负缓存：空结果写短 TTL 哨兵 |
//! | 防击穿 | 热点 key 过期瞬间并发重建 | singleflight：同 key 并发只放一个重建 |
//!
//! # 示例
//!
//! ```ignore
//! let guard = CacheGuard::new(provider);
//! let bytes = guard
//!     .get_or_load("user:42", Duration::from_secs(60), async {
//!         Ok(db.load_user_bytes(42).await) // None → 写入负缓存哨兵
//!     })
//!     .await?;
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::domain::DbCacheProvider;
use crate::foundation::DbError;

/// 负缓存哨兵：标记「数据库确认不存在」，命中时直接返回空值
pub(crate) const NEGATIVE_TOMBSTONE: &[u8] = b"\0DBNEXUS_NEGATIVE_CACHE\0";

/// 全局抖动随机状态（xorshift64，非加密用途；TTL 抖动只需不相关采样）
static JITTER_STATE: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0x9E37_79B9_7F4A_7C15);

fn next_jitter_sample() -> f64 {
    use std::sync::atomic::Ordering;
    let mut x = JITTER_STATE.fetch_add(0x2545_F491_4F6C_DD1D, Ordering::Relaxed);
    if x == 0 {
        x = 0x9E37_79B9_7F4A_7C15;
    }
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    // 归一到 [0, 1)
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// 带随机抖动的 TTL（防雪崩）
///
/// 输出落在 `[base * (1-ratio), base * (1+ratio)]`。`ratio = 0` 恒等于
/// `base`；非法 `ratio`（<0 或 >1）返回 `base`（不 panic、不静默改语义）。
pub fn jittered_ttl(base: Duration, ratio: f64) -> Duration {
    if !(0.0..=1.0).contains(&ratio) {
        return base;
    }
    if ratio == 0.0 {
        return base;
    }
    let base_ms = base.as_millis() as f64;
    let offset_ms = (next_jitter_sample() * 2.0 - 1.0) * ratio * base_ms;
    Duration::from_millis((base_ms + offset_ms).max(1.0) as u64)
}

/// 缓存三防门卫：在 [`DbCacheProvider`] 上叠加负缓存与 singleflight
pub struct CacheGuard {
    provider: Arc<dyn DbCacheProvider>,
    /// per-key 重建互斥体（singleflight）。Mutex 只保护 map 的取放（纳秒级），
    /// 锁本身在锁外 await。
    rebuild_locks: std::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl CacheGuard {
    /// 创建门卫
    pub fn new(provider: Arc<dyn DbCacheProvider>) -> Self {
        Self {
            provider,
            rebuild_locks: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// 取（或插入）key 对应的重建互斥体——短临界区，不跨 await 持锁
    fn lock_for(&self, key: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut map = self.rebuild_locks.lock().expect("rebuild locks lock");
        map.entry(key.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    /// 读取或加载（防穿透 + 防击穿）
    ///
    /// 流程：
    /// 1. 缓存命中：负缓存哨兵 → `Ok(vec![])`；正常值 → `Ok(value)`
    /// 2. 未命中：取该 key 的重建互斥体（singleflight），再查一次缓存
    ///    （double-check），仍缺失才执行 `loader`
    /// 3. loader 返回 `Some(value)` → 写缓存（TTL 经 [`jittered_ttl`] 抖动）
    ///    并返回；`None` → 写短 TTL 负缓存哨兵（`ttl/10`，最短 1s）并返回
    ///    `Ok(vec![])`
    /// 4. loader 失败：错误透传，互斥体已释放，后续调用可重试
    pub async fn get_or_load<F, Fut>(
        &self,
        key: &str,
        ttl: Duration,
        loader: F,
    ) -> Result<Vec<u8>, DbError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<Option<Vec<u8>>, DbError>>,
    {
        // 1. 快路径：缓存命中
        if let Some(bytes) = self.provider.get(key).await? {
            return Ok(self.decode(bytes));
        }

        // 2. singleflight：同 key 并发只放一个重建者
        let lock = self.lock_for(key);
        let _guard = lock.lock().await;
        // double-check：等锁期间可能已被其他调用方填充
        if let Some(bytes) = self.provider.get(key).await? {
            return Ok(self.decode(bytes));
        }

        // 3. 真重建
        match loader().await? {
            Some(value) => {
                self.provider
                    .set(key, value.clone(), Some(jittered_ttl(ttl, 0.1)))
                    .await?;
                Ok(value)
            }
            None => {
                // 负缓存：短 TTL 哨兵挡住对不存在 key 的反复穿透
                let negative_ttl = (ttl / 10).max(Duration::from_secs(1));
                self.provider
                    .set(key, NEGATIVE_TOMBSTONE.to_vec(), Some(negative_ttl))
                    .await?;
                Ok(Vec::new())
            }
        }
    }

    /// 解码缓存值：哨兵 → 空值，其余原样
    fn decode(&self, bytes: Vec<u8>) -> Vec<u8> {
        if bytes == NEGATIVE_TOMBSTONE {
            Vec::new()
        } else {
            bytes
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// 进程内 mock 缓存（与 cache_provider 测试同口径）
    struct MockCache {
        inner: std::sync::Mutex<HashMap<String, Vec<u8>>>,
    }

    impl MockCache {
        fn shared() -> Arc<Self> {
            Arc::new(Self {
                inner: std::sync::Mutex::new(HashMap::new()),
            })
        }
    }

    impl DbCacheProvider for MockCache {
        fn get<'a>(
            &'a self,
            key: &'a str,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<Option<Vec<u8>>, DbError>> + Send + 'a>,
        > {
            Box::pin(async move { Ok(self.inner.lock().expect("mock lock").get(key).cloned()) })
        }

        fn set<'a>(
            &'a self,
            key: &'a str,
            value: Vec<u8>,
            _ttl: Option<Duration>,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), DbError>> + Send + 'a>>
        {
            Box::pin(async move {
                self.inner
                    .lock()
                    .expect("mock lock")
                    .insert(key.to_string(), value);
                Ok(())
            })
        }

        fn delete<'a>(
            &'a self,
            key: &'a str,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), DbError>> + Send + 'a>>
        {
            Box::pin(async move {
                self.inner.lock().expect("mock lock").remove(key);
                Ok(())
            })
        }
    }

    /// R-cache-001: 抖动边界 + 非恒值 + ratio=0 + 非法 ratio
    #[test]
    fn jittered_ttl_stays_in_bounds_and_varies() {
        let base = Duration::from_secs(60);
        let lo = Duration::from_secs(54);
        let hi = Duration::from_secs(66);
        let mut samples = std::collections::HashSet::new();
        for _ in 0..1000 {
            let t = jittered_ttl(base, 0.1);
            assert!(t >= lo && t <= hi, "ttl {t:?} out of [{lo:?}, {hi:?}]");
            samples.insert(t);
        }
        assert!(samples.len() > 1, "1000 samples must not be constant");
        // ratio = 0 → 恒等于 base
        assert_eq!(jittered_ttl(base, 0.0), base);
        // 非法 ratio → base（不 panic）
        assert_eq!(jittered_ttl(base, -0.5), base);
        assert_eq!(jittered_ttl(base, 1.5), base);
    }

    /// R-cache-002: 负缓存——loader 返回 None 后第二次不再调用 loader
    #[tokio::test]
    async fn negative_cache_blocks_repeat_penetration() {
        let guard = CacheGuard::new(MockCache::shared());
        let calls = Arc::new(AtomicUsize::new(0));
        let key = "absent-key";

        for i in 0..3 {
            let calls = calls.clone();
            let bytes = guard
                .get_or_load(key, Duration::from_secs(60), || async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, DbError>(None)
                })
                .await
                .expect("get_or_load");
            assert!(bytes.is_empty(), "negative cache returns empty bytes");
            let _ = i;
        }
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "loader must run exactly once"
        );
    }

    /// R-cache-002: 正常值路径 cache-aside 语义
    #[tokio::test]
    async fn value_path_is_plain_cache_aside() {
        let guard = CacheGuard::new(MockCache::shared());
        let calls = Arc::new(AtomicUsize::new(0));
        for _ in 0..2 {
            let calls = calls.clone();
            let bytes = guard
                .get_or_load("present-key", Duration::from_secs(60), || async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, DbError>(Some(b"value".to_vec()))
                })
                .await
                .expect("get_or_load");
            assert_eq!(bytes, b"value");
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    /// R-cache-003: 同 key 32 路并发，loader 仅执行一次
    #[tokio::test]
    async fn singleflight_coalesces_concurrent_rebuilds() {
        let guard = Arc::new(CacheGuard::new(MockCache::shared()));
        let calls = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();
        for _ in 0..32 {
            let guard = guard.clone();
            let calls = calls.clone();
            tasks.push(tokio::spawn(async move {
                guard
                    .get_or_load("hot-key", Duration::from_secs(60), || async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        Ok::<_, DbError>(Some(b"rebuilt".to_vec()))
                    })
                    .await
                    .expect("get_or_load")
            }));
        }
        for t in tasks {
            assert_eq!(t.await.expect("join"), b"rebuilt");
        }
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "loader must execute exactly once under concurrent rebuild"
        );
    }

    /// R-cache-003: loader 失败可重试（互斥体在错误路径释放）
    #[tokio::test]
    async fn loader_failure_is_retryable() {
        let guard = CacheGuard::new(MockCache::shared());
        let calls = Arc::new(AtomicUsize::new(0));
        for _ in 0..2 {
            let calls = calls.clone();
            let result = guard
                .get_or_load("fail-key", Duration::from_secs(60), || async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Err::<Option<Vec<u8>>, _>(DbError::Query("boom".to_string()))
                })
                .await;
            assert!(result.is_err());
        }
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "each failed attempt must re-run loader"
        );
    }
}
