// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
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

impl crate::i18n::error_ext::LocalizedMsg for PermissionProviderError {
    fn message_key(&self) -> &'static str {
        match self {
            Self::RoleNotFound(_) => "perm-provider-role-not-found",
            Self::LoadError(_) => "perm-provider-load-error",
            Self::CheckError(_) => "perm-provider-check-error",
            Self::Unknown(_) => "perm-provider-unknown",
        }
    }

    fn message_args(&self) -> Vec<(&str, String)> {
        match self {
            Self::RoleNotFound(role) => vec![("role", role.clone())],
            Self::LoadError(reason) => vec![("reason", reason.clone())],
            Self::CheckError(reason) => vec![("reason", reason.clone())],
            Self::Unknown(reason) => vec![("reason", reason.clone())],
        }
    }
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
    /// 配置文件路径（用于 `refresh()` 时重新加载配置文件）
    path: Option<String>,
}

impl YamlPermissionProvider {
    /// 创建新的 YAML 权限提供者
    ///
    /// 通过 `serde_yaml_ng` 直接解析 YAML 文件，与项目配置管理策略一致
    ///
    /// # Arguments
    ///
    /// * `path` - YAML 配置文件路径
    ///
    /// # Errors
    ///
    /// 如果文件读取失败或 YAML 解析失败，返回 `PermissionProviderError::LoadError`
    pub fn new(path: &str) -> Result<Self, PermissionProviderError> {
        let config = match std::fs::read_to_string(path) {
            Ok(content) => Self::parse_yaml_content(&content, path).map_err(|e| {
                PermissionProviderError::LoadError(format!("YAML parse error (path='{}'): {}", path, e))
            })?,
            Err(e) => {
                return Err(PermissionProviderError::LoadError(format!(
                    "Permission config file read error (path='{}'): {}",
                    path, e
                )));
            }
        };

        Ok(Self {
            config: Arc::new(config),
            path: Some(path.to_string()),
        })
    }

    /// 使用 `serde_yaml_ng` 解析 YAML 内容
    ///
    /// YAML 是 JSON 的超集，因此 `serde_yaml_ng` 同时兼容 JSON 输入，
    /// 无需根据 `json` feature 切换解析器。
    fn parse_yaml_content(content: &str, source: &str) -> Result<PermissionConfig, String> {
        #[cfg(feature = "yaml")]
        {
            serde_yaml_ng::from_str(content).map_err(|e| format!("YAML parse error in '{}': {}", source, e))
        }
        #[cfg(not(feature = "yaml"))]
        {
            let _ = (content, source);
            Err("Cannot parse permission config: 'yaml' feature is not enabled".to_string())
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
}

/// 使用 `serde_yaml_ng` 异步解析 YAML 内容（独立函数）
///
/// YAML 是 JSON 的超集，因此 `serde_yaml_ng` 同时兼容 JSON 输入，
/// 无需根据 `json` feature 切换解析器。
async fn parse_permission_yaml_async(content: &str, source: &str) -> Result<PermissionConfig, String> {
    #[cfg(feature = "yaml")]
    {
        serde_yaml_ng::from_str(content).map_err(|e| format!("YAML parse error in '{}': {}", source, e))
    }
    #[cfg(not(feature = "yaml"))]
    {
        let _ = (content, source);
        Err("Cannot parse permission config: 'yaml' feature is not enabled".to_string())
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
    use crate::access::TablePermission;

    #[tokio::test]
    async fn memory_provider_new_is_empty() {
        let p = MemoryPermissionProvider::new();
        assert!(p.get_roles().is_empty());
    }

    #[tokio::test]
    async fn memory_provider_add_role() {
        let p = MemoryPermissionProvider::new();
        p.add_role(
            "admin",
            RolePolicy {
                tables: vec![TablePermission {
                    name: "*".into(),
                    operations: vec![PermissionAction::Select, PermissionAction::Insert],
                }],
            },
        )
        .await;
        let policy = p.get_role_policy("admin");
        assert!(policy.is_some());
    }

    #[tokio::test]
    async fn memory_provider_check_access() {
        let p = MemoryPermissionProvider::new();
        p.add_role(
            "reader",
            RolePolicy {
                tables: vec![TablePermission {
                    name: "docs".into(),
                    operations: vec![PermissionAction::Select],
                }],
            },
        )
        .await;
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
        config.roles.insert(
            "admin".into(),
            RolePolicy {
                tables: vec![TablePermission {
                    name: "*".into(),
                    operations: vec![PermissionAction::Select],
                }],
            },
        );
        let p = YamlPermissionProvider::from_config(config);
        assert!(p.get_role_policy("admin").is_some());
        assert!(p.get_role_policy("nobody").is_none());
    }

    #[test]
    fn yaml_provider_check_access_from_config() {
        let mut config = PermissionConfig::default();
        config.roles.insert(
            "viewer".into(),
            RolePolicy {
                tables: vec![TablePermission {
                    name: "reports".into(),
                    operations: vec![PermissionAction::Select],
                }],
            },
        );
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

    // ========================================================================
    // YamlPermissionProvider::new 边界测试（文件不存在 / 解析失败）
    // ========================================================================

    /// 不存在的文件路径应返回错误
    #[test]
    fn yaml_provider_new_nonexistent_file_returns_error() {
        let result = YamlPermissionProvider::new("/nonexistent/path/permissions.yaml");
        assert!(result.is_err(), "nonexistent file should return error");
        match result {
            Err(PermissionProviderError::LoadError(_)) => { /* expected */ }
            Err(other) => panic!("expected LoadError, got {:?}", other),
            Ok(_) => panic!("expected error, got Ok"),
        }
    }

    /// 提供有效的 YAML 配置文件路径应成功加载
    #[test]
    fn yaml_provider_new_with_valid_file_loads_roles() {
        // 创建临时文件
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("dbnexus_test_permissions_valid.yaml");
        let yaml_content = r#"{"roles": {"admin": {"tables": [{"name": "*", "operations": ["select"]}]}}}"#;
        std::fs::write(&file_path, yaml_content).expect("failed to write temp file");

        let p = YamlPermissionProvider::new(file_path.to_str().unwrap()).unwrap();
        let roles = p.get_roles();
        assert!(
            roles.iter().any(|r| r == "admin"),
            "admin role should be loaded from valid file, got: {:?}",
            roles
        );
        let policy = p.get_role_policy("admin");
        assert!(policy.is_some(), "admin policy should be Some");
        // 验证权限检查
        assert!(
            p.check_access("admin", "any_table", PermissionAction::Select).unwrap(),
            "admin should have Select on any_table"
        );

        // 清理
        let _ = std::fs::remove_file(&file_path);
    }

    /// 提供格式错误的 YAML/JSON 文件应返回错误
    #[test]
    fn yaml_provider_new_with_malformed_file_returns_error() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("dbnexus_test_permissions_malformed.yaml");
        let malformed_content = "this is not valid: yaml: content: [[[";
        std::fs::write(&file_path, malformed_content).expect("failed to write temp file");

        let result = YamlPermissionProvider::new(file_path.to_str().unwrap());
        assert!(result.is_err(), "malformed file should return error");
        match result {
            Err(PermissionProviderError::LoadError(_)) => { /* expected */ }
            Err(other) => panic!("expected LoadError, got {:?}", other),
            Ok(_) => panic!("expected error, got Ok"),
        }

        // 清理
        let _ = std::fs::remove_file(&file_path);
    }

    // ========================================================================
    // YamlPermissionProvider::refresh 异步刷新测试
    // ========================================================================

    /// from_config 创建的 provider 没有 path，refresh 应返回 Ok（无需刷新）
    #[tokio::test]
    async fn yaml_provider_refresh_no_path_returns_ok() {
        let mut config = PermissionConfig::default();
        config.roles.insert("admin".into(), RolePolicy::default());
        let mut p = YamlPermissionProvider::from_config(config);
        let result = p.refresh().await;
        assert!(result.is_ok(), "refresh on pathless provider should return Ok");
    }

    /// refresh 成功路径：文件存在且有效
    #[tokio::test]
    async fn yaml_provider_refresh_valid_file_succeeds() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("dbnexus_test_refresh_valid.yaml");
        let yaml_content = r#"{"roles": {"admin": {"tables": [{"name": "*", "operations": ["select"]}]}}}"#;
        std::fs::write(&file_path, yaml_content).expect("failed to write temp file");

        let mut p = YamlPermissionProvider::new(file_path.to_str().unwrap()).unwrap();
        // 初始加载应有 admin 角色
        assert!(p.get_role_policy("admin").is_some());

        // 重写文件，添加新角色
        let updated_content = r#"{"roles": {"admin": {"tables": [{"name": "*", "operations": ["select"]}]}, "user": {"tables": [{"name": "docs", "operations": ["select"]}]}}}"#;
        std::fs::write(&file_path, updated_content).expect("failed to rewrite temp file");

        // 刷新
        let result = p.refresh().await;
        assert!(result.is_ok(), "refresh should succeed with valid file");

        // 刷新后应有 user 角色
        let roles = p.get_roles();
        assert!(
            roles.iter().any(|r| r == "user"),
            "refresh should load new 'user' role: {:?}",
            roles
        );

        // 清理
        let _ = std::fs::remove_file(&file_path);
    }

    /// refresh 文件读取失败路径
    #[tokio::test]
    async fn yaml_provider_refresh_file_deleted_returns_err() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("dbnexus_test_refresh_deleted.yaml");
        let yaml_content = r#"{"roles": {"admin": {"tables": [{"name": "*", "operations": ["select"]}]}}}"#;
        std::fs::write(&file_path, yaml_content).expect("failed to write temp file");

        let mut p = YamlPermissionProvider::new(file_path.to_str().unwrap()).unwrap();

        // 删除文件，refresh 应失败
        std::fs::remove_file(&file_path).expect("failed to delete temp file");

        let result = p.refresh().await;
        assert!(result.is_err(), "refresh should fail when file is deleted");
        match result {
            Err(PermissionProviderError::LoadError(_)) => { /* expected */ }
            Err(other) => panic!("expected LoadError, got {:?}", other),
            Ok(_) => panic!("expected error, got Ok"),
        }
    }

    /// refresh 解析失败路径
    #[tokio::test]
    async fn yaml_provider_refresh_malformed_returns_err() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("dbnexus_test_refresh_malformed.yaml");
        let yaml_content = r#"{"roles": {"admin": {"tables": [{"name": "*", "operations": ["select"]}]}}}"#;
        std::fs::write(&file_path, yaml_content).expect("failed to write temp file");

        let mut p = YamlPermissionProvider::new(file_path.to_str().unwrap()).unwrap();

        // 写入无效内容
        let malformed = "not valid: yaml: [[[";
        std::fs::write(&file_path, malformed).expect("failed to rewrite temp file");

        let result = p.refresh().await;
        assert!(result.is_err(), "refresh should fail with malformed file");
        match result {
            Err(PermissionProviderError::LoadError(_)) => { /* expected */ }
            Err(other) => panic!("expected LoadError, got {:?}", other),
            Ok(_) => panic!("expected error, got Ok"),
        }

        // 清理
        let _ = std::fs::remove_file(&file_path);
    }

    // ========================================================================
    // RefreshablePermissionProvider trait 对象测试
    // ========================================================================

    /// YamlPermissionProvider 应实现 RefreshablePermissionProvider trait
    #[tokio::test]
    async fn yaml_provider_is_refreshable() {
        let mut config = PermissionConfig::default();
        config.roles.insert("admin".into(), RolePolicy::default());
        let mut p = YamlPermissionProvider::from_config(config);
        // 调用 trait 方法
        use super::RefreshablePermissionProvider;
        let result = <YamlPermissionProvider as RefreshablePermissionProvider>::refresh(&mut p).await;
        assert!(result.is_ok());
    }
}
