// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 默认权限提供者实现（YAML 文件）

use crate::domain::{
    PermissionAction, PermissionChecker, PermissionConfig, PermissionError, PermissionLifecycle,
    PermissionProvider, PolicyManager, RolePolicy,
};
use crate::i18n;
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
            // 读取前校验路径安全性（构造时不一定调用过 validate）
            PermissionConfig::validate_policy_path(path)?;

            let content = tokio::fs::read_to_string(path).await.map_err(|e| {
                PermissionError::ParseError(format!("Failed to read {}: {}", path, e))
            })?;

            let policies: HashMap<String, RolePolicy> = serde_yaml_ng::from_str(&content)
                .map_err(|e| PermissionError::ParseError(e.to_string()))?;

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
    async fn check(
        &self,
        role: &str,
        table: &str,
        action: PermissionAction,
    ) -> Result<bool, PermissionError> {
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
                crate::domain::DefaultPolicy::AllowAll => Ok(true),
                crate::domain::DefaultPolicy::DenyAll => Ok(false),
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
        if let Some(path) = &self.config.policy_path {
            // 读取前校验路径安全性（与 load_policies 共用同一校验）
            PermissionConfig::validate_policy_path(path)?;

            tokio::fs::read(path).await.map_err(|e| {
                anyhow::anyhow!(i18n::t(
                    "perm-provider-yaml-file-unreadable",
                    &[
                        ("path", std::path::Path::new(path).display().to_string()),
                        ("error", e.to_string())
                    ],
                ))
            })?;
        }
        Ok(())
    }

    async fn shutdown(&self) {
        let mut guard = self.policies.write().unwrap();
        guard.clear();
    }
}

impl PermissionProvider for YamlPermissionProvider {}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_provider(config: PermissionConfig) -> YamlPermissionProvider {
        YamlPermissionProvider {
            config,
            policies: RwLock::new(HashMap::new()),
            #[cfg(feature = "cache")]
            cache: None,
        }
    }

    #[tokio::test]
    async fn health_check_ok_without_policy_file() {
        let provider = make_provider(PermissionConfig::default());
        assert!(provider.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn health_check_fails_when_policy_file_missing() {
        let config = PermissionConfig {
            policy_path: Some("__nonexistent_permissions_file.yaml".to_string()),
            ..PermissionConfig::default()
        };
        let provider = make_provider(config);
        assert!(provider.health_check().await.is_err());
    }

    #[tokio::test]
    async fn health_check_ok_when_policy_file_readable() {
        let path = std::env::temp_dir().join(format!("dbnexus_health_{}.yaml", std::process::id()));
        std::fs::write(&path, b"roles:\n  admin: {tables: []}\n").unwrap();
        let config = PermissionConfig {
            policy_path: Some(path.to_string_lossy().to_string()),
            ..PermissionConfig::default()
        };
        let provider = make_provider(config);
        let result = provider.health_check().await;
        std::fs::remove_file(&path).unwrap();
        assert!(
            result.is_ok(),
            "a readable policy file must be healthy: {result:?}"
        );
    }

    #[tokio::test]
    async fn health_check_rejects_relative_policy_path() {
        let config = PermissionConfig {
            policy_path: Some("relative/permissions.yaml".to_string()),
            ..PermissionConfig::default()
        };
        let provider = make_provider(config);
        assert!(provider.health_check().await.is_err());
    }

    #[tokio::test]
    async fn health_check_rejects_parent_dir_policy_path() {
        let config = PermissionConfig {
            policy_path: Some("/etc/../tmp/permissions.yaml".to_string()),
            ..PermissionConfig::default()
        };
        let provider = make_provider(config);
        assert!(provider.health_check().await.is_err());
    }

    #[tokio::test]
    async fn load_policies_rejects_unsafe_policy_path() {
        let provider = make_provider(PermissionConfig {
            policy_path: Some("relative/permissions.yaml".to_string()),
            ..PermissionConfig::default()
        });
        assert!(provider.load_policies().await.is_err());
    }

    #[tokio::test]
    async fn new_rejects_unsafe_policy_path() {
        let config = PermissionConfig {
            policy_path: Some("../escape.yaml".to_string()),
            ..PermissionConfig::default()
        };
        assert!(YamlPermissionProvider::new(config).await.is_err());
    }

    /// 绝对路径通过安全校验但文件不可读：health_check 必须报错（读取失败路径）
    #[tokio::test]
    async fn health_check_reports_unreadable_absolute_policy_file() {
        let path = std::env::temp_dir().join(format!(
            "dbnexus_missing_policy_{}.yaml",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let provider = make_provider(PermissionConfig {
            policy_path: Some(path.to_string_lossy().to_string()),
            ..PermissionConfig::default()
        });

        let err = provider
            .health_check()
            .await
            .expect_err("an absolute but unreadable policy file must be unhealthy");
        assert!(
            !err.to_string().is_empty(),
            "the error must carry the locale message"
        );
    }

    /// 带缓存构造：加载策略文件时同步写入注入的策略缓存
    #[cfg(feature = "cache")]
    #[tokio::test]
    async fn with_cache_populates_policy_cache_on_load() {
        let path =
            std::env::temp_dir().join(format!("dbnexus_perm_cache_{}.yaml", std::process::id()));
        std::fs::write(
            &path,
            b"admin:\n  tables:\n    - name: \"*\"\n      operations: [Select, Insert, Update, Delete]\n",
        )
        .expect("write policy file");

        let cache = std::sync::Arc::new(
            oxcache::Cache::<String, RolePolicy>::builder()
                .capacity(8)
                .build()
                .await
                .expect("cache"),
        );
        let config = PermissionConfig {
            policy_path: Some(path.to_string_lossy().to_string()),
            ..PermissionConfig::default()
        };
        let provider = YamlPermissionProvider::with_cache(config, cache.clone())
            .await
            .expect("provider with cache");
        std::fs::remove_file(&path).unwrap();

        // 加载的每个角色都进入缓存，provider 自身同样可查询
        let cached = cache
            .get(&"admin".to_string())
            .await
            .expect("cache lookup")
            .expect("the loaded role must be cached");
        assert_eq!(cached.tables.len(), 1);
        assert!(
            provider
                .get_policy("admin")
                .await
                .expect("get_policy")
                .is_some(),
            "the provider must expose the loaded policy"
        );
    }
}
