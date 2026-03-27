// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 依赖注入完整示例
//!
//! 展示如何使用 dbnexus 的依赖注入功能进行企业级配置管理。
//!
//! 本示例演示以下场景：
//! - 使用 `DbPoolBuilder` 进行配置构建
//! - 在测试中使用 Mock 实现替换真实组件
//! - 在不同环境（开发/生产）切换配置
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --bin dependency_injection --features sqlite
//! ```

use std::sync::Arc;

// 导入 dbnexus 核心组件
use dbnexus::{
    DbPool, pool::DbPoolBuilder,
    config::DbConfig,
};

#[cfg(feature = "metrics")]
use dbnexus::metrics::{MetricsCollector, MockMetrics};

/// 环境类型枚举
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Environment {
    /// 开发环境
    Development,
    /// 测试环境
    Test,
    /// 生产环境
    Production,
}

impl Environment {
    /// 从环境变量获取当前环境
    pub fn from_env() -> Self {
        match std::env::var("APP_ENV").unwrap_or_default().as_str() {
            "production" | "prod" => Environment::Production,
            "test" => Environment::Test,
            _ => Environment::Development,
        }
    }
}

/// 应用配置构建器
///
/// 根据不同环境构建不同的配置
pub struct AppConfigBuilder {
    environment: Environment,
    db_url: Option<String>,
    max_connections: Option<u32>,
}

impl AppConfigBuilder {
    /// 创建新的配置构建器
    pub fn new() -> Self {
        Self {
            environment: Environment::from_env(),
            db_url: None,
            max_connections: None,
        }
    }

    /// 设置数据库 URL
    pub fn db_url(mut self, url: &str) -> Self {
        self.db_url = Some(url.to_string());
        self
    }

    /// 设置最大连接数
    pub fn max_connections(mut self, max: u32) -> Self {
        self.max_connections = Some(max);
        self
    }

    /// 构建配置
    pub async fn build(self) -> Result<DbPool, Box<dyn std::error::Error>> {
        match self.environment {
            Environment::Development => self.build_development().await,
            Environment::Test => self.build_test().await,
            Environment::Production => self.build_production().await,
        }
    }

    /// 构建开发环境配置
    async fn build_development(&self) -> Result<DbPool, Box<dyn std::error::Error>> {
        let url = self.db_url.clone().unwrap_or_else(|| "sqlite::memory:".to_string());
        let max_conn = self.max_connections.unwrap_or(10);

        let pool = DbPoolBuilder::new()
            .url(&url)
            .max_connections(max_conn)
            .min_connections(2)
            .build()
            .await?;

        Ok(pool)
    }

    /// 构建测试环境配置
    async fn build_test(&self) -> Result<DbPool, Box<dyn std::error::Error>> {
        let url = self.db_url.clone().unwrap_or_else(|| "sqlite::memory:".to_string());
        let max_conn = self.max_connections.unwrap_or(5);

        let pool = DbPoolBuilder::new()
            .url(&url)
            .max_connections(max_conn)
            .min_connections(1)
            .build()
            .await?;

        Ok(pool)
    }

    /// 构建生产环境配置
    async fn build_production(&self) -> Result<DbPool, Box<dyn std::error::Error>> {
        let url = self.db_url.clone().ok_or("Database URL is required in production")?;
        let max_conn = self.max_connections.unwrap_or(50);

        let pool = DbPoolBuilder::new()
            .url(&url)
            .max_connections(max_conn)
            .min_connections(10)
            .build()
            .await?;

        Ok(pool)
    }
}

impl Default for AppConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// 测试示例：使用 Mock 进行单元测试
#[cfg(test)]
mod tests {
    use super::*;

    /// 测试开发环境配置
    #[tokio::test]
    async fn test_development_config() {
        let pool = AppConfigBuilder::new()
            .db_url("sqlite::memory:")
            .max_connections(5)
            .build()
            .await;

        assert!(pool.is_ok());
        let pool = pool.unwrap();
        assert_eq!(pool.config().max_connections, 5);
    }

    /// 测试 Builder 模式
    #[tokio::test]
    async fn test_builder_pattern() {
        let pool = DbPoolBuilder::new()
            .url("sqlite::memory:")
            .max_connections(10)
            .min_connections(2)
            .build()
            .await;

        assert!(pool.is_ok());
    }

    /// 测试快速开始模式
    #[tokio::test]
    async fn test_quickstart() {
        let pool = DbPool::new("sqlite::memory:").await;
        assert!(pool.is_ok());
    }

    /// 测试使用 MemoryPermissionProvider 进行测试
    #[cfg(feature = "permission")]
    #[tokio::test]
    async fn test_with_memory_permission_provider() {
        use dbnexus::access::permission::{MemoryPermissionProvider, PermissionProvider, RolePolicy, PermissionAction, TablePermission};

        let provider = MemoryPermissionProvider::new();

        // 添加测试角色
        let policy = RolePolicy {
            tables: vec![TablePermission {
                name: "*".to_string(),
                operations: vec![PermissionAction::Select, PermissionAction::Insert],
            }],
        };

        // 由于 MemoryPermissionProvider 使用 AsyncMutex，我们需要使用 tokio runtime
        provider.add_role("test_user", policy).await;

        // 验证角色已添加
        assert!(provider.has_role("test_user"));
    }
}

/// 主函数示例
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== DBNexus 依赖注入示例 ===\n");

    // 示例 1: 快速开始（适用于简单场景）
    println!("1. 快速开始模式:");
    let pool = DbPool::new("sqlite::memory:").await?;
    println!("   ✓ 连接池创建成功");
    println!("   状态: {:?}", pool.status());

    // 示例 2: Builder 模式（适用于需要部分配置的场景）
    println!("\n2. Builder 模式:");
    let pool: DbPool = DbPoolBuilder::new()
        .url("sqlite::memory:")
        .max_connections(20)
        .min_connections(5)
        .build()
        .await?;
    println!("   ✓ 自定义配置的连接池创建成功");
    println!("   状态: {:?}", pool.status());

    // 示例 3: 使用配置结构体（适用于需要完整配置的场景）
    println!("\n3. 配置结构体模式:");
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 15,
        min_connections: 3,
        ..Default::default()
    };

    let pool: DbPool = DbPoolBuilder::new()
        .config(config)
        .build()
        .await?;
    println!("   ✓ 使用配置结构体的连接池创建成功");
    println!("   状态: {:?}", pool.status());

    // 示例 4: 环境特定配置
    println!("\n4. 环境特定配置:");
    let env = Environment::from_env();
    println!("   当前环境: {:?}", env);

    let pool = AppConfigBuilder::new()
        .db_url("sqlite::memory:")
        .max_connections(10)
        .build()
        .await?;
    println!("   ✓ 基于环境的连接池创建成功");
    println!("   状态: {:?}", pool.status());

    // 示例 5: 在测试中使用 Mock
    println!("\n5. 测试中的 Mock 使用:");
    #[cfg(feature = "metrics")]
    {
        let mock_metrics = MockMetrics::new();
        mock_metrics.record_query(std::time::Duration::from_millis(10));
        mock_metrics.record_connection(std::time::Duration::from_millis(5));
        println!("   ✓ MockMetrics 工作正常");
        println!("   导出结果: '{}'", mock_metrics.export_prometheus());
    }

    println!("\n=== 所有示例完成 ===");
    Ok(())
}
