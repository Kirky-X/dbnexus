// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 可插拔权限引擎模块
//!
//! 提供灵活的权限引擎架构，支持多种权限提供者实现：
//! - 基于 YAML 配置的权限提供者
//! - 基于 RBAC (Role-Based Access Control) 的权限提供者
//! - 自定义权限提供者
//!
//! # 核心组件
//!
//! - [`PermissionProvider`] - 权限提供者 trait，定义权限检查接口
//! - [`PolicyDecisionPoint`] - 策略决策点，统一处理权限决策
//! - [`YamlPermissionProvider`] - 基于 YAML 文件的权限提供者
//! - [`RbacPermissionProvider`] - 基于角色的权限提供者
//!
//! # 使用示例
//!
//! ```rust,no_run
//! use std::sync::Arc;
//!
//! use dbnexus::access::permission_engine::{PolicyDecisionPoint, YamlPermissionProvider};
//!
//! fn main() -> Result<(), String> {
//!     let provider = YamlPermissionProvider::new("permissions.yaml")?;
//!     let pdp = PolicyDecisionPoint::new(Arc::new(provider));
//!
//!     let rt = tokio::runtime::Runtime::new().unwrap();
//!     let _decision = rt.block_on(async { pdp.check("admin", "users", "SELECT").await });
//!
//!     Ok(())
//! }
//! ```

pub use super::permission::PermissionAction;
use async_trait::async_trait;
use dashmap::DashMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// 预编译的正则表达式，用于检测路径遍历攻击模式
/// 使用 LazyLock 确保线程安全的单次初始化
static PATH_TRAVERSAL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\.\.|%2e%2e|%252e%252e|\\/|\\\\").expect("Regex pattern should be valid")
});

/// 检查配置路径是否安全
///
/// 防止路径遍历攻击，确保路径不会访问预期目录之外的文件
fn is_safe_config_path(path: &str) -> bool {
    // 检查空路径
    if path.is_empty() {
        return false;
    }

    // 检查路径遍历攻击模式
    if PATH_TRAVERSAL_REGEX.is_match(path) {
        return false;
    }

    // 检查绝对路径是否在允许的目录内
    let path_buf = std::path::Path::new(path);
    if path_buf.is_absolute() {
        // 允许的配置目录前缀
        let allowed_prefixes = ["/etc/dbnexus/", "/opt/dbnexus/config/", "./config/", "./"];
        if allowed_prefixes
            .iter()
            .any(|prefix| path.starts_with(prefix))
        {
            return true;
        }
        // 也允许系统临时目录（用于测试场景）
        let temp_dir = std::env::temp_dir();
        return path_allowed_under(path, &temp_dir);
    }

    // 相对路径检查
    !path.contains("..") && !path.contains('\\')
}

/// 检查路径是否位于指定目录前缀之内
///
/// `temp_dir` 无法转为 UTF-8 或为空前缀时返回 false（fail-closed），
/// 避免 `starts_with("")` 恒真导致前缀检查失效。
fn path_allowed_under(path: &str, temp_dir: &std::path::Path) -> bool {
    match temp_dir.to_str() {
        Some(prefix) if !prefix.is_empty() => path.starts_with(prefix),
        _ => false,
    }
}

/// 权限资源
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PermissionResource {
    /// 资源名称（如表名）
    pub name: String,
    /// 资源类型
    #[serde(default)]
    pub resource_type: String,
}

impl PermissionResource {
    /// 创建新资源
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            resource_type: "table".to_string(),
        }
    }

    /// 创建带类型的资源
    pub fn with_type(name: &str, resource_type: &str) -> Self {
        Self {
            name: name.to_string(),
            resource_type: resource_type.to_string(),
        }
    }
}

/// 权限主体（用户或角色）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PermissionSubject {
    /// 主体 ID（用户 ID 或角色名称）
    pub id: String,
    /// 主体类型
    #[serde(default)]
    pub subject_type: SubjectType,
}

impl PermissionSubject {
    /// 创建用户主体
    pub fn user(id: &str) -> Self {
        Self {
            id: id.to_string(),
            subject_type: SubjectType::User,
        }
    }

    /// 创建角色主体
    pub fn role(id: &str) -> Self {
        Self {
            id: id.to_string(),
            subject_type: SubjectType::Role,
        }
    }
}

/// 主体类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectType {
    /// 用户类型
    #[default]
    User,
    /// 角色类型
    Role,
    /// 组类型
    Group,
}

/// 权限决策结果
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionDecision {
    /// 允许
    Allow,
    /// 拒绝
    Deny,
    /// 不适用（未找到相关策略）
    NotApplicable,
    /// 错误
    Error(String),
}

/// 权限上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionContext {
    /// 主体
    pub subject: PermissionSubject,
    /// 资源
    pub resource: PermissionResource,
    /// 操作
    pub action: PermissionAction,
    /// 额外属性
    #[serde(default)]
    pub attributes: HashMap<String, String>,
    /// 环境信息
    #[serde(default)]
    pub environment: HashMap<String, String>,
}

impl PermissionContext {
    /// 创建权限上下文
    pub fn new(
        subject: PermissionSubject,
        resource: PermissionResource,
        action: PermissionAction,
    ) -> Self {
        Self {
            subject,
            resource,
            action,
            attributes: HashMap::new(),
            environment: HashMap::new(),
        }
    }

    /// 添加属性
    pub fn with_attribute(mut self, key: &str, value: &str) -> Self {
        self.attributes.insert(key.to_string(), value.to_string());
        self
    }

    /// 添加环境信息
    pub fn with_environment(mut self, key: &str, value: &str) -> Self {
        self.environment.insert(key.to_string(), value.to_string());
        self
    }
}

/// 权限规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    /// 规则名称
    pub name: String,
    /// 优先级（数值越大优先级越高）
    #[serde(default)]
    pub priority: i32,
    /// 目标主体（支持通配符 *）
    pub subject: String,
    /// 目标资源（支持通配符 *）
    pub resource: String,
    /// 允许的操作
    pub allow: Vec<PermissionAction>,
    /// 拒绝的操作
    #[serde(default)]
    pub deny: Vec<PermissionAction>,
    /// 条件表达式
    #[serde(default)]
    pub condition: Option<String>,
    /// 规则是否启用
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

/// 检查规则是否匹配当前上下文
///
/// 通用匹配逻辑：检查主体、资源、操作以及条件是否在 allow/deny 列表中。
/// 若操作既不在 allow 也不在 deny，则规则不匹配（提前过滤，避免无谓排序）。
///
/// 条件评估：若规则定义了 `condition`，则必须匹配上下文的 attributes 或 environment。
/// 条件格式为 `key=value` 对（逗号分隔），所有条件必须满足（AND 语义）。
fn matches_rule(rule: &PermissionRule, context: &PermissionContext) -> bool {
    // 检查主体匹配
    if rule.subject != "*" && rule.subject != context.subject.id {
        return false;
    }

    // 检查资源匹配
    if rule.resource != "*" && rule.resource != context.resource.name {
        return false;
    }

    // 检查操作匹配（允许列表或拒绝列表）
    let in_allow = rule.allow.contains(&context.action);
    let in_deny = rule.deny.contains(&context.action);

    // 如果操作既不在 allow 也不在 deny 中，则不匹配
    if !in_allow && !in_deny {
        return false;
    }

    // 条件评估：若规则定义了 condition，必须匹配上下文属性
    if let Some(ref condition) = rule.condition
        && !evaluate_condition(condition, context)
    {
        return false;
    }

    true
}

/// 评估条件表达式是否匹配上下文
///
/// 条件格式：`key=value` 对（逗号分隔），所有条件必须满足（AND 语义）。
/// 查找顺序：先查 context.attributes，再查 context.environment。
/// 无法解析的条件默认不匹配（fail-safe）。
fn evaluate_condition(condition: &str, context: &PermissionContext) -> bool {
    let condition = condition.trim();
    if condition.is_empty() {
        return true; // 空条件视为匹配
    }

    for pair in condition.split(',') {
        let pair = pair.trim();
        if let Some((key, expected)) = pair.split_once('=') {
            let key = key.trim();
            let expected = expected.trim();
            // 先查 attributes，再查 environment
            let actual = context
                .attributes
                .get(key)
                .or_else(|| context.environment.get(key));
            match actual {
                Some(val) if val == expected => continue,
                _ => return false,
            }
        } else {
            // 无法解析的条件片段，fail-safe 拒绝
            return false;
        }
    }

    true
}

/// 获取主体的所有角色（含继承）
///
/// 仅从角色映射表中获取，不允许 subject ID 直接匹配预定义角色名（防止权限提升）。
fn get_subject_roles<V>(
    mapping: &HashMap<String, Vec<String>>,
    _roles: &HashMap<String, V>,
    subject: &str,
) -> Vec<String> {
    // 仅从显式映射中获取角色，禁止 subject ID 直接匹配角色名
    mapping.get(subject).cloned().unwrap_or_default()
}

/// 权限提供者 trait
/// 定义权限检查的标准接口
#[async_trait]
pub trait PermissionProvider: Send + Sync + Debug {
    /// 检查权限
    ///
    /// # 参数
    ///
    /// * `context` - 权限上下文
    ///
    /// # 返回
    ///
    /// 权限决策结果
    async fn check_permission(&self, context: &PermissionContext) -> PermissionDecision;

    /// 获取主体可访问的资源列表
    async fn get_allowed_resources(&self, subject: &str) -> Vec<PermissionResource>;

    /// 获取主体可执行的操作列表
    async fn get_allowed_actions(&self, subject: &str, resource: &str) -> Vec<PermissionAction>;

    /// 刷新权限缓存
    async fn refresh(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// 获取提供者名称
    fn name(&self) -> &str;
}

/// 缓存的权限决策（包含时间戳）
#[derive(Debug, Clone)]
struct CachedDecision {
    decision: PermissionDecision,
    cached_at: Instant,
}

impl CachedDecision {
    fn new(decision: PermissionDecision) -> Self {
        Self {
            decision,
            cached_at: Instant::now(),
        }
    }

    fn is_expired(&self, ttl_seconds: u64) -> bool {
        self.cached_at.elapsed().as_secs() >= ttl_seconds
    }
}

/// 速率限制器条目
#[derive(Debug, Clone)]
struct RateLimitEntry {
    count: u32,
    window_start: Instant,
}

/// 默认缓存 TTL（5 分钟）
const DEFAULT_CACHE_TTL_SECONDS: u64 = 300;
/// 默认速率限制最大请求数（每分钟 100 次）
const DEFAULT_RATE_LIMIT_MAX_REQUESTS: u32 = 100;
/// 默认速率限制窗口（1 分钟）
const DEFAULT_RATE_LIMIT_WINDOW_SECONDS: u32 = 60;

/// 策略决策点
/// 统一处理权限决策，支持多种权限提供者
#[derive(Debug)]
pub struct PolicyDecisionPoint {
    /// 权限提供者
    provider: Arc<dyn PermissionProvider>,
    /// 缓存（使用 DashMap 实现细粒度锁）
    cache: DashMap<String, CachedDecision>,
    /// 缓存配置
    cache_ttl_seconds: u64,
    /// 是否启用缓存
    cache_enabled: bool,
    /// 速率限制：最大请求数（每分钟）
    rate_limit_max_requests: u32,
    /// 速率限制：时间窗口（秒）
    rate_limit_window_seconds: u32,
    /// 速率限制器存储
    rate_limit_store: DashMap<String, RateLimitEntry>,
    /// 默认决策（当提供者返回 NotApplicable 时使用）
    default_decision: PermissionDecision,
}

/// PolicyDecisionPoint 构建器
///
/// 支持部分依赖注入和自定义配置
///
/// # Example
///
/// ```rust,no_run
/// use std::sync::Arc;
/// use dbnexus::{PolicyDecisionPoint, RbacPermissionProvider};
///
/// let provider = Arc::new(RbacPermissionProvider::new());
/// let pdp = PolicyDecisionPoint::builder()
///     .provider(provider)
///     .cache_ttl_seconds(600)
///     .rate_limit(200, 60)
///     .build();
/// ```
pub struct PolicyDecisionPointBuilder {
    provider: Option<Arc<dyn PermissionProvider>>,
    cache_ttl_seconds: Option<u64>,
    cache_enabled: Option<bool>,
    rate_limit_max_requests: Option<u32>,
    rate_limit_window_seconds: Option<u32>,
    default_decision: Option<PermissionDecision>,
}

impl PolicyDecisionPointBuilder {
    /// 创建新的构建器
    fn new() -> Self {
        Self {
            provider: None,
            cache_ttl_seconds: None,
            cache_enabled: None,
            rate_limit_max_requests: None,
            rate_limit_window_seconds: None,
            default_decision: None,
        }
    }

    /// 设置权限提供者
    ///
    /// # Arguments
    ///
    /// * `provider` - 权限提供者实例
    pub fn provider(mut self, provider: Arc<dyn PermissionProvider>) -> Self {
        self.provider = Some(provider);
        self
    }

    /// 设置缓存 TTL（秒）
    ///
    /// # Arguments
    ///
    /// * `seconds` - 缓存过期时间（秒）
    pub fn cache_ttl_seconds(mut self, seconds: u64) -> Self {
        self.cache_ttl_seconds = Some(seconds);
        self
    }

    /// 设置是否启用缓存
    ///
    /// # Arguments
    ///
    /// * `enabled` - 是否启用缓存
    pub fn cache_enabled(mut self, enabled: bool) -> Self {
        self.cache_enabled = Some(enabled);
        self
    }

    /// 设置速率限制
    ///
    /// # Arguments
    ///
    /// * `max_requests` - 时间窗口内最大请求数
    /// * `window_seconds` - 时间窗口（秒）
    pub fn rate_limit(mut self, max_requests: u32, window_seconds: u32) -> Self {
        self.rate_limit_max_requests = Some(max_requests);
        self.rate_limit_window_seconds = Some(window_seconds);
        self
    }

    /// 设置默认决策（当提供者返回 NotApplicable 时使用）
    ///
    /// # Arguments
    ///
    /// * `decision` - 默认决策
    pub fn default_decision(mut self, decision: PermissionDecision) -> Self {
        self.default_decision = Some(decision);
        self
    }

    /// 构建策略决策点
    ///
    /// # Panics
    ///
    /// 如果未设置权限提供者，将 panic
    pub fn build(self) -> PolicyDecisionPoint {
        let provider = self
            .provider
            .expect("Provider is required for PolicyDecisionPoint");

        PolicyDecisionPoint {
            provider,
            cache: DashMap::new(),
            cache_ttl_seconds: self.cache_ttl_seconds.unwrap_or(DEFAULT_CACHE_TTL_SECONDS),
            cache_enabled: self.cache_enabled.unwrap_or(true),
            rate_limit_max_requests: self
                .rate_limit_max_requests
                .unwrap_or(DEFAULT_RATE_LIMIT_MAX_REQUESTS),
            rate_limit_window_seconds: self
                .rate_limit_window_seconds
                .unwrap_or(DEFAULT_RATE_LIMIT_WINDOW_SECONDS),
            rate_limit_store: DashMap::new(),
            default_decision: self
                .default_decision
                .unwrap_or(PermissionDecision::NotApplicable),
        }
    }
}

impl PolicyDecisionPoint {
    /// 创建策略决策点（默认 TTL 5 分钟，速率限制 100 请求/分钟）
    pub fn new(provider: Arc<dyn PermissionProvider>) -> Self {
        Self {
            provider,
            cache: DashMap::new(),
            cache_ttl_seconds: DEFAULT_CACHE_TTL_SECONDS,
            cache_enabled: true,
            rate_limit_max_requests: DEFAULT_RATE_LIMIT_MAX_REQUESTS,
            rate_limit_window_seconds: DEFAULT_RATE_LIMIT_WINDOW_SECONDS,
            rate_limit_store: DashMap::new(),
            default_decision: PermissionDecision::NotApplicable,
        }
    }

    /// 创建构建器
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use std::sync::Arc;
    /// use dbnexus::{PolicyDecisionPoint, RbacPermissionProvider};
    ///
    /// let provider = Arc::new(RbacPermissionProvider::new());
    /// let pdp = PolicyDecisionPoint::builder()
    ///     .provider(provider)
    ///     .cache_ttl_seconds(600)
    ///     .build();
    /// ```
    pub fn builder() -> PolicyDecisionPointBuilder {
        PolicyDecisionPointBuilder::new()
    }

    /// 完全依赖注入：由调用方提供权限提供者
    ///
    /// # Arguments
    ///
    /// * `provider` - 权限提供者实例
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use std::sync::Arc;
    /// use dbnexus::{PolicyDecisionPoint, RbacPermissionProvider};
    ///
    /// let provider = Arc::new(RbacPermissionProvider::new());
    /// let pdp = PolicyDecisionPoint::with_dependencies(provider);
    /// ```
    pub fn with_dependencies(provider: Arc<dyn PermissionProvider>) -> Self {
        Self::new(provider)
    }

    /// 创建带缓存配置的策略决策点
    pub fn with_cache(provider: Arc<dyn PermissionProvider>, cache_ttl_seconds: u64) -> Self {
        Self {
            provider,
            cache: DashMap::new(),
            cache_ttl_seconds,
            cache_enabled: true,
            rate_limit_max_requests: DEFAULT_RATE_LIMIT_MAX_REQUESTS,
            rate_limit_window_seconds: DEFAULT_RATE_LIMIT_WINDOW_SECONDS,
            rate_limit_store: DashMap::new(),
            default_decision: PermissionDecision::NotApplicable,
        }
    }

    /// 创建带速率限制配置的策略决策点
    pub fn with_rate_limit(
        provider: Arc<dyn PermissionProvider>,
        max_requests: u32,
        window_seconds: u32,
    ) -> Self {
        Self {
            provider,
            cache: DashMap::new(),
            cache_ttl_seconds: DEFAULT_CACHE_TTL_SECONDS,
            cache_enabled: true,
            rate_limit_max_requests: max_requests,
            rate_limit_window_seconds: window_seconds,
            rate_limit_store: DashMap::new(),
            default_decision: PermissionDecision::NotApplicable,
        }
    }

    /// 创建带完整配置的策略决策点
    ///
    /// 正确应用 `PolicyDecisionPointConfig` 中的所有字段，包括：
    /// - `default_decision`：当提供者返回 `NotApplicable` 时使用的默认决策
    /// - `cache_ttl_seconds`：缓存 TTL
    /// - `cache_enabled`：是否启用缓存
    pub fn with_config(
        provider: Arc<dyn PermissionProvider>,
        config: PolicyDecisionPointConfig,
    ) -> Self {
        Self {
            provider,
            cache: DashMap::new(),
            cache_ttl_seconds: config.cache_ttl_seconds,
            cache_enabled: config.cache_enabled,
            rate_limit_max_requests: DEFAULT_RATE_LIMIT_MAX_REQUESTS,
            rate_limit_window_seconds: DEFAULT_RATE_LIMIT_WINDOW_SECONDS,
            rate_limit_store: DashMap::new(),
            default_decision: config.default_decision,
        }
    }

    /// 检查速率限制
    ///
    /// 注意：DashMap `entry()` API 在条目生命周期内持有分片写锁，
    /// 因此 read-check-increment 操作在同一分片内是原子的。
    /// 不同 subject 之间互不影响（各自独立分片）。
    fn check_rate_limit(&self, subject_id: &str) -> bool {
        let key = subject_id.to_string();
        let now = Instant::now();
        let window_duration = Duration::from_secs(self.rate_limit_window_seconds as u64);

        // 获取或创建速率限制条目
        let mut entry = self
            .rate_limit_store
            .entry(key.clone())
            .or_insert(RateLimitEntry {
                count: 0,
                window_start: now,
            });

        // 检查窗口是否过期
        if now.duration_since(entry.window_start) >= window_duration {
            entry.count = 0;
            entry.window_start = now;
        }

        // 检查是否超过限制
        if entry.count >= self.rate_limit_max_requests {
            false
        } else {
            entry.count += 1;
            true
        }
    }

    /// 检查权限（带 TTL 缓存和速率限制）
    pub async fn check_permission(&self, context: &PermissionContext) -> PermissionDecision {
        // 检查速率限制
        if !self.check_rate_limit(&context.subject.id) {
            return PermissionDecision::Deny;
        }

        // 生成缓存键
        let cache_key = self.generate_cache_key(context);

        // 检查缓存（带 TTL 验证）
        if self.cache_enabled
            && let Some(decision) = self.get_cached_decision(&cache_key)
        {
            return decision;
        }

        // 获取权限决策
        let decision = self.provider.check_permission(context).await;

        // 应用默认决策：当提供者返回 NotApplicable 时，使用配置的默认决策
        let decision = match decision {
            PermissionDecision::NotApplicable => self.default_decision.clone(),
            other => other,
        };

        // 更新缓存（带时间戳）
        if self.cache_enabled {
            self.update_cache(&cache_key, decision.clone());
        }

        decision
    }

    /// 检查用户是否有权限执行操作
    pub async fn check(&self, subject: &str, resource: &str, action: &str) -> PermissionDecision {
        let action = match action.to_uppercase().as_str() {
            "SELECT" => PermissionAction::Select,
            "INSERT" => PermissionAction::Insert,
            "UPDATE" => PermissionAction::Update,
            "DELETE" => PermissionAction::Delete,
            // 未知操作默认拒绝（fail-closed）
            _ => return PermissionDecision::Deny,
        };

        let context = PermissionContext::new(
            PermissionSubject::user(subject),
            PermissionResource::new(resource),
            action,
        );

        self.check_permission(&context).await
    }

    /// 批量检查权限
    pub async fn check_batch(
        &self,
        contexts: Vec<PermissionContext>,
    ) -> Vec<(PermissionContext, PermissionDecision)> {
        let mut results = Vec::with_capacity(contexts.len());

        for context in contexts {
            let decision = self.check_permission(&context).await;
            results.push((context, decision));
        }

        results
    }

    /// 获取主体可访问的资源
    pub async fn get_allowed_resources(&self, subject: &str) -> Vec<PermissionResource> {
        self.provider.get_allowed_resources(subject).await
    }

    /// 刷新缓存
    pub async fn refresh_cache(&self) {
        self.provider.refresh().await.ok();
        // DashMap 清空
        self.cache.clear();
    }

    /// 启用/禁用缓存
    pub fn set_cache_enabled(&mut self, enabled: bool) {
        self.cache_enabled = enabled;
        if !enabled {
            // DashMap 清空
            self.cache.clear();
        }
    }

    /// 生成缓存键
    fn generate_cache_key(&self, context: &PermissionContext) -> String {
        format!(
            "{}:{}:{}:{}",
            context.subject.id,
            context.resource.name,
            context.action,
            context
                .attributes
                .iter()
                .fold(String::new(), |acc, (k, v)| format!("{}:{}={}", acc, k, v))
        )
    }

    /// 获取缓存的决策（带 TTL 检查）
    fn get_cached_decision(&self, key: &str) -> Option<PermissionDecision> {
        // DashMap 直接读取，无需锁
        if let Some(cached) = self.cache.get(key) {
            // 检查是否过期
            if !cached.is_expired(self.cache_ttl_seconds) {
                return Some(cached.decision.clone());
            }
        }
        None
    }

    /// 更新缓存（带时间戳）
    fn update_cache(&self, key: &str, decision: PermissionDecision) {
        // DashMap 直接写入，无需锁
        self.cache
            .insert(key.to_string(), CachedDecision::new(decision));
    }
}

/// 基于 YAML 配置的权限提供者
#[derive(Debug)]
pub struct YamlPermissionProvider {
    /// 配置文件路径
    config_path: String,
    /// 角色权限映射
    roles: RwLock<HashMap<String, Vec<PermissionRule>>>,
    /// 缓存时间
    last_refresh: RwLock<Instant>,
    /// 提供者名称
    name: String,
    /// 角色映射表（禁止用户名直接作为角色）
    role_mapping: RwLock<HashMap<String, Vec<String>>>,
}

/// 早于当前时间的 last_refresh 初值：惰性加载以 `age > 刷新阈值` 判定，
/// 初值若为 `Instant::now()` 则新建实例的首次 check（age≈0）永不触发加载，
/// 在阈值窗口内静默返回空规则（deny-everything）。Unix 纪元不可用（Instant
/// 为单调钟），故从当前时刻回退一个远超阈值（1h）的偏移；极短 uptime 平台
/// 回退不足时最多延迟一个阈值周期后自然加载。
fn stale_last_refresh() -> Instant {
    let now = Instant::now();
    now.checked_sub(Duration::from_secs(3600)).unwrap_or(now)
}

impl Default for YamlPermissionProvider {
    fn default() -> Self {
        Self {
            config_path: String::new(),
            roles: RwLock::new(HashMap::new()),
            last_refresh: RwLock::new(stale_last_refresh()),
            name: "yaml".to_string(),
            role_mapping: RwLock::new(HashMap::new()),
        }
    }
}

impl YamlPermissionProvider {
    /// 创建 YAML 权限提供者
    ///
    /// # Arguments
    ///
    /// * `config_path` - 权限配置文件路径
    ///
    /// # Errors
    ///
    /// 如果路径无效或不在允许的目录内，返回错误
    pub fn new(config_path: &str) -> Result<Self, String> {
        // 验证配置文件路径安全性

        // 1. 检查空路径
        if config_path.is_empty() {
            return Err("Config path cannot be empty".to_string());
        }

        // 2. 检查路径是否包含父目录引用（防止路径遍历攻击）
        // 使用预编译的正则表达式进行检测
        if PATH_TRAVERSAL_REGEX.is_match(config_path) {
            return Err("Config path contains invalid parent directory reference".to_string());
        }

        if !is_safe_config_path(config_path) {
            return Err("Config path failed safety validation".to_string());
        }

        Ok(Self {
            config_path: config_path.to_string(),
            roles: RwLock::new(HashMap::new()),
            last_refresh: RwLock::new(stale_last_refresh()),
            name: "yaml".to_string(),
            role_mapping: RwLock::new(HashMap::new()),
        })
    }

    /// 加载配置
    async fn load_config(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use serde::Deserialize;

        let content = tokio::fs::read_to_string(&self.config_path).await?;

        // 解析配置
        #[derive(Debug, Deserialize)]
        struct YamlConfig {
            roles: HashMap<String, Vec<PermissionRule>>,
        }

        // 使用 serde_yaml_ng 解析（YAML 是 JSON 超集，兼容两种输入）
        #[cfg(feature = "yaml")]
        {
            let config: YamlConfig = serde_yaml_ng::from_str(&content)?;

            // 更新角色权限
            if let Ok(mut roles) = self.roles.write() {
                *roles = config.roles;
            }
        }
        #[cfg(not(feature = "yaml"))]
        {
            return Err("Cannot parse permission config: 'yaml' feature is not enabled".into());
        }

        // 构建角色映射：每个角色名映射到自身（subject 通过角色名匹配）
        if let Ok(mut role_mapping) = self.role_mapping.write() {
            role_mapping.clear();
            if let Ok(roles) = self.roles.read() {
                for role_name in roles.keys() {
                    role_mapping.insert(role_name.clone(), vec![role_name.clone()]);
                }
            }
        }

        if let Ok(mut last_refresh) = self.last_refresh.write() {
            *last_refresh = Instant::now();
        }

        Ok(())
    }
}

#[async_trait]
impl PermissionProvider for YamlPermissionProvider {
    async fn check_permission(&self, context: &PermissionContext) -> PermissionDecision {
        // 加载配置（如果需要）
        let age = self
            .last_refresh
            .read()
            .map(|r| r.elapsed())
            .unwrap_or_default();
        if age.as_secs() > 60
            && let Err(e) = self.load_config().await
        {
            return PermissionDecision::Error(format!("Failed to load config: {}", e));
        }

        let roles = match self.roles.read() {
            Ok(r) => r,
            Err(_) => return PermissionDecision::Error("Lock error".to_string()),
        };
        let subject_roles = self.get_subject_roles(&context.subject.id);

        // 优化：收集所有匹配的规则
        let mut matched_rules: Vec<(i32, &PermissionRule)> = Vec::new();

        for role_name in &subject_roles {
            if let Some(rules) = roles.get(role_name) {
                for rule in rules {
                    if rule.enabled && matches_rule(rule, context) {
                        matched_rules.push((rule.priority, rule));
                    }
                }
            }
        }

        // 按优先级从高到低排序
        matched_rules.sort_by_key(|b| std::cmp::Reverse(b.0));

        // 评估规则：按优先级从高到低，一旦找到决策立即返回
        for (_, rule) in matched_rules {
            // 检查 Allow 规则（优先级最高）
            if rule.allow.contains(&context.action) {
                return PermissionDecision::Allow;
            }
            // 检查 Deny 规则
            if rule.deny.contains(&context.action) {
                return PermissionDecision::Deny;
            }
        }

        PermissionDecision::NotApplicable
    }

    async fn get_allowed_resources(&self, subject: &str) -> Vec<PermissionResource> {
        let roles = match self.roles.read() {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        let subject_roles = self.get_subject_roles(subject);
        let mut resources = std::collections::HashSet::new();

        for role_name in &subject_roles {
            if let Some(rules) = roles.get(role_name) {
                for rule in rules {
                    if rule.enabled && (rule.subject == "*" || rule.subject == subject) {
                        resources.insert(PermissionResource::new(&rule.resource));
                    }
                }
            }
        }

        resources.into_iter().collect()
    }

    async fn get_allowed_actions(&self, subject: &str, resource: &str) -> Vec<PermissionAction> {
        let roles = match self.roles.read() {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        let subject_roles = self.get_subject_roles(subject);
        let mut actions = std::collections::HashSet::new();

        for role_name in &subject_roles {
            if let Some(rules) = roles.get(role_name) {
                for rule in rules {
                    if rule.enabled
                        && (rule.subject == "*" || rule.subject == subject)
                        && (rule.resource == "*" || rule.resource == resource)
                    {
                        for action in &rule.allow {
                            actions.insert(action.clone());
                        }
                    }
                }
            }
        }

        actions.into_iter().collect()
    }

    async fn refresh(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.load_config().await
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl YamlPermissionProvider {
    fn get_subject_roles(&self, subject: &str) -> Vec<String> {
        let mapping = match self.role_mapping.read() {
            Ok(m) => m,
            Err(_) => return Vec::new(),
        };
        let roles = match self.roles.read() {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        get_subject_roles(&mapping, &roles, subject)
    }
}

/// 基于 RBAC 的权限提供者
#[derive(Debug)]
pub struct RbacPermissionProvider {
    /// 角色层次结构
    roles: RwLock<HashMap<String, Role>>,
    /// 权限规则
    permissions: RwLock<HashMap<String, Vec<PermissionRule>>>,
    /// 角色继承
    role_hierarchy: RwLock<HashMap<String, Vec<String>>>,
    /// 缓存时间
    last_refresh: RwLock<Instant>,
    /// 提供者名称
    name: String,
    /// 角色映射表（禁止用户名直接作为角色）
    role_mapping: RwLock<HashMap<String, Vec<String>>>,
}

impl Default for RbacPermissionProvider {
    fn default() -> Self {
        Self {
            roles: RwLock::new(HashMap::new()),
            permissions: RwLock::new(HashMap::new()),
            role_hierarchy: RwLock::new(HashMap::new()),
            last_refresh: RwLock::new(Instant::now()),
            name: "rbac".to_string(),
            role_mapping: RwLock::new(HashMap::new()),
        }
    }
}

/// RBAC 角色
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    /// 角色名称
    pub name: String,
    /// 角色描述
    #[serde(default)]
    pub description: String,
    /// 角色是否启用
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 继承的角色
    #[serde(default)]
    pub extends: Vec<String>,
}

impl Default for Role {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            enabled: true,
            extends: Vec::new(),
        }
    }
}

impl RbacPermissionProvider {
    /// 创建 RBAC 权限提供者
    pub fn new() -> Self {
        Self {
            roles: RwLock::new(HashMap::new()),
            permissions: RwLock::new(HashMap::new()),
            role_hierarchy: RwLock::new(HashMap::new()),
            last_refresh: RwLock::new(Instant::now()),
            name: "rbac".to_string(),
            role_mapping: RwLock::new(HashMap::new()),
        }
    }

    /// 添加角色
    pub fn add_role(&self, role: Role) {
        if let Ok(mut roles) = self.roles.write() {
            roles.insert(role.name.clone(), role.clone());
        }
        if let Ok(mut hierarchy) = self.role_hierarchy.write() {
            hierarchy.insert(role.name, role.extends);
        }
    }

    /// 添加权限规则
    pub fn add_permission(&self, role: &str, rule: PermissionRule) {
        if let Ok(mut permissions) = self.permissions.write() {
            permissions.entry(role.to_string()).or_default().push(rule);
        }
    }

    /// 将角色分配给主体（用户）
    pub fn add_role_to_subject(&self, subject: &str, role: &str) {
        if let Ok(mut mapping) = self.role_mapping.write() {
            mapping
                .entry(subject.to_string())
                .or_default()
                .push(role.to_string());
        }
    }

    /// 获取角色的所有权限（包括继承的）
    async fn get_role_permissions(&self, role: &str) -> Vec<PermissionRule> {
        let mut all_permissions = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut to_visit = vec![role.to_string()];

        let permissions = if let Ok(p) = self.permissions.read() {
            p
        } else {
            return Vec::new();
        };
        let hierarchy = if let Ok(h) = self.role_hierarchy.read() {
            h
        } else {
            return Vec::new();
        };

        while let Some(current_role) = to_visit.pop() {
            if visited.contains(&current_role) {
                continue;
            }
            visited.insert(current_role.clone());

            // 添加当前角色的权限
            if let Some(rules) = permissions.get(&current_role) {
                all_permissions.extend(rules.iter().cloned());
            }

            // 添加继承角色的权限
            if let Some(extends) = hierarchy.get(&current_role) {
                for parent_role in extends {
                    if !visited.contains(parent_role) {
                        to_visit.push(parent_role.clone());
                    }
                }
            }
        }

        all_permissions
    }
}

#[async_trait]
impl PermissionProvider for RbacPermissionProvider {
    async fn check_permission(&self, context: &PermissionContext) -> PermissionDecision {
        let subject_roles = self.get_subject_roles(&context.subject.id);

        // 获取所有角色的权限
        let mut all_rules = Vec::new();
        for role in &subject_roles {
            let rules = self.get_role_permissions(role).await;
            all_rules.extend(rules);
        }

        // 按优先级排序
        all_rules.sort_by_key(|b| std::cmp::Reverse(b.priority));

        // 评估规则
        for rule in all_rules {
            if rule.enabled && matches_rule(&rule, context) {
                if rule.allow.contains(&context.action) {
                    return PermissionDecision::Allow;
                }
                if rule.deny.contains(&context.action) {
                    return PermissionDecision::Deny;
                }
            }
        }

        PermissionDecision::NotApplicable
    }

    async fn get_allowed_resources(&self, subject: &str) -> Vec<PermissionResource> {
        let subject_roles = self.get_subject_roles(subject);
        let mut resources = std::collections::HashSet::new();

        for role in &subject_roles {
            let rules = self.get_role_permissions(role).await;
            for rule in rules {
                if rule.enabled {
                    resources.insert(PermissionResource::new(&rule.resource));
                }
            }
        }

        resources.into_iter().collect()
    }

    async fn get_allowed_actions(&self, subject: &str, resource: &str) -> Vec<PermissionAction> {
        let subject_roles = self.get_subject_roles(subject);
        let mut actions = std::collections::HashSet::new();

        for role in &subject_roles {
            let rules = self.get_role_permissions(role).await;
            for rule in rules {
                if rule.enabled && (rule.resource == "*" || rule.resource == resource) {
                    for action in &rule.allow {
                        actions.insert(action.clone());
                    }
                }
            }
        }

        actions.into_iter().collect()
    }

    async fn refresh(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(mut last_refresh) = self.last_refresh.write() {
            *last_refresh = Instant::now();
        }
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl RbacPermissionProvider {
    /// 获取主体的角色列表
    fn get_subject_roles(&self, subject: &str) -> Vec<String> {
        let mapping = match self.role_mapping.read() {
            Ok(m) => m,
            Err(_) => return Vec::new(),
        };
        let roles = match self.roles.read() {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        get_subject_roles(&mapping, &roles, subject)
    }

    /// 检查角色是否存在
    pub fn has_role(&self, role: &str) -> bool {
        if let Ok(roles) = self.roles.read() {
            roles.contains_key(role)
        } else {
            false
        }
    }
}

/// 策略决策点配置
///
/// 用于 `PolicyDecisionPoint::with_config` 构造器，正确应用所有配置字段。
#[derive(Debug, Clone)]
pub struct PolicyDecisionPointConfig {
    /// 默认决策（当没有匹配规则时）
    pub default_decision: PermissionDecision,
    /// 缓存配置
    pub cache_ttl_seconds: u64,
    /// 是否启用缓存
    pub cache_enabled: bool,
}

impl Default for PolicyDecisionPointConfig {
    fn default() -> Self {
        Self {
            default_decision: PermissionDecision::Deny,
            cache_ttl_seconds: 300,
            cache_enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 回归：新建 YamlPermissionProvider 的首次 check 必须触发配置加载。
    /// 修复前 last_refresh 初值为 Instant::now()，60s 惰性阈值下首次 check
    /// 的 age≈0 永不加载，静默以空规则返回 NotApplicable（deny-everything
    /// 无任何信号）。stale_last_refresh 初值使首查即加载。
    #[tokio::test]
    async fn yaml_provider_first_check_loads_config() {
        let dir = std::env::temp_dir().join(format!(
            "dbnexus_perm_engine_{}_{}",
            std::process::id(),
            std::time::Instant::now().elapsed().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("permissions.json");
        std::fs::write(
            &config_path,
            r#"{"roles": {"ops": [{"name": "ops-read", "subject": "ops", "resource": "users", "allow": ["select"], "deny": []}]}}"#,
        )
        .unwrap();

        let provider = YamlPermissionProvider::new(config_path.to_string_lossy().as_ref()).unwrap();
        let decision = provider
            .check_permission(&PermissionContext::new(
                PermissionSubject::user("ops"),
                PermissionResource::new("users"),
                PermissionAction::Select,
            ))
            .await;

        assert!(
            matches!(decision, PermissionDecision::Allow),
            "first check must load config and allow, got: {decision:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_yaml_permission_provider() {
        // 使用 RBAC 提供者进行测试，因为它不需要配置文件
        let provider = Arc::new(RbacPermissionProvider::new());

        // 添加角色和权限
        provider.add_role(Role {
            name: "admin".to_string(),
            description: "管理员角色".to_string(),
            enabled: true,
            extends: vec![],
        });

        provider.add_permission(
            "admin",
            PermissionRule {
                name: "admin_select".to_string(),
                priority: 100,
                subject: "*".to_string(),
                resource: "users".to_string(),
                allow: vec![PermissionAction::Select],
                deny: vec![],
                condition: None,
                enabled: true,
            },
        );

        // 将用户 "admin" 映射到角色 "admin"
        provider.add_role_to_subject("admin", "admin");

        let pdp = PolicyDecisionPoint::new(provider);

        // 测试权限检查
        let result = pdp.check("admin", "users", "SELECT").await;
        assert_eq!(result, PermissionDecision::Allow);
    }

    #[tokio::test]
    async fn test_rbac_permission_provider() {
        let provider = Arc::new(RbacPermissionProvider::new());

        // 添加角色
        provider.add_role(Role {
            name: "admin".to_string(),
            description: "管理员角色".to_string(),
            enabled: true,
            extends: vec![],
        });

        // 添加权限规则
        provider.add_permission(
            "admin",
            PermissionRule {
                name: "admin_all".to_string(),
                priority: 100,
                subject: "*".to_string(),
                resource: "*".to_string(),
                allow: vec![
                    PermissionAction::Select,
                    PermissionAction::Insert,
                    PermissionAction::Update,
                    PermissionAction::Delete,
                ],
                deny: vec![],
                condition: None,
                enabled: true,
            },
        );

        // 将用户 "admin" 映射到角色 "admin"
        provider.add_role_to_subject("admin", "admin");

        let pdp = PolicyDecisionPoint::new(provider);

        // 测试权限检查
        let result = pdp.check("admin", "users", "SELECT").await;
        assert_eq!(result, PermissionDecision::Allow);

        let result = pdp.check("admin", "users", "DELETE").await;
        assert_eq!(result, PermissionDecision::Allow);
    }

    #[tokio::test]
    async fn test_permission_engine() {
        let provider = Arc::new(RbacPermissionProvider::new());

        // 添加角色
        provider.add_role(Role {
            name: "admin".to_string(),
            description: "管理员角色".to_string(),
            enabled: true,
            extends: vec![],
        });

        // 添加权限规则
        provider.add_permission(
            "admin",
            PermissionRule {
                name: "admin_all".to_string(),
                priority: 100,
                subject: "*".to_string(),
                resource: "*".to_string(),
                allow: vec![
                    PermissionAction::Select,
                    PermissionAction::Insert,
                    PermissionAction::Update,
                    PermissionAction::Delete,
                ],
                deny: vec![],
                condition: None,
                enabled: true,
            },
        );

        // 将用户 "admin" 映射到角色 "admin"
        provider.add_role_to_subject("admin", "admin");

        let pdp = PolicyDecisionPoint::new(provider);

        // 测试权限检查
        let decision = pdp.check("admin", "users", "SELECT").await;
        assert_eq!(decision, PermissionDecision::Allow);
    }

    #[tokio::test]
    async fn test_permission_context() {
        let context = PermissionContext::new(
            PermissionSubject::user("admin"),
            PermissionResource::new("users"),
            PermissionAction::Select,
        )
        .with_attribute("ip", "192.168.1.1")
        .with_environment("time", "2024-01-01");

        assert_eq!(context.subject.id, "admin");
        assert_eq!(context.resource.name, "users");
        assert_eq!(context.action, PermissionAction::Select);
        assert!(context.attributes.contains_key("ip"));
    }

    #[tokio::test]
    async fn test_policy_decision_point_with_rate_limit() {
        let provider = Arc::new(RbacPermissionProvider::new());

        // 添加角色
        provider.add_role(Role {
            name: "admin".to_string(),
            description: "管理员角色".to_string(),
            enabled: true,
            extends: vec![],
        });

        // 添加权限规则
        provider.add_permission(
            "admin",
            PermissionRule {
                name: "admin_select".to_string(),
                priority: 100,
                subject: "*".to_string(),
                resource: "users".to_string(),
                allow: vec![PermissionAction::Select],
                deny: vec![],
                condition: None,
                enabled: true,
            },
        );

        // 将用户 "admin" 映射到角色 "admin"
        provider.add_role_to_subject("admin", "admin");

        // 创建带速率限制的 PDP
        let pdp = PolicyDecisionPoint::with_rate_limit(provider, 10, 60);

        // 前 10 次请求应该成功
        for i in 0..10 {
            let result = pdp.check("admin", "users", "SELECT").await;
            assert_eq!(
                result,
                PermissionDecision::Allow,
                "Request {} should be allowed",
                i
            );
        }

        // 第 11 次请求应该被速率限制
        let result = pdp.check("admin", "users", "SELECT").await;
        assert_eq!(result, PermissionDecision::Deny);
    }

    #[tokio::test]
    async fn test_permission_subject_creation() {
        // 测试用户主体
        let user = PermissionSubject::user("test_user");
        assert_eq!(user.id, "test_user");
        assert_eq!(user.subject_type, SubjectType::User);

        // 测试角色主体
        let role = PermissionSubject::role("admin");
        assert_eq!(role.id, "admin");
        assert_eq!(role.subject_type, SubjectType::Role);
    }

    #[tokio::test]
    async fn test_permission_resource_creation() {
        // 测试基本资源
        let resource = PermissionResource::new("users");
        assert_eq!(resource.name, "users");
        assert_eq!(resource.resource_type, "table");

        // 测试带类型的资源
        let resource_with_type = PermissionResource::with_type("logs", "log");
        assert_eq!(resource_with_type.name, "logs");
        assert_eq!(resource_with_type.resource_type, "log");
    }

    #[tokio::test]
    async fn test_permission_decision_types() {
        assert_eq!(PermissionDecision::Allow, PermissionDecision::Allow);
        assert_eq!(PermissionDecision::Deny, PermissionDecision::Deny);
        assert_eq!(
            PermissionDecision::NotApplicable,
            PermissionDecision::NotApplicable
        );

        let error_decision = PermissionDecision::Error("Test error".to_string());
        assert!(matches!(error_decision, PermissionDecision::Error(msg) if msg == "Test error"));
    }

    #[tokio::test]
    async fn test_role_creation() {
        let role = Role {
            name: "test_role".to_string(),
            description: "测试角色".to_string(),
            enabled: true,
            extends: vec!["base_role".to_string()],
        };

        assert_eq!(role.name, "test_role");
        assert_eq!(role.description, "测试角色");
        assert!(role.enabled);
        assert_eq!(role.extends.len(), 1);
        assert_eq!(role.extends[0], "base_role");
    }

    #[tokio::test]
    async fn test_permission_rule_creation() {
        let rule = PermissionRule {
            name: "test_rule".to_string(),
            priority: 50,
            subject: "admin".to_string(),
            resource: "users".to_string(),
            allow: vec![PermissionAction::Select, PermissionAction::Insert],
            deny: vec![PermissionAction::Delete],
            condition: Some("active = true".to_string()),
            enabled: true,
        };

        assert_eq!(rule.name, "test_rule");
        assert_eq!(rule.priority, 50);
        assert_eq!(rule.allow.len(), 2);
        assert_eq!(rule.deny.len(), 1);
        assert!(rule.enabled);
        assert!(rule.condition.is_some());
    }

    #[tokio::test]
    async fn test_role_hierarchy() {
        let provider = RbacPermissionProvider::new();

        // 添加角色及其继承
        let base_role = Role {
            name: "base_user".to_string(),
            description: "基础用户角色".to_string(),
            enabled: true,
            extends: vec![],
        };
        provider.add_role(base_role);

        let child_role = Role {
            name: "premium_user".to_string(),
            description: "高级用户角色".to_string(),
            enabled: true,
            extends: vec!["base_user".to_string()],
        };
        provider.add_role(child_role.clone());

        // 验证角色存在
        assert!(provider.has_role("base_user"));
        assert!(provider.has_role("premium_user"));
    }

    #[test]
    fn test_path_allowed_under_prefix_match() {
        // 正常前缀：临时目录内的路径允许
        assert!(path_allowed_under(
            "/tmp/dbnexus/policies.yaml",
            std::path::Path::new("/tmp")
        ));
    }

    #[test]
    fn test_path_allowed_under_prefix_mismatch() {
        // 非前缀：临时目录外的路径拒绝
        assert!(!path_allowed_under(
            "/etc/passwd",
            std::path::Path::new("/tmp")
        ));
    }

    #[test]
    fn test_path_allowed_under_empty_prefix_fails_closed() {
        // 空前缀（模拟 temp_dir 非 UTF-8 时回退为空串的场景）必须 fail-closed
        assert!(!path_allowed_under("/etc/passwd", std::path::Path::new("")));
    }

    #[tokio::test]
    async fn test_check_unknown_action_denies() {
        // 未知操作默认拒绝（fail-closed），不再返回 Error
        let provider = Arc::new(RbacPermissionProvider::new());
        let pdp = PolicyDecisionPoint::new(provider);

        assert_eq!(
            pdp.check("admin", "users", "DROP").await,
            PermissionDecision::Deny
        );
        // 小写未知操作同样拒绝
        assert_eq!(
            pdp.check("admin", "users", "unknown").await,
            PermissionDecision::Deny
        );
    }

    // ========================================================================
    // 主体 / 资源 / 上下文构造器
    // ========================================================================

    #[test]
    fn test_permission_resource_with_type() {
        let resource = PermissionResource::with_type("orders", "view");
        assert_eq!(resource.name, "orders");
        assert_eq!(resource.resource_type, "view");
        assert_eq!(PermissionResource::new("users").resource_type, "table");
    }

    #[test]
    fn test_permission_subject_role_type() {
        let subject = PermissionSubject::role("admins");
        assert_eq!(subject.id, "admins");
        assert_eq!(subject.subject_type, SubjectType::Role);
        assert_eq!(
            PermissionSubject::user("u1").subject_type,
            SubjectType::User
        );
    }

    // ========================================================================
    // 规则匹配与条件评估（纯函数直测）
    // ========================================================================

    fn rule(subject: &str, resource: &str, allow: Vec<PermissionAction>) -> PermissionRule {
        PermissionRule {
            name: "r".to_string(),
            priority: 1,
            subject: subject.to_string(),
            resource: resource.to_string(),
            allow,
            deny: vec![],
            condition: None,
            enabled: true,
        }
    }

    fn ctx() -> PermissionContext {
        PermissionContext::new(
            PermissionSubject::user("ops"),
            PermissionResource::new("users"),
            PermissionAction::Select,
        )
    }

    #[test]
    fn test_matches_rule_subject_resource_action_gates() {
        assert!(matches_rule(
            &rule("*", "*", vec![PermissionAction::Select]),
            &ctx()
        ));
        assert!(matches_rule(
            &rule("ops", "users", vec![PermissionAction::Select]),
            &ctx()
        ));
        // 主体不匹配
        assert!(!matches_rule(
            &rule("bob", "*", vec![PermissionAction::Select]),
            &ctx()
        ));
        // 资源不匹配
        assert!(!matches_rule(
            &rule("*", "orders", vec![PermissionAction::Select]),
            &ctx()
        ));
        // 操作不在 allow/deny 列表 → 不匹配（提前过滤，allow 为空也失配）
        assert!(!matches_rule(&rule("*", "*", vec![]), &ctx()));
        assert!(!matches_rule(
            &rule("*", "*", vec![PermissionAction::Delete]),
            &ctx()
        ));
    }

    #[test]
    fn test_matches_rule_condition_gate() {
        let mut r = rule("*", "*", vec![PermissionAction::Select]);
        r.condition = Some("tenant=acme".to_string());
        // 条件不满足 → 不匹配
        assert!(!matches_rule(&r, &ctx()));
        // 属性满足 → 匹配
        let matched_ctx = ctx().with_attribute("tenant", "acme");
        assert!(matches_rule(&r, &matched_ctx));
        // 环境变量兜底满足 → 匹配
        let env_ctx = ctx().with_environment("tenant", "acme");
        assert!(matches_rule(&r, &env_ctx));
    }

    #[test]
    fn test_evaluate_condition_semantics() {
        let base = ctx().with_attribute("dept", "ops");
        // 空条件恒匹配
        assert!(evaluate_condition("", &base));
        assert!(evaluate_condition("  ", &base));
        // AND 语义：全部满足才匹配
        assert!(evaluate_condition("dept=ops", &base));
        assert!(evaluate_condition(
            "dept=ops, region=cn",
            &base.clone().with_attribute("region", "cn")
        ));
        // 部分不满足（region 期望 cn 实际 us）→ false
        assert!(!evaluate_condition(
            "dept=ops, region=cn",
            &base.clone().with_attribute("region", "us")
        ));
        // attributes 优先于 environment
        let both = ctx()
            .with_attribute("k", "from_attr")
            .with_environment("k", "from_env");
        assert!(evaluate_condition("k=from_attr", &both));
        assert!(!evaluate_condition("k=from_env", &both));
        // 无法解析的片段 fail-safe 拒绝
        assert!(!evaluate_condition("noequals", &base));
    }

    // ========================================================================
    // 角色继承与 RBAC 查询接口
    // ========================================================================

    #[tokio::test]
    async fn test_rbac_inheritance_and_query_apis() {
        let provider = RbacPermissionProvider::default();
        provider.add_role(Role {
            name: "viewer".to_string(),
            description: String::new(),
            enabled: true,
            extends: vec![],
        });
        provider.add_role(Role {
            name: "editor".to_string(),
            description: "继承 viewer".to_string(),
            enabled: true,
            extends: vec!["viewer".to_string()],
        });
        provider.add_permission(
            "viewer",
            PermissionRule {
                name: "view".to_string(),
                priority: 1,
                subject: "ops".to_string(),
                resource: "reports".to_string(),
                allow: vec![PermissionAction::Select],
                deny: vec![],
                condition: None,
                enabled: true,
            },
        );
        provider.add_permission(
            "editor",
            PermissionRule {
                name: "edit".to_string(),
                priority: 2,
                subject: "ops".to_string(),
                resource: "*".to_string(),
                allow: vec![PermissionAction::Update],
                deny: vec![],
                condition: None,
                enabled: true,
            },
        );
        provider.add_role_to_subject("ops", "editor");

        assert!(provider.has_role("viewer"));
        assert!(provider.has_role("editor"));
        assert!(!provider.has_role("ghost"));

        // 继承角色的权限参与决策：editor 无 select，但继承的 viewer 有
        let decision = provider
            .check_permission(&PermissionContext::new(
                PermissionSubject::user("ops"),
                PermissionResource::new("reports"),
                PermissionAction::Select,
            ))
            .await;
        assert_eq!(decision, PermissionDecision::Allow);

        // get_allowed_resources / get_allowed_actions 含继承权限
        let resources = provider.get_allowed_resources("ops").await;
        assert!(
            resources
                .iter()
                .any(|r| r.name == "reports" || r.name == "*")
        );
        let actions = provider.get_allowed_actions("ops", "reports").await;
        assert!(actions.contains(&PermissionAction::Select));
        assert!(actions.contains(&PermissionAction::Update));

        // 角色未分配的主体 → NotApplicable
        let denied = provider
            .check_permission(&PermissionContext::new(
                PermissionSubject::user("mallory"),
                PermissionResource::new("reports"),
                PermissionAction::Select,
            ))
            .await;
        assert_eq!(denied, PermissionDecision::NotApplicable);

        // refresh 只更新时间戳，不改变决策
        provider.refresh().await.unwrap();
    }

    // ========================================================================
    // PDP 构造器全家桶 + 缓存行为
    // ========================================================================

    fn allow_all_provider() -> Arc<RbacPermissionProvider> {
        let provider = Arc::new(RbacPermissionProvider::new());
        provider.add_role(Role {
            name: "admin".to_string(),
            description: String::new(),
            enabled: true,
            extends: vec![],
        });
        provider.add_permission(
            "admin",
            PermissionRule {
                name: "all".to_string(),
                priority: 100,
                subject: "*".to_string(),
                resource: "*".to_string(),
                allow: vec![PermissionAction::Select],
                deny: vec![],
                condition: None,
                enabled: true,
            },
        );
        provider.add_role_to_subject("admin", "admin");
        provider
    }

    #[test]
    fn test_builder_without_provider_panics() {
        let result = std::panic::catch_unwind(PolicyDecisionPointBuilder::new);
        assert!(result.is_ok());
        // 未设置 provider 时 build 必须 panic（文档契约）
        let built = std::panic::catch_unwind(|| PolicyDecisionPoint::builder().build());
        assert!(built.is_err(), "缺 provider 的 build 应 panic");
    }

    #[tokio::test]
    async fn test_builder_full_options_and_cache_flow() {
        let provider = allow_all_provider();
        let pdp = PolicyDecisionPoint::builder()
            .provider(provider.clone())
            .cache_ttl_seconds(600)
            .cache_enabled(true)
            .rate_limit(1000, 60)
            .default_decision(PermissionDecision::Deny)
            .build();

        // 首查：provider 决策 + 写缓存
        let first = pdp.check("admin", "users", "SELECT").await;
        assert_eq!(first, PermissionDecision::Allow);
        // 二查：命中缓存（同样结果）
        let second = pdp.check("admin", "users", "SELECT").await;
        assert_eq!(second, PermissionDecision::Allow);

        // refresh_cache 清空条目后决策不变
        pdp.refresh_cache().await;
        let third = pdp.check("admin", "users", "SELECT").await;
        assert_eq!(third, PermissionDecision::Allow);
    }

    #[tokio::test]
    async fn test_with_dependencies_and_with_cache_constructors() {
        let provider = allow_all_provider();
        let pdp = PolicyDecisionPoint::with_dependencies(provider.clone());
        assert_eq!(
            pdp.check("admin", "users", "SELECT").await,
            PermissionDecision::Allow
        );

        let pdp = PolicyDecisionPoint::with_cache(provider.clone(), 1);
        assert_eq!(
            pdp.check("admin", "users", "SELECT").await,
            PermissionDecision::Allow
        );

        // with_config：默认决策 Deny + NotApplicable 时兜底
        let pdp = PolicyDecisionPoint::with_config(
            provider.clone(),
            PolicyDecisionPointConfig::default(),
        );
        assert_eq!(
            pdp.check("admin", "users", "SELECT").await,
            PermissionDecision::Allow
        );

        // 默认决策生效：无匹配规则的主体走 default_decision
        let empty_provider = Arc::new(RbacPermissionProvider::new());
        let pdp = PolicyDecisionPoint::with_config(
            empty_provider,
            PolicyDecisionPointConfig {
                default_decision: PermissionDecision::Deny,
                cache_ttl_seconds: 300,
                cache_enabled: false,
            },
        );
        assert_eq!(
            pdp.check("stranger", "secret", "SELECT").await,
            PermissionDecision::Deny
        );
    }

    #[tokio::test]
    async fn test_check_batch_and_get_allowed_resources() {
        let provider = allow_all_provider();
        let pdp = PolicyDecisionPoint::new(provider.clone());

        let contexts = vec![
            PermissionContext::new(
                PermissionSubject::user("admin"),
                PermissionResource::new("users"),
                PermissionAction::Select,
            ),
            PermissionContext::new(
                PermissionSubject::user("ghost"),
                PermissionResource::new("users"),
                PermissionAction::Delete,
            ),
        ];
        let decisions = pdp.check_batch(contexts).await;
        assert_eq!(decisions.len(), 2);
        assert_eq!(decisions[0].1, PermissionDecision::Allow);
        assert_eq!(decisions[1].1, PermissionDecision::NotApplicable);

        let resources = pdp.get_allowed_resources("admin").await;
        assert!(resources.iter().any(|r| r.name == "*"));
    }

    // ========================================================================
    // 配置路径安全检查
    // ========================================================================

    #[test]
    fn test_is_safe_config_path_matrix() {
        // 空路径拒绝
        assert!(!is_safe_config_path(""));
        // 路径遍历拒绝
        assert!(!is_safe_config_path("config/../../etc/passwd"));
        assert!(!is_safe_config_path("a\\b"));
        // 允许前缀的绝对路径放行
        assert!(is_safe_config_path("/etc/dbnexus/permissions.yaml"));
        assert!(is_safe_config_path("/opt/dbnexus/config/p.yaml"));
        assert!(is_safe_config_path("./config/p.yaml"));
        assert!(is_safe_config_path("./p.yaml"));
        // 前缀外的绝对路径走临时目录检查
        let temp = std::env::temp_dir().join("dbnexus_test_perm.yaml");
        assert!(is_safe_config_path(temp.to_str().unwrap()));
        assert!(!is_safe_config_path("/definitely/not/allowed/p.yaml"));
        // 相对路径无遍历放行
        assert!(is_safe_config_path("permissions.yaml"));
    }

    // ========================================================================
    // YamlPermissionProvider 错误路径
    // ========================================================================

    #[tokio::test]
    async fn test_yaml_provider_missing_file_errors_on_check() {
        // 配置文件缺失：构造是惰性的；首次 check（stale last_refresh）触发
        // 加载失败，必须显性返回 Error 决策而非静默空规则
        let missing = std::env::temp_dir().join("dbnexus_no_such_perm_config.yaml");
        let _ = std::fs::remove_file(&missing);
        let provider = YamlPermissionProvider::new(missing.to_str().unwrap()).unwrap();
        let decision = provider
            .check_permission(&PermissionContext::new(
                PermissionSubject::user("ops"),
                PermissionResource::new("users"),
                PermissionAction::Select,
            ))
            .await;
        assert!(
            matches!(decision, PermissionDecision::Error(_)),
            "missing config must surface Error, got: {decision:?}"
        );
    }

    // ===== 补充测试：YamlPermissionProvider 全流程（允许/拒绝/优先级/查询） =====

    #[tokio::test]
    async fn test_yaml_provider_full_flow() {
        let dir = std::env::temp_dir().join(format!(
            "dbnexus_perm_flow_{}_{}",
            std::process::id(),
            std::time::Instant::now().elapsed().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("permissions.json");
        std::fs::write(
            &config_path,
            r#"{"roles": {
                "ops": [
                    {"name": "ops-read", "priority": 10, "subject": "ops", "resource": "users", "allow": ["select"], "deny": []},
                    {"name": "ops-no-delete", "priority": 100, "subject": "ops", "resource": "users", "allow": [], "deny": ["delete"]}
                ]
            }}"#,
        )
        .unwrap();

        let provider = YamlPermissionProvider::new(config_path.to_string_lossy().as_ref()).unwrap();

        // 高优先级 deny 压过低优先级 allow
        let denied = provider
            .check_permission(&PermissionContext::new(
                PermissionSubject::user("ops"),
                PermissionResource::new("users"),
                PermissionAction::Delete,
            ))
            .await;
        assert_eq!(denied, PermissionDecision::Deny);

        // allow 命中
        let allowed = provider
            .check_permission(&PermissionContext::new(
                PermissionSubject::user("ops"),
                PermissionResource::new("users"),
                PermissionAction::Select,
            ))
            .await;
        assert_eq!(allowed, PermissionDecision::Allow);

        // 未覆盖的操作 → NotApplicable
        let na = provider
            .check_permission(&PermissionContext::new(
                PermissionSubject::user("ops"),
                PermissionResource::new("users"),
                PermissionAction::Update,
            ))
            .await;
        assert_eq!(na, PermissionDecision::NotApplicable);

        // 资源 / 动作查询
        let resources = provider.get_allowed_resources("ops").await;
        assert!(resources.iter().any(|r| r.name == "users"));
        let actions = provider.get_allowed_actions("ops", "users").await;
        assert!(actions.contains(&PermissionAction::Select));

        // refresh 走 load_config（时间戳刷新）
        provider.refresh().await.expect("refresh");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_yaml_provider_new_path_validation_branches() {
        // 空路径
        assert!(YamlPermissionProvider::new("").is_err());
        // 父目录引用
        assert!(YamlPermissionProvider::new("../etc/passwd").is_err());
        // 非允许目录的绝对路径
        assert!(YamlPermissionProvider::new("/definitely/not/allowed/x.yaml").is_err());

        // Default 实现与 new 产物同构（懒加载语义）
        let provider = YamlPermissionProvider::default();
        assert_eq!(provider.name(), "yaml");
    }

    #[test]
    fn test_role_and_rbac_provider_defaults() {
        let role = Role::default();
        assert!(role.name.is_empty());
        assert!(role.enabled);
        assert!(role.extends.is_empty());

        let provider = RbacPermissionProvider::default();
        assert_eq!(provider.name(), "rbac");
        assert!(!provider.has_role("anything"));
    }

    #[tokio::test]
    async fn test_pdp_cache_ttl_zero_bypasses_cache() {
        let provider = allow_all_provider();
        // TTL=0：缓存条目即时过期 → 每次查询都穿透到 provider
        let pdp = PolicyDecisionPoint::with_cache(provider, 0);
        let first = pdp.check("admin", "users", "SELECT").await;
        assert_eq!(first, PermissionDecision::Allow);
        let second = pdp.check("admin", "users", "SELECT").await;
        assert_eq!(second, PermissionDecision::Allow);
    }
}
