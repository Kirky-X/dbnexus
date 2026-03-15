// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 配置验证
//!
//! 包含配置验证相关函数。

use super::security::validate_url_format;
use super::types::{ConfigError, DbConfig};

/// 验证配置必填字段
///
/// # Errors
///
/// 如果缺少必填字段或格式无效，返回错误
pub fn validate_config(config: &DbConfig) -> Result<(), ConfigError> {
    if config.url.is_empty() {
        return Err(ConfigError::MissingField);
    }

    // URL 格式验证
    validate_url_format(&config.url)?;

    if config.max_connections == 0 {
        return Err(ConfigError::MissingField);
    }

    // 验证 max_connections 范围（1-1000）
    if config.max_connections > 1000 {
        return Err(ConfigError::ValidationFailed);
    }

    // 验证 min_connections 范围（1-100）
    if config.min_connections == 0 || config.min_connections > 100 {
        return Err(ConfigError::ValidationFailed);
    }

    if config.min_connections > config.max_connections {
        return Err(ConfigError::InvalidFormat);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::config::DbConfigBuilder;

    /// TEST-U-010: 配置验证测试 - 空URL
    #[test]
    fn test_config_validation_empty_url() {
        let config = DbConfigBuilder::new().build().unwrap_err();

        assert_eq!(config.to_string(), "Missing required configuration field");
    }

    /// TEST-U-011: 配置验证测试 - 无效的连接数
    #[test]
    fn test_config_validation_invalid_connections() {
        let result = DbConfigBuilder::new().url("sqlite::memory:").max_connections(0).build();

        assert!(result.is_err());
    }
}
