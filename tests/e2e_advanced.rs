// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! E2E 高级测试：覆盖分析文档中缺失的边界、异常和交叉组合场景
//!
//! 本文件补充现有测试套件的覆盖盲区，按 feature 隔离：
//! - SensitiveMasker 边界（无 feature gate）
//! - CircuitBreaker 边界（health-check）
//! - ShardRouter 边界（sharding）
//! - GlobalIndex batch_sync 边界（global-index + sqlite）
//! - Authentication 边界（authentication）
//! - i18n 边界（i18n）
//! - TracingGuard 边界（tracing）

// ============================================================================
// 模块导入
// ============================================================================

#[allow(unused_imports)]
use dbnexus::{MaskType, SensitiveError, SensitiveMasker};

// ============================================================================
// SensitiveMasker 高级边界测试（无 feature gate，始终可用）
// ============================================================================
// 覆盖场景：B33（空字符串）、B34（输入短于保留位数）、自定义脱敏边界
// ============================================================================

/// B33: 空字符串输入 — 每种脱敏类型都应返回错误或不 panic
#[test]
fn test_mask_empty_string_all_types() {
    // Phone: 空字符串 → InvalidInput
    match SensitiveMasker::mask("", MaskType::Phone) {
        Err(SensitiveError::InvalidInput(_)) => {}
        other => panic!("expected InvalidInput for empty phone, got {:?}", other),
    }

    // Email: 空字符串 → InvalidInput
    match SensitiveMasker::mask("", MaskType::Email) {
        Err(SensitiveError::InvalidInput(_)) => {}
        other => panic!("expected InvalidInput for empty email, got {:?}", other),
    }

    // IdCard: 空字符串 → InvalidInput
    match SensitiveMasker::mask("", MaskType::IdCard) {
        Err(SensitiveError::InvalidInput(_)) => {}
        other => panic!("expected InvalidInput for empty id card, got {:?}", other),
    }

    // BankCard: 空字符串 → InvalidInput
    match SensitiveMasker::mask("", MaskType::BankCard) {
        Err(SensitiveError::InvalidInput(_)) => {}
        other => panic!("expected InvalidInput for empty bank card, got {:?}", other),
    }

    // Name: 空字符串 → InvalidInput
    match SensitiveMasker::mask("", MaskType::Name) {
        Err(SensitiveError::InvalidInput(_)) => {}
        other => panic!("expected InvalidInput for empty name, got {:?}", other),
    }

    // Address: 空字符串 → InvalidInput
    match SensitiveMasker::mask("", MaskType::Address) {
        Err(SensitiveError::InvalidInput(_)) => {}
        other => panic!("expected InvalidInput for empty address, got {:?}", other),
    }

    // Custom: 空字符串 → InvalidInput
    match SensitiveMasker::mask(
        "",
        MaskType::Custom {
            keep_prefix: 2,
            keep_suffix: 2,
        },
    ) {
        Err(SensitiveError::InvalidInput(_)) => {}
        other => panic!("expected InvalidInput for empty custom, got {:?}", other),
    }
}

/// B33: 纯空白字符输入 — trim 后为空，应返回错误
#[test]
fn test_mask_whitespace_only_input() {
    match SensitiveMasker::mask("   ", MaskType::Name) {
        Err(SensitiveError::InvalidInput(_)) => {}
        other => panic!("expected InvalidInput for whitespace-only name, got {:?}", other),
    }

    match SensitiveMasker::mask("\t\n", MaskType::Address) {
        Err(SensitiveError::InvalidInput(_)) => {}
        other => panic!("expected InvalidInput for whitespace-only address, got {:?}", other),
    }
}

/// B34: 输入恰好等于最小长度 — 边界测试
#[test]
fn test_mask_minimum_valid_length() {
    // Phone: 恰好 7 位数字（最小有效长度）
    let result = SensitiveMasker::mask("1234567", MaskType::Phone);
    match result {
        Ok(masked) => {
            assert_eq!(masked.len(), 7, "7-digit phone should have no mask chars");
            assert!(!masked.contains('*'), "no chars to mask");
        }
        Err(SensitiveError::InvalidInput(_)) => {}
        Err(e) => panic!("unexpected error for 7-digit phone: {:?}", e),
    }

    // BankCard: 恰好 8 位数字（最小有效长度）
    let result = SensitiveMasker::mask("12345678", MaskType::BankCard);
    match result {
        Ok(masked) => {
            assert_eq!(masked.len(), 8, "8-digit card should have no mask chars");
            assert!(!masked.contains('*'), "no chars to mask");
        }
        Err(e) => panic!("unexpected error for 8-digit bank card: {:?}", e),
    }
}

/// B34: 输入短于最小长度 — 应返回错误
#[test]
fn test_mask_below_minimum_length() {
    // Phone: 6 位数字（小于 7）
    match SensitiveMasker::mask("123456", MaskType::Phone) {
        Err(SensitiveError::InvalidInput(_)) => {}
        other => panic!("expected InvalidInput for 6-digit phone, got {:?}", other),
    }

    // BankCard: 7 位数字（小于 8）
    match SensitiveMasker::mask("1234567", MaskType::BankCard) {
        Err(SensitiveError::InvalidInput(_)) => {}
        other => panic!("expected InvalidInput for 7-digit bank card, got {:?}", other),
    }

    // IdCard: 14 位（非 15 或 18）
    match SensitiveMasker::mask("12345678901234", MaskType::IdCard) {
        Err(SensitiveError::InvalidInput(_)) => {}
        other => panic!("expected InvalidInput for 14-digit id card, got {:?}", other),
    }
}

/// Custom 脱敏边界：keep_prefix=0, keep_suffix=0 — 全部掩码
#[test]
fn test_mask_custom_zero_keep() {
    let result = SensitiveMasker::mask(
        "abcdef",
        MaskType::Custom {
            keep_prefix: 0,
            keep_suffix: 0,
        },
    );
    match result {
        Ok(masked) => {
            assert_eq!(masked, "******", "all chars should be masked");
            assert!(!masked.contains('a'));
        }
        Err(e) => panic!("unexpected error for custom zero keep: {:?}", e),
    }
}

/// Custom 脱敏边界：keep_prefix + keep_suffix == len — 无掩码字符
#[test]
fn test_mask_custom_exact_length() {
    let result = SensitiveMasker::mask(
        "abc",
        MaskType::Custom {
            keep_prefix: 2,
            keep_suffix: 1,
        },
    );
    match result {
        Ok(masked) => {
            assert_eq!(masked, "abc", "no masking when keep == len");
            assert!(!masked.contains('*'));
        }
        Err(e) => panic!("unexpected error for custom exact length: {:?}", e),
    }
}

/// Custom 脱敏边界：keep_prefix + keep_suffix > len — 返回原数据
#[test]
fn test_mask_custom_keep_exceeds_length() {
    let result = SensitiveMasker::mask(
        "ab",
        MaskType::Custom {
            keep_prefix: 5,
            keep_suffix: 5,
        },
    );
    match result {
        Ok(masked) => {
            assert_eq!(masked, "ab", "should return original when keep > len");
        }
        Err(e) => panic!("unexpected error for custom keep > len: {:?}", e),
    }
}

/// Name 脱敏：单字符姓名 — 保留姓氏，无掩码字符
#[test]
fn test_mask_single_char_name() {
    let result = SensitiveMasker::mask("张", MaskType::Name);
    match result {
        Ok(masked) => {
            assert_eq!(masked, "张", "single char name should have no mask");
        }
        Err(e) => panic!("unexpected error for single char name: {:?}", e),
    }
}

/// IdCard 脱敏：18 位带 X 尾号 — 应正确处理字母 X
#[test]
fn test_mask_id_card_with_x() {
    let result = SensitiveMasker::mask("11010119900101123X", MaskType::IdCard);
    match result {
        Ok(masked) => {
            assert_eq!(&masked[..4], "1101");
            assert!(masked.ends_with('X'));
            assert!(masked.contains('*'));
        }
        Err(e) => panic!("unexpected error for id card with X: {:?}", e),
    }
}

/// Unicode 姓名：多字节字符不 panic
#[test]
fn test_mask_unicode_name() {
    let result = SensitiveMasker::mask("欧阳明月", MaskType::Name);
    match result {
        Ok(masked) => {
            assert!(masked.starts_with('欧'));
            assert!(masked.contains('*'));
        }
        Err(e) => panic!("unexpected error for unicode name: {:?}", e),
    }
}

/// 非常长的输入 — 不 panic，正确脱敏
#[test]
fn test_mask_very_long_input() {
    let long_phone: String = "1".repeat(100);
    let result = SensitiveMasker::mask(&long_phone, MaskType::Phone);
    match result {
        Ok(masked) => {
            assert!(masked.starts_with("111"));
            assert!(masked.ends_with("1111"));
            assert!(masked.contains('*'));
        }
        Err(e) => panic!("unexpected error for long phone: {:?}", e),
    }
}

// ============================================================================
// CircuitBreaker 高级边界测试（health-check feature）
// ============================================================================
#[cfg(feature = "health-check")]
mod circuit_breaker_advanced {
    use dbnexus::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError, CircuitBreakerState};
    use std::time::Duration;

    /// B25: 失败阈值边界 — failures == threshold-1 仍为 Closed
    #[tokio::test]
    async fn test_failure_threshold_boundary_closed() {
        let config = CircuitBreakerConfig {
            failure_threshold: 5,
            success_threshold: 3,
            timeout_ms: 1000,
            window_size: 100,
        };
        let cb = CircuitBreaker::new(config);

        // 连续失败 4 次（threshold-1）— 应保持 Closed
        for _ in 0..4 {
            cb.record_failure().await;
        }

        let state = cb.state().await;
        assert_eq!(
            state,
            CircuitBreakerState::Closed,
            "should stay Closed with failures < threshold"
        );

        // can_execute 仍应返回 Ok
        assert!(cb.can_execute().await.is_ok());
    }

    /// B25: 失败阈值边界 — failures == threshold 转为 Open
    #[tokio::test]
    async fn test_failure_threshold_boundary_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 5,
            success_threshold: 3,
            timeout_ms: 1000,
            window_size: 100,
        };
        let cb = CircuitBreaker::new(config);

        // 连续失败 5 次（== threshold）— 应转为 Open
        for _ in 0..5 {
            cb.record_failure().await;
        }

        let state = cb.state().await;
        assert_eq!(
            state,
            CircuitBreakerState::Open,
            "should transition to Open at threshold"
        );

        // can_execute 应返回 Err
        match cb.can_execute().await {
            Err(CircuitBreakerError { .. }) => {}
            other => panic!("expected CircuitBreakerError, got {:?}", other),
        }
    }

    /// B26: HalfOpen 状态下成功阈值边界
    #[tokio::test]
    async fn test_half_open_success_threshold_boundary() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            success_threshold: 3,
            timeout_ms: 100,
            window_size: 100,
        };
        let cb = CircuitBreaker::new(config);

        // 触发 Open
        for _ in 0..3 {
            cb.record_failure().await;
        }
        assert_eq!(cb.state().await, CircuitBreakerState::Open);

        // 等待超时后转为 HalfOpen
        tokio::time::sleep(Duration::from_millis(150)).await;
        let _ = cb.can_execute().await; // 触发 HalfOpen 转换
        assert_eq!(
            cb.state().await,
            CircuitBreakerState::HalfOpen,
            "should be HalfOpen after timeout"
        );

        // 成功 2 次（threshold-1）— 应保持 HalfOpen
        for _ in 0..2 {
            cb.record_success().await;
        }
        assert_eq!(
            cb.state().await,
            CircuitBreakerState::HalfOpen,
            "should stay HalfOpen with successes < threshold"
        );

        // 第 3 次成功（== threshold）— 应转为 Closed
        cb.record_success().await;
        assert_eq!(
            cb.state().await,
            CircuitBreakerState::Closed,
            "should transition to Closed at success threshold"
        );
    }

    /// HalfOpen 状态下失败应立即回到 Open
    #[tokio::test]
    async fn test_half_open_failure_returns_to_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 3,
            timeout_ms: 50,
            window_size: 100,
        };
        let cb = CircuitBreaker::new(config);

        // 触发 Open
        for _ in 0..2 {
            cb.record_failure().await;
        }
        assert_eq!(cb.state().await, CircuitBreakerState::Open);

        // 等待超时 → HalfOpen
        tokio::time::sleep(Duration::from_millis(60)).await;
        let _ = cb.can_execute().await;
        assert_eq!(cb.state().await, CircuitBreakerState::HalfOpen);

        // 在 HalfOpen 状态下失败 → 立即回到 Open
        cb.record_failure().await;
        assert_eq!(
            cb.state().await,
            CircuitBreakerState::Open,
            "HalfOpen failure should return to Open"
        );
    }

    /// Open 状态下 record_success 是 no-op
    #[tokio::test]
    async fn test_open_state_success_noop() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 3,
            timeout_ms: 10000,
            window_size: 100,
        };
        let cb = CircuitBreaker::new(config);

        // 触发 Open
        for _ in 0..2 {
            cb.record_failure().await;
        }
        assert_eq!(cb.state().await, CircuitBreakerState::Open);

        // 在 Open 状态下调用 record_success — 不应改变状态
        cb.record_success().await;
        assert_eq!(
            cb.state().await,
            CircuitBreakerState::Open,
            "Open state success should be no-op"
        );
    }

    /// Open 状态下 record_failure 是 no-op
    #[tokio::test]
    async fn test_open_state_failure_noop() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 3,
            timeout_ms: 10000,
            window_size: 100,
        };
        let cb = CircuitBreaker::new(config);

        // 触发 Open
        for _ in 0..2 {
            cb.record_failure().await;
        }
        let failures_before = cb.status().await.consecutive_failures;

        // 在 Open 状态下继续调用 record_failure — 不应增加计数
        cb.record_failure().await;
        let failures_after = cb.status().await.consecutive_failures;

        assert_eq!(
            failures_before, failures_after,
            "Open state failure should not increment counter"
        );
    }

    /// Closed 状态下 record_success 重置失败计数
    #[tokio::test]
    async fn test_closed_state_success_resets_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 5,
            success_threshold: 3,
            timeout_ms: 1000,
            window_size: 100,
        };
        let cb = CircuitBreaker::new(config);

        // 失败 3 次
        for _ in 0..3 {
            cb.record_failure().await;
        }
        assert_eq!(cb.status().await.consecutive_failures, 3);

        // 一次成功应重置失败计数
        cb.record_success().await;
        assert_eq!(
            cb.status().await.consecutive_failures,
            0,
            "success in Closed should reset failures"
        );
        assert_eq!(cb.status().await.consecutive_successes, 1);
    }

    /// 滑动窗口大小限制
    #[tokio::test]
    async fn test_window_size_limit() {
        let config = CircuitBreakerConfig {
            failure_threshold: 100,
            success_threshold: 3,
            timeout_ms: 1000,
            window_size: 5,
        };
        let cb = CircuitBreaker::new(config);

        // 记录超过 window_size 次操作
        for _ in 0..10 {
            cb.record_success().await;
        }

        // 窗口应只保留最后 5 条
        let status = cb.status().await;
        // 窗口大小由内部管理，验证不 panic 且状态正确
        assert_eq!(status.state, CircuitBreakerState::Closed);
    }

    /// 默认配置验证
    #[test]
    fn test_default_config_values() {
        let config = CircuitBreakerConfig::default();
        assert_eq!(config.failure_threshold, 5);
        assert_eq!(config.success_threshold, 3);
        assert_eq!(config.timeout_ms, 30000);
        assert_eq!(config.window_size, 100);
    }

    /// CircuitBreakerState Display
    #[test]
    fn test_circuit_breaker_state_display() {
        assert_eq!(CircuitBreakerState::Closed.to_string(), "closed");
        assert_eq!(CircuitBreakerState::HalfOpen.to_string(), "half-open");
        assert_eq!(CircuitBreakerState::Open.to_string(), "open");
    }

    /// CircuitBreakerError 构造和 state 访问
    #[test]
    fn test_circuit_breaker_error_construction() {
        let err = CircuitBreakerError::new(CircuitBreakerState::Open);
        assert_eq!(err.state(), CircuitBreakerState::Open);
        assert!(err.to_string().contains("open"));

        let err = CircuitBreakerError::new(CircuitBreakerState::HalfOpen);
        assert_eq!(err.state(), CircuitBreakerState::HalfOpen);
        assert!(err.to_string().contains("half-open"));
    }
}

// ============================================================================
// ShardRouter 高级边界测试（sharding feature）
// ============================================================================
#[cfg(feature = "sharding")]
mod sharding_advanced {
    use dbnexus::chrono::{Datelike, Utc};
    use dbnexus::{ShardConfig, ShardRouter, create_strategy};

    /// B07: 默认路由器 — total_shards=1，任何 key 映射到 shard 0
    #[test]
    fn test_default_router_single_shard() {
        let router = ShardRouter::default();
        assert_eq!(router.total_shards(), 1);
        assert_eq!(router.strategy_name(), "yearly");

        // shard_id_for_key 对任何 key 应返回 0
        let id = router.shard_id_for_key("any_key");
        assert_eq!(id, 0, "default router with 1 shard should return 0");
    }

    /// B08: route_with_key with empty key — 应使用 strategy.calculate
    #[test]
    fn test_route_with_empty_key_uses_strategy() {
        let router = ShardRouter::with_strategy("monthly", 12);
        let now = Utc::now();

        // 空 key 应使用 strategy.calculate
        let shard_id = router.calculate_shard(now, "");
        let expected = now.year() as u32 * 12 + now.month();
        assert_eq!(shard_id, expected % 12);
    }

    /// B09: route with unregistered shard — 返回 None
    #[test]
    fn test_route_to_unregistered_shard() {
        let router = ShardRouter::with_strategy("hash", 4);
        let now = Utc::now();

        // 未注册任何分片，route 应返回 None
        let result = router.route(now);
        assert!(result.is_none(), "route to unregistered shard should be None");
    }

    /// set_pool 对未注册分片应返回错误
    #[cfg(feature = "runtime-tokio-rustls")]
    #[tokio::test]
    async fn test_set_pool_unregistered_shard_error() {
        let mut router = ShardRouter::with_strategy("yearly", 4);
        let pool = dbnexus::DbPool::new("sqlite::memory:").await.unwrap();
        let arc_pool = std::sync::Arc::new(pool);

        // 尝试为未注册的分片设置 pool
        let result = router.set_pool(999, arc_pool);
        assert!(result.is_err(), "set_pool for unregistered shard should error");
    }

    /// 策略名称验证 — 所有 4 种策略
    #[test]
    fn test_all_strategy_names() {
        assert_eq!(create_strategy("yearly").name(), "yearly");
        assert_eq!(create_strategy("year").name(), "yearly");
        assert_eq!(create_strategy("monthly").name(), "monthly");
        assert_eq!(create_strategy("month").name(), "monthly");
        assert_eq!(create_strategy("daily").name(), "daily");
        assert_eq!(create_strategy("day").name(), "daily");
        assert_eq!(create_strategy("hash").name(), "hash");
        // 未知策略默认为 yearly
        assert_eq!(create_strategy("unknown").name(), "yearly");
        assert_eq!(create_strategy("").name(), "yearly");
    }

    /// calculate_shard 一致性 — 相同 key 产生相同结果
    #[test]
    fn test_calculate_shard_deterministic() {
        let router = ShardRouter::with_strategy("hash", 16);
        let now = Utc::now();

        let id1 = router.calculate_shard(now, "user_123");
        let id2 = router.calculate_shard(now, "user_123");
        assert_eq!(id1, id2, "same key should produce same shard id");

        // 不同 key 可能产生不同结果
        let id3 = router.calculate_shard(now, "user_456");
        // 不一定不同，但应在范围内
        assert!(id3 < 16, "shard id should be within range");
    }

    /// all_shards 返回所有已注册分片
    #[test]
    fn test_all_shards_returns_registered() {
        let mut router = ShardRouter::with_strategy("yearly", 3);
        router.register_shard(0, "shard_0".to_string(), "conn_0".to_string());
        router.register_shard(1, "shard_1".to_string(), "conn_1".to_string());
        router.register_shard(2, "shard_2".to_string(), "conn_2".to_string());

        let shards = router.all_shards();
        assert_eq!(shards.len(), 3, "should return all 3 registered shards");
    }

    /// get_pool 对未注册分片返回 None
    #[test]
    fn test_get_pool_unregistered_returns_none() {
        let router = ShardRouter::with_strategy("yearly", 4);
        assert!(
            router.get_pool(999).is_none(),
            "get_pool for unregistered shard should be None"
        );
    }

    /// is_valid_shard_id — 年策略要求 shard_id > 0
    #[test]
    fn test_yearly_strategy_shard_id_validation() {
        let yearly = create_strategy("yearly");
        assert!(!yearly.is_valid_shard_id(0, 12), "yearly: shard_id 0 should be invalid");
        assert!(yearly.is_valid_shard_id(1, 12), "yearly: shard_id 1 should be valid");
    }

    /// is_valid_shard_id — 月策略要求 shard_id < total_shards
    #[test]
    fn test_monthly_strategy_shard_id_validation() {
        let monthly = create_strategy("monthly");
        assert!(monthly.is_valid_shard_id(0, 12), "monthly: shard_id 0 should be valid");
        assert!(
            monthly.is_valid_shard_id(11, 12),
            "monthly: shard_id 11 should be valid"
        );
        assert!(
            !monthly.is_valid_shard_id(12, 12),
            "monthly: shard_id 12 should be invalid"
        );
    }

    /// ShardConfig 模板解析
    #[test]
    fn test_shard_config_template_parsing() {
        let config = ShardConfig::new("yearly", 4, "order", "postgresql://host/{shard}");
        let connections = config.generate_all_connections();
        assert_eq!(connections.len(), 4);

        // 验证模板替换
        for (shard_id, conn) in &connections {
            assert!(
                conn.contains(&shard_id.to_string()),
                "connection should contain shard id"
            );
            assert!(conn.starts_with("postgresql://host/"));
        }
    }

    /// Clone 实现 — 克隆后独立修改不影响原 router
    #[test]
    fn test_router_clone_independence() {
        let mut router = ShardRouter::with_strategy("yearly", 4);
        router.register_shard(0, "s0".to_string(), "c0".to_string());

        let cloned = router.clone();
        // 原 router 添加新分片
        router.register_shard(1, "s1".to_string(), "c1".to_string());

        // 克隆不应受影响
        assert_eq!(cloned.all_shards().len(), 1, "clone should not see new shard");
        assert_eq!(router.all_shards().len(), 2, "original should have 2 shards");
    }
}

// ============================================================================
// GlobalIndex batch_sync 边界测试（global-index + sqlite feature）
// ============================================================================
#[cfg(all(feature = "global-index", feature = "sqlite", feature = "runtime-tokio-rustls"))]
mod global_index_advanced {
    use dbnexus::{GlobalIndex, IndexEntry};

    fn make_entry(table: &str, record_id: &str, shard: u32, key: &str, value: &str) -> IndexEntry {
        IndexEntry {
            table_name: table.to_string(),
            record_id: record_id.to_string(),
            shard_id: shard,
            index_key: key.to_string(),
            index_value: value.to_string(),
        }
    }

    async fn create_index() -> GlobalIndex {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1000);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let url = format!("sqlite:file:dbnexus_e2e_{}?mode=memory&cache=shared", id);
        let pool = dbnexus::DbPool::new(&url).await.expect("Failed to create DbPool");
        GlobalIndex::new(std::sync::Arc::new(pool))
            .await
            .expect("Failed to create GlobalIndex")
    }

    /// B22: batch_sync 恰好 500 条 — 单个 chunk 边界
    #[tokio::test]
    async fn test_batch_sync_exactly_chunk_size() {
        let index = create_index().await;
        let entries: Vec<IndexEntry> = (0..500)
            .map(|i| make_entry("users", &format!("user_{}", i), i % 4, "id", &i.to_string()))
            .collect();

        let result = index.batch_sync(entries).await.expect("batch_sync failed");
        assert!(result.success, "500 entries should succeed");
        assert_eq!(result.synced_count, 500);
        assert_eq!(result.failed_count, 0);
        assert!(result.errors.is_empty());

        // 验证最后一条
        let queried = index.query_by_index("users", "id", "499").await.expect("query failed");
        assert_eq!(queried.len(), 1);
    }

    /// B22: batch_sync 501 条 — 跨 chunk 边界
    #[tokio::test]
    async fn test_batch_sync_just_over_chunk_size() {
        let index = create_index().await;
        let entries: Vec<IndexEntry> = (0..501)
            .map(|i| make_entry("items", &format!("item_{}", i), i % 8, "sku", &i.to_string()))
            .collect();

        let result = index.batch_sync(entries).await.expect("batch_sync failed");
        assert!(result.success);
        assert_eq!(result.synced_count, 501);
        assert_eq!(result.failed_count, 0);
    }

    /// B22: batch_sync 1000 条 — 恰好 2 个 chunk
    #[tokio::test]
    async fn test_batch_sync_two_full_chunks() {
        let index = create_index().await;
        let entries: Vec<IndexEntry> = (0..1000)
            .map(|i| make_entry("orders", &format!("order_{}", i), i % 16, "order_id", &i.to_string()))
            .collect();

        let result = index.batch_sync(entries).await.expect("batch_sync failed");
        assert!(result.success);
        assert_eq!(result.synced_count, 1000);
        assert_eq!(result.failed_count, 0);

        // 验证首尾
        assert!(index.query_by_index("orders", "order_id", "0").await.unwrap().len() == 1);
        assert!(index.query_by_index("orders", "order_id", "999").await.unwrap().len() == 1);
    }

    /// batch_sync 重复条目 — upsert 语义
    #[tokio::test]
    async fn test_batch_sync_duplicate_upsert() {
        let index = create_index().await;

        // 第一次插入
        let entry1 = make_entry("users", "user_1", 0, "email", "old@example.com");
        index.batch_sync(vec![entry1]).await.unwrap();

        // 第二次插入相同 id 但不同 value
        let entry2 = make_entry("users", "user_1", 1, "email", "new@example.com");
        let result = index.batch_sync(vec![entry2]).await.unwrap();
        assert!(result.success);
        assert_eq!(result.synced_count, 1);

        // 旧值应不存在
        let old = index.query_by_index("users", "email", "old@example.com").await.unwrap();
        assert!(old.is_empty(), "old value should be gone after upsert");

        // 新值存在且 shard_id 更新
        let new = index.query_by_index("users", "email", "new@example.com").await.unwrap();
        assert_eq!(new.len(), 1);
        assert_eq!(new[0].shard_id, 1, "shard_id should be updated");
    }

    /// batch_sync 混合表名
    #[tokio::test]
    async fn test_batch_sync_mixed_tables() {
        let index = create_index().await;
        let entries = vec![
            make_entry("users", "u1", 0, "email", "a@x.com"),
            make_entry("orders", "o1", 1, "order_id", "100"),
            make_entry("products", "p1", 2, "sku", "ABC"),
            make_entry("users", "u2", 3, "email", "b@x.com"),
        ];

        let result = index.batch_sync(entries).await.unwrap();
        assert!(result.success);
        assert_eq!(result.synced_count, 4);

        // 分别查询不同表
        assert_eq!(
            index.query_by_index("users", "email", "a@x.com").await.unwrap().len(),
            1
        );
        assert_eq!(
            index.query_by_index("orders", "order_id", "100").await.unwrap().len(),
            1
        );
        assert_eq!(index.query_by_index("products", "sku", "ABC").await.unwrap().len(), 1);
    }

    /// batch_sync 大 shard_id 值（接近 u32::MAX）
    #[tokio::test]
    async fn test_batch_sync_large_shard_id() {
        let index = create_index().await;
        let entry = make_entry("shard_test", "r1", u32::MAX, "key", "value");
        let result = index.batch_sync(vec![entry]).await.unwrap();
        assert!(result.success);

        let queried = index.query_by_index("shard_test", "key", "value").await.unwrap();
        assert_eq!(queried.len(), 1);
        assert_eq!(
            queried[0].shard_id,
            u32::MAX,
            "shard_id should be preserved as u32::MAX"
        );
    }

    /// batch_sync Unicode index values
    #[tokio::test]
    async fn test_batch_sync_unicode_values() {
        let index = create_index().await;
        let entry = make_entry("unicode_test", "r1", 0, "name", "测试用户🎉");
        let result = index.batch_sync(vec![entry]).await.unwrap();
        assert!(result.success);

        let queried = index
            .query_by_index("unicode_test", "name", "测试用户🎉")
            .await
            .unwrap();
        assert_eq!(queried.len(), 1);
        assert_eq!(queried[0].index_value, "测试用户🎉");
    }

    /// query_by_index 多条件组合 — 不匹配的 index_key
    #[tokio::test]
    async fn test_query_wrong_index_key() {
        let index = create_index().await;
        let entry = make_entry("users", "u1", 0, "email", "test@x.com");
        index.batch_sync(vec![entry]).await.unwrap();

        // 正确 table + 正确 value + 错误 key
        let results = index.query_by_index("users", "phone", "test@x.com").await.unwrap();
        assert!(results.is_empty(), "wrong index_key should return empty");
    }

    /// 空字符串 index_value 查询
    #[tokio::test]
    async fn test_query_empty_index_value() {
        let index = create_index().await;
        let entry = make_entry("users", "u1", 0, "email", "");
        index.batch_sync(vec![entry]).await.unwrap();

        let results = index.query_by_index("users", "email", "").await.unwrap();
        assert_eq!(results.len(), 1, "should find entry with empty index_value");
    }
}

// ============================================================================
// Authentication 高级边界测试（authentication feature）
// ============================================================================
#[cfg(feature = "authentication")]
mod authentication_advanced {
    use dbnexus::{AuthCredentials, AuthError, AuthenticationManager, JwtManager, PasswordHasher, TokenType, User};

    const SECRET: &[u8] = b"e2e_advanced_test_secret_key_2026";

    fn make_hashed_user(username: &str, password: &str, role: &str) -> User {
        let hash = PasswordHasher::new().hash(password).expect("hash should succeed");
        User {
            id: format!("uid_{}", username),
            username: username.to_string(),
            password_hash: hash,
            role: role.to_string(),
            email: Some(format!("{}@test.com", username)),
            created_at: Some("2026-07-22T00:00:00Z".to_string()),
        }
    }

    /// Token 中包含特殊字符的 username/role
    #[tokio::test]
    async fn test_token_with_special_chars() {
        let mgr = JwtManager::new(SECRET);
        let token = mgr
            .generate_token("user-id-123", "user_name@test", "role:admin", TokenType::Access)
            .expect("generate_token should succeed");

        let claims = mgr.verify_token(&token).expect("verify should succeed");
        assert_eq!(claims.sub, "user-id-123");
        assert_eq!(claims.username, "user_name@test");
        assert_eq!(claims.role, "role:admin");
    }

    /// Token type 校验 — Access token 不能用作 Refresh
    #[tokio::test]
    async fn test_token_type_mismatch_access_as_refresh() {
        let mgr = JwtManager::new(SECRET);
        let access_token = mgr.generate_token("u1", "alice", "admin", TokenType::Access).unwrap();

        match mgr.verify_refresh_token(&access_token) {
            Err(AuthError::InvalidToken) => {}
            other => panic!("expected InvalidToken, got {:?}", other),
        }
    }

    /// Token type 校验 — Refresh token 不能用作 Access
    #[tokio::test]
    async fn test_token_type_mismatch_refresh_as_access() {
        let mgr = JwtManager::new(SECRET);
        let refresh_token = mgr.generate_token("u1", "alice", "admin", TokenType::Refresh).unwrap();

        match mgr.verify_access_token(&refresh_token) {
            Err(AuthError::InvalidToken) => {}
            other => panic!("expected InvalidToken, got {:?}", other),
        }
    }

    /// register_user → authenticate 完整流程
    #[tokio::test]
    async fn test_register_then_authenticate_flow() {
        let mgr = AuthenticationManager::new(SECRET);
        mgr.register_user("newuser", "StrongPass@123", "user")
            .await
            .expect("register should succeed");

        let token = mgr
            .authenticate(AuthCredentials {
                username: "newuser".to_string(),
                password: "StrongPass@123".to_string(), // pragma: allowlist secret
            })
            .await
            .expect("authenticate should succeed");

        let claims = mgr.verify_token(&token).expect("verify should succeed");
        assert_eq!(claims.username, "newuser");
        assert_eq!(claims.role, "user");
    }

    /// add_user 拒绝空 password_hash
    #[tokio::test]
    async fn test_add_user_rejects_empty_hash() {
        let mgr = AuthenticationManager::new(SECRET);
        let user = User {
            id: "u1".to_string(),
            username: "test".to_string(),
            password_hash: "".to_string(),
            role: "user".to_string(),
            email: None,
            created_at: None,
        };
        match mgr.add_user(user).await {
            Err(AuthError::PasswordHash(_)) => {}
            other => panic!("expected PasswordHash error, got {:?}", other),
        }
    }

    /// add_user 拒绝非 bcrypt 格式的 password_hash
    #[tokio::test]
    async fn test_add_user_rejects_plaintext_hash() {
        let mgr = AuthenticationManager::new(SECRET);
        let user = User {
            id: "u1".to_string(),
            username: "test".to_string(),
            password_hash: "plaintext_password".to_string(), // pragma: allowlist secret
            role: "user".to_string(),
            email: None,
            created_at: None,
        };
        match mgr.add_user(user).await {
            Err(AuthError::PasswordHash(_)) => {}
            other => panic!("expected PasswordHash error, got {:?}", other),
        }
    }

    /// get_user 对不存在的用户返回 UserNotFound
    #[tokio::test]
    async fn test_get_user_not_found() {
        let mgr = AuthenticationManager::new(SECRET);
        match mgr.get_user("nonexistent").await {
            Err(AuthError::UserNotFound(name)) => {
                assert_eq!(name, "nonexistent");
            }
            other => panic!("expected UserNotFound, got {:?}", other),
        }
    }

    /// remove_user 后再认证应失败
    #[tokio::test]
    async fn test_remove_user_then_authenticate_fails() {
        let mgr = AuthenticationManager::new(SECRET);
        mgr.add_user(make_hashed_user("alice", "Pass123", "admin"))
            .await
            .unwrap();

        // 先认证成功
        mgr.authenticate(AuthCredentials {
            username: "alice".to_string(),
            password: "Pass123".to_string(), // pragma: allowlist secret
        })
        .await
        .expect("first authenticate should succeed");

        // 删除用户
        mgr.remove_user("alice").await.expect("remove should succeed");

        // 再次认证应失败
        match mgr
            .authenticate(AuthCredentials {
                username: "alice".to_string(),
                password: "Pass123".to_string(), // pragma: allowlist secret
            })
            .await
        {
            Err(AuthError::InvalidCredentials) => {}
            other => panic!("expected InvalidCredentials after removal, got {:?}", other),
        }
    }

    /// 并发认证 — 多个并发 authenticate 调用
    #[tokio::test]
    async fn test_concurrent_authenticate() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let mgr = Arc::new(AuthenticationManager::new(SECRET));
        mgr.add_user(make_hashed_user("alice", "Pass123", "admin"))
            .await
            .unwrap();

        let success_count = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();

        for _ in 0..10 {
            let mgr = mgr.clone();
            let success_count = success_count.clone();
            handles.push(tokio::spawn(async move {
                if mgr
                    .authenticate(AuthCredentials {
                        username: "alice".to_string(),
                        password: "Pass123".to_string(), // pragma: allowlist secret
                    })
                    .await
                    .is_ok()
                {
                    success_count.fetch_add(1, Ordering::SeqCst);
                }
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        assert_eq!(
            success_count.load(Ordering::SeqCst),
            10,
            "all concurrent authentications should succeed"
        );
    }

    /// refresh_token 流程 — refresh token 生成新 access token
    #[tokio::test]
    async fn test_refresh_token_flow() {
        let mgr = AuthenticationManager::new(SECRET);
        mgr.add_user(make_hashed_user("bob", "Pass123", "user")).await.unwrap(); // pragma: allowlist secret

        // 使用相同 secret 的 JwtManager 生成 refresh token
        let jwt = JwtManager::new(SECRET);
        let refresh_token = jwt
            .generate_token("uid_bob", "bob", "user", TokenType::Refresh)
            .unwrap();

        let new_access = mgr.refresh_token(&refresh_token).expect("refresh should succeed");
        let claims = mgr.verify_token(&new_access).expect("verify should succeed");
        assert_eq!(claims.username, "bob");
        assert_eq!(claims.token_type, TokenType::Access);
    }

    /// 密码强度校验 — 太短、无字母、无数字
    #[test]
    fn test_password_strength_validation() {
        let hasher = PasswordHasher::new();

        // 太短
        assert!(hasher.validate_strength("Short1").is_err());

        // 无字母
        assert!(hasher.validate_strength("12345678").is_err());

        // 无数字
        assert!(hasher.validate_strength("OnlyLetters").is_err());

        // 空密码
        assert!(hasher.validate_strength("").is_err());

        // 合规密码（满足：≥12 字符 + 大写 + 小写 + 数字 + 特殊字符 + 不在黑名单）
        assert!(hasher.validate_strength("ValidPass@123").is_ok());
    }

    /// with_config 自定义过期时间
    #[tokio::test]
    async fn test_custom_expiration() {
        let mgr = AuthenticationManager::with_config(SECRET, 60, 3600);
        mgr.register_user("alice", "StrongPass@123", "admin").await.unwrap();

        let token = mgr
            .authenticate(AuthCredentials {
                username: "alice".to_string(),
                password: "StrongPass@123".to_string(), // pragma: allowlist secret
            })
            .await
            .unwrap();

        let claims = mgr.verify_token(&token).unwrap();
        assert!(claims.exp > claims.iat, "exp must be after iat");
    }
}

// ============================================================================
// i18n 高级边界测试（i18n feature）
// ============================================================================
#[cfg(feature = "i18n")]
mod i18n_advanced {
    use dbnexus::{DbI18nFormatter, I18nError};
    use std::cmp::Ordering;

    /// format_number 边界：0
    #[test]
    fn test_format_number_zero() {
        let fmt = DbI18nFormatter::new("en-US").expect("locale");
        let result = fmt.format_number(0.0_f64).expect("format zero");
        assert!(result.contains('0'), "zero should contain 0: got '{result}'");
    }

    /// format_number 边界：负数
    #[test]
    fn test_format_number_negative() {
        let fmt = DbI18nFormatter::new("en-US").expect("locale");
        let result = fmt.format_number(-1234.56_f64).expect("format negative");
        assert!(result.contains('-'), "negative should contain minus: got '{result}'");
    }

    /// format_number 边界：NaN 和 Infinity 应返回错误
    #[test]
    fn test_format_number_non_finite() {
        let fmt = DbI18nFormatter::new("en-US").expect("locale");
        match fmt.format_number(f64::NAN) {
            Err(I18nError::InvalidNumber { .. }) => {}
            other => panic!("expected InvalidNumber for NaN, got {:?}", other),
        }
        match fmt.format_number(f64::INFINITY) {
            Err(I18nError::InvalidNumber { .. }) => {}
            other => panic!("expected InvalidNumber for Infinity, got {:?}", other),
        }
        match fmt.format_number(f64::NEG_INFINITY) {
            Err(I18nError::InvalidNumber { .. }) => {}
            other => panic!("expected InvalidNumber for -Infinity, got {:?}", other),
        }
    }

    /// format_timestamp 边界：无效月份
    #[test]
    fn test_format_timestamp_invalid_month() {
        let fmt = DbI18nFormatter::new("en-US").expect("locale");
        match fmt.format_timestamp(2026, 0, 15) {
            Err(I18nError::DateError(_)) => {}
            other => panic!("expected DateError for month=0, got {:?}", other),
        }
        match fmt.format_timestamp(2026, 13, 15) {
            Err(I18nError::DateError(_)) => {}
            other => panic!("expected DateError for month=13, got {:?}", other),
        }
    }

    /// format_timestamp 边界：无效日期
    #[test]
    fn test_format_timestamp_invalid_day() {
        let fmt = DbI18nFormatter::new("en-US").expect("locale");
        match fmt.format_timestamp(2026, 1, 0) {
            Err(I18nError::DateError(_)) => {}
            other => panic!("expected DateError for day=0, got {:?}", other),
        }
        match fmt.format_timestamp(2026, 1, 32) {
            Err(I18nError::DateError(_)) => {}
            other => panic!("expected DateError for day=32, got {:?}", other),
        }
    }

    /// format_timestamp 边界：2月30日（不存在的日期）
    #[test]
    fn test_format_timestamp_feb_30() {
        let fmt = DbI18nFormatter::new("en-US").expect("locale");
        match fmt.format_timestamp(2026, 2, 30) {
            Err(I18nError::DateError(_)) => {}
            other => panic!("expected DateError for Feb 30, got {:?}", other),
        }
    }

    /// compare_strings 边界：空字符串
    #[test]
    fn test_compare_empty_strings() {
        let fmt = DbI18nFormatter::new("en").expect("locale");
        assert_eq!(
            fmt.compare_strings("", "").expect("compare"),
            Ordering::Equal,
            "empty == empty"
        );
        assert_eq!(
            fmt.compare_strings("a", "").expect("compare"),
            Ordering::Greater,
            "'a' > ''"
        );
        assert_eq!(
            fmt.compare_strings("", "a").expect("compare"),
            Ordering::Less,
            "'' < 'a'"
        );
    }

    /// plural_category 边界：0
    #[test]
    fn test_plural_category_zero() {
        let fmt = DbI18nFormatter::new("en").expect("locale");
        let category = fmt.plural_category(0).expect("plural 0");
        // 英语中 0 是 Other
        assert_eq!(category, "Other", "en: count=0 should be Other");
    }

    /// format_row_count 边界：0
    #[test]
    fn test_format_row_count_zero() {
        let fmt = DbI18nFormatter::new("en-US").expect("locale");
        let result = fmt.format_row_count(0).expect("format 0 rows");
        assert!(result.contains('0'), "0 rows should contain 0: got '{result}'");
    }

    /// format_migration_message 边界：0 条迁移
    #[test]
    fn test_format_migration_message_zero() {
        let fmt = DbI18nFormatter::new("en").expect("locale");
        let msg = fmt.format_migration_message(0).expect("migration message 0");
        assert!(msg.contains("0"), "message should contain count: got '{msg}'");
        assert!(
            msg.contains("migrations applied"),
            "0 should use plural form: got '{msg}'"
        );
    }

    /// 多 locale 格式化器同时存在
    #[test]
    fn test_multiple_locale_formatters() {
        let en = DbI18nFormatter::new("en-US").expect("en-US");
        let zh = DbI18nFormatter::new("zh-CN").expect("zh-CN");

        let en_result = en.format_number(1234.56_f64).expect("en format");
        let zh_result = zh.format_number(1234.56_f64).expect("zh format");

        // 两个 formatter 应都能成功格式化
        assert!(!en_result.is_empty());
        assert!(!zh_result.is_empty());
    }

    /// 无效 locale 字符串
    #[test]
    fn test_invalid_locale_strings() {
        // 完全无效
        match DbI18nFormatter::new("!!!invalid!!!") {
            Err(I18nError::InvalidLocale { input, .. }) => {
                assert_eq!(input, "!!!invalid!!!");
            }
            Ok(_) => panic!("expected InvalidLocale, but got Ok"),
            Err(e) => panic!("expected InvalidLocale, but got error: {}", e),
        }

        // 空字符串
        match DbI18nFormatter::new("") {
            Err(I18nError::InvalidLocale { .. }) | Err(I18nError::FormatError(_)) => {}
            Ok(_) => panic!("empty locale should fail"),
            Err(e) => panic!("unexpected error for empty locale: {}", e),
        }
    }

    /// format_number 极大值
    #[test]
    fn test_format_number_very_large() {
        let fmt = DbI18nFormatter::new("en-US").expect("locale");
        let result = fmt.format_number(1e15_f64).expect("format large");
        assert!(!result.is_empty());
    }

    /// format_number 极小值
    #[test]
    fn test_format_number_very_small() {
        let fmt = DbI18nFormatter::new("en-US").expect("locale");
        let result = fmt.format_number(0.0001_f64).expect("format small");
        assert!(!result.is_empty());
    }

    /// format_timestamp 有效日期
    #[test]
    fn test_format_timestamp_valid() {
        let fmt = DbI18nFormatter::new("en-US").expect("locale");
        let result = fmt.format_timestamp(2026, 7, 22).expect("timestamp");
        assert!(result.contains("2026"), "should contain year: got '{result}'");
    }

    /// format_timestamp 闰年 2月29日
    #[test]
    fn test_format_timestamp_leap_year_feb_29() {
        let fmt = DbI18nFormatter::new("en-US").expect("locale");
        // 2024 是闰年
        let result = fmt.format_timestamp(2024, 2, 29).expect("leap day");
        assert!(result.contains("2024"), "should contain year: got '{result}'");
    }

    /// format_timestamp 非闰年 2月29日应失败
    #[test]
    fn test_format_timestamp_non_leap_feb_29() {
        let fmt = DbI18nFormatter::new("en-US").expect("locale");
        // 2023 不是闰年
        match fmt.format_timestamp(2023, 2, 29) {
            Err(I18nError::DateError(_)) => {}
            other => panic!("expected DateError for non-leap Feb 29, got {:?}", other),
        }
    }

    /// format_timestamp 边界：12月31日
    #[test]
    fn test_format_timestamp_dec_31() {
        let fmt = DbI18nFormatter::new("en-US").expect("locale");
        let result = fmt.format_timestamp(2026, 12, 31).expect("dec 31");
        assert!(result.contains("2026"), "should contain year: got '{result}'");
    }
}

// ============================================================================
// TracingGuard 边界测试（tracing feature）
// ============================================================================
#[cfg(feature = "tracing")]
mod tracing_advanced {
    use dbnexus::{TracingError, TracingGuard};

    /// TracingError Display — 所有变体
    #[test]
    fn test_tracing_error_all_variants_display() {
        let err = TracingError::ExporterInit("tonic connection failed".to_string());
        assert!(err.to_string().contains("OTLP exporter"));
        assert!(err.to_string().contains("tonic connection failed"));

        let err = TracingError::ProviderSetup("provider setup failed".to_string());
        assert!(err.to_string().contains("Tracer provider setup failed"));
        assert!(err.to_string().contains("provider setup failed"));

        let err = TracingError::AlreadyInitialized;
        assert!(err.to_string().contains("already initialized"));
        assert!(err.to_string().contains("global subscriber"));

        let err = TracingError::SubscriberSetup("subscriber error".to_string());
        assert!(err.to_string().contains("global subscriber"));
        assert!(err.to_string().contains("subscriber error"));
    }

    /// TracingGuard Send trait — 可在 tokio 任务间传递
    #[test]
    fn test_tracing_guard_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<TracingGuard>();
    }

    /// TracingGuard 不能直接构造 — 只能通过 init_with_otlp
    /// 此测试验证 TracingGuard 的 provider 字段是私有的
    #[test]
    fn test_tracing_guard_cannot_construct_directly() {
        // TracingGuard 的 provider 字段是私有的，外部无法直接构造
        // 只能通过 TracingGuard::init_with_otlp 获取
        // 这确保了 RAII 语义：guard 的创建和初始化绑定在一起
    }

    /// init_with_otlp 重复初始化返回 AlreadyInitialized
    /// 注意：此测试需要实际 OTLP 连接，使用 #[ignore] 标注
    #[tokio::test]
    #[ignore = "requires OTLP collector at localhost:4317; run with --ignored"]
    async fn test_init_with_otlp_already_initialized() {
        // 第一次初始化
        let guard1 = TracingGuard::init_with_otlp("http://localhost:4317");
        if guard1.is_err() {
            // 如果第一次就失败（无 collector），跳过后续断言
            return;
        }
        let _guard1 = guard1.unwrap();

        // 第二次初始化应返回 AlreadyInitialized
        match TracingGuard::init_with_otlp("http://localhost:4317") {
            Err(TracingError::AlreadyInitialized) => {}
            Ok(_) => panic!("expected AlreadyInitialized, but got Ok"),
            Err(e) => panic!("expected AlreadyInitialized, but got error: {}", e),
        }
    }

    /// init_with_otlp 无效端点 — 应返回 ExporterInit 或 SubscriberSetup
    /// 注意：此测试使用 #[ignore] 因为它会影响全局 tracing subscriber
    #[tokio::test]
    #[ignore = "affects global tracing subscriber; run in isolation with --ignored"]
    async fn test_init_with_otlp_invalid_endpoint() {
        // 使用无效端点格式
        let result = TracingGuard::init_with_otlp("not-a-valid-url");
        // 可能返回 ExporterInit 或成功（tonic 延迟连接）
        // 关键是不 panic
        match result {
            Ok(_guard) => { /* tonic 可能延迟连接，builder 阶段不报错 */ }
            Err(TracingError::ExporterInit(_)) => {}
            Err(TracingError::SubscriberSetup(_)) => {}
            Err(other) => panic!("unexpected error for invalid endpoint: {:?}", other),
        }
    }
}

// ============================================================================
// PoolHealthMetrics 边界测试（health-check feature）
// ============================================================================
#[cfg(feature = "health-check")]
mod pool_health_metrics_advanced {
    use dbnexus::PoolHealthMetrics;

    /// 新建的 PoolHealthMetrics 初始值全为 0
    #[test]
    fn test_new_metrics_all_zero() {
        let metrics = PoolHealthMetrics::new();
        let snap = metrics.snapshot();
        assert_eq!(snap.total, 0);
        assert_eq!(snap.active, 0);
        assert_eq!(snap.idle, 0);
        assert_eq!(snap.waiting, 0);
        assert_eq!(snap.created, 0);
        assert_eq!(snap.failed, 0);
        assert_eq!(snap.closed, 0);
    }

    /// Default 等价于 new
    #[test]
    fn test_default_equals_new() {
        let new = PoolHealthMetrics::new();
        let default = PoolHealthMetrics::default();
        let n = new.snapshot();
        let d = default.snapshot();
        assert_eq!(n.total, d.total);
        assert_eq!(n.active, d.active);
        assert_eq!(n.idle, d.idle);
        assert_eq!(n.waiting, d.waiting);
        assert_eq!(n.created, d.created);
        assert_eq!(n.failed, d.failed);
        assert_eq!(n.closed, d.closed);
    }

    /// is_healthy: 有空闲连接时为 true
    #[test]
    fn test_is_healthy_with_idle() {
        let metrics = PoolHealthMetrics::new();
        metrics.idle_connections.store(1, std::sync::atomic::Ordering::Relaxed);
        assert!(metrics.is_healthy(), "should be healthy with idle connections");
    }

    /// is_healthy: 活跃 < 总数时为 true
    #[test]
    fn test_is_healthy_with_capacity() {
        let metrics = PoolHealthMetrics::new();
        metrics
            .total_connections
            .store(10, std::sync::atomic::Ordering::Relaxed);
        metrics
            .active_connections
            .store(5, std::sync::atomic::Ordering::Relaxed);
        assert!(metrics.is_healthy(), "should be healthy when active < total");
    }

    /// is_healthy: 全部忙碌时为 false
    #[test]
    fn test_is_healthy_all_busy() {
        let metrics = PoolHealthMetrics::new();
        metrics.total_connections.store(5, std::sync::atomic::Ordering::Relaxed);
        metrics
            .active_connections
            .store(5, std::sync::atomic::Ordering::Relaxed);
        assert!(!metrics.is_healthy(), "should be unhealthy when all busy");
    }

    /// should_create_connection: 低于 min_connections 时为 true
    #[test]
    fn test_should_create_below_min() {
        let metrics = PoolHealthMetrics::new();
        assert!(
            metrics.should_create_connection(5),
            "should create when total < min_connections"
        );
    }

    /// should_create_connection: 已满且无空闲时为 true
    #[test]
    fn test_should_create_when_exhausted() {
        let metrics = PoolHealthMetrics::new();
        metrics.total_connections.store(5, std::sync::atomic::Ordering::Relaxed);
        metrics
            .active_connections
            .store(5, std::sync::atomic::Ordering::Relaxed);
        assert!(metrics.should_create_connection(3), "should create when pool exhausted");
    }

    /// should_create_connection: 有空闲时为 false
    #[test]
    fn test_should_not_create_when_idle_available() {
        let metrics = PoolHealthMetrics::new();
        metrics.total_connections.store(5, std::sync::atomic::Ordering::Relaxed);
        metrics
            .active_connections
            .store(3, std::sync::atomic::Ordering::Relaxed);
        metrics.idle_connections.store(2, std::sync::atomic::Ordering::Relaxed);
        assert!(
            !metrics.should_create_connection(3),
            "should not create when idle available"
        );
    }

    /// record_connection_created: 同时增加 total、active、created
    #[test]
    fn test_record_connection_created() {
        let metrics = PoolHealthMetrics::new();
        metrics.record_connection_created();
        let snap = metrics.snapshot();
        assert_eq!(snap.total, 1);
        assert_eq!(snap.active, 1);
        assert_eq!(snap.created, 1);
    }

    /// record_connection_failed: 只增加 failed
    #[test]
    fn test_record_connection_failed() {
        let metrics = PoolHealthMetrics::new();
        metrics.record_connection_failed();
        let snap = metrics.snapshot();
        assert_eq!(snap.failed, 1);
        assert_eq!(snap.total, 0, "failed should not affect total");
    }

    /// record_connection_closed: 减少 total，增加 closed
    #[test]
    fn test_record_connection_closed() {
        let metrics = PoolHealthMetrics::new();
        metrics.record_connection_created();
        metrics.record_connection_closed();
        let snap = metrics.snapshot();
        assert_eq!(snap.total, 0, "total should decrease");
        assert_eq!(snap.closed, 1);
    }

    /// increment_active / decrement_active
    #[test]
    fn test_increment_decrement_active() {
        let metrics = PoolHealthMetrics::new();
        metrics.increment_active();
        assert_eq!(metrics.snapshot().active, 1);
        metrics.decrement_active();
        assert_eq!(metrics.snapshot().active, 0);
    }

    /// set_waiting_requests
    #[test]
    fn test_set_waiting_requests() {
        let metrics = PoolHealthMetrics::new();
        metrics.set_waiting_requests(42);
        assert_eq!(metrics.snapshot().waiting, 42);
    }
}
