// Copyright (c) 2026 Kirky.X
//
// Licensed under MIT License
// See LICENSE file in project root for full license information.

//! 权限提供者 trait 和实现
//!
//! 定义权限配置的通用接口，便于测试和替换实现。

use super::types::{PermissionAction, PermissionConfig, RolePolicy};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex as AsyncMutex;

/// 权限提供者错误类型
#[derive(Debug, Error)]
pub enum PermissionProviderError {
    /// 角色未找到
    #[error("Role '{0}' not found")]
    RoleNotFound(String),

    /// 配置加载失败
    #[error("Failed to load config: {0}")]
    LoadError(String),

    /// 权限检查失败
    #[error("Permission check failed: {0}")]
    CheckError(String),

    /// 未知错误
    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// 权限提供者 trait 接口
///
/// 定义权限配置的通用接口，便于测试和替换实现。
/// 所有实现必须支持 `Send + Sync` 以便在多线程环境中使用。
///
/// # Example
///
/// ```ignore
/// use std::sync::Arc;
/// use dbnexus::permission::PermissionProvider;
///
/// // 使用 trait 对象进行动态分发
/// let provider: Arc<dyn PermissionProvider> = Arc::new(YamlPermissionProvider::new());
///
/// // 或者在测试中使用 mock 实现
/// struct MockProvider;
/// impl PermissionProvider for MockProvider {
///     fn get_role_policy(&self, role: &str) -> Option<RolePolicy> {
///         Some(RolePolicy::default())
///     }
/// }
/// ```
pub trait PermissionProvider: Send + Sync {
    /// 获取角色策略
    ///
    /// # Arguments
    ///
    /// * `role` - 角色名称
    ///
    /// # Returns
    ///
    /// 返回角色的权限策略，如果角色不存在则返回 None
    fn get_role_policy(&self, role: &str) -> Option<RolePolicy>;

    /// 检查权限
    ///
    /// # Arguments
    ///
    /// * `role` - 角色名称
    /// * `table` - 表名
    /// * `operation` - 操作类型
    ///
    /// # Returns
    ///
    /// - `Ok(true)` - 有权限
    /// - `Ok(false)` - 无权限
    /// - `Err(_)` - 检查失败
    fn check_access(
        &self,
        role: &str,
        table: &str,
        operation: PermissionAction,
    ) -> Result<bool, PermissionProviderError>;

    /// 获取所有角色名称
    ///
    /// # Returns
    ///
    /// 返回所有已配置的角色名称
    fn get_roles(&self) -> Vec<String>;

    /// 检查角色是否存在
    fn has_role(&self, role: &str) -> bool {
        self.get_role_policy(role).is_some()
    }
}

/// 可刷新的权限提供者 trait
///
/// 支持动态刷新配置的权限提供者需要实现此 trait。
/// 此 trait 与 `PermissionProvider` 分离，以确保 `PermissionProvider` 是 dyn compatible。
pub trait RefreshablePermissionProvider: PermissionProvider {
    /// 刷新配置（如果支持动态加载）
    fn refresh(&mut self) -> impl std::future::Future<Output = Result<(), PermissionProviderError>> + Send;
}

/// YAML 文件权限提供者
///
/// 从 YAML 文件加载权限配置
#[derive(Debug, Clone)]
pub struct YamlPermissionProvider {
    /// 权限配置
    config: Arc<PermissionConfig>,
    /// 配置文件路径（保留用于将来实现）
    #[allow(dead_code)]
    path: Option<String>,
}

impl YamlPermissionProvider {
    /// 创建新的 YAML 权限提供者
    ///
    /// 通过 confers::loader::parse_yaml 解析 YAML 文件，与项目配置管理策略一致
    ///
    /// # Arguments
    ///
    /// * `path` - YAML 配置文件路径
    ///
    /// # Errors
    ///
    /// 如果文件读取失败或 YAML 解析失败，返回错误
    #[cfg(feature = "confers")]
    pub fn new(path: &str) -> Self {
        let config = if let Ok(content) = std::fs::read_to_string(path) {
            // 使用 confers 解析 YAML 配置
            match Self::parse_yaml_content(&content, path) {
                Ok(cfg) => cfg,
                Err(_e) => {
                    // 解析失败，使用默认拒绝策略
                    PermissionConfig::deny_all()
                }
            }
        } else {
            PermissionConfig::deny_all()
        };

        Self {
            config: Arc::new(config),
            path: Some(path.to_string()),
        }
    }

    /// 解析权限配置内容
    /// 直接使用 JSON 解析（绕过 confers 的键路径展平问题）
    #[cfg(feature = "confers")]
    #[cfg(feature = "json")]
    fn parse_json_content(content: &str, source: &str) -> Result<PermissionConfig, String> {
        serde_json::from_str(content).map_err(|e| format!("JSON parse error in '{}': {}", source, e))
    }

    /// 使用 confers 解析 YAML 内容
    #[cfg(feature = "confers")]
    fn parse_yaml_content(content: &str, source: &str) -> Result<PermissionConfig, String> {
        // 直接使用 JSON 解析
        #[cfg(feature = "json")]
        {
            Self::parse_json_content(content, source)
        }
        #[cfg(not(feature = "json"))]
        {
            // 如果没有 json feature，使用 serde_yaml_ng 直接解析
            #[cfg(feature = "yaml")]
            {
                serde_yaml_ng::from_str(content).map_err(|e| format!("YAML parse error in '{}': {}", source, e))
            }
            #[cfg(not(feature = "yaml"))]
            {
                Err(format!(
                    "Cannot parse permission config from '{}': neither JSON nor YAML support available",
                    source
                ))
            }
        }
    }

    /// 从配置创建
    pub fn from_config(config: PermissionConfig) -> Self {
        Self {
            config: Arc::new(config),
            path: None,
        }
    }
}

impl PermissionProvider for YamlPermissionProvider {
    fn get_role_policy(&self, role: &str) -> Option<RolePolicy> {
        self.config.get_role_policy(role).cloned()
    }

    fn check_access(
        &self,
        role: &str,
        table: &str,
        operation: PermissionAction,
    ) -> Result<bool, PermissionProviderError> {
        Ok(self.config.check_access(role, table, operation))
    }

    fn get_roles(&self) -> Vec<String> {
        self.config.roles.keys().cloned().collect()
    }
}

impl RefreshablePermissionProvider for YamlPermissionProvider {
    #[cfg(feature = "confers")]
    async fn refresh(&mut self) -> Result<(), PermissionProviderError> {
        if let Some(ref path) = self.path {
            match tokio::fs::read_to_string(path).await {
                Ok(content) => match parse_permission_yaml_async(&content, path).await {
                    Ok(config) => {
                        self.config = Arc::new(config);
                        Ok(())
                    }
                    Err(e) => Err(PermissionProviderError::LoadError(e.to_string())),
                },
                Err(e) => Err(PermissionProviderError::LoadError(e.to_string())),
            }
        } else {
            Ok(())
        }
    }

    #[cfg(not(feature = "confers"))]
    async fn refresh(&mut self) -> Result<(), PermissionProviderError> {
        Err(PermissionProviderError::LoadError(
            "Confers support not enabled".to_string(),
        ))
    }
}

/// 使用 confers 异步解析 YAML 内容（独立函数）
#[cfg(feature = "confers")]
async fn parse_permission_yaml_async(content: &str, source: &str) -> Result<PermissionConfig, String> {
    use confers::loader::parse_yaml;
    use confers::value::SourceId;

    let source_id = SourceId::new(source);
    let annotated = parse_yaml(content, source_id, Some(std::path::Path::new(source)))
        .map_err(|e| format!("YAML parse error: {}", e))?;

    // 转换为 JSON Value 然后反序列化为 PermissionConfig
    #[cfg(feature = "json")]
    {
        let json_value = annotated.to_json();
        serde_json::from_value(json_value).map_err(|e| format!("Config deserialization error: {}", e))
    }
    #[cfg(not(feature = "json"))]
    {
        // 如果没有 json feature，直接尝试从 ConfigValue 反序列化
        let json_str =
            serde_json::to_string(&annotated.inner).map_err(|e| format!("JSON serialization error: {}", e))?;
        serde_json::from_str(&json_str).map_err(|e| format!("Config deserialization error: {}", e))
    }
}

/// 内存权限提供者
///
/// 允许程序化配置权限
#[derive(Debug, Clone)]
pub struct MemoryPermissionProvider {
    config: Arc<AsyncMutex<PermissionConfig>>,
}

impl Default for MemoryPermissionProvider {
    fn default() -> Self {
        Self {
            config: Arc::new(AsyncMutex::new(PermissionConfig::default())),
        }
    }
}

impl MemoryPermissionProvider {
    /// 创建新的内存权限提供者
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加角色策略
    pub async fn add_role(&self, role: &str, policy: RolePolicy) {
        let mut config = self.config.lock().await;
        config.roles.insert(role.to_string(), policy);
    }

    /// 移除角色
    pub async fn remove_role(&self, role: &str) -> bool {
        let mut config = self.config.lock().await;
        config.roles.remove(role).is_some()
    }
}

impl PermissionProvider for MemoryPermissionProvider {
    fn get_role_policy(&self, role: &str) -> Option<RolePolicy> {
        // 使用 try_poll 避免阻塞，如果锁不可用则返回 None
        // 这是安全的，因为在权限检查的上下文中，短暂的不可接受性是可以接受的
        if let Ok(config) = self.config.try_lock() {
            config.get_role_policy(role).cloned()
        } else {
            // 锁被占用，暂时无法获取策略
            // 在这种情况下，返回 None 比阻塞更安全
            None
        }
    }

    fn check_access(
        &self,
        role: &str,
        table: &str,
        operation: PermissionAction,
    ) -> Result<bool, PermissionProviderError> {
        if let Ok(config) = self.config.try_lock() {
            Ok(config.check_access(role, table, operation))
        } else {
            // 锁被占用，暂时拒绝访问
            Ok(false)
        }
    }

    fn get_roles(&self) -> Vec<String> {
        if let Ok(config) = self.config.try_lock() {
            config.roles.keys().cloned().collect()
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access::permission::types::TablePermission;

    #[tokio::test]
    async fn memory_provider_new_is_empty() {
        let p = MemoryPermissionProvider::new();
        assert!(p.get_roles().is_empty());
    }

    #[tokio::test]
    async fn memory_provider_add_role() {
        let p = MemoryPermissionProvider::new();
        p.add_role("admin", RolePolicy {
            tables: vec![TablePermission {
                name: "*".into(),
                operations: vec![PermissionAction::Select, PermissionAction::Insert],
            }],
        }).await;
        let policy = p.get_role_policy("admin");
        assert!(policy.is_some());
    }

    #[tokio::test]
    async fn memory_provider_check_access() {
        let p = MemoryPermissionProvider::new();
        p.add_role("reader", RolePolicy {
            tables: vec![TablePermission {
                name: "docs".into(),
                operations: vec![PermissionAction::Select],
            }],
        }).await;
        assert!(p.check_access("reader", "docs", PermissionAction::Select).unwrap());
        assert!(!p.check_access("reader", "docs", PermissionAction::Delete).unwrap());
        // non-existent role returns Ok(false) from PermissionConfig, never Err
        assert!(!p.check_access("ghost", "docs", PermissionAction::Select).unwrap());
    }

    #[tokio::test]
    async fn memory_provider_remove_role() {
        let p = MemoryPermissionProvider::new();
        p.add_role("temp", RolePolicy::default()).await;
        assert!(p.remove_role("temp").await);
        assert!(!p.remove_role("temp").await);
    }

    #[tokio::test]
    async fn memory_provider_get_roles() {
        let p = MemoryPermissionProvider::new();
        p.add_role("a", RolePolicy::default()).await;
        p.add_role("b", RolePolicy::default()).await;
        let mut roles = p.get_roles();
        roles.sort();
        assert_eq!(roles, vec!["a", "b"]);
    }

    #[test]
    fn yaml_provider_from_config() {
        let mut config = PermissionConfig::default();
        config.roles.insert("admin".into(), RolePolicy {
            tables: vec![TablePermission {
                name: "*".into(),
                operations: vec![PermissionAction::Select],
            }],
        });
        let p = YamlPermissionProvider::from_config(config);
        assert!(p.get_role_policy("admin").is_some());
        assert!(p.get_role_policy("nobody").is_none());
    }

    #[test]
    fn yaml_provider_check_access_from_config() {
        let mut config = PermissionConfig::default();
        config.roles.insert("viewer".into(), RolePolicy {
            tables: vec![TablePermission {
                name: "reports".into(),
                operations: vec![PermissionAction::Select],
            }],
        });
        let p = YamlPermissionProvider::from_config(config);
        assert!(p.check_access("viewer", "reports", PermissionAction::Select).unwrap());
        assert!(!p.check_access("viewer", "reports", PermissionAction::Update).unwrap());
        assert!(!p.check_access("viewer", "secret", PermissionAction::Select).unwrap());
    }

    #[test]
    fn permission_provider_has_role() {
        let mut config = PermissionConfig::default();
        config.roles.insert("admin".into(), RolePolicy::default());
        let p: Arc<dyn PermissionProvider> = Arc::new(YamlPermissionProvider::from_config(config));
        assert!(p.has_role("admin"));
        assert!(!p.has_role("nobody"));
    }
}
