// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 权限控制模块
//!
//! 提供基于角色的表级权限控制功能

use lru::LruCache;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::RwLock;

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

/// 简单速率限制器
///
/// 使用固定时间窗口算法限制请求频率
#[derive(Debug, Clone)]
pub struct RateLimiter {
    /// 每个时间窗口允许的最大请求数
    max_requests: u32,
    /// 时间窗口大小
    window_duration: Duration,
    /// 请求记录存储
    requests: Arc<RwLock<HashMap<String, Vec<Instant>>>>,
}

impl RateLimiter {
    /// 创建新的速率限制器
    ///
    /// # Arguments
    ///
    /// * `max_requests` - 每个时间窗口允许的最大请求数
    /// * `window_duration` - 时间窗口大小
    pub fn new(max_requests: u32, window_duration: Duration) -> Self {
        Self {
            max_requests,
            window_duration,
            requests: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 检查是否允许请求
    ///
    /// # Arguments
    ///
    /// * `key` - 速率限制的键（如 IP 地址、用户 ID）
    ///
    /// # Returns
    ///
    /// 如果允许请求返回 true，否则返回 false
    pub async fn check(&self, key: &str) -> bool {
        let now = Instant::now();
        let window_start = now - self.window_duration;

        let mut requests = self.requests.write().await;

        // 清理过期的请求记录
        if let Some(timestamps) = requests.get_mut(key) {
            timestamps.retain(|&t| t > window_start);
        }

        // 检查是否超过限制
        let count = requests.entry(key.to_string()).or_default().len();
        if count < self.max_requests as usize {
            if let Some(timestamps) = requests.get_mut(key) {
                timestamps.push(now);
            }
            true
        } else {
            false
        }
    }

    /// 获取剩余请求数量
    pub async fn remaining(&self, key: &str) -> u32 {
        let now = Instant::now();
        let window_start = now - self.window_duration;

        let requests = self.requests.read().await;

        if let Some(timestamps) = requests.get(key) {
            let valid_count = timestamps.iter().filter(|&&t| t > window_start).count();
            self.max_requests.saturating_sub(valid_count as u32)
        } else {
            self.max_requests
        }
    }

    /// 重置指定键的速率限制
    pub async fn reset(&self, key: &str) {
        let mut requests = self.requests.write().await;
        requests.remove(key);
    }
}

/// 默认速率限制配置
impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(100, Duration::from_secs(60))
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

/// 权限上下文
#[derive(Debug, Clone)]
pub struct PermissionContext {
    /// 角色名称
    role: String,

    /// 权限策略 LRU 缓存（使用 tokio 异步锁保护以支持异步上下文）
    policy_cache: Arc<AsyncMutex<LruCache<String, RolePolicy>>>,

    /// 权限检查速率限制器
    rate_limiter: Option<Arc<RateLimiter>>,
}

/// LRU 缓存容量默认值
const DEFAULT_CACHE_CAPACITY: usize = 256;

/// 权限检查速率限制默认值
const DEFAULT_RATE_LIMIT_MAX_REQUESTS: u32 = 100;
const DEFAULT_RATE_LIMIT_WINDOW_SECS: u64 = 60;

impl Default for PermissionContext {
    fn default() -> Self {
        Self::with_cache_size("admin".to_string(), DEFAULT_CACHE_CAPACITY)
    }
}

impl PermissionContext {
    /// 创建新的权限上下文（使用默认缓存大小和速率限制）
    pub fn new(role: String, policy_cache: Arc<AsyncMutex<LruCache<String, RolePolicy>>>) -> Self {
        Self {
            role,
            policy_cache,
            rate_limiter: Some(Arc::new(RateLimiter::new(
                DEFAULT_RATE_LIMIT_MAX_REQUESTS,
                Duration::from_secs(DEFAULT_RATE_LIMIT_WINDOW_SECS),
            ))),
        }
    }

    /// 创建新的权限上下文（使用自定义缓存大小）
    pub fn with_cache_size(role: String, cache_capacity: usize) -> Self {
        let capacity = if cache_capacity == 0 { 1 } else { cache_capacity };
        Self {
            role,
            policy_cache: Arc::new(AsyncMutex::new(LruCache::new(
                NonZeroUsize::new(capacity).expect("Cache capacity must be non-zero"),
            ))),
            rate_limiter: Some(Arc::new(RateLimiter::new(
                DEFAULT_RATE_LIMIT_MAX_REQUESTS,
                Duration::from_secs(DEFAULT_RATE_LIMIT_WINDOW_SECS),
            ))),
        }
    }

    /// 创建新的权限上下文（使用自定义缓存大小和速率限制）
    pub fn with_cache_size_and_rate_limit(
        role: String,
        cache_capacity: usize,
        max_requests: u32,
        window_secs: u64,
    ) -> Self {
        let capacity = if cache_capacity == 0 { 1 } else { cache_capacity };
        Self {
            role,
            policy_cache: Arc::new(AsyncMutex::new(LruCache::new(
                NonZeroUsize::new(capacity).expect("Cache capacity must be non-zero"),
            ))),
            rate_limiter: Some(Arc::new(RateLimiter::new(
                max_requests,
                Duration::from_secs(window_secs),
            ))),
        }
    }

    /// 获取角色
    pub fn role(&self) -> &str {
        &self.role
    }

    /// 检查表访问权限
    ///
    /// 此方法会先检查速率限制，然后检查缓存
    pub async fn check_table_access(&self, table: &str, operation: &PermissionAction) -> bool {
        // 检查速率限制
        if let Some(limiter) = &self.rate_limiter {
            if !limiter.check(&self.role).await {
                tracing::warn!(
                    target: "security",
                    "Rate limit exceeded for role '{}' on table '{}'",
                    self.role,
                    table
                );
                return false;
            }
        }

        let mut cache = self.policy_cache.lock().await;

        // 尝试从缓存获取
        if let Some(policy) = cache.get(self.role.as_str()) {
            let allowed = policy.allows(table, operation);
            tracing::trace!(
                "Permission check: role='{}' table='{}' operation='{}' result={}",
                self.role,
                table,
                operation,
                allowed
            );
            return allowed;
        }

        // 缓存未命中：返回 false（安全默认）
        // 使用 debug 级别避免日志膨胀
        tracing::debug!(
            target: "security",
            "Permission cache miss for role '{}' on table '{}'. Access denied by default.",
            self.role,
            table,
        );
        false
    }

    /// 检查表访问权限（自动加载策略版本）
    ///
    /// 如果缓存未命中，自动从配置加载权限策略
    /// 注意：此方法会在每次缓存未命中时尝试加载，可能影响性能
    pub async fn check_table_access_with_config(
        &self,
        table: &str,
        operation: &PermissionAction,
        config: &PermissionConfig,
    ) -> bool {
        // 检查速率限制
        if let Some(limiter) = &self.rate_limiter {
            if !limiter.check(&self.role).await {
                tracing::warn!(
                    target: "security",
                    "Rate limit exceeded for role '{}' on table '{}'",
                    self.role,
                    table
                );
                return false;
            }
        }

        let mut cache = self.policy_cache.lock().await;

        // 尝试从缓存获取
        if let Some(policy) = cache.get(self.role.as_str()) {
            return policy.allows(table, operation);
        }

        // 缓存未命中：自动加载策略
        if let Some(policy) = config.get_role_policy(&self.role) {
            cache.put(self.role.clone(), policy.clone());
            tracing::info!("Auto-loaded permission policy for role '{}' on cache miss", self.role);
            return policy.allows(table, operation);
        }

        // 角色未定义
        tracing::warn!(
            target: "security",
            "Role '{}' not found in permission config. Access denied.",
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
    pub async fn load_policy(&self, config: &PermissionConfig) -> Result<(), String> {
        let mut cache = self.policy_cache.lock().await;

        if let Some(policy) = config.get_role_policy(&self.role) {
            cache.put(self.role.clone(), policy.clone());
            tracing::info!("Loaded permission policy for role '{}'", self.role);
            Ok(())
        } else {
            Err(format!("Role '{}' not found in permission config", self.role))
        }
    }

    /// 获取缓存统计信息
    pub async fn cache_stats(&self) -> CacheStats {
        let cache = self.policy_cache.lock().await;
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

    /// TEST-U-013: PermissionContext 创建和访问测试
    #[test]
    fn test_permission_context_creation() {
        let cache = Arc::new(tokio::sync::Mutex::new(LruCache::new(NonZeroUsize::new(256).unwrap())));
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

    /// TEST-U-019: 速率限制器测试 - 基本功能
    #[tokio::test]
    async fn test_rate_limiter_basic() {
        let limiter = RateLimiter::new(3, std::time::Duration::from_secs(60));

        // 前3个请求应该允许
        assert!(limiter.check("user1").await);
        assert!(limiter.check("user1").await);
        assert!(limiter.check("user1").await);

        // 第4个请求应该被拒绝
        assert!(!limiter.check("user1").await);
    }

    /// TEST-U-020: 速率限制器测试 - 不同键独立计数
    #[tokio::test]
    async fn test_rate_limiter_different_keys() {
        let limiter = RateLimiter::new(2, std::time::Duration::from_secs(60));

        assert!(limiter.check("user1").await);
        assert!(limiter.check("user1").await);
        assert!(!limiter.check("user1").await);

        assert!(limiter.check("user2").await);
        assert!(limiter.check("user2").await);
        assert!(!limiter.check("user2").await);
    }

    /// TEST-U-021: 速率限制器测试 - 重置功能
    #[tokio::test]
    async fn test_rate_limiter_reset() {
        let limiter = RateLimiter::new(1, std::time::Duration::from_secs(60));

        assert!(limiter.check("user1").await);
        assert!(!limiter.check("user1").await);

        limiter.reset("user1").await;

        assert!(limiter.check("user1").await);
    }

    /// TEST-U-022: 速率限制器测试 - 剩余请求计数
    #[tokio::test]
    async fn test_rate_limiter_remaining() {
        let limiter = RateLimiter::new(3, std::time::Duration::from_secs(60));

        assert_eq!(limiter.remaining("user1").await, 3);

        limiter.check("user1").await;
        assert_eq!(limiter.remaining("user1").await, 2);

        limiter.check("user1").await;
        assert_eq!(limiter.remaining("user1").await, 1);

        limiter.check("user1").await;
        assert_eq!(limiter.remaining("user1").await, 0);

        assert!(!limiter.check("user1").await);
        assert_eq!(limiter.remaining("user1").await, 0);
    }
}
