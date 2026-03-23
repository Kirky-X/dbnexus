// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! confers DI集成测试
//!
//! 测试dbnexus的with_confers()依赖注入功能
//!
//! 注意：此测试模块中的测试暂时被忽略，因为 with_confers() 方法尚未实现。
//! 目前可以使用 DbConfig::from_confers() 从 confers 配置创建配置实例。
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use confers::ConfigProvider;
//! use dbnexus::config::DbConfig;
//! use dbnexus::DbPool;
//!
//! async fn example(provider: &dyn confers::ConfigProvider) {
//!     // 从 confers 配置创建 DbConfig
//!     let config = DbConfig::from_confers(provider).expect("Failed to load config");
//!
//!     // 使用配置创建连接池
//!     let pool = DbPool::with_config(config).await.expect("Failed to create pool");
//! }
//! ```

#[cfg(all(feature = "confers", feature = "sqlite"))]
mod confers_tests {
    // 注意：以下测试暂时忽略，因为 with_confers() 方法尚未实现
    // 功能实现后可以移除 #[ignore] 属性

    #[tokio::test]
    #[ignore = "with_confers() 方法尚未实现"]
    async fn test_pool_with_confers_sqlite_memory() {
        // 此测试需要 with_confers() 方法实现后才能运行
        // 目前请使用 DbConfig::from_confers() + DbPool::with_config()
    }

    #[tokio::test]
    #[ignore = "with_confers() 方法尚未实现"]
    async fn test_pool_builder_with_confers() {
        // 此测试需要 with_confers() 方法实现后才能运行
    }

    #[tokio::test]
    #[ignore = "with_confers() 方法尚未实现"]
    async fn test_pool_builder_manual_override_confers() {
        // 此测试需要 with_confers() 方法实现后才能运行
    }

    #[tokio::test]
    #[ignore = "with_confers() 方法尚未实现"]
    async fn test_pool_with_confers_missing_required_field() {
        // 此测试需要 with_confers() 方法实现后才能运行
    }

    #[tokio::test]
    #[ignore = "with_confers() 方法尚未实现"]
    async fn test_pool_with_confers_default_values() {
        // 此测试需要 with_confers() 方法实现后才能运行
    }
}
