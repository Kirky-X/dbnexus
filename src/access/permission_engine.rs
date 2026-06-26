// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

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

pub use crate::access::permission::PermissionAction;
use async_trait::async_trait;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// 预编译的正则表达式，用于检测路径遍历攻击模式
/// 使用 once_cell 确保线程安全的单次初始化
static PATH_TRAVERSAL_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\.\.|%2e%2e|%252e%252e|\\/|\\\\").expect("Regex pattern should be valid"));

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
        if allowed_prefixes.iter().any(|prefix| path.starts_with(prefix)) {
            return true;
        }
        // 也允许系统临时目录（用于测试场景）
        let temp_dir = std::env::temp_dir();
        return path.starts_with(temp_dir.to_str().unwrap_or(""));
    }

    // 相对路径检查
    !path.contains("..") && !path.contains('\\')
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
    pub fn new(subject: PermissionSubject, resource: PermissionResource, action: PermissionAction) -> Self {
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

    /// 构建策略决策点
    ///
    /// # Panics
    ///
    /// 如果未设置权限提供者，将 panic
    pub fn build(self) -> PolicyDecisionPoint {
        let provider = self.provider.expect("Provider is required for PolicyDecisionPoint");

        PolicyDecisionPoint {
            provider,
            cache: DashMap::new(),
            cache_ttl_seconds: self.cache_ttl_seconds.unwrap_or(DEFAULT_CACHE_TTL_SECONDS),
            cache_enabled: self.cache_enabled.unwrap_or(true),
            rate_limit_max_requests: self.rate_limit_max_requests.unwrap_or(DEFAULT_RATE_LIMIT_MAX_REQUESTS),
            rate_limit_window_seconds: self.rate_limit_window_seconds.unwrap_or(DEFAULT_RATE_LIMIT_WINDOW_SECONDS),
            rate_limit_store: DashMap::new(),
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
        }
    }

    /// 创建带速率限制配置的策略决策点
    pub fn with_rate_limit(provider: Arc<dyn PermissionProvider>, max_requests: u32, window_seconds: u32) -> Self {
        Self {
            provider,
            cache: DashMap::new(),
            cache_ttl_seconds: DEFAULT_CACHE_TTL_SECONDS,
            cache_enabled: true,
            rate_limit_max_requests: max_requests,
            rate_limit_window_seconds: window_seconds,
            rate_limit_store: DashMap::new(),
        }
    }

    /// 检查速率限制
    fn check_rate_limit(&self, subject_id: &str) -> bool {
        let key = subject_id.to_string();
        let now = Instant::now();
        let window_duration = Duration::from_secs(self.rate_limit_window_seconds as u64);

        // 获取或创建速率限制条目
        let mut entry = self.rate_limit_store.entry(key.clone()).or_insert(RateLimitEntry {
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
        if self.cache_enabled {
            if let Some(decision) = self.get_cached_decision(&cache_key) {
                return decision;
            }
        }

        // 获取权限决策
        let decision = self.provider.check_permission(context).await;

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
            // 未知操作返回错误，拒绝访问（安全考虑）
            _ => return PermissionDecision::Error(format!("Unknown action: {}", action)),
        };

        let context = PermissionContext::new(
            PermissionSubject::user(subject),
            PermissionResource::new(resource),
            action,
        );

        self.check_permission(&context).await
    }

    /// 批量检查权限
    pub async fn check_batch(&self, contexts: Vec<PermissionContext>) -> Vec<(PermissionContext, PermissionDecision)> {
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
        self.cache.insert(key.to_string(), CachedDecision::new(decision));
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

impl Default for YamlPermissionProvider {
    fn default() -> Self {
        Self {
            config_path: String::new(),
            roles: RwLock::new(HashMap::new()),
            last_refresh: RwLock::new(Instant::now()),
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
            last_refresh: RwLock::new(Instant::now()),
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

        // 直接使用 JSON 解析
        #[cfg(feature = "json")]
        {
            let config: YamlConfig = serde_json::from_str(&content)?;

            // 更新角色权限
            if let Ok(mut roles) = self.roles.write() {
                *roles = config.roles;
            }
        }
        #[cfg(not(feature = "json"))]
        {
            // 如果没有 json feature，使用 serde_yaml_ng 直接解析
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
                return Err("Cannot parse permission config: neither JSON nor YAML support available".into());
            }
        }

        // 初始化角色映射（从角色定义中提取）
        if let Ok(mut role_mapping) = self.role_mapping.write() {
            role_mapping.clear();
        }

        if let Ok(mut last_refresh) = self.last_refresh.write() {
            *last_refresh = Instant::now();
        }

        Ok(())
    }

    /// 检查规则是否匹配
    fn matches_rule(&self, rule: &PermissionRule, context: &PermissionContext) -> bool {
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

        true
    }
}

#[async_trait]
impl PermissionProvider for YamlPermissionProvider {
    async fn check_permission(&self, context: &PermissionContext) -> PermissionDecision {
        // 加载配置（如果需要）
        let age = self.last_refresh.read().map(|r| r.elapsed()).unwrap_or_default();
        if age.as_secs() > 60 {
            if let Err(e) = self.load_config().await {
                return PermissionDecision::Error(format!("Failed to load config: {}", e));
            }
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
                    if rule.enabled && self.matches_rule(rule, context) {
                        matched_rules.push((rule.priority, rule));
                    }
                }
            }
        }

        // 按优先级从高到低排序
        matched_rules.sort_by(|a, b| b.0.cmp(&a.0));

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
        // 优先从角色映射中获取
        if let Ok(mapping) = self.role_mapping.read() {
            if let Some(roles) = mapping.get(subject) {
                return roles.clone();
            }
        }
        // 如果没有映射，尝试直接将主体名作为角色名（用于简单用例）
        // 但要确保只返回预定义的角色（防止安全问题）
        if let Ok(roles) = self.roles.read() {
            if roles.contains_key(subject) {
                return vec![subject.to_string()];
            }
        }
        Vec::new()
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
            mapping.entry(subject.to_string()).or_default().push(role.to_string());
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
        all_rules.sort_by(|a, b| b.priority.cmp(&a.priority));

        // 评估规则
        for rule in all_rules {
            if rule.enabled && self.matches_rule(&rule, context) {
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
        // 优先从角色映射中获取
        if let Ok(mapping) = self.role_mapping.read() {
            if let Some(roles) = mapping.get(subject) {
                return roles.clone();
            }
        }
        // 如果没有映射，检查 subject 本身是否是预定义的角色
        if let Ok(roles) = self.roles.read() {
            if roles.contains_key(subject) {
                return vec![subject.to_string()];
            }
        }
        Vec::new()
    }

    /// 检查规则是否匹配
    fn matches_rule(&self, rule: &PermissionRule, context: &PermissionContext) -> bool {
        if rule.subject != "*" && rule.subject != context.subject.id {
            return false;
        }
        if rule.resource != "*" && rule.resource != context.resource.name {
            return false;
        }
        true
    }

    /// 检查角色是否存在
    pub fn has_role(&self, role: &str) -> bool {
        if let Ok(roles) = self.roles.read() {
            roles.contains_key(role) || self.get_subject_roles(role).contains(&role.to_string())
        } else {
            false
        }
    }
}

/// 权限引擎配置
#[derive(Debug, Clone)]
pub struct PermissionEngineConfig {
    /// 默认决策（当没有匹配规则时）
    pub default_decision: PermissionDecision,
    /// 是否记录拒绝的决策
    pub log_denied: bool,
    /// 缓存配置
    pub cache_ttl_seconds: u64,
    /// 是否启用缓存
    pub cache_enabled: bool,
}

impl Default for PermissionEngineConfig {
    fn default() -> Self {
        Self {
            default_decision: PermissionDecision::Deny,
            log_denied: true,
            cache_ttl_seconds: 300,
            cache_enabled: true,
        }
    }
}

/// 权限引擎
/// 统一的权限管理入口
#[derive(Debug)]
pub struct PermissionEngine {
    /// 策略决策点
    pdp: PolicyDecisionPoint,
}

impl PermissionEngine {
    /// 创建权限引擎
    pub fn new(provider: Arc<dyn PermissionProvider>) -> Self {
        let config = PermissionEngineConfig::default();
        Self {
            pdp: PolicyDecisionPoint::with_cache(provider, config.cache_ttl_seconds),
        }
    }

    /// 创建带配置的权限引擎
    pub fn with_config(provider: Arc<dyn PermissionProvider>, config: PermissionEngineConfig) -> Self {
        Self {
            pdp: PolicyDecisionPoint::with_cache(provider, config.cache_ttl_seconds),
        }
    }

    /// 检查权限
    pub async fn check(&self, subject: &str, resource: &str, action: &str) -> bool {
        let decision = self.pdp.check(subject, resource, action).await;
        decision == PermissionDecision::Allow
    }

    /// 检查权限（带详细决策）
    pub async fn check_with_decision(&self, subject: &str, resource: &str, action: &str) -> PermissionDecision {
        self.pdp.check(subject, resource, action).await
    }

    /// 获取主体可访问的资源
    pub async fn get_allowed_resources(&self, subject: &str) -> Vec<PermissionResource> {
        self.pdp.get_allowed_resources(subject).await
    }

    /// 刷新权限缓存
    pub async fn refresh(&self) {
        self.pdp.refresh_cache().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let engine = PermissionEngine::new(provider);

        // 测试权限检查
        let allowed = engine.check("admin", "users", "SELECT").await;
        assert!(allowed);
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
            assert_eq!(result, PermissionDecision::Allow, "Request {} should be allowed", i);
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
        assert_eq!(PermissionDecision::NotApplicable, PermissionDecision::NotApplicable);

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
}
