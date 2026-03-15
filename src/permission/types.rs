// Copyright (c) 2026 Kirky.X
//
// Licensed under MIT License
// See LICENSE file in project root for full license information.

//! 权限类型定义
//!
//! 提供权限控制相关的核心类型定义。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// 权限相关错误类型
#[derive(Debug, Error)]
pub enum PermissionError {
    /// 缓存容量无效（不能为 0）
    #[error("Cache capacity must be non-zero")]
    InvalidCacheCapacity,

    /// 角色未找到
    #[error("Role '{0}' not found in permission config")]
    RoleNotFound(String),

    /// 配置文件加载失败
    #[error("Failed to load permission config: {0}")]
    ConfigLoadError(String),

    /// 权限检查被速率限制拒绝
    #[error("Permission check rate limited")]
    RateLimited,
}

/// 权限操作类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionAction {
    /// 查询操作
    Select,
    /// 插入操作
    Insert,
    /// 更新操作
    Update,
    /// 删除操作
    Delete,
}

impl std::fmt::Display for PermissionAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PermissionAction::Select => write!(f, "SELECT"),
            PermissionAction::Insert => write!(f, "INSERT"),
            PermissionAction::Update => write!(f, "UPDATE"),
            PermissionAction::Delete => write!(f, "DELETE"),
        }
    }
}

/// Operation 是 PermissionAction 的别名，用于简化使用
pub type Operation = PermissionAction;

/// 表权限配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TablePermission {
    /// 表名（支持通配符 *）
    pub name: String,

    /// 允许的操作列表
    pub operations: Vec<PermissionAction>,
}

/// 角色策略
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RolePolicy {
    /// 角色允许的表权限
    pub tables: Vec<TablePermission>,
}

impl RolePolicy {
    /// 检查角色是否有权限执行操作
    pub fn allows(&self, table: &str, operation: &PermissionAction) -> bool {
        for perm in &self.tables {
            // 检查表名匹配（支持通配符）
            if perm.name == "*" || perm.name == table {
                // 检查操作权限
                if perm.operations.contains(operation) {
                    return true;
                }
            }
        }
        false
    }
}

/// 权限配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PermissionConfig {
    /// 角色到策略的映射
    #[serde(default)]
    pub roles: HashMap<String, RolePolicy>,
}

impl PermissionConfig {
    /// 从 YAML 字符串加载配置
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    /// 加载角色策略
    pub fn get_role_policy(&self, role: &str) -> Option<&RolePolicy> {
        self.roles.get(role)
    }

    /// 检查角色是否有权限
    pub fn check_access(&self, role: &str, table: &str, operation: PermissionAction) -> bool {
        if let Some(policy) = self.get_role_policy(role) {
            policy.allows(table, &operation)
        } else {
            false
        }
    }

    /// 创建拒绝所有的安全默认配置
    ///
    /// 当配置加载失败时使用此方法作为安全默认策略
    pub fn deny_all() -> Self {
        Self {
            roles: HashMap::new(), // 空角色映射，任何角色都无权限
        }
    }

    /// 创建允许所有的配置（仅用于开发/测试环境）
    pub fn allow_all() -> Self {
        Self {
            roles: HashMap::from([(
                "admin".to_string(),
                RolePolicy {
                    tables: vec![TablePermission {
                        name: "*".to_string(),
                        operations: vec![
                            PermissionAction::Select,
                            PermissionAction::Insert,
                            PermissionAction::Update,
                            PermissionAction::Delete,
                        ],
                    }],
                },
            )]),
        }
    }

    /// 验证配置完整性
    ///
    /// # Errors
    ///
    /// 如果配置不完整，返回错误信息
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // 检查是否定义了至少一个角色
        if self.roles.is_empty() {
            errors.push("No roles defined in permission config".to_string());
        }

        // 检查每个角色的配置
        for (role_name, policy) in &self.roles {
            // 检查角色是否有表权限配置
            if policy.tables.is_empty() {
                errors.push(format!("Role '{}' has no table permissions defined", role_name));
            }

            // 检查每个表权限
            for table_perm in &policy.tables {
                // 检查表名是否为空
                if table_perm.name.trim().is_empty() {
                    errors.push(format!("Role '{}' has a table permission with empty name", role_name));
                }

                // 检查操作列表是否为空
                if table_perm.operations.is_empty() {
                    errors.push(format!(
                        "Table '{}' in role '{}' has no operations defined",
                        table_perm.name, role_name
                    ));
                }
            }
        }

        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    /// 验证并返回验证结果
    /// 如果验证失败，返回第一个错误
    pub fn validate_with_first_error(&self) -> Result<(), String> {
        self.validate().map_err(|errors| errors.join("; "))
    }

    /// 检查角色是否可以执行 DDL 操作
    ///
    /// DDL 权限定义为：角色对 "*" 表拥有所有操作权限（SELECT, INSERT, UPDATE, DELETE）
    /// 这表示该角色是管理员角色，可以执行 DDL 操作
    ///
    /// # Arguments
    ///
    /// * `role` - 要检查的角色名称
    ///
    /// # Returns
    ///
    /// 如果角色可以执行 DDL 操作返回 true
    pub fn is_ddl_allowed_role(&self, role: &str) -> bool {
        if let Some(policy) = self.get_role_policy(role) {
            // 检查角色是否有 "*" 表的所有操作权限
            if let Some(table_perm) = policy.tables.iter().find(|tp| tp.name == "*") {
                // 检查是否包含所有 DDL 相关操作
                table_perm.operations.contains(&PermissionAction::Select)
                    && table_perm.operations.contains(&PermissionAction::Insert)
                    && table_perm.operations.contains(&PermissionAction::Update)
                    && table_perm.operations.contains(&PermissionAction::Delete)
            } else {
                false
            }
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TEST-U-010: Operation (PermissionAction) Display 实现测试
    #[test]
    fn test_operation_display() {
        assert_eq!(PermissionAction::Select.to_string(), "SELECT");
        assert_eq!(PermissionAction::Insert.to_string(), "INSERT");
        assert_eq!(PermissionAction::Update.to_string(), "UPDATE");
        assert_eq!(PermissionAction::Delete.to_string(), "DELETE");
    }

    /// TEST-U-011: RolePolicy allows 测试
    #[test]
    fn test_role_policy_allows() {
        let policy = RolePolicy {
            tables: vec![
                TablePermission {
                    name: "users".to_string(),
                    operations: vec![PermissionAction::Select, PermissionAction::Insert],
                },
                TablePermission {
                    name: "*".to_string(),
                    operations: vec![PermissionAction::Select],
                },
            ],
        };

        // 精确表名匹配
        assert!(policy.allows("users", &PermissionAction::Select));
        assert!(policy.allows("users", &PermissionAction::Insert));
        assert!(!policy.allows("users", &PermissionAction::Delete));

        // 通配符匹配
        assert!(policy.allows("orders", &PermissionAction::Select));
        assert!(!policy.allows("orders", &PermissionAction::Update));
    }

    /// TEST-U-012: PermissionConfig YAML 解析测试
    #[test]
    fn test_permission_config_yaml_parsing() {
        let yaml = r#"
roles:
  admin:
    tables:
      - name: users
        operations:
          - select
          - insert
          - update
          - delete
  user:
    tables:
      - name: users
        operations:
          - select
"#;

        let config = PermissionConfig::from_yaml(yaml).unwrap();

        // 检查 admin 角色
        let admin_policy = config.get_role_policy("admin").unwrap();
        assert!(admin_policy.allows("users", &PermissionAction::Select));
        assert!(admin_policy.allows("users", &PermissionAction::Delete));

        // 检查 user 角色
        let user_policy = config.get_role_policy("user").unwrap();
        assert!(user_policy.allows("users", &PermissionAction::Select));
        assert!(!user_policy.allows("users", &PermissionAction::Insert));

        // 检查不存在的角色
        assert!(config.get_role_policy("guest").is_none());
    }

    /// TEST-U-014: PermissionConfig check_access 测试
    #[test]
    fn test_permission_config_check_access() {
        let config = PermissionConfig {
            roles: {
                let mut map = HashMap::new();
                map.insert(
                    "admin".to_string(),
                    RolePolicy {
                        tables: vec![TablePermission {
                            name: "*".to_string(),
                            operations: vec![PermissionAction::Select, PermissionAction::Insert],
                        }],
                    },
                );
                map
            },
        };

        assert!(config.check_access("admin", "users", PermissionAction::Select));
        assert!(!config.check_access("admin", "users", PermissionAction::Delete));
        assert!(!config.check_access("guest", "users", PermissionAction::Select));
    }

    /// TEST-U-015: PermissionConfig 验证测试 - 有效配置
    #[test]
    fn test_permission_config_validation_valid() {
        let config = PermissionConfig {
            roles: {
                let mut map = HashMap::new();
                map.insert(
                    "admin".to_string(),
                    RolePolicy {
                        tables: vec![TablePermission {
                            name: "users".to_string(),
                            operations: vec![PermissionAction::Select, PermissionAction::Insert],
                        }],
                    },
                );
                map
            },
        };

        assert!(config.validate().is_ok());
        assert!(config.validate_with_first_error().is_ok());
    }

    /// TEST-U-016: PermissionConfig 验证测试 - 空角色
    #[test]
    fn test_permission_config_validation_empty_roles() {
        let config = PermissionConfig { roles: HashMap::new() };

        let result = config.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("No roles defined")));
    }

    /// TEST-U-017: PermissionConfig 验证测试 - 空表权限
    #[test]
    fn test_permission_config_validation_empty_table_permissions() {
        let config = PermissionConfig {
            roles: {
                let mut map = HashMap::new();
                map.insert(
                    "admin".to_string(),
                    RolePolicy {
                        tables: vec![], // 空表权限
                    },
                );
                map
            },
        };

        let result = config.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("has no table permissions")));
    }

    /// TEST-U-018: PermissionConfig 验证测试 - 空操作列表
    #[test]
    fn test_permission_config_validation_empty_operations() {
        let config = PermissionConfig {
            roles: {
                let mut map = HashMap::new();
                map.insert(
                    "admin".to_string(),
                    RolePolicy {
                        tables: vec![TablePermission {
                            name: "users".to_string(),
                            operations: vec![], // 空操作列表
                        }],
                    },
                );
                map
            },
        };

        let result = config.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("has no operations defined")));
    }
}
