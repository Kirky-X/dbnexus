//! 权限控制模块
//!
//! 提供基于角色的表级权限控制功能

use lru::LruCache;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

/// 数据库操作类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Operation {
    /// SELECT 查询
    #[serde(rename = "SELECT")]
    Select,

    /// INSERT 插入
    #[serde(rename = "INSERT")]
    Insert,

    /// UPDATE 更新
    #[serde(rename = "UPDATE")]
    Update,

    /// DELETE 删除
    #[serde(rename = "DELETE")]
    Delete,
}

impl std::fmt::Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Operation::Select => write!(f, "SELECT"),
            Operation::Insert => write!(f, "INSERT"),
            Operation::Update => write!(f, "UPDATE"),
            Operation::Delete => write!(f, "DELETE"),
        }
    }
}

/// 表权限配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TablePermission {
    /// 表名（支持通配符 *）
    pub name: String,

    /// 允许的操作列表
    pub operations: Vec<Operation>,
}

/// 角色策略
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RolePolicy {
    /// 角色允许的表权限
    pub tables: Vec<TablePermission>,
}

impl RolePolicy {
    /// 检查角色是否有权限执行操作
    pub fn allows(&self, table: &str, operation: &Operation) -> bool {
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
    pub fn check_access(&self, role: &str, table: &str, operation: Operation) -> bool {
        if let Some(policy) = self.get_role_policy(role) {
            policy.allows(table, &operation)
        } else {
            false
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
    ///
    /// 如果验证失败，返回第一个错误
    pub fn validate_with_first_error(&self) -> Result<(), String> {
        self.validate().map_err(|errors| errors.join("; "))
    }
}

/// 权限上下文
#[derive(Debug, Clone)]
pub struct PermissionContext {
    /// 角色名称
    role: String,

    /// 权限策略 LRU 缓存（使用 Mutex 保护以支持线程安全）
    policy_cache: Arc<Mutex<LruCache<String, RolePolicy>>>,
}

/// LRU 缓存容量默认值
const DEFAULT_CACHE_CAPACITY: usize = 256;

impl Default for PermissionContext {
    fn default() -> Self {
        Self::with_cache_size("admin".to_string(), DEFAULT_CACHE_CAPACITY)
    }
}

impl PermissionContext {
    /// 创建新的权限上下文（使用默认缓存大小）
    pub fn new(role: String, policy_cache: Arc<Mutex<LruCache<String, RolePolicy>>>) -> Self {
        Self { role, policy_cache }
    }

    /// 创建新的权限上下文（使用自定义缓存大小）
    pub fn with_cache_size(role: String, cache_capacity: usize) -> Self {
        Self {
            role,
            policy_cache: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(cache_capacity).expect("Cache capacity must be non-zero"),
            ))),
        }
    }

    /// 获取角色
    pub fn role(&self) -> &str {
        &self.role
    }

    /// 检查表访问权限
    ///
    /// 此方法会先检查缓存，如果缓存未命中则加载权限策略到缓存
    pub fn check_table_access(&self, table: &str, operation: &Operation) -> bool {
        let mut cache = match self.policy_cache.lock() {
            Ok(guard) => guard,
            Err(_) => {
                tracing::error!("Permission cache mutex poisoned");
                return false; // 如果锁被破坏，拒绝访问
            }
        };

        // 尝试从缓存获取
        if let Some(policy) = cache.get(self.role.as_str()) {
            let allowed = policy.allows(table, operation);
            tracing::debug!(
                "Permission check: role='{}' table='{}' operation='{}' result={}",
                self.role,
                table,
                operation,
                allowed
            );
            return allowed;
        }

        // 缓存未命中，返回 false（实际使用时应该先调用 load_policy）
        // 注意：权限策略加载需要 I/O 操作，应在外部异步加载
        tracing::debug!(
            "Permission cache miss for role '{}', consider loading policy first",
            self.role
        );
        false
    }

    /// 加载权限策略到缓存
    ///
    /// 从权限配置文件中加载指定角色的策略并缓存
    ///
    /// # Errors
    ///
    /// 如果加载失败，返回错误信息
    pub fn load_policy(&self, config: &PermissionConfig) -> Result<(), String> {
        let mut cache = match self.policy_cache.lock() {
            Ok(guard) => guard,
            Err(_) => return Err("Permission cache mutex poisoned".to_string()),
        };

        if let Some(policy) = config.get_role_policy(&self.role) {
            cache.put(self.role.clone(), policy.clone());
            tracing::info!("Loaded permission policy for role '{}'", self.role);
            Ok(())
        } else {
            Err(format!("Role '{}' not found in permission config", self.role))
        }
    }

    /// 获取缓存统计信息
    pub fn cache_stats(&self) -> CacheStats {
        let cache = match self.policy_cache.lock() {
            Ok(guard) => guard,
            Err(_) => {
                return CacheStats {
                    cached_roles: 0,
                    capacity: 0,
                };
            }
        };
        CacheStats {
            cached_roles: cache.len(),
            capacity: cache.cap().get(),
        }
    }
}

/// 缓存统计信息
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// 已缓存的角色数
    pub cached_roles: usize,

    /// 缓存容量
    pub capacity: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TEST-U-010: Operation Display 实现测试
    #[test]
    fn test_operation_display() {
        assert_eq!(Operation::Select.to_string(), "SELECT");
        assert_eq!(Operation::Insert.to_string(), "INSERT");
        assert_eq!(Operation::Update.to_string(), "UPDATE");
        assert_eq!(Operation::Delete.to_string(), "DELETE");
    }

    /// TEST-U-011: RolePolicy allows 测试
    #[test]
    fn test_role_policy_allows() {
        let policy = RolePolicy {
            tables: vec![
                TablePermission {
                    name: "users".to_string(),
                    operations: vec![Operation::Select, Operation::Insert],
                },
                TablePermission {
                    name: "*".to_string(),
                    operations: vec![Operation::Select],
                },
            ],
        };

        // 精确表名匹配
        assert!(policy.allows("users", &Operation::Select));
        assert!(policy.allows("users", &Operation::Insert));
        assert!(!policy.allows("users", &Operation::Delete));

        // 通配符匹配
        assert!(policy.allows("orders", &Operation::Select));
        assert!(!policy.allows("orders", &Operation::Update));
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
          - SELECT
          - INSERT
          - UPDATE
          - DELETE
  user:
    tables:
      - name: users
        operations:
          - SELECT
"#;

        let config = PermissionConfig::from_yaml(yaml).unwrap();

        // 检查 admin 角色
        let admin_policy = config.get_role_policy("admin").unwrap();
        assert!(admin_policy.allows("users", &Operation::Select));
        assert!(admin_policy.allows("users", &Operation::Delete));

        // 检查 user 角色
        let user_policy = config.get_role_policy("user").unwrap();
        assert!(user_policy.allows("users", &Operation::Select));
        assert!(!user_policy.allows("users", &Operation::Insert));

        // 检查不存在的角色
        assert!(config.get_role_policy("guest").is_none());
    }

    /// TEST-U-013: PermissionContext 创建和访问测试
    #[test]
    fn test_permission_context_creation() {
        let cache = Arc::new(std::sync::Mutex::new(LruCache::new(NonZeroUsize::new(256).unwrap())));
        let ctx = PermissionContext::new("admin".to_string(), cache);

        assert_eq!(ctx.role(), "admin");
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
                            operations: vec![Operation::Select, Operation::Insert],
                        }],
                    },
                );
                map
            },
        };

        assert!(config.check_access("admin", "users", Operation::Select));
        assert!(!config.check_access("admin", "users", Operation::Delete));
        assert!(!config.check_access("guest", "users", Operation::Select));
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
                            operations: vec![Operation::Select, Operation::Insert],
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
