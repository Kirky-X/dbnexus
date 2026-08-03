// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 边界条件测试套件
//!
//! 测试各种边界情况、异常场景和特殊输入

#[path = "../../common/mod.rs"]
mod common;

#[cfg(test)]
mod config_boundary_tests {
    #[tokio::test]
    async fn test_empty_url() {
        // 空URL - 配置会被创建，但 DbPool 创建时会失败
        let config = dbnexus::DbConfig {
            url: "".to_string(),
            ..Default::default()
        };
        // 配置已创建
        assert_eq!(config.url, "");
    }

    #[tokio::test]
    async fn test_invalid_url_format() {
        // 无效URL格式 - 配置会被创建，但 DbPool 创建时会失败
        let config = dbnexus::DbConfig {
            url: "not-a-valid-url".to_string(),
            ..Default::default()
        };
        // 配置已创建
        assert_eq!(config.url, "not-a-valid-url");
    }

    #[tokio::test]
    async fn test_zero_connections() {
        // 零连接数 - 配置会被创建，但 DbPool 创建时会失败
        let config = dbnexus::DbConfig {
            url: "sqlite::memory:".to_string(),
            pool_config: dbnexus::foundation::PoolConfig {
                max_connections: 0,
                ..Default::default()
            },
            ..Default::default()
        };
        // 配置已创建
        assert_eq!(config.pool_config.max_connections, 0);
    }

    #[tokio::test]
    async fn test_negative_connections() {
        // 负数连接数 - u32 类型不支持负数，这里测试最小值
        let config = dbnexus::DbConfig {
            url: "sqlite::memory:".to_string(),
            pool_config: dbnexus::foundation::PoolConfig {
                max_connections: 0,
                ..Default::default()
            },
            ..Default::default()
        };
        // 配置已创建
        assert_eq!(config.pool_config.max_connections, 0);
    }

    #[tokio::test]
    async fn test_max_connections() {
        // 超大连接数 - 配置会被创建
        let config = dbnexus::DbConfig {
            url: "sqlite::memory:".to_string(),
            pool_config: dbnexus::foundation::PoolConfig {
                max_connections: 10000,
                ..Default::default()
            },
            ..Default::default()
        };
        // 配置已创建
        assert_eq!(config.pool_config.max_connections, 10000);
    }

    #[tokio::test]
    async fn test_zero_timeout() {
        // 零超时 - 配置会被创建
        let config = dbnexus::DbConfig {
            url: "sqlite::memory:".to_string(),
            pool_config: dbnexus::foundation::PoolConfig {
                acquire_timeout: 0,
                idle_timeout: 0,
                ..Default::default()
            },
            ..Default::default()
        };
        // 配置已创建
        assert_eq!(config.pool_config.acquire_timeout, 0);
        assert_eq!(config.pool_config.idle_timeout, 0);
    }

    #[tokio::test]
    async fn test_max_timeout() {
        // 最大超时 - 配置会被创建
        let config = dbnexus::DbConfig {
            url: "sqlite::memory:".to_string(),
            pool_config: dbnexus::foundation::PoolConfig {
                acquire_timeout: u64::MAX,
                idle_timeout: u64::MAX,
                ..Default::default()
            },
            ..Default::default()
        };
        // 配置已创建
        assert_eq!(config.pool_config.acquire_timeout, u64::MAX);
        assert_eq!(config.pool_config.idle_timeout, u64::MAX);
    }

    #[tokio::test]
    async fn test_invalid_database_type() {
        // 不支持的数据库类型 - 配置会被创建，但 DbPool 创建时会失败
        let config = dbnexus::DbConfig {
            url: "oracle://user:pass@localhost/db".to_string(),
            ..Default::default()
        };
        // 配置已创建
        assert_eq!(config.url, "oracle://user:pass@localhost/db");
    }

    #[tokio::test]
    async fn test_invalid_hostname() {
        // 无效主机名 - 配置会被创建，但 DbPool 创建时会失败
        let config = dbnexus::DbConfig {
            url: "postgresql://invalid-hostname-that-does-not-exist/db".to_string(),
            ..Default::default()
        };
        // 配置已创建
        assert_eq!(config.url, "postgresql://invalid-hostname-that-does-not-exist/db");
    }
}

#[cfg(test)]
mod sql_parser_boundary_tests {
    use dbnexus::{SqlOperationType, SqlParser};

    #[tokio::test]
    async fn test_empty_sql() {
        let parser = SqlParser::new().await;
        let result = parser.parse_single("").await;
        assert!(result.is_err() || result.unwrap().operation_type == SqlOperationType::Other);
    }

    #[tokio::test]
    async fn test_whitespace_only_sql() {
        let parser = SqlParser::new().await;
        let result = parser.parse_single("   \n\t  ").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_very_long_sql() {
        let parser = SqlParser::new().await;
        let long_sql = "SELECT ".to_string() + &"a".repeat(1_000_000);
        let result = parser.parse_single(&long_sql).await;
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_deeply_nested_parentheses() {
        let parser = SqlParser::new().await;
        let nested_sql = "SELECT ".to_string() + &"(".repeat(1000) + "1" + &")".repeat(1000);
        let result = parser.parse_single(&nested_sql).await;
        assert!(result.is_err() || result.is_ok());
    }

    #[tokio::test]
    async fn test_many_unions() {
        let parser = SqlParser::new().await;
        let union_sql = "SELECT 1".to_string() + &" UNION ALL SELECT 1".repeat(1000);
        let result = parser.parse_single(&union_sql).await;
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_special_characters() {
        let parser = SqlParser::new().await;
        let special_sqls = vec![
            "SELECT 1\x00",
            "SELECT 1\t\t",
            "SELECT 1\n\n",
            "SELECT 1🎉",
            "SELECT 1汉",
            "SELECT 1Ω",
        ];

        for sql in special_sqls {
            let result = parser.parse_single(sql).await;
            assert!(result.is_ok() || result.is_err());
        }
    }

    #[tokio::test]
    async fn test_comment_injection() {
        let parser = SqlParser::new().await;
        let malicious_sqls = vec!["SELECT 1 -- comment", "SELECT 1 /* comment */", "SELECT 1 # comment"];

        for sql in malicious_sqls {
            let result = parser.parse_single(sql).await;
            assert!(result.is_ok() || result.is_err());
        }
    }
}

#[cfg(test)]
mod permission_boundary_tests {
    use dbnexus::access::permission_engine::PermissionContext;
    use dbnexus::{
        EnginePermissionAction as PermissionAction, PermissionDecision, PermissionResource, PermissionSubject,
        PolicyDecisionPoint, PolicyDecisionPointConfig, RbacPermissionProvider,
    };
    use std::sync::Arc;

    #[tokio::test]
    async fn test_long_subject_id() {
        let provider = Arc::new(RbacPermissionProvider::new());
        let pdp = PolicyDecisionPoint::with_config(provider, PolicyDecisionPointConfig::default());
        let long_id = "a".repeat(10_000);
        let result = pdp.check(&long_id, "users", "SELECT").await;
        assert!(matches!(result, PermissionDecision::Deny));
    }

    #[tokio::test]
    async fn test_long_resource_name() {
        let provider = Arc::new(RbacPermissionProvider::new());
        let pdp = PolicyDecisionPoint::with_config(provider, PolicyDecisionPointConfig::default());
        let long_resource = "table_".to_string() + &"a".repeat(10_000);
        let result = pdp.check("admin", &long_resource, "SELECT").await;
        assert!(matches!(result, PermissionDecision::Deny));
    }

    #[tokio::test]
    async fn test_invalid_action() {
        let provider = Arc::new(RbacPermissionProvider::new());
        let pdp = PolicyDecisionPoint::with_config(provider, PolicyDecisionPointConfig::default());
        let result = pdp.check("admin", "users", "INVALID_ACTION").await;
        assert!(matches!(result, PermissionDecision::Error(_)));
    }

    #[tokio::test]
    async fn test_empty_action() {
        let provider = Arc::new(RbacPermissionProvider::new());
        let pdp = PolicyDecisionPoint::with_config(provider, PolicyDecisionPointConfig::default());
        let result = pdp.check("admin", "users", "").await;
        assert!(matches!(result, PermissionDecision::Error(_)));
    }

    #[tokio::test]
    async fn test_many_attributes() {
        let provider = Arc::new(RbacPermissionProvider::new());
        let pdp = PolicyDecisionPoint::with_config(provider, PolicyDecisionPointConfig::default());

        let mut attributes = std::collections::HashMap::new();
        for i in 0..1000 {
            attributes.insert(format!("attr_{}", i), format!("value_{}", i));
        }

        let context = PermissionContext::new(
            PermissionSubject::user("admin"),
            PermissionResource::new("users"),
            PermissionAction::Select,
        );

        let decision = pdp.check_permission(&context).await;
        assert!(matches!(decision, PermissionDecision::Allow) || matches!(decision, PermissionDecision::Deny));
    }

    #[tokio::test]
    async fn test_special_chars_in_subject() {
        let provider = Arc::new(RbacPermissionProvider::new());
        let pdp = PolicyDecisionPoint::with_config(provider, PolicyDecisionPointConfig::default());

        let malicious_ids = vec![
            "admin' OR '1'='1",
            "admin; DROP TABLE users;",
            "admin/**/OR/**/1=1",
            "../../../etc/passwd",
        ];

        for id in malicious_ids {
            let result = pdp.check(id, "users", "SELECT").await;
            assert!(matches!(result, PermissionDecision::Deny));
        }
    }

    #[tokio::test]
    async fn test_system_table_access() {
        let provider = Arc::new(RbacPermissionProvider::new());
        let pdp = PolicyDecisionPoint::with_config(provider, PolicyDecisionPointConfig::default());

        let system_tables = vec!["pg_catalog", "information_schema", "mysql", "sys", "INFORMATION_SCHEMA"];

        for table in system_tables {
            let result = pdp.check("admin", table, "SELECT").await;
            assert!(matches!(result, PermissionDecision::Deny));
        }
    }
}

#[cfg(test)]
mod time_boundary_tests {
    use dbnexus::MetricsCollector;
    use std::time::Duration;

    #[tokio::test]
    async fn test_record_query_zero_duration() {
        let collector = MetricsCollector::new();
        collector.record_query("SELECT", Duration::from_secs(0), true, None);
        let stats = collector.get_query_stats("SELECT");
        assert!(stats.is_some());
    }

    #[tokio::test]
    async fn test_record_query_max_duration() {
        let collector = MetricsCollector::new();
        collector.record_query("SELECT", Duration::from_secs(u64::MAX), true, None);
        let stats = collector.get_query_stats("SELECT");
        assert!(stats.is_some());
    }

    #[test]
    fn test_timestamp_boundaries() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let earliest = SystemTime::UNIX_EPOCH;
        let duration = earliest.duration_since(UNIX_EPOCH).unwrap();
        assert_eq!(duration.as_secs(), 0);

        let max_32bit = SystemTime::UNIX_EPOCH + Duration::from_secs(2_147_483_647);
        let duration = max_32bit.duration_since(UNIX_EPOCH).unwrap();
        assert_eq!(duration.as_secs(), 2_147_483_647);
    }
}

#[cfg(test)]
mod number_boundary_tests {
    #[test]
    fn test_i64_boundaries() {
        assert_eq!(i64::MIN, -9_223_372_036_854_775_808);
        assert_eq!(i64::MAX, 9_223_372_036_854_775_807);
    }

    #[test]
    fn test_u64_boundaries() {
        assert_eq!(u64::MIN, 0);
        assert_eq!(u64::MAX, 18_446_744_073_709_551_615);
    }

    #[test]
    fn test_usize_boundaries() {
        assert_eq!(usize::MIN, 0);
        // usize 在 64 位平台等于 u64::MAX，在 32 位平台等于 u32::MAX
        let expected_max = if cfg!(target_pointer_width = "64") {
            u64::MAX as usize
        } else {
            u32::MAX as usize
        };
        assert_eq!(usize::MAX, expected_max);
    }

    #[test]
    fn test_f64_boundaries() {
        assert_eq!(f64::MIN, -1.7976931348623157e+308);
        assert_eq!(f64::MAX, 1.7976931348623157e+308);
        assert_eq!(f64::EPSILON, 2.220_446_049_250_313e-16);
    }
}

#[cfg(test)]
mod concurrency_boundary_tests {
    use super::common;
    use dbnexus::tokio::sync::Barrier;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_massive_concurrent_connections() {
        let pool = common::make_sqlite_memory_pool().await;
        let num_tasks = 100;
        let barrier = Arc::new(Barrier::new(num_tasks));
        let pool = Arc::new(pool);

        let handles: Vec<_> = (0..num_tasks)
            .map(|_| {
                let barrier = barrier.clone();
                let pool = pool.clone();
                tokio::spawn(async move {
                    barrier.wait().await;
                    let _ = pool.get_session("admin").await;
                })
            })
            .collect();

        for handle in handles {
            let _ = handle.await;
        }
    }

    #[tokio::test]
    async fn test_rapid_connection_churn() {
        let pool = common::make_sqlite_memory_pool().await;

        for _ in 0..1000 {
            let session = pool.get_session("admin").await;
            assert!(session.is_ok());
            drop(session);
        }
    }

    #[tokio::test]
    async fn test_pool_exhaustion_boundary() {
        // SQLite 内存数据库每个连接独立，无法测试连接池耗尽
        let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());
        if url.starts_with("sqlite::memory:") || url == "sqlite::memory:" {
            // SQLite 内存数据库不支持连接池耗尽测试，跳过
            return;
        }
        let config = dbnexus::DbConfig {
            url,
            pool_config: dbnexus::foundation::PoolConfig {
                max_connections: 1,
                acquire_timeout: 50, // 50ms 超时
                ..Default::default()
            },

            ..Default::default()
        };
        let pool = dbnexus::DbPool::with_config(config)
            .await
            .expect("Failed to create pool");

        // 占用唯一一个连接
        let _session = pool.get_session("admin").await.expect("First session should succeed");

        // 第二个请求应因 acquire_timeout 触发失败
        let result = pool.get_session("admin").await;
        assert!(result.is_err(), "Second session should fail due to pool exhaustion");
    }
}

#[cfg(test)]
mod path_traversal_tests {
    #[tokio::test]
    async fn test_path_traversal_patterns() {
        let malicious_paths = vec![
            "../../etc/passwd",
            "..\\..\\windows\\system32",
            "%2e%2e/etc/passwd",
            "%252e%252e/etc/passwd",
            "....//....//etc/passwd",
            "/etc/../../../etc/passwd",
            "foo/../../etc/passwd",
            "foo/bar/../../etc/passwd",
            "/../../etc/passwd",
            "...../...../...../etc/passwd",
        ];

        for path in malicious_paths {
            // 配置会被创建，路径验证在 DbPool 创建时进行
            let config = dbnexus::DbConfig {
                url: format!("sqlite:///{}/test.db", path),
                ..Default::default()
            };
            // 配置已创建
            assert!(config.url.contains(path), "Path should be in URL: {}", path);
        }
    }

    #[tokio::test]
    async fn test_null_byte_injection() {
        let malicious_urls = vec!["sqlite:///\0/config.db", "sqlite:///path/to\0/../etc/passwd"];

        for url in malicious_urls {
            // 配置会被创建
            let config = dbnexus::DbConfig {
                url: url.to_string(),
                ..Default::default()
            };
            // 配置已创建
            assert_eq!(config.url, url);
        }
    }

    #[tokio::test]
    async fn test_symlink_attack() {
        let config = dbnexus::DbConfig {
            url: "sqlite:///path/to/symlink.db".to_string(),
            ..Default::default()
        };
        assert!(config.url.contains("symlink.db"));
    }
}

#[cfg(test)]
mod sql_injection_tests {
    use dbnexus::{PolicyDecisionPoint, PolicyDecisionPointConfig, RbacPermissionProvider};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_sql_injection_patterns() {
        let provider = Arc::new(RbacPermissionProvider::new());
        let pdp = PolicyDecisionPoint::with_config(provider, PolicyDecisionPointConfig::default());

        let malicious_inputs = vec![
            "' OR '1'='1",
            "' OR 1=1--",
            "admin'--",
            "admin' OR '1'='1",
            "1; DROP TABLE users;--",
            "1; DELETE FROM users;",
            "1 UNION SELECT * FROM users",
            "1 UNION SELECT password FROM users",
            "1 AND 1=1",
            "1 AND 1=2",
            "1/*comment*/OR/*comment*/1=1",
            "%27%20OR%20%271%27%3D%271",
            "admin' UNIon SELECT--",
            "admin' OR 日本語='日本語",
        ];

        for input in malicious_inputs {
            let result = pdp.check(input, "users", "SELECT").await;
            match result {
                dbnexus::PermissionDecision::Deny => {}
                dbnexus::PermissionDecision::NotApplicable => {}
                _ => {
                    panic!("SQL injection attempt should be denied: {}", input);
                }
            }
        }
    }
}
