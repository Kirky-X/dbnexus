// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 内存权限提供者实现

use crate::domain::{
    PermissionAction, PermissionChecker, PermissionConfig, PermissionError, PermissionLifecycle, PermissionProvider,
    PolicyManager, RolePolicy,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

/// 内存权限提供者
pub struct MemoryPermissionProvider {
    config: PermissionConfig,
    policies: RwLock<HashMap<String, RolePolicy>>,
}

impl MemoryPermissionProvider {
    /// 创建新的内存权限提供者
    pub fn new() -> Self {
        Self {
            config: PermissionConfig::default(),
            policies: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for MemoryPermissionProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PermissionChecker for MemoryPermissionProvider {
    async fn check(&self, role: &str, table: &str, action: PermissionAction) -> Result<bool, PermissionError> {
        // 管理员始终允许
        if role == self.config.admin_role {
            return Ok(true);
        }

        let guard = self.policies.read().unwrap();
        let policy = guard
            .get(role)
            .ok_or_else(|| PermissionError::RoleNotFound(role.into()))?;
        Ok(policy.allows(table, &action))
    }
}

#[async_trait]
impl PolicyManager for MemoryPermissionProvider {
    async fn get_policy(&self, role: &str) -> Result<Option<RolePolicy>, PermissionError> {
        let guard = self.policies.read().unwrap();
        Ok(guard.get(role).cloned())
    }

    async fn refresh(&self) -> Result<(), PermissionError> {
        // 内存实现无需刷新
        Ok(())
    }
}

#[async_trait]
impl PermissionLifecycle for MemoryPermissionProvider {
    async fn health_check(&self) -> anyhow::Result<()> {
        let guard = self.policies.read().unwrap();
        if guard.is_empty() {
            return Err(anyhow::anyhow!(
                "MemoryPermissionProvider 策略表未初始化：无任何角色策略（容量 0）"
            ));
        }
        Ok(())
    }

    async fn shutdown(&self) {
        let mut guard = self.policies.write().unwrap();
        guard.clear();
    }
}

impl PermissionProvider for MemoryPermissionProvider {}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn health_check_fails_when_strategy_table_empty() {
        let provider = MemoryPermissionProvider::new();
        let result = provider.health_check().await;
        assert!(result.is_err(), "空策略表必须报告不健康");
    }

    #[tokio::test]
    async fn health_check_ok_after_policy_loaded() {
        let provider = MemoryPermissionProvider::new();
        provider
            .policies
            .write()
            .unwrap()
            .insert("admin".to_string(), RolePolicy { tables: Vec::new() });
        let result = provider.health_check().await;
        assert!(result.is_ok(), "已加载策略的 provider 必须健康: {result:?}");
    }
}
