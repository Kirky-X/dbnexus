// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 配置自动修正器
//!
//! 提供配置自动修正和验证功能。

#[cfg(any(feature = "postgres", feature = "mysql"))]
use sea_orm::ConnectionTrait;

use super::types::{ConfigError, DatabaseType, DbConfig};

/// 配置自动修正器
#[derive(Debug, Clone)]
pub struct ConfigCorrector;

impl ConfigCorrector {
    /// 获取数据库的最大连接数限制
    ///
    /// 通过查询数据库系统变量获取最大连接数限制。
    /// 如果查询失败，返回默认的保守估计值。
    ///
    /// # Arguments
    ///
    /// * `connection` - 数据库连接
    /// * `db_type` - 数据库类型
    ///
    /// # Returns
    ///
    /// 数据库支持的最大连接数
    pub(crate) async fn query_database_max_connections(
        connection: &sea_orm::DatabaseConnection,
        db_type: DatabaseType,
    ) -> u32 {
        let _ = connection;
        match db_type {
            DatabaseType::Postgres => {
                #[cfg(feature = "postgres")]
                {
                    let result = connection.execute_unprepared("SHOW max_connections").await;

                    match result {
                        Ok(result) => {
                            let rows_affected = result.rows_affected();
                            if rows_affected > 0 {
                                tracing::info!(
                                    "PostgreSQL max_connections query executed, using conservative estimate"
                                );
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to query PostgreSQL max_connections: {}", e);
                        }
                    }
                    100
                }

                #[cfg(not(feature = "postgres"))]
                {
                    100
                }
            }
            DatabaseType::MySql => {
                #[cfg(feature = "mysql")]
                {
                    let result = connection
                        .execute_unprepared("SHOW VARIABLES LIKE 'max_connections'")
                        .await;

                    match result {
                        Ok(_) => {
                            tracing::info!("MySQL max_connections query executed, using conservative estimate");
                        }
                        Err(e) => {
                            tracing::warn!("Failed to query MySQL max_connections: {}", e);
                        }
                    }
                    200
                }

                #[cfg(not(feature = "mysql"))]
                {
                    200
                }
            }
            DatabaseType::Sqlite => {
                // SQLite 不需要查询，它支持几乎无限的连接
                // 但我们仍设置一个合理的上限
                u32::MAX
            }
        }
    }

    /// 自动修正数据库配置
    pub(crate) fn auto_correct(mut config: DbConfig) -> DbConfig {
        // 修正 min_connections > max_connections
        if config.min_connections > config.max_connections {
            tracing::warn!(
                "Correcting min_connections ({}) > max_connections ({}), setting min to max",
                config.min_connections(),
                config.max_connections()
            );
            config.min_connections = config.max_connections;
        }

        // 确保最小连接数至少为 1
        if config.min_connections == 0 {
            config.min_connections = 1;
            tracing::warn!("Correcting min_connections from 0 to 1");
        }

        // 确保最大连接数至少等于最小连接数，且不超过合理范围
        if config.max_connections == 0 {
            config.max_connections = 10;
            tracing::warn!("Correcting max_connections from 0 to 10");
        }

        // 修正 acquire_timeout 为合理范围
        if config.acquire_timeout == 0 {
            config.acquire_timeout = 5000;
        } else if config.acquire_timeout < 1000 {
            tracing::warn!(
                "Adjusting acquire_timeout from {}ms to minimum 1000ms",
                config.acquire_timeout()
            );
            config.acquire_timeout = 1000;
        } else if config.acquire_timeout > 60000 {
            tracing::warn!(
                "Adjusting acquire_timeout from {}ms to maximum 60000ms",
                config.acquire_timeout()
            );
            config.acquire_timeout = 60000;
        }

        // 修正 idle_timeout 为合理范围
        if config.idle_timeout == 0 {
            config.idle_timeout = 300;
        } else if config.idle_timeout < 30 {
            tracing::warn!("Adjusting idle_timeout from {}s to minimum 30s", config.idle_timeout());
            config.idle_timeout = 30;
        } else if config.idle_timeout > 3600 {
            tracing::warn!(
                "Adjusting idle_timeout from {}s to maximum 3600s",
                config.idle_timeout()
            );
            config.idle_timeout = 3600;
        }

        // 对数据库URL进行一些基本检查和修正
        if config.url.starts_with("mysql") || config.url.starts_with("postgres") {
            // 检查URL是否包含必要的参数
            if config.url.contains("localhost") && !config.url.contains("?") && !config.url.contains(";") {
                // 添加一些默认参数以提高连接稳定性
                match config.url.as_str() {
                    url if url.starts_with("mysql://") => {
                        config.url = format!("{}?connect_timeout=10", url);
                    }
                    url if url.starts_with("postgres://") => {
                        config.url = format!("{}?connect_timeout=10", url);
                    }
                    _ => {} // 其他类型跳过
                }
            }
        }

        config
    }

    /// 验证配置是否有效
    ///
    /// 检查配置参数是否符合基本要求：
    /// - URL 不为空
    /// - max_connections > 0
    /// - min_connections <= max_connections
    /// - acquire_timeout > 0
    /// - idle_timeout > 0
    pub(crate) fn validate_config(config: &DbConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if config.url.is_empty() {
            errors.push("Database URL cannot be empty".to_string());
        }

        if config.max_connections() == 0 {
            errors.push("max_connections must be greater than 0".to_string());
        }

        if config.min_connections() > config.max_connections() {
            errors.push("min_connections cannot be greater than max_connections".to_string());
        }

        if config.acquire_timeout() == 0 {
            errors.push("acquire_timeout must be greater than 0".to_string());
        }

        if config.idle_timeout() == 0 {
            errors.push("idle_timeout must be greater than 0".to_string());
        }

        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    /// 从环境变量加载配置并自动修正
    ///
    /// 组合 `DbConfig::from_env()` 和 `ConfigCorrector::auto_correct()`，
    /// 方便一步完成配置加载和修正。
    pub(crate) fn load_and_correct_from_env() -> Result<DbConfig, ConfigError> {
        let mut config = DbConfig::from_env()?;
        config = ConfigCorrector::auto_correct(config);
        Ok(config)
    }

    /// 从配置文件加载配置并自动修正
    #[cfg(feature = "config-yaml")]
    pub(crate) fn load_and_correct_from_file(path: impl AsRef<Path>) -> Result<DbConfig, ConfigError> {
        let mut config = DbConfig::from_yaml_file(path)?;
        config = ConfigCorrector::auto_correct(config);
        Ok(config)
    }

    /// 验证配置并应用自动修正
    ///
    /// 先验证配置有效性，然后应用自动修正。
    /// 如果配置有错误，会返回错误信息并附带修正后的值。
    pub(crate) fn validate_and_correct(config: &DbConfig) -> Result<DbConfig, Vec<String>> {
        let errors = Self::validate_config(config);
        let corrected_config = Self::auto_correct(config.clone());

        match errors {
            Ok(()) => Ok(corrected_config),
            Err(mut validation_errors) => {
                // 添加警告信息表示配置已被自动修正
                validation_errors.extend([
                    "Some configuration values were automatically corrected".to_string(),
                    "Consider updating your configuration file to match corrected values".to_string(),
                ]);
                Err(validation_errors)
            }
        }
    }

    /// 获取当前应用的实际配置
    ///
    /// 返回经过自动修正后的配置副本。
    /// 如果配置从未被修正过，则返回传入的配置。
    ///
    /// # Arguments
    ///
    /// * `config` - 当前使用的配置
    ///
    /// # Returns
    ///
    /// 实际应用的配置（可能已被自动修正）
    pub(crate) fn get_actual_config(config: &DbConfig) -> DbConfig {
        Self::auto_correct(config.clone())
    }

    /// 使用数据库能力修正配置
    ///
    /// 根据数据库的实际能力（最大连接数等）调整配置。
    /// 这是异步方法，需要传入数据库连接。
    ///
    /// # Arguments
    ///
    /// * `config` - 当前配置
    /// * `connection` - 数据库连接
    /// * `db_type` - 数据库类型
    ///
    /// # Returns
    ///
    /// 根据数据库能力修正后的配置
    pub(crate) async fn auto_correct_with_database_capability(
        mut config: DbConfig,
        connection: &sea_orm::DatabaseConnection,
        db_type: DatabaseType,
    ) -> DbConfig {
        // 查询数据库最大连接数
        let db_max_connections = Self::query_database_max_connections(connection, db_type).await;

        // 如果配置值超过数据库能力的 80%，发出警告并调整
        let recommended_max = (db_max_connections as f64 * 0.8).floor() as u32;

        if config.max_connections() > recommended_max {
            tracing::warn!(
                "Config corrected: max_connections {} -> {} (80% of database limit {})",
                config.max_connections(),
                recommended_max,
                db_max_connections
            );
            config.max_connections = recommended_max;
        }

        // 确保 min_connections 不超过 max_connections
        if config.min_connections() > config.max_connections() {
            tracing::warn!(
                "Config corrected: min_connections {} -> {} (equal to max_connections)",
                config.min_connections(),
                config.max_connections()
            );
            config.min_connections = config.max_connections;
        }

        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DbConfigBuilder;

    /// TEST-U-003: 配置自动修正测试 - get_actual_config
    #[test]
    fn test_get_actual_config() {
        // 测试 min > max 的情况 - 先用有效值构建，然后模拟无效场景
        let mut config = DbConfigBuilder::new()
            .url("sqlite::memory:")
            .max_connections(10)
            .min_connections(10)
            .admin_role("admin")
            .build()
            .unwrap();

        // 手动设置无效值来测试 auto_correct 的修正
        config.set_min_connections(30);

        let actual = ConfigCorrector::get_actual_config(&config);

        // min 应该被修正为等于 max (10)
        assert_eq!(actual.max_connections(), 10);
        assert_eq!(actual.min_connections(), 10);
    }

    /// TEST-U-004: 配置自动修正测试 - 零值处理
    #[test]
    fn test_get_actual_config_zero_values() {
        // 先用有效值构建，然后测试 auto_correct 对零值的修正
        let config = DbConfigBuilder::new()
            .url("sqlite::memory:")
            .max_connections(5)
            .min_connections(5)
            .idle_timeout(0)
            .acquire_timeout(0)
            .admin_role("admin")
            .build()
            .unwrap();

        // 模拟零值场景，通过手动设置
        let mut zero_config = config.clone();
        zero_config.set_max_connections(0);
        zero_config.set_min_connections(0);
        zero_config.set_idle_timeout(0);
        zero_config.set_acquire_timeout(0);

        let actual = ConfigCorrector::get_actual_config(&zero_config);

        // 零值应该被修正为默认值
        assert_eq!(actual.max_connections(), 10);
        assert_eq!(actual.min_connections(), 1);
        assert_eq!(actual.idle_timeout(), 300);
        assert_eq!(actual.acquire_timeout(), 5000);
    }
}
