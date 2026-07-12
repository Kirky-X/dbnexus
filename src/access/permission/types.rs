// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
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
    /// 图遍历操作（图数据库专用，ladybug/neo4j feature 启用时可用）
    #[cfg(any(feature = "ladybug", feature = "neo4j"))]
    Traverse,
    /// 图匹配操作（图数据库专用，ladybug/neo4j feature 启用时可用）
    #[cfg(any(feature = "ladybug", feature = "neo4j"))]
    Match,
}

impl std::fmt::Display for PermissionAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PermissionAction::Select => write!(f, "SELECT"),
            PermissionAction::Insert => write!(f, "INSERT"),
            PermissionAction::Update => write!(f, "UPDATE"),
            PermissionAction::Delete => write!(f, "DELETE"),
            #[cfg(any(feature = "ladybug", feature = "neo4j"))]
            PermissionAction::Traverse => write!(f, "TRAVERSE"),
            #[cfg(any(feature = "ladybug", feature = "neo4j"))]
            PermissionAction::Match => write!(f, "MATCH"),
        }
    }
}

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
    ///
    /// 使用 `serde_yaml_ng` 直接反序列化，与项目配置管理策略一致
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dbnexus::permission::PermissionConfig;
    ///
    /// let yaml = r#"
    /// roles:
    ///   admin:
    ///     tables:
    ///       - name: "*"
    ///         operations: ["select", "insert", "update", "delete"]
    /// "#;
    /// let config = PermissionConfig::from_yaml_str(yaml)?;
    /// ```
    ///
    /// # Errors
    ///
    /// 如果 YAML 格式无效或字段类型不匹配，返回解析错误
    #[cfg(feature = "yaml")]
    pub fn from_yaml_str(yaml: &str) -> Result<Self, serde_yaml_ng::Error> {
        serde_yaml_ng::from_str(yaml)
    }

    /// 从 JSON 字符串加载配置
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dbnexus::permission::PermissionConfig;
    ///
    /// let json = r#"{"roles":{"admin":{"tables":[{"name":"*","operations":["select"]}]}}}"#;
    /// let config = PermissionConfig::from_json_str(json)?;
    /// ```
    ///
    /// # Errors
    ///
    /// 如果 JSON 格式无效或字段类型不匹配，返回解析错误
    pub fn from_json_str(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
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

// ============================================================================
// 图权限上下文（Phase 1 stub）
// ============================================================================

/// 图权限上下文（Phase 1 stub）
///
/// 提供图数据库操作的权限检查。当前为 Phase 1 stub 实现：
/// - admin 角色绕过所有检查
/// - 非 admin 角色被拒绝
///
/// 后续 Phase 2 将实现细粒度的图标签/关系类型权限控制。
#[cfg(any(feature = "ladybug", feature = "neo4j"))]
pub struct GraphPermissionContext {
    /// 当前角色名称
    pub role: String,
    /// 管理员角色名称（绕过所有权限检查）
    pub admin_role: String,
}

#[cfg(any(feature = "ladybug", feature = "neo4j"))]
impl GraphPermissionContext {
    /// 创建新的图权限上下文
    ///
    /// # 参数
    ///
    /// * `role` - 当前角色名称
    /// * `admin_role` - 管理员角色名称
    pub fn new(role: &str, admin_role: &str) -> Self {
        Self {
            role: role.to_string(),
            admin_role: admin_role.to_string(),
        }
    }

    /// 检查图操作权限（Phase 1 stub）
    ///
    /// Phase 1 实现：admin 角色绕过所有检查，非 admin 角色被拒绝。
    ///
    /// # 参数
    ///
    /// * `_action` - 权限操作类型（Phase 1 中未使用，Phase 2 将基于操作类型做细粒度检查）
    ///
    /// # 返回
    ///
    /// admin 角色返回 `Ok(())`，非 admin 角色返回 `Err(DbError::Permission)`。
    ///
    /// # Errors
    ///
    /// 非 admin 角色调用时返回 `DbError::Permission`。
    pub fn check_graph_access(&self, _action: PermissionAction) -> crate::foundation::DbResult<()> {
        if self.role == self.admin_role {
            Ok(())
        } else {
            Err(crate::foundation::DbError::Permission(format!(
                "Graph operation denied for role '{}': admin role '{}' required",
                self.role, self.admin_role
            )))
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

    /// TEST-U-010-graph: 图操作变体 Display 测试（ladybug/neo4j feature）
    #[cfg(any(feature = "ladybug", feature = "neo4j"))]
    #[test]
    fn test_operation_display_graph_variants() {
        assert_eq!(PermissionAction::Traverse.to_string(), "TRAVERSE");
        assert_eq!(PermissionAction::Match.to_string(), "MATCH");
    }

    /// TEST-U-010-graph-construct: 图操作变体可构造且可比较
    #[cfg(any(feature = "ladybug", feature = "neo4j"))]
    #[test]
    fn test_operation_graph_variants_construct() {
        let traverse = PermissionAction::Traverse;
        let match_op = PermissionAction::Match;
        assert_ne!(traverse, match_op);
        assert_ne!(traverse, PermissionAction::Select);
        assert_ne!(match_op, PermissionAction::Delete);
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

    /// TEST-U-012: PermissionConfig YAML 解析测试（通过 serde_yaml_ng）
    #[cfg(feature = "yaml")]
    #[test]
    fn test_permission_config_yaml_parsing() {
        // 测试 YAML 格式与 serde_yaml_ng + serde Deserialize 的兼容性
        let yaml = r#"
{
  "roles": {
    "admin": {
      "tables": [
        {
          "name": "users",
          "operations": ["select", "insert", "update", "delete"]
        }
      ]
    },
    "user": {
      "tables": [
        {
          "name": "users",
          "operations": ["select"]
        }
      ]
    }
  }
}
"#;

        // 使用 serde_yaml_ng 解析 JSON（YAML 是 JSON 的超集）
        let config: PermissionConfig = serde_yaml_ng::from_str(yaml).expect("Failed to parse YAML");

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

    /// TEST-U-012b: PermissionConfig YAML 解析测试（使用结构体构造）
    #[test]
    fn test_permission_config_yaml_parsing_struct() {
        let config = PermissionConfig {
            roles: {
                let mut map = HashMap::new();
                map.insert(
                    "admin".to_string(),
                    RolePolicy {
                        tables: vec![TablePermission {
                            name: "users".to_string(),
                            operations: vec![
                                PermissionAction::Select,
                                PermissionAction::Insert,
                                PermissionAction::Update,
                                PermissionAction::Delete,
                            ],
                        }],
                    },
                );
                map.insert(
                    "user".to_string(),
                    RolePolicy {
                        tables: vec![TablePermission {
                            name: "users".to_string(),
                            operations: vec![PermissionAction::Select],
                        }],
                    },
                );
                map
            },
        };

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

    // ========================================================================
    // GraphPermissionContext 测试（Phase 1 stub）
    // ========================================================================

    /// TEST-GRAPH-PERM-001: admin 角色调用 check_graph_access 应返回 Ok
    #[cfg(any(feature = "ladybug", feature = "neo4j"))]
    #[test]
    fn test_graph_permission_context_admin_bypass() {
        let ctx = GraphPermissionContext::new("admin", "admin");
        let result = ctx.check_graph_access(PermissionAction::Traverse);
        assert!(result.is_ok(), "admin role should bypass graph permission check");
    }

    /// TEST-GRAPH-PERM-002: 非 admin 角色调用 check_graph_access 应返回 Err
    #[cfg(any(feature = "ladybug", feature = "neo4j"))]
    #[test]
    fn test_graph_permission_context_non_admin_denied() {
        let ctx = GraphPermissionContext::new("user", "admin");
        let result = ctx.check_graph_access(PermissionAction::Match);
        assert!(result.is_err(), "non-admin role should be denied");
        let err = result.unwrap_err();
        assert!(
            matches!(err, crate::foundation::DbError::Permission(ref msg) if msg.contains("Graph operation denied")),
            "expected Permission error with 'Graph operation denied', got {:?}",
            err
        );
    }

    /// TEST-GRAPH-PERM-003: admin 角色对所有图操作都返回 Ok
    #[cfg(any(feature = "ladybug", feature = "neo4j"))]
    #[test]
    fn test_graph_permission_context_admin_all_actions() {
        let ctx = GraphPermissionContext::new("admin", "admin");
        assert!(ctx.check_graph_access(PermissionAction::Traverse).is_ok());
        assert!(ctx.check_graph_access(PermissionAction::Match).is_ok());
        assert!(ctx.check_graph_access(PermissionAction::Select).is_ok());
    }

    /// TEST-GRAPH-PERM-004: 非 admin 角色对所有图操作都返回 Err
    #[cfg(any(feature = "ladybug", feature = "neo4j"))]
    #[test]
    fn test_graph_permission_context_non_admin_all_actions_denied() {
        let ctx = GraphPermissionContext::new("guest", "admin");
        assert!(ctx.check_graph_access(PermissionAction::Traverse).is_err());
        assert!(ctx.check_graph_access(PermissionAction::Match).is_err());
        assert!(ctx.check_graph_access(PermissionAction::Select).is_err());
    }

    /// TEST-GRAPH-PERM-005: 错误消息包含角色名和管理员角色名
    #[cfg(any(feature = "ladybug", feature = "neo4j"))]
    #[test]
    fn test_graph_permission_context_error_message_contains_roles() {
        let ctx = GraphPermissionContext::new("editor", "superadmin");
        let result = ctx.check_graph_access(PermissionAction::Traverse);
        assert!(result.is_err());
        let err_msg = match result.unwrap_err() {
            crate::foundation::DbError::Permission(msg) => msg,
            other => panic!("expected Permission error, got {:?}", other),
        };
        assert!(
            err_msg.contains("editor"),
            "error should contain current role 'editor': {}",
            err_msg
        );
        assert!(
            err_msg.contains("superadmin"),
            "error should contain admin role 'superadmin': {}",
            err_msg
        );
    }
}
