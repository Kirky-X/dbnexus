// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 连接池配置

use super::error::PoolConfigError;
use serde::Deserialize;

/// 连接池配置
#[derive(Debug, Clone, Deserialize)]
pub struct PoolConfig {
    /// 数据库连接 URL（必填）
    pub url: String,

    /// 最大连接数
    #[serde(default = "PoolConfig::default_max_connections")]
    pub max_connections: u32,

    /// 最小连接数
    #[serde(default = "PoolConfig::default_min_connections")]
    pub min_connections: u32,

    /// 空闲超时（秒）
    #[serde(default = "PoolConfig::default_idle_timeout")]
    pub idle_timeout: u64,

    /// 获取连接超时（毫秒）
    #[serde(default = "PoolConfig::default_acquire_timeout")]
    pub acquire_timeout: u64,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            max_connections: Self::default_max_connections(),
            min_connections: Self::default_min_connections(),
            idle_timeout: Self::default_idle_timeout(),
            acquire_timeout: Self::default_acquire_timeout(),
        }
    }
}

impl PoolConfig {
    /// 语义校验
    pub fn validate(&self) -> Result<(), PoolConfigError> {
        if self.url.is_empty() {
            return Err(PoolConfigError::MissingField("url".into()));
        }
        if self.max_connections == 0 {
            return Err(PoolConfigError::InvalidValue {
                field: "max_connections".into(),
                reason: "must be greater than 0".into(),
            });
        }
        if self.min_connections > self.max_connections {
            return Err(PoolConfigError::InvalidValue {
                field: "min_connections".into(),
                reason: "cannot exceed max_connections".into(),
            });
        }
        Ok(())
    }

    fn default_max_connections() -> u32 {
        20
    }
    fn default_min_connections() -> u32 {
        5
    }
    fn default_idle_timeout() -> u64 {
        300
    }
    fn default_acquire_timeout() -> u64 {
        5000
    }
}
