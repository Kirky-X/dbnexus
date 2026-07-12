// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Permission module implementation details.
//!
//! Contains factory functions extracted from [`super`].

use super::*;

/// 标准工厂函数
///
/// # Errors
/// 返回 `PermissionConfigError` 当配置验证失败时
pub async fn new(config: PermissionConfig) -> Result<impl PermissionProvider, PermissionConfigError> {
    config.validate()?;
    impl_::default::YamlPermissionProvider::new(config)
        .await
        .map_err(|e| PermissionConfigError::InvalidValue {
            field: "policy_path".into(),
            reason: e.to_string(),
        })
}

/// 带缓存注入的工厂函数
///
/// # Errors
/// 返回 `PermissionConfigError` 当配置验证失败时
#[cfg(feature = "cache")]
pub async fn with_cache(
    config: PermissionConfig,
    cache: std::sync::Arc<oxcache::Cache<String, RolePolicy>>,
) -> Result<impl PermissionProvider, PermissionConfigError> {
    config.validate()?;
    impl_::default::YamlPermissionProvider::with_cache(config, cache)
        .await
        .map_err(|e| PermissionConfigError::InvalidValue {
            field: "policy_path".into(),
            reason: e.to_string(),
        })
}

/// 内存实现工厂函数（测试用）
pub fn new_in_memory() -> impl PermissionProvider {
    impl_::memory::MemoryPermissionProvider::new()
}
