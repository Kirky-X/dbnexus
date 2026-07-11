// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
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
    /// 缓存击穿事件数（stampede）
    pub stampede_events: AtomicU64,
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
            stampede_events: AtomicU64::new(0),
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

    /// 记录缓存击穿事件（stampede）
    pub fn record_stampede(&self) {
        self.stampede_events.fetch_add(1, Ordering::SeqCst);
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
            stampede_events: self.stampede_events.load(Ordering::SeqCst),
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
    /// 缓存击穿事件数
    pub stampede_events: u64,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_stats_all_zero() {
        let s = PermissionCheckStats::new();
        let snap = s.snapshot();
        assert_eq!(snap.total_checks, 0);
        assert_eq!(snap.allowed_checks, 0);
        assert_eq!(snap.denied_checks, 0);
        assert_eq!(snap.rate_limited_checks, 0);
        assert_eq!(snap.cache_hits, 0);
        assert_eq!(snap.cache_misses, 0);
        assert_eq!(snap.stampede_events, 0);
    }

    #[test]
    fn record_allowed_increments() {
        let s = PermissionCheckStats::new();
        s.record_allowed();
        let snap = s.snapshot();
        assert_eq!(snap.total_checks, 1);
        assert_eq!(snap.allowed_checks, 1);
        assert_eq!(snap.denied_checks, 0);
    }

    #[test]
    fn record_denied_increments() {
        let s = PermissionCheckStats::new();
        s.record_denied();
        let snap = s.snapshot();
        assert_eq!(snap.total_checks, 1);
        assert_eq!(snap.denied_checks, 1);
    }

    #[test]
    fn record_rate_limited_increments() {
        let s = PermissionCheckStats::new();
        s.record_rate_limited();
        let snap = s.snapshot();
        assert_eq!(snap.total_checks, 1);
        assert_eq!(snap.rate_limited_checks, 1);
    }

    #[test]
    fn record_cache_hit() {
        let s = PermissionCheckStats::new();
        s.record_cache_hit();
        assert_eq!(s.snapshot().cache_hits, 1);
    }

    #[test]
    fn record_cache_miss() {
        let s = PermissionCheckStats::new();
        s.record_cache_miss();
        assert_eq!(s.snapshot().cache_misses, 1);
    }

    #[test]
    fn record_stampede() {
        let s = PermissionCheckStats::new();
        s.record_stampede();
        assert_eq!(s.snapshot().stampede_events, 1);
    }

    #[test]
    fn snapshot_is_independent_copy() {
        let s = PermissionCheckStats::new();
        s.record_allowed();
        let snap1 = s.snapshot();
        s.record_allowed();
        let snap2 = s.snapshot();
        assert_eq!(snap1.total_checks, 1);
        assert_eq!(snap2.total_checks, 2);
    }

    #[test]
    fn cache_hit_rate_zero_when_no_data() {
        let snap = PermissionCheckStatsSnapshot {
            total_checks: 0,
            allowed_checks: 0,
            denied_checks: 0,
            rate_limited_checks: 0,
            cache_hits: 0,
            cache_misses: 0,
            stampede_events: 0,
        };
        assert_eq!(snap.cache_hit_rate(), 0.0);
    }

    #[test]
    fn cache_hit_rate_partial() {
        let snap = PermissionCheckStatsSnapshot {
            total_checks: 0,
            allowed_checks: 0,
            denied_checks: 0,
            rate_limited_checks: 0,
            cache_hits: 3,
            cache_misses: 1,
            stampede_events: 0,
        };
        assert!((snap.cache_hit_rate() - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn denial_rate_zero_when_no_data() {
        let snap = PermissionCheckStatsSnapshot {
            total_checks: 0,
            allowed_checks: 0,
            denied_checks: 0,
            rate_limited_checks: 0,
            cache_hits: 0,
            cache_misses: 0,
            stampede_events: 0,
        };
        assert_eq!(snap.denial_rate(), 0.0);
    }

    #[test]
    fn denial_rate_correct() {
        let snap = PermissionCheckStatsSnapshot {
            total_checks: 10,
            allowed_checks: 7,
            denied_checks: 3,
            rate_limited_checks: 0,
            cache_hits: 0,
            cache_misses: 0,
            stampede_events: 0,
        };
        assert!((snap.denial_rate() - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn cache_stats_is_copy() {
        let c = CacheStats {
            cached_roles: 5,
            capacity: 100,
        };
        assert_eq!(c.cached_roles, 5);
        assert_eq!(c.capacity, 100);
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
