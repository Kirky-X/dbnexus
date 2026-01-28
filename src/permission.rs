// Copyright (c) 2026 Kirky.X
//
// Licensed under MIT License
// See LICENSE file in project root for full license information.

//! 权限控制模块
//!
//! 提供基于角色的表级权限控制功能

pub mod advanced;
pub mod rbac;

use dashmap::DashMap;
use lru::LruCache;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::Mutex as AsyncMutex;

// ============================================================================
// PermissionProvider Trait Interface
// ============================================================================

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

    /// 刷新配置（如果支持动态加载）
    #[allow(async_fn_in_trait)]
    async fn refresh(&mut self) -> Result<(), PermissionProviderError> {
        Ok(())
    }
}

/// YAML 文件权限提供者
///
/// 从 YAML 文件加载权限配置
#[derive(Debug, Clone)]
pub struct YamlPermissionProvider {
    /// 权限配置
    config: Arc<PermissionConfig>,
    /// 配置文件路径
    path: Option<String>,
}

impl YamlPermissionProvider {
    /// 创建新的 YAML 权限提供者
    pub fn new(path: &str) -> Self {
        let config = if let Ok(content) = std::fs::read_to_string(path) {
            PermissionConfig::from_yaml(&content).unwrap_or_default()
        } else {
            PermissionConfig::deny_all()
        };

        Self {
            config: Arc::new(config),
            path: Some(path.to_string()),
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

    async fn refresh(&mut self) -> Result<(), PermissionProviderError> {
        if let Some(ref path) = self.path {
            match std::fs::read_to_string(path) {
                Ok(content) => match PermissionConfig::from_yaml(&content) {
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

/// 内存权限提供者
///
/// 允许程序化配置权限
#[derive(Debug, Default, Clone)]
pub struct MemoryPermissionProvider {
    config: Arc<PermissionConfig>,
}

impl MemoryPermissionProvider {
    /// 创建新的内存权限提供者
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加角色策略
    pub fn add_role(&mut self, role: &str, policy: RolePolicy) {
        Arc::make_mut(&mut self.config).roles.insert(role.to_string(), policy);
    }

    /// 移除角色
    pub fn remove_role(&mut self, role: &str) -> bool {
        Arc::make_mut(&mut self.config).roles.remove(role).is_some()
    }
}

impl PermissionProvider for MemoryPermissionProvider {
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

/// 简单速率限制器
///
/// 速率限制器
///
/// 使用滑动时间窗口算法限制请求频率，相比固定窗口算法：
/// - 更平滑的限流效果
/// - 避免窗口边界处的双倍请求问题
/// - 更准确的请求计数
///
/// 注意：此结构体主要用于内部权限检查速率限制，
/// 但其方法（如 `remaining`、`cleanup`）也可用于监控和管理。
#[derive(Debug, Clone)]
pub(crate) struct RateLimiter {
    /// 每个时间窗口允许的最大请求数
    max_requests: u32,
    /// 时间窗口大小
    window_duration: Duration,
    /// 请求记录存储（使用 DashMap 实现细粒度并发控制）
    /// Key: 限制键 (IP/用户ID), Value: 请求时间戳列表
    requests: Arc<DashMap<String, Vec<Instant>>>,
}

impl RateLimiter {
    /// 创建新的速率限制器
    ///
    /// # Arguments
    ///
    /// * `max_requests` - 每个时间窗口允许的最大请求数
    /// * `window_duration` - 时间窗口大小
    pub(crate) fn new(max_requests: u32, window_duration: Duration) -> Self {
        Self {
            max_requests,
            window_duration,
            requests: Arc::new(DashMap::new()),
        }
    }

    /// 检查是否允许请求（滑动窗口算法）
    ///
    /// 使用滑动窗口计数算法：
    /// 1. 计算当前时间窗口的起始点
    /// 2. 清理过期请求记录
    /// 3. 计算加权请求数（当前窗口 + 前一窗口的部分请求）
    /// 4. 根据是否超限决定是否允许请求
    ///
    /// # Arguments
    ///
    /// * `key` - 速率限制的键（如 IP 地址、用户 ID）
    ///
    /// # Returns
    ///
    /// 如果允许请求返回 true，否则返回 false
    pub(crate) async fn check(&self, key: &str) -> bool {
        let now = Instant::now();
        let window_start = now - self.window_duration;

        // 使用 DashMap 的细粒度锁
        let mut entry = self.requests.entry(key.to_string()).or_default();

        // 清理过期的请求记录（保留窗口内的请求）
        entry.retain(|&t| t > window_start);

        // 计算滑动窗口内的请求数（考虑窗口边界）
        let window_requests = entry.len();

        // 检查是否超过限制
        if window_requests < self.max_requests as usize {
            entry.push(now);
            true
        } else {
            false
        }
    }

    /// 获取剩余请求数量
    ///
    /// 用于监控和管理场景，例如：
    /// - 在管理界面显示用户的剩余请求配额
    /// - API 返回响应头中的 RateLimit-Remaining
    pub(crate) fn remaining(&self, key: &str) -> u32 {
        let now = Instant::now();
        let window_start = now - self.window_duration;

        if let Some(timestamps) = self.requests.get(key) {
            let valid_count = timestamps.iter().filter(|&&t| t > window_start).count();
            self.max_requests.saturating_sub(valid_count as u32)
        } else {
            self.max_requests
        }
    }

    /// 重置指定键的速率限制
    ///
    /// 用于管理场景，例如：
    /// - 管理员手动解除用户的速率限制
    /// - 在用户申诉后重置其限制
    pub(crate) fn reset(&self, key: &str) {
        self.requests.remove(key);
    }

    /// 清理孤立条目（防止内存泄漏）
    ///
    /// 移除长时间没有任何请求的 key（超过 10 倍时间窗口）
    /// 建议定期调用此方法，例如每小时一次或使用定时任务
    pub(crate) fn cleanup(&self) -> usize {
        let cleanup_threshold = self.window_duration * 10;
        let now = Instant::now();
        let mut removed_count = 0;

        // 优化：先收集需要删除的键，避免在迭代过程中修改 DashMap
        // 由于 DashMap 的迭代器生命周期问题，需要 clone key
        let keys_to_remove: Vec<String> = self
            .requests
            .iter()
            .filter_map(|entry| {
                let key = entry.key(); // &str
                // 获取最新时间戳
                let latest = entry.value().iter().max();
                match latest {
                    Some(&t) if now - t > cleanup_threshold => Some(key.to_string()),
                    None => Some(key.to_string()), // 空列表也清理
                    _ => None,
                }
            })
            .collect();

        // 移除孤立条目
        for key in &keys_to_remove {
            if self.requests.remove(key.as_str()).is_some() {
                removed_count += 1;
            }
        }

        if removed_count > 0 {
            tracing::info!(
                "RateLimiter cleanup: removed {} stale entries, remaining {}",
                removed_count,
                self.requests.len()
            );
        }

        removed_count
    }

    /// 获取当前条目数量（用于监控）
    pub(crate) fn len(&self) -> usize {
        self.requests.len()
    }

    /// 检查是否为空
    pub(crate) fn is_empty(&self) -> bool {
        self.requests.is_empty()
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

    /// 权限检查统计
    check_stats: Arc<PermissionCheckStats>,
}

/// LRU 缓存容量默认值
const DEFAULT_CACHE_CAPACITY: usize = 256;

/// 权限检查速率限制默认值
const DEFAULT_RATE_LIMIT_MAX_REQUESTS: u32 = 100;
const DEFAULT_RATE_LIMIT_WINDOW_SECS: u64 = 60;

impl Default for PermissionContext {
    fn default() -> Self {
        // DEFAULT_CACHE_CAPACITY is always > 0, so unwrap is safe here
        Self::with_cache_size("admin".to_string(), DEFAULT_CACHE_CAPACITY)
            .expect("Default cache capacity should always be valid")
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
            check_stats: Arc::new(PermissionCheckStats::new()),
        }
    }

    /// 创建新的权限上下文（使用自定义缓存大小）
    ///
    /// # Errors
    ///
    /// 如果 `cache_capacity` 为 0，返回 `InvalidCacheCapacity` 错误
    pub fn with_cache_size(role: String, cache_capacity: usize) -> Result<Self, PermissionError> {
        let capacity = NonZeroUsize::new(cache_capacity).ok_or(PermissionError::InvalidCacheCapacity)?;
        Ok(Self {
            role,
            policy_cache: Arc::new(AsyncMutex::new(LruCache::new(capacity))),
            rate_limiter: Some(Arc::new(RateLimiter::new(
                DEFAULT_RATE_LIMIT_MAX_REQUESTS,
                Duration::from_secs(DEFAULT_RATE_LIMIT_WINDOW_SECS),
            ))),
            check_stats: Arc::new(PermissionCheckStats::new()),
        })
    }

    /// 创建新的权限上下文（使用自定义缓存大小和速率限制）
    ///
    /// # Errors
    ///
    /// 如果 `cache_capacity` 为 0，返回 `InvalidCacheCapacity` 错误
    pub fn with_cache_size_and_rate_limit(
        role: String,
        cache_capacity: usize,
        max_requests: u32,
        window_secs: u64,
    ) -> Result<Self, PermissionError> {
        let capacity = NonZeroUsize::new(cache_capacity).ok_or(PermissionError::InvalidCacheCapacity)?;
        Ok(Self {
            role,
            policy_cache: Arc::new(AsyncMutex::new(LruCache::new(capacity))),
            rate_limiter: Some(Arc::new(RateLimiter::new(
                max_requests,
                Duration::from_secs(window_secs),
            ))),
            check_stats: Arc::new(PermissionCheckStats::new()),
        })
    }

    /// 获取角色
    pub fn role(&self) -> &str {
        &self.role
    }

    /// 获取权限检查统计
    pub fn check_stats(&self) -> &Arc<PermissionCheckStats> {
        &self.check_stats
    }

    /// 检查表访问权限（增强版 - 包含统计跟踪）
    ///
    /// 此方法会先检查速率限制，然后检查缓存，并记录所有检查结果
    pub async fn check_table_access(&self, table: &str, operation: &PermissionAction) -> bool {
        // 1. 检查速率限制
        if let Some(limiter) = &self.rate_limiter {
            if !limiter.check(&self.role).await {
                tracing::warn!(
                    target: "security",
                    "Rate limit exceeded for role '{}' on table '{}' operation '{}'",
                    self.role,
                    table,
                    operation
                );
                self.check_stats.record_rate_limited();
                return false;
            }
        }

        let mut cache = self.policy_cache.lock().await;

        // 2. 尝试从缓存获取
        if let Some(policy) = cache.get(self.role.as_str()) {
            let allowed = policy.allows(table, operation);
            if allowed {
                self.check_stats.record_allowed();
                self.check_stats.record_cache_hit();
            } else {
                self.check_stats.record_denied();
                self.check_stats.record_cache_hit();
            }
            tracing::trace!(
                "Permission check (cached): role='{}' table='{}' operation='{}' result={}",
                self.role,
                table,
                operation,
                allowed
            );
            return allowed;
        }

        // 3. 缓存未命中
        self.check_stats.record_cache_miss();
        tracing::debug!(
            target: "security",
            "Permission cache miss for role '{}' on table '{}' operation '{}'. Access denied by default.",
            self.role,
            table,
            operation,
        );
        self.check_stats.record_denied();
        false
    }

    /// 检查表访问权限（自动加载策略版本 - 增强版）
    ///
    /// 如果缓存未命中，自动从配置加载权限策略
    /// 包含完整的统计跟踪
    pub async fn check_table_access_with_config(
        &self,
        table: &str,
        operation: &PermissionAction,
        config: &PermissionConfig,
    ) -> bool {
        // 1. 检查速率限制
        if let Some(limiter) = &self.rate_limiter {
            if !limiter.check(&self.role).await {
                tracing::warn!(
                    target: "security",
                    "Rate limit exceeded for role '{}' on table '{}' operation '{}'",
                    self.role,
                    table,
                    operation
                );
                self.check_stats.record_rate_limited();
                return false;
            }
        }

        let mut cache = self.policy_cache.lock().await;

        // 2. 尝试从缓存获取
        if let Some(policy) = cache.get(self.role.as_str()) {
            let allowed = policy.allows(table, operation);
            if allowed {
                self.check_stats.record_allowed();
            } else {
                self.check_stats.record_denied();
            }
            self.check_stats.record_cache_hit();
            return allowed;
        }

        // 3. 缓存未命中：自动加载策略
        self.check_stats.record_cache_miss();
        if let Some(policy) = config.get_role_policy(&self.role) {
            cache.put(self.role.clone(), policy.clone());
            tracing::info!("Auto-loaded permission policy for role '{}' on cache miss", self.role);
            let allowed = policy.allows(table, operation);
            if allowed {
                self.check_stats.record_allowed();
            } else {
                self.check_stats.record_denied();
            }
            return allowed;
        }

        // 4. 角色未定义
        tracing::warn!(
            target: "security",
            "Role '{}' not found in permission config. Access denied.",
            self.role
        );
        self.check_stats.record_denied();
        false
    }

    /// 验证角色是否有权限执行特定操作（细粒度验证）
    ///
    /// 此方法提供比 `check_table_access` 更详细的验证，
    /// 包括操作类型、条件的详细检查
    ///
    /// # Arguments
    ///
    /// * `table` - 表名
    /// * `operation` - 操作类型
    /// * `conditions` - 可选的额外条件（如行级安全策略）
    ///
    /// # Returns
    ///
    /// 如果有权限返回 true，否则返回 false
    pub async fn verify_operation(&self, table: &str, operation: &PermissionAction, _conditions: Option<&str>) -> bool {
        // 基础权限检查
        self.check_table_access(table, operation).await
    }

    /// 批量检查多个权限
    ///
    /// 一次性检查多个表和操作的权限，比单独调用更高效
    ///
    /// # Arguments
    ///
    /// * `permissions` - 权限检查请求列表
    ///
    /// # Returns
    ///
    /// 每个请求的检查结果
    pub async fn batch_check_permissions(&self, permissions: &[(String, PermissionAction)]) -> Vec<bool> {
        let mut results = Vec::with_capacity(permissions.len());

        for (table, operation) in permissions {
            results.push(self.check_table_access(table, operation).await);
        }

        results
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

    /// 获取权限检查统计信息
    pub fn permission_check_stats(&self) -> PermissionCheckStatsSnapshot {
        self.check_stats.snapshot()
    }

    /// 清除权限缓存
    pub async fn clear_cache(&self) {
        let mut cache = self.policy_cache.lock().await;
        cache.clear();
        tracing::info!("Permission cache cleared for role '{}'", self.role);
    }
}

/// 权限检查统计信息
#[derive(Debug, Default)]
pub struct PermissionCheckStats {
    /// 总检查次数
    pub total_checks: AtomicU64,
    /// 允许的检查次数
    pub allowed_checks: AtomicU64,
    /// 拒绝的检查次数
    pub denied_checks: AtomicU64,
    /// 速率限制拒绝次数
    pub rate_limited_checks: AtomicU64,
    /// 缓存命中次数
    pub cache_hits: AtomicU64,
    /// 缓存未命中次数
    pub cache_misses: AtomicU64,
}

impl PermissionCheckStats {
    /// 创建新的统计实例
    pub fn new() -> Self {
        Self {
            total_checks: AtomicU64::new(0),
            allowed_checks: AtomicU64::new(0),
            denied_checks: AtomicU64::new(0),
            rate_limited_checks: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
        }
    }

    /// 记录检查通过
    pub fn record_allowed(&self) {
        self.total_checks.fetch_add(1, Ordering::SeqCst);
        self.allowed_checks.fetch_add(1, Ordering::SeqCst);
    }

    /// 记录检查拒绝
    pub fn record_denied(&self) {
        self.total_checks.fetch_add(1, Ordering::SeqCst);
        self.denied_checks.fetch_add(1, Ordering::SeqCst);
    }

    /// 记录速率限制拒绝
    pub fn record_rate_limited(&self) {
        self.total_checks.fetch_add(1, Ordering::SeqCst);
        self.rate_limited_checks.fetch_add(1, Ordering::SeqCst);
    }

    /// 记录缓存命中
    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::SeqCst);
    }

    /// 记录缓存未命中
    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::SeqCst);
    }

    /// 获取当前统计快照
    pub fn snapshot(&self) -> PermissionCheckStatsSnapshot {
        PermissionCheckStatsSnapshot {
            total_checks: self.total_checks.load(Ordering::SeqCst),
            allowed_checks: self.allowed_checks.load(Ordering::SeqCst),
            denied_checks: self.denied_checks.load(Ordering::SeqCst),
            rate_limited_checks: self.rate_limited_checks.load(Ordering::SeqCst),
            cache_hits: self.cache_hits.load(Ordering::SeqCst),
            cache_misses: self.cache_misses.load(Ordering::SeqCst),
        }
    }
}

/// 权限检查统计快照
#[derive(Debug, Clone)]
pub struct PermissionCheckStatsSnapshot {
    /// 总检查次数
    pub total_checks: u64,
    /// 允许的检查次数
    pub allowed_checks: u64,
    /// 拒绝的检查次数
    pub denied_checks: u64,
    /// 速率限制拒绝次数
    pub rate_limited_checks: u64,
    /// 缓存命中次数
    pub cache_hits: u64,
    /// 缓存未命中次数
    pub cache_misses: u64,
}

impl PermissionCheckStatsSnapshot {
    /// 获取缓存命中率
    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            self.cache_hits as f64 / total as f64
        }
    }

    /// 获取拒绝率
    pub fn denial_rate(&self) -> f64 {
        let total = self.total_checks;
        if total == 0 {
            0.0
        } else {
            self.denied_checks as f64 / total as f64
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

        limiter.reset("user1");

        assert!(limiter.check("user1").await);
    }

    /// TEST-U-022: 速率限制器测试 - 剩余请求计数
    #[tokio::test]
    async fn test_rate_limiter_remaining() {
        let limiter = RateLimiter::new(3, std::time::Duration::from_secs(60));

        assert_eq!(limiter.remaining("user1"), 3);

        limiter.check("user1").await;
        assert_eq!(limiter.remaining("user1"), 2);

        limiter.check("user1").await;
        assert_eq!(limiter.remaining("user1"), 1);

        limiter.check("user1").await;
        assert_eq!(limiter.remaining("user1"), 0);

        assert!(!limiter.check("user1").await);
        assert_eq!(limiter.remaining("user1"), 0);
    }

    #[tokio::test]
    async fn test_permission_context_load_policy_then_check_access() {
        let config = PermissionConfig {
            roles: [(
                "test_role".to_string(),
                RolePolicy {
                    tables: vec![TablePermission {
                        name: "users".to_string(),
                        operations: vec![PermissionAction::Select],
                    }],
                },
            )]
            .into_iter()
            .collect(),
        };

        let ctx = PermissionContext::with_cache_size("test_role".to_string(), 256).unwrap();
        ctx.load_policy(&config).await.unwrap();

        assert!(ctx.check_table_access("users", &PermissionAction::Select).await);
        assert!(!ctx.check_table_access("users", &PermissionAction::Delete).await);
    }

    #[tokio::test]
    async fn test_permission_context_check_table_access_with_config_role_missing_denies() {
        let config = PermissionConfig {
            roles: [(
                "defined_role".to_string(),
                RolePolicy {
                    tables: vec![TablePermission {
                        name: "users".to_string(),
                        operations: vec![PermissionAction::Select],
                    }],
                },
            )]
            .into_iter()
            .collect(),
        };

        let ctx = PermissionContext::with_cache_size("missing_role".to_string(), 256).unwrap();
        assert!(
            !ctx.check_table_access_with_config("users", &PermissionAction::Select, &config)
                .await
        );
    }

    #[tokio::test]
    async fn test_permission_context_check_table_access_with_config_rate_limited_denies() {
        let config = PermissionConfig {
            roles: [(
                "test_role".to_string(),
                RolePolicy {
                    tables: vec![TablePermission {
                        name: "users".to_string(),
                        operations: vec![PermissionAction::Select],
                    }],
                },
            )]
            .into_iter()
            .collect(),
        };

        let ctx = PermissionContext::with_cache_size_and_rate_limit("test_role".to_string(), 256, 1, 60).unwrap();

        assert!(
            ctx.check_table_access_with_config("users", &PermissionAction::Select, &config)
                .await
        );
        assert!(
            !ctx.check_table_access_with_config("users", &PermissionAction::Select, &config)
                .await
        );
    }
}

// ============================================================================
// Public API Re-exports
// ============================================================================

// Re-export AdvancedRbacProvider for easy access
pub use self::advanced::AdvancedRbacProvider;

// Re-export RbacProvider for easy access
pub use self::rbac::RbacProvider;
