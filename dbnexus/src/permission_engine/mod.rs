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
//! ```rust,ignore
//! use dbnexus::permission_engine::{PolicyDecisionPoint, YamlPermissionProvider};
//!
//! let provider = YamlPermissionProvider::new("permissions.yaml").await?;
//! let pdp = PolicyDecisionPoint::new(provider);
//!
//! let result = pdp.check_permission("admin", "users", "SELECT").await;
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Instant;
use std::sync::RwLock;

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
    /// 所有操作
    All,
}

impl std::fmt::Display for PermissionAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PermissionAction::Select => write!(f, "SELECT"),
            PermissionAction::Insert => write!(f, "INSERT"),
            PermissionAction::Update => write!(f, "UPDATE"),
            PermissionAction::Delete => write!(f, "DELETE"),
            PermissionAction::All => write!(f, "*"),
        }
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

/// 策略决策点
/// 统一处理权限决策，支持多种权限提供者
#[derive(Debug)]
pub struct PolicyDecisionPoint {
    /// 权限提供者
    provider: Arc<dyn PermissionProvider>,
    /// 缓存
    cache: RwLock<HashMap<String, PermissionDecision>>,
    /// 缓存配置
    cache_ttl_seconds: u64,
    /// 是否启用缓存
    cache_enabled: bool,
}

impl PolicyDecisionPoint {
    /// 创建策略决策点
    pub fn new(provider: Arc<dyn PermissionProvider>) -> Self {
        Self {
            provider,
            cache: RwLock::new(HashMap::new()),
            cache_ttl_seconds: 300,
            cache_enabled: true,
        }
    }

    /// 创建带缓存配置的策略决策点
    pub fn with_cache(provider: Arc<dyn PermissionProvider>, cache_ttl_seconds: u64) -> Self {
        Self {
            provider,
            cache: RwLock::new(HashMap::new()),
            cache_ttl_seconds,
            cache_enabled: true,
        }
    }

    /// 检查权限
    pub async fn check_permission(&self, context: &PermissionContext) -> PermissionDecision {
        // 生成缓存键
        let cache_key = self.generate_cache_key(context);

        // 检查缓存
        if self.cache_enabled {
            if let Some(decision) = self.get_cached_decision(&cache_key) {
                return decision;
            }
        }

        // 获取权限决策
        let decision = self.provider.check_permission(context).await;

        // 更新缓存
        if self.cache_enabled {
            self.update_cache(&cache_key, decision.clone());
        }

        decision
    }

    /// 检查用户是否有权限执行操作
    pub async fn check(
        &self,
        subject: &str,
        resource: &str,
        action: &str,
    ) -> PermissionDecision {
        let action = match action.to_uppercase().as_str() {
            "SELECT" => PermissionAction::Select,
            "INSERT" => PermissionAction::Insert,
            "UPDATE" => PermissionAction::Update,
            "DELETE" => PermissionAction::Delete,
            _ => PermissionAction::All,
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
        let mut cache = self.cache.write().expect("RwLock poisoned");
        cache.clear();
    }

    /// 启用/禁用缓存
    pub fn set_cache_enabled(&mut self, enabled: bool) {
        self.cache_enabled = enabled;
        if !enabled {
            let mut cache = self.cache.write().expect("RwLock poisoned");
            cache.clear();
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

    /// 获取缓存的决策
    fn get_cached_decision(&self, key: &str) -> Option<PermissionDecision> {
        let cache = self.cache.read().expect("RwLock poisoned");
        cache.get(key).cloned()
    }

    /// 更新缓存
    fn update_cache(&self, key: &str, decision: PermissionDecision) {
        let mut cache = self.cache.write().expect("RwLock poisoned");
        cache.insert(key.to_string(), decision);
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
}

impl Default for YamlPermissionProvider {
    fn default() -> Self {
        Self {
            config_path: String::new(),
            roles: RwLock::new(HashMap::new()),
            last_refresh: RwLock::new(Instant::now()),
            name: "yaml".to_string(),
        }
    }
}

impl YamlPermissionProvider {
    /// 创建 YAML 权限提供者
    pub fn new(config_path: &str) -> Self {
        Self {
            config_path: config_path.to_string(),
            roles: RwLock::new(HashMap::new()),
            last_refresh: RwLock::new(Instant::now()),
            name: "yaml".to_string(),
        }
    }

    /// 加载配置
    async fn load_config(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let content = tokio::fs::read_to_string(&self.config_path).await?;

        // 解析 YAML 配置
        #[derive(Debug, Deserialize)]
        struct YamlConfig {
            roles: HashMap<String, Vec<PermissionRule>>,
        }

        let config: YamlConfig = serde_yaml::from_str(&content)?;

        // 更新角色权限
        let mut roles = self.roles.write().expect("RwLock poisoned");
        *roles = config.roles;

        let mut last_refresh = self.last_refresh.write().expect("RwLock poisoned");
        *last_refresh = Instant::now();

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

        // 检查操作匹配
        if !rule.allow.is_empty() && !rule.allow.contains(&context.action) {
            if context.action != PermissionAction::All {
                return false;
            }
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

        // 按优先级排序规则
        let mut matching_rules: Vec<&PermissionRule> = Vec::new();

        for role_name in &subject_roles {
            if let Some(rules) = roles.get(role_name) {
                for rule in rules {
                    if rule.enabled && self.matches_rule(rule, context) {
                        matching_rules.push(rule);
                    }
                }
            }
        }

        // 按优先级排序
        matching_rules.sort_by(|a, b| b.priority.cmp(&a.priority));

        // 评估规则
        for rule in matching_rules {
            if rule.allow.contains(&context.action) || rule.allow.contains(&PermissionAction::All) {
                return PermissionDecision::Allow;
            }
            if rule.deny.contains(&context.action) || rule.deny.contains(&PermissionAction::All) {
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
    /// 获取主体的角色列表
    fn get_subject_roles(&self, subject: &str) -> Vec<String> {
        vec![subject.to_string()]
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
}

impl Default for RbacPermissionProvider {
    fn default() -> Self {
        Self {
            roles: RwLock::new(HashMap::new()),
            permissions: RwLock::new(HashMap::new()),
            role_hierarchy: RwLock::new(HashMap::new()),
            last_refresh: RwLock::new(Instant::now()),
            name: "rbac".to_string(),
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
        }
    }

    /// 添加角色
    pub fn add_role(&self, role: Role) {
        let mut roles = self.roles.write().expect("RwLock poisoned");
        roles.insert(role.name.clone(), role.clone());

        let mut hierarchy = self.role_hierarchy.write().expect("RwLock poisoned");
        hierarchy.insert(role.name, role.extends);
    }

    /// 添加权限规则
    pub fn add_permission(&self, role: &str, rule: PermissionRule) {
        let mut permissions = self.permissions.write().expect("RwLock poisoned");
        permissions.entry(role.to_string()).or_insert_with(Vec::new).push(rule);
    }

    /// 获取角色的所有权限（包括继承的）
    async fn get_role_permissions(&self, role: &str) -> Vec<PermissionRule> {
        let mut all_permissions = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut to_visit = vec![role.to_string()];

        let permissions = self.permissions.read().expect("RwLock poisoned");
        let hierarchy = self.role_hierarchy.read().expect("RwLock poisoned");

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
                if rule.allow.contains(&context.action) || rule.allow.contains(&PermissionAction::All) {
                    return PermissionDecision::Allow;
                }
                if rule.deny.contains(&context.action) || rule.deny.contains(&PermissionAction::All) {
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
                if rule.enabled
                    && (rule.resource == "*" || rule.resource == resource)
                {
                    for action in &rule.allow {
                        actions.insert(action.clone());
                    }
                }
            }
        }

        actions.into_iter().collect()
    }

    async fn refresh(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut last_refresh = self.last_refresh.write().expect("RwLock poisoned");
        *last_refresh = Instant::now();
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl RbacPermissionProvider {
    /// 获取主体的角色列表
    fn get_subject_roles(&self, subject: &str) -> Vec<String> {
        // 默认情况下，用户名即角色名
        vec![subject.to_string()]
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
    /// 配置
    config: PermissionEngineConfig,
}

impl PermissionEngine {
    /// 创建权限引擎
    pub fn new(provider: Arc<dyn PermissionProvider>) -> Self {
        Self {
            pdp: PolicyDecisionPoint::with_cache(provider, 300),
            config: PermissionEngineConfig::default(),
        }
    }

    /// 创建带配置的权限引擎
    pub fn with_config(provider: Arc<dyn PermissionProvider>, config: PermissionEngineConfig) -> Self {
        Self {
            pdp: PolicyDecisionPoint::with_cache(provider, config.cache_ttl_seconds),
            config,
        }
    }

    /// 检查权限
    pub async fn check(
        &self,
        subject: &str,
        resource: &str,
        action: &str,
    ) -> bool {
        let decision = self.pdp.check(subject, resource, action).await;
        decision == PermissionDecision::Allow
    }

    /// 检查权限（带详细决策）
    pub async fn check_with_decision(
        &self,
        subject: &str,
        resource: &str,
        action: &str,
    ) -> PermissionDecision {
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
                allow: vec![PermissionAction::Select, PermissionAction::Insert, PermissionAction::Update, PermissionAction::Delete],
                deny: vec![],
                condition: None,
                enabled: true,
            },
        );

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
}
