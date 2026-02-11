// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! confers DI集成测试
//!
//! 测试dbnexus的with_confers()依赖注入功能

#[cfg(all(feature = "confers", feature = "sqlite"))]
mod confers_tests {
    use dbnexus::{DbPool, DbPoolBuilder};
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Arc;

    /// 创建测试用的confers配置
    fn create_test_config(
        pairs: Vec<(&str, serde_json::Value)>,
    ) -> Arc<dyn confers::core::config_trait::ConfersConfig> {
        let mut config_map = HashMap::new();
        for (key, value) in pairs {
            config_map.insert(key.to_string(), value);
        }
        Arc::new(config_map) as Arc<dyn confers::core::config_trait::ConfersConfig>
    }

    #[tokio::test]
    async fn test_pool_with_confers_sqlite_memory() {
        let config = create_test_config(vec![
            ("dbnexus.url", json!("sqlite::memory:")),
            ("dbnexus.max_connections", json!(10)),
            ("dbnexus.min_connections", json!(2)),
        ]);

        let pool = DbPool::with_confers(config.clone())
            .await
            .expect("Failed to create pool with confers");

        // 验证池已创建并可以获取会话
        let status = pool.status();
        assert!(status.total > 0, "Pool should have connections");

        // 验证可以通过配置获取最大连接数
        let pool_config = pool.config();
        assert_eq!(pool_config.max_connections(), 10);
    }

    #[tokio::test]
    async fn test_pool_builder_with_confers() {
        let config = create_test_config(vec![
            ("dbnexus.url", json!("sqlite::memory:")),
            ("dbnexus.max_connections", json!(15)),
            ("dbnexus.min_connections", json!(3)),
            ("dbnexus.idle_timeout", json!(600)),
        ]);

        let pool = DbPoolBuilder::new()
            .with_confers(config)
            .build()
            .await
            .expect("Failed to build pool with confers");

        // 验证池已创建
        let pool_config = pool.config();
        assert_eq!(pool_config.max_connections(), 15);
    }

    #[tokio::test]
    async fn test_pool_builder_manual_override_confers() {
        let config = create_test_config(vec![
            ("dbnexus.url", json!("sqlite::memory:")),
            ("dbnexus.max_connections", json!(20)), // confers中是20
        ]);

        // 手动设置的max_connections应该覆盖confers中的配置
        let pool = DbPoolBuilder::new()
            .with_confers(config)
            .max_connections(50)  // 覆盖confers的20
            .build()
            .await
            .expect("Failed to build pool");

        // 验证配置已应用
        let pool_config = pool.config();
        assert_eq!(pool_config.max_connections(), 50);
    }

    #[tokio::test]
    async fn test_pool_with_confers_missing_required_field() {
        // 缺少必需的dbnexus.url字段
        let config = create_test_config(vec![("dbnexus.max_connections", json!(10))]);

        let result = DbPool::with_confers(config).await;
        assert!(result.is_err(), "Expected error without dbnexus.url");
    }

    #[tokio::test]
    async fn test_pool_with_confers_default_values() {
        // 只提供URL，其他使用默认值
        let config = create_test_config(vec![("dbnexus.url", json!("sqlite::memory:"))]);

        let pool = DbPool::with_confers(config.clone())
            .await
            .expect("Failed to create pool with defaults");

        // 验证默认值已应用
        let pool_config = pool.config();
        assert_eq!(pool_config.max_connections(), 20); // 默认值
    }
}
