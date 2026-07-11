// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 默认权限提供者实现（YAML 文件）

use crate::domain::permission::{
    PermissionAction, PermissionChecker, PermissionConfig, PermissionError, PermissionLifecycle, PermissionProvider,
    PolicyManager, RolePolicy,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

/// YAML 权限提供者
pub struct YamlPermissionProvider {
    config: PermissionConfig,
    policies: RwLock<HashMap<String, RolePolicy>>,
    #[cfg(feature = "cache")]
    cache: Option<std::sync::Arc<oxcache::Cache<String, RolePolicy>>>,
}

impl YamlPermissionProvider {
    /// 创建新的 YAML 权限提供者
    pub async fn new(config: PermissionConfig) -> Result<Self, PermissionError> {
        let provider = Self {
            config: config.clone(),
            policies: RwLock::new(HashMap::new()),
            #[cfg(feature = "cache")]
            cache: None,
        };

        // 如果指定了策略文件，尝试加载
        if config.policy_path.is_some() {
            provider.load_policies().await?;
        }

        Ok(provider)
    }

    /// 带缓存注入的构造函数
    #[cfg(feature = "cache")]
    pub async fn with_cache(
        config: PermissionConfig,
        cache: std::sync::Arc<oxcache::Cache<String, RolePolicy>>,
    ) -> Result<Self, PermissionError> {
        let provider = Self {
            config: config.clone(),
            policies: RwLock::new(HashMap::new()),
            cache: Some(cache),
        };

        // 如果指定了策略文件，尝试加载
        if config.policy_path.is_some() {
            provider.load_policies().await?;
        }

        Ok(provider)
    }

    /// 加载策略文件
    async fn load_policies(&self) -> Result<(), PermissionError> {
        if let Some(path) = &self.config.policy_path {
            let content = tokio::fs::read_to_string(path)
                .await
                .map_err(|e| PermissionError::ParseError(format!("Failed to read {}: {}", path, e)))?;

            let policies: HashMap<String, RolePolicy> =
                serde_yaml_ng::from_str(&content).map_err(|e| PermissionError::ParseError(e.to_string()))?;

            // 将策略存入缓存
            #[cfg(feature = "cache")]
            if let Some(cache) = &self.cache {
                for (role, policy) in &policies {
                    let _ = cache.set(role, policy).await;
                }
            }

            let mut guard = self.policies.write().unwrap();
            *guard = policies;
        }
        Ok(())
    }
}

#[async_trait]
impl PermissionChecker for YamlPermissionProvider {
    async fn check(&self, role: &str, table: &str, action: PermissionAction) -> Result<bool, PermissionError> {
        // 管理员始终允许
        if role == self.config.admin_role {
            return Ok(true);
        }

        let guard = self.policies.read().unwrap();
        if let Some(policy) = guard.get(role) {
            Ok(policy.allows(table, &action))
        } else {
            // 根据默认策略
            match self.config.default_policy {
                crate::domain::permission::DefaultPolicy::AllowAll => Ok(true),
                crate::domain::permission::DefaultPolicy::DenyAll => Ok(false),
            }
        }
    }
}

#[async_trait]
impl PolicyManager for YamlPermissionProvider {
    async fn get_policy(&self, role: &str) -> Result<Option<RolePolicy>, PermissionError> {
        let guard = self.policies.read().unwrap();
        Ok(guard.get(role).cloned())
    }

    async fn refresh(&self) -> Result<(), PermissionError> {
        self.load_policies().await
    }
}

#[async_trait]
impl PermissionLifecycle for YamlPermissionProvider {
    async fn health_check(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn shutdown(&self) {
        let mut guard = self.policies.write().unwrap();
        guard.clear();
    }
}

impl PermissionProvider for YamlPermissionProvider {}
