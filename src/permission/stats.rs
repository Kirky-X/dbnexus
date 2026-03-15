// Copyright (c) 2026 Kirky.X
//
// Licensed under MIT License
// See LICENSE file in project root for full license information.

//! 权限检查统计
//!
//! 提供权限检查的统计信息收集功能。

use std::sync::atomic::{AtomicU64, Ordering};

/// 权限检查统计信息
#[derive(Debug, Default)]
pub struct PermissionCheckStats {
    /// 总检查次数
    pub total_checks: AtomicU64,
    /// 允许的检查次数
    pub allowed_checks: AtomicU64,
    /// 拒绝的检查次数
    pub denied_checks: AtomicU64,
    /// 速率限制拒绝次数
    pub rate_limited_checks: AtomicU64,
    /// 缓存命中次数
    pub cache_hits: AtomicU64,
    /// 缓存未命中次数
    pub cache_misses: AtomicU64,
}

impl PermissionCheckStats {
    /// 创建新的统计实例
    pub fn new() -> Self {
        Self {
            total_checks: AtomicU64::new(0),
            allowed_checks: AtomicU64::new(0),
            denied_checks: AtomicU64::new(0),
            rate_limited_checks: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
        }
    }

    /// 记录检查通过
    pub fn record_allowed(&self) {
        self.total_checks.fetch_add(1, Ordering::SeqCst);
        self.allowed_checks.fetch_add(1, Ordering::SeqCst);
    }

    /// 记录检查拒绝
    pub fn record_denied(&self) {
        self.total_checks.fetch_add(1, Ordering::SeqCst);
        self.denied_checks.fetch_add(1, Ordering::SeqCst);
    }

    /// 记录速率限制拒绝
    pub fn record_rate_limited(&self) {
        self.total_checks.fetch_add(1, Ordering::SeqCst);
        self.rate_limited_checks.fetch_add(1, Ordering::SeqCst);
    }

    /// 记录缓存命中
    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::SeqCst);
    }

    /// 记录缓存未命中
    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::SeqCst);
    }

    /// 获取当前统计快照
    pub fn snapshot(&self) -> PermissionCheckStatsSnapshot {
        PermissionCheckStatsSnapshot {
            total_checks: self.total_checks.load(Ordering::SeqCst),
            allowed_checks: self.allowed_checks.load(Ordering::SeqCst),
            denied_checks: self.denied_checks.load(Ordering::SeqCst),
            rate_limited_checks: self.rate_limited_checks.load(Ordering::SeqCst),
            cache_hits: self.cache_hits.load(Ordering::SeqCst),
            cache_misses: self.cache_misses.load(Ordering::SeqCst),
        }
    }
}

/// 权限检查统计快照
#[derive(Debug, Clone)]
pub struct PermissionCheckStatsSnapshot {
    /// 总检查次数
    pub total_checks: u64,
    /// 允许的检查次数
    pub allowed_checks: u64,
    /// 拒绝的检查次数
    pub denied_checks: u64,
    /// 速率限制拒绝次数
    pub rate_limited_checks: u64,
    /// 缓存命中次数
    pub cache_hits: u64,
    /// 缓存未命中次数
    pub cache_misses: u64,
}

impl PermissionCheckStatsSnapshot {
    /// 获取缓存命中率
    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            self.cache_hits as f64 / total as f64
        }
    }

    /// 获取拒绝率
    pub fn denial_rate(&self) -> f64 {
        let total = self.total_checks;
        if total == 0 {
            0.0
        } else {
            self.denied_checks as f64 / total as f64
        }
    }
}

/// 缓存统计信息
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// 已缓存的角色数
    pub cached_roles: usize,

    /// 缓存容量
    pub capacity: usize,
}
