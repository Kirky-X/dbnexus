// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 审计日志模块
//!
//! 提供数据库操作审计功能，支持：
//! - CRUD 操作审计
//! - 用户身份追踪
//! - 敏感操作告警
//! - 审计日志持久化
//!
//! # Example
//!
//! ```rust,no_run
//! use dbnexus::audit::{AuditEvent, AuditLogger};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!     let logger = AuditLogger::with_default_storage();
//!     let event = AuditEvent::create("users", "1", "admin");
//!
//!     tokio::runtime::Runtime::new()
//!         .unwrap()
//!         .block_on(async { logger.log(event).await })?;
//!
//!     Ok(())
//! }
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use thiserror::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

/// AuditEventBuilder 构建错误
#[derive(Debug, Error)]
pub enum BuildError {
    /// 缺少必需字段 operation
    #[error("operation is required")]
    OperationRequired,
    /// 缺少必需字段 entity_type
    #[error("entity_type is required")]
    EntityTypeRequired,
    /// 缺少必需字段 entity_id
    #[error("entity_id is required")]
    EntityIdRequired,
}

/// 审计操作类型
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AuditOperation {
    /// 创建操作
    Create,
    /// 读取操作
    Read,
    /// 更新操作
    Update,
    /// 删除操作
    Delete,
    /// 登录操作
    Login,
    /// 登出操作
    Logout,
    /// 权限变更
    PermissionChange,
    /// 配置变更
    ConfigChange,
    /// 其他操作
    Other(String),
}

impl fmt::Display for AuditOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuditOperation::Create => write!(f, "CREATE"),
            AuditOperation::Read => write!(f, "READ"),
            AuditOperation::Update => write!(f, "UPDATE"),
            AuditOperation::Delete => write!(f, "DELETE"),
            AuditOperation::Login => write!(f, "LOGIN"),
            AuditOperation::Logout => write!(f, "LOGOUT"),
            AuditOperation::PermissionChange => write!(f, "PERMISSION_CHANGE"),
            AuditOperation::ConfigChange => write!(f, "CONFIG_CHANGE"),
            AuditOperation::Other(s) => write!(f, "{}", s.to_uppercase()),
        }
    }
}

impl Default for AuditOperation {
    fn default() -> Self {
        AuditOperation::Other("UNKNOWN".to_string())
    }
}

/// 审计事件严重级别
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum AuditSeverity {
    /// 信息
    #[default]
    Info,
    /// 低
    Low,
    /// 中
    Medium,
    /// 高
    High,
    /// 严重
    Critical,
}

impl fmt::Display for AuditSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuditSeverity::Info => write!(f, "INFO"),
            AuditSeverity::Low => write!(f, "LOW"),
            AuditSeverity::Medium => write!(f, "MEDIUM"),
            AuditSeverity::High => write!(f, "HIGH"),
            AuditSeverity::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// 审计状态枚举
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum AuditStatus {
    /// 成功
    #[default]
    Success,
    /// 失败
    Failure,
    /// 部分成功
    Partial,
    /// 未知
    Unknown,
}

impl fmt::Display for AuditStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuditStatus::Success => write!(f, "SUCCESS"),
            AuditStatus::Failure => write!(f, "FAILURE"),
            AuditStatus::Partial => write!(f, "PARTIAL"),
            AuditStatus::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

/// AuditResult 的类型别名（向后兼容）
pub type AuditResult = AuditStatus;
/// 审计事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// 事件 ID
    pub id: String,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 操作类型
    pub operation: AuditOperation,
    /// 实体类型（如 "users", "orders"）
    pub entity_type: String,
    /// 实体 ID
    pub entity_id: String,
    /// 用户 ID
    pub user_id: String,
    /// 用户角色
    pub user_role: String,
    /// 客户端 IP
    pub client_ip: String,
    /// 事件严重级别
    pub severity: AuditSeverity,
    /// 操作结果状态
    pub result: AuditStatus,
    /// 错误信息（如果操作失败）
    pub error_message: Option<String>,
    /// 变更前的值（JSON）
    pub before_value: Option<String>,
    /// 变更后的值（JSON）
    pub after_value: Option<String>,
    /// 附加信息（JSON）
    pub extra: Option<String>,
    /// 请求 ID（用于追踪）
    pub request_id: String,
    /// 会话 ID
    pub session_id: String,
    /// 追踪上下文
    pub trace_context: Option<TraceContext>,
}

/// 追踪上下文（用于分布式追踪）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TraceContext {
    /// 追踪 ID
    pub trace_id: String,
    /// 跨度 ID
    pub span_id: String,
    /// 父跨度 ID
    pub parent_span_id: Option<String>,
    /// 追踪标志
    pub trace_flags: u8,
}

impl AuditEvent {
    /// 创建审计事件（推荐使用构建器模式）
    ///
    /// # 推荐方式
    /// 使用 `AuditEventBuilder` 进行链式构建：
    /// ```rust
    /// # use dbnexus::audit::{AuditEvent, AuditOperation, AuditSeverity};
    /// # fn example() -> Result<(), dbnexus::audit::BuildError> {
    /// let event = AuditEvent::builder()
    ///     .operation(AuditOperation::Create)
    ///     .entity_type("users")
    ///     .entity_id("1")
    ///     .user_id("admin")
    ///     .user_role("admin")
    ///     .client_ip("127.0.0.1")
    ///     .severity(AuditSeverity::High)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # 简单方式
    /// 使用快捷方法：
    /// ```rust
    /// # use dbnexus::audit::{AuditEvent, AuditSeverity};
    /// AuditEvent::create("users", "1", "admin")
    ///     .with_severity(AuditSeverity::High);
    /// ```
    pub fn new(
        operation: AuditOperation,
        entity_type: &str,
        entity_id: &str,
        user_id: &str,
        user_role: &str,
        client_ip: &str,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            operation,
            entity_type: entity_type.to_string(),
            entity_id: entity_id.to_string(),
            user_id: user_id.to_string(),
            user_role: user_role.to_string(),
            client_ip: client_ip.to_string(),
            severity: AuditSeverity::Info,
            result: AuditStatus::Success,
            error_message: None,
            before_value: None,
            after_value: None,
            extra: None,
            request_id: Uuid::new_v4().to_string(),
            session_id: String::new(),
            trace_context: None,
        }
    }

    /// 获取构建器
    pub fn builder() -> AuditEventBuilder {
        AuditEventBuilder::new()
    }

    /// 创建带错误的审计事件
    pub fn with_error(
        operation: AuditOperation,
        entity_type: &str,
        entity_id: &str,
        user_id: &str,
        error: &str,
    ) -> Self {
        let mut event = Self::new(operation, entity_type, entity_id, user_id, "", "");
        event.error_message = Some(error.to_string());
        event.result = AuditStatus::Failure;
        event.severity = AuditSeverity::High;
        event
    }

    /// 设置追踪上下文
    pub fn with_trace_context(mut self, trace_id: &str, span_id: &str) -> Self {
        self.trace_context = Some(TraceContext {
            trace_id: trace_id.to_string(),
            span_id: span_id.to_string(),
            parent_span_id: None,
            trace_flags: 1,
        });
        self
    }

    /// 创建操作事件
    pub fn create(entity_type: &str, entity_id: &str, user_id: &str) -> Self {
        Self::new(AuditOperation::Create, entity_type, entity_id, user_id, "", "")
    }

    /// 读取操作事件
    pub fn read(entity_type: &str, entity_id: &str, user_id: &str) -> Self {
        Self::new(AuditOperation::Read, entity_type, entity_id, user_id, "", "")
    }

    /// 更新操作事件
    pub fn update(
        entity_type: &str,
        entity_id: &str,
        user_id: &str,
        before: Option<String>,
        after: Option<String>,
    ) -> Self {
        let mut event = Self::new(AuditOperation::Update, entity_type, entity_id, user_id, "", "");
        event.before_value = before;
        event.after_value = after;
        event
    }

    /// 删除操作事件
    pub fn delete(entity_type: &str, entity_id: &str, user_id: &str) -> Self {
        Self::new(AuditOperation::Delete, entity_type, entity_id, user_id, "", "")
    }

    /// 设置用户信息
    pub fn with_user(mut self, role: &str, client_ip: &str) -> Self {
        self.user_role = role.to_string();
        self.client_ip = client_ip.to_string();
        self
    }

    /// 设置结果
    pub fn with_result(mut self, result: AuditStatus) -> Self {
        self.result = result;
        self
    }

    /// 设置严重级别
    pub fn with_severity(mut self, severity: AuditSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// 设置附加信息
    pub fn with_extra(mut self, extra: &str) -> Self {
        self.extra = Some(extra.to_string());
        self
    }

    /// 设置变更前值
    pub fn with_before_value(mut self, value: &str) -> Self {
        self.before_value = Some(value.to_string());
        self
    }

    /// 设置变更后值
    pub fn with_after_value(mut self, value: &str) -> Self {
        self.after_value = Some(value.to_string());
        self
    }

    /// 设置请求 ID
    pub fn with_request_id(mut self, request_id: &str) -> Self {
        self.request_id = request_id.to_string();
        self
    }

    /// 设置会话 ID
    pub fn with_session_id(mut self, session_id: &str) -> Self {
        self.session_id = session_id.to_string();
        self
    }

    /// 转换为 JSON 字符串
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// 从 JSON 字符串解析
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// 敏感数据脱敏最大递归深度
const MAX_SANITIZE_DEPTH: usize = 10;

/// 默认敏感字段列表
fn default_sensitive_fields() -> Vec<String> {
    vec![
        "password".to_string(),
        "token".to_string(),
        "secret".to_string(),
        "key".to_string(),
        "credential".to_string(),
        "api_key".to_string(),
        "access_token".to_string(),
        "refresh_token".to_string(),
        "private_key".to_string(),
        "credit_card".to_string(),
        "ssn".to_string(),
        "social_security".to_string(),
    ]
}

/// 递归脱敏 JSON 值
///
/// # Arguments
///
/// * `value` - JSON 值
/// * `sensitive_fields` - 敏感字段列表
/// * `depth` - 当前递归深度
///
/// # Returns
///
/// 脱敏后的 JSON 值
fn sanitize_json_object(value: &serde_json::Value, sensitive_fields: &[String], depth: usize) -> serde_json::Value {
    // 防止栈溢出：超过最大深度时返回占位符
    if depth > MAX_SANITIZE_DEPTH {
        return serde_json::Value::String("[MAX_DEPTH_EXCEEDED]".to_string());
    }

    match value {
        serde_json::Value::Object(obj) => {
            let mut new_obj = serde_json::Map::new();
            for (key, val) in obj {
                // 检查当前字段名是否为敏感字段（不区分大小写）
                let is_sensitive = sensitive_fields
                    .iter()
                    .any(|f| key.to_lowercase().contains(&f.to_lowercase()));

                if is_sensitive {
                    // 敏感字段直接替换为 [REDACTED]
                    new_obj.insert(key.clone(), serde_json::Value::String("[REDACTED]".to_string()));
                } else {
                    // 非敏感字段递归处理
                    new_obj.insert(key.clone(), sanitize_json_object(val, sensitive_fields, depth + 1));
                }
            }
            serde_json::Value::Object(new_obj)
        }
        serde_json::Value::Array(arr) => {
            // 数组中的每个元素递归处理
            serde_json::Value::Array(
                arr.iter()
                    .map(|v| sanitize_json_object(v, sensitive_fields, depth + 1))
                    .collect(),
            )
        }
        // 其他类型（字符串、数字、布尔、null）直接克隆
        other => other.clone(),
    }
}

impl AuditEvent {
    /// 对 JSON 值进行敏感数据脱敏
    ///
    /// 脱敏策略：
    /// - 递归遍历 JSON 对象和数组
    /// - 识别 JSON 中的敏感字段（包括嵌套字段）
    /// - 将敏感字段的值替换为 "[REDACTED]"
    /// - 支持自定义敏感字段列表
    /// - 最大递归深度为 10 层，防止栈溢出
    ///
    /// # Arguments
    ///
    /// * `value` - 原始 JSON 字符串
    /// * `sensitive_fields` - 敏感字段列表（默认包含常见敏感字段）
    ///
    /// # Returns
    ///
    /// 脱敏后的 JSON 字符串
    pub fn sanitize_value(value: &str, sensitive_fields: Option<Vec<String>>) -> String {
        let fields = sensitive_fields.unwrap_or_else(default_sensitive_fields);

        // 尝试解析 JSON
        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(value) {
            let sanitized = sanitize_json_object(&json_value, &fields, 0);
            serde_json::to_string(&sanitized).unwrap_or_else(|_| "***SANITIZATION_ERROR***".to_string())
        } else {
            // 非 JSON 值，检查是否包含敏感关键字
            let lower = value.to_lowercase();
            for field in &fields {
                // 检查 JSON 格式: "field":
                if lower.contains(&format!("\"{}\":", field)) || lower.contains(&format!("\"{}\" :", field)) {
                    return "***REDACTED***".to_string();
                }
                // 检查非 JSON 格式: field:
                if lower.contains(&format!("{}:", field)) {
                    return "***REDACTED***".to_string();
                }
            }
            value.to_string()
        }
    }

    /// 创建脱敏后的审计事件副本（用于日志记录）
    ///
    /// 返回一个副本，其中敏感数据已被脱敏
    pub fn sanitized(&self) -> Self {
        let sensitive_fields = vec![
            "password".to_string(),
            "token".to_string(),
            "secret".to_string(),
            "key".to_string(),
            "credential".to_string(),
        ];

        let mut sanitized = self.clone();
        if let Some(ref mut before) = sanitized.before_value {
            *before = Self::sanitize_value(before, Some(sensitive_fields.clone()));
        }
        if let Some(ref mut after) = sanitized.after_value {
            *after = Self::sanitize_value(after, Some(sensitive_fields.clone()));
        }
        if let Some(ref mut extra) = sanitized.extra {
            *extra = Self::sanitize_value(extra, Some(sensitive_fields.clone()));
        }
        sanitized
    }
}

/// 审计配置
#[derive(Debug, Clone)]
pub struct AuditConfig {
    /// 是否启用审计
    pub enabled: bool,
    /// 审计日志存储路径
    pub storage_path: Option<String>,
    /// 是否同步写入（影响性能但更安全）
    pub sync_write: bool,
    /// 日志文件最大大小（字节）
    pub max_file_size: u64,
    /// 保留日志文件数
    pub retention_count: u32,
    /// 敏感字段列表（记录时脱敏）
    pub sensitive_fields: Vec<String>,
    /// 需要高危告警的操作
    pub alert_operations: Vec<AuditOperation>,
    /// 高危操作的严重级别
    pub alert_severity: AuditSeverity,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            storage_path: None,
            sync_write: false,
            max_file_size: 10 * 1024 * 1024, // 10MB
            retention_count: 7,
            sensitive_fields: vec![
                "password".to_string(),
                "token".to_string(),
                "secret".to_string(),
                "api_key".to_string(),
            ],
            alert_operations: vec![
                AuditOperation::Delete,
                AuditOperation::PermissionChange,
                AuditOperation::ConfigChange,
            ],
            alert_severity: AuditSeverity::High,
        }
    }
}

/// 审计存储后端特质
#[async_trait]
pub trait AuditStorage: Send + Sync {
    /// 存储审计事件
    async fn store(&self, event: &AuditEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// 查询审计事件
    async fn query(
        &self,
        filters: &AuditQueryFilters,
    ) -> Result<Vec<AuditEvent>, Box<dyn std::error::Error + Send + Sync>>;

    /// 清理旧日志
    async fn cleanup(&self, before: &DateTime<Utc>) -> Result<u64, Box<dyn std::error::Error + Send + Sync>>;
}

/// 审计查询过滤器
#[derive(Debug, Default)]
pub struct AuditQueryFilters {
    /// 用户 ID
    pub user_id: Option<String>,
    /// 实体类型
    pub entity_type: Option<String>,
    /// 操作类型
    pub operation: Option<AuditOperation>,
    /// 开始时间
    pub start_time: Option<DateTime<Utc>>,
    /// 结束时间
    pub end_time: Option<DateTime<Utc>>,
    /// 严重级别
    pub severity: Option<AuditSeverity>,
    /// 结果
    pub result: Option<AuditStatus>,
}

/// 内存审计存储（默认实现）
#[derive(Debug)]
pub struct MemoryAuditStorage {
    events: Mutex<Vec<AuditEvent>>,
    max_events: usize,
    dropped_count: AtomicU64,
}

impl Default for MemoryAuditStorage {
    fn default() -> Self {
        Self::new(10000) // 默认最多存储 10000 条审计日志
    }
}

impl MemoryAuditStorage {
    /// 创建内存审计存储
    pub fn new(max_events: usize) -> Self {
        Self {
            events: Mutex::new(Vec::with_capacity(max_events)),
            max_events: if max_events == 0 { 10000 } else { max_events },
            dropped_count: AtomicU64::new(0),
        }
    }

    /// 获取已丢弃的事件数量
    pub fn dropped_count(&self) -> u64 {
        self.dropped_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 获取当前事件数量
    pub async fn event_count(&self) -> usize {
        let events = self.events.lock().await;
        events.len()
    }
}

#[async_trait]
impl AuditStorage for MemoryAuditStorage {
    async fn store(&self, event: &AuditEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut events = self.events.lock().await;

        // 如果超过最大容量，移除最旧的
        if events.len() >= self.max_events {
            events.remove(0);
            self.dropped_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        events.push(event.clone());

        Ok(())
    }

    async fn query(
        &self,
        filters: &AuditQueryFilters,
    ) -> Result<Vec<AuditEvent>, Box<dyn std::error::Error + Send + Sync>> {
        let events = self.events.lock().await;

        let mut result = events.clone();

        if let Some(user_id) = &filters.user_id {
            result.retain(|e| e.user_id == *user_id);
        }

        if let Some(entity_type) = &filters.entity_type {
            result.retain(|e| e.entity_type == *entity_type);
        }

        if let Some(operation) = &filters.operation {
            result.retain(|e| e.operation == *operation);
        }

        if let Some(start_time) = &filters.start_time {
            result.retain(|e| e.timestamp >= *start_time);
        }

        if let Some(end_time) = &filters.end_time {
            result.retain(|e| e.timestamp <= *end_time);
        }

        if let Some(severity) = &filters.severity {
            result.retain(|e| e.severity == *severity);
        }

        if let Some(result_status) = &filters.result {
            result.retain(|e| e.result == *result_status);
        }

        Ok(result)
    }

    async fn cleanup(&self, before: &DateTime<Utc>) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        let mut events = self.events.lock().await;
        let before_count = events.len();
        events.retain(|e| e.timestamp > *before);
        let after_count = events.len();
        Ok((before_count - after_count) as u64)
    }
}

/// 审计告警回调类型
type AuditAlertCallback = Arc<dyn Fn(&AuditEvent) + Send + Sync>;

/// 审计日志器
pub struct AuditLogger {
    /// 配置
    config: AuditConfig,
    /// 存储后端
    storage: Arc<dyn AuditStorage>,
    /// 告警回调
    alert_callback: Option<AuditAlertCallback>,
}

impl AuditLogger {
    /// 创建带默认配置的审计日志器
    pub fn new() -> Self {
        Self::with_default_storage()
    }

    /// 获取构建器
    pub fn builder() -> AuditLoggerBuilder {
        AuditLoggerBuilder::new()
    }

    /// 创建带自定义配置和存储的审计日志器
    pub fn with_config(config: AuditConfig, storage: Arc<dyn AuditStorage>) -> Self {
        Self {
            config,
            storage,
            alert_callback: None,
        }
    }

    /// 创建带默认配置的审计日志器
    pub fn with_default_storage() -> Self {
        Self::with_config(AuditConfig::default(), Arc::new(MemoryAuditStorage::new(10000)))
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditLogger {
    /// 设置告警回调
    pub fn set_alert_callback<F>(&mut self, callback: F)
    where
        F: Fn(&AuditEvent) + Send + Sync + 'static,
    {
        self.alert_callback = Some(Arc::new(callback));
    }

    /// 记录审计事件
    pub async fn log(&self, event: AuditEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.config.enabled {
            return Ok(());
        }

        // 脱敏处理
        let event = self.sanitize_event(event);

        // 存储事件
        self.storage.store(&event).await?;

        // 检查是否需要告警
        if self.should_alert(&event) {
            self.trigger_alert(&event);
        }

        Ok(())
    }

    /// 记录创建操作
    pub async fn log_create(
        &self,
        entity_type: &str,
        entity_id: &str,
        user_id: &str,
        value: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let event = AuditEvent::create(entity_type, entity_id, user_id);
        let event = match value {
            Some(ref v) => event.with_after_value(v),
            None => event,
        };
        self.log(event).await
    }

    /// 记录读取操作
    pub async fn log_read(
        &self,
        entity_type: &str,
        entity_id: &str,
        user_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let event = AuditEvent::read(entity_type, entity_id, user_id);
        self.log(event).await
    }

    /// 记录更新操作
    pub async fn log_update(
        &self,
        entity_type: &str,
        entity_id: &str,
        user_id: &str,
        before: Option<String>,
        after: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let event = AuditEvent::update(entity_type, entity_id, user_id, before, after);
        self.log(event).await
    }

    /// 记录删除操作
    pub async fn log_delete(
        &self,
        entity_type: &str,
        entity_id: &str,
        user_id: &str,
        before: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let event = AuditEvent::delete(entity_type, entity_id, user_id).with_severity(AuditSeverity::High);
        let event = match before {
            Some(ref v) => event.with_before_value(v),
            None => event,
        };
        self.log(event).await
    }

    /// 查询审计日志
    pub async fn query(
        &self,
        filters: &AuditQueryFilters,
    ) -> Result<Vec<AuditEvent>, Box<dyn std::error::Error + Send + Sync>> {
        self.storage.query(filters).await
    }

    /// 清理旧日志
    pub async fn cleanup(&self, days: i64) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        let delta = chrono::Duration::try_days(days).ok_or("Invalid date calculation")?;
        let before = Utc::now().checked_sub_signed(delta).ok_or("Invalid date calculation")?;
        self.storage.cleanup(&before).await
    }

    /// 脱敏处理
    fn sanitize_event(&self, mut event: AuditEvent) -> AuditEvent {
        let sanitize_value = |value: Option<String>| -> Option<String> {
            if let Some(v) = value {
                let mut result = v;
                for field in &self.config.sensitive_fields {
                    let replacement = format!("***REDACTED_{}***", field.to_uppercase());

                    // 1. JSON 格式: "field":
                    result = result.replace(&format!(r#""{}":"#, field), &format!(r#""{}":"#, &replacement));

                    // 2. 非 JSON 格式: field:
                    result = result.replace(&format!(r#"{}:"#, field), &format!(r#"{}:"#, &replacement));

                    // 3. 嵌套字段 (如 user.password)
                    if field.contains('.') {
                        let parts: Vec<&str> = field.split('.').collect();
                        if parts.len() >= 2 {
                            let nested_pattern = format!(r#""{}""#, field);
                            result = result.replace(&nested_pattern, &format!(r#""{}""#, &replacement));
                        }
                    }

                    // 4. 通用 Base64 值检测和脱敏（不依赖 JSON 结构）
                    result = Self::sanitize_generic_base64(&result, field, &replacement);

                    // 5. JSON 数组中的敏感字段脱敏
                    result = Self::sanitize_json_arrays(&result, field, &replacement);
                }
                Some(result)
            } else {
                None
            }
        };

        event.before_value = sanitize_value(event.before_value);
        event.after_value = sanitize_value(event.after_value);
        event.extra = sanitize_value(event.extra);

        event
    }

    /// 通用 Base64 值脱敏（不依赖 JSON 结构）
    fn sanitize_generic_base64(value: &str, field: &str, replacement: &str) -> String {
        let mut result = value.to_string();

        // 尝试解析为 JSON，如果失败仍然尝试脱敏
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(value) {
            // 如果是对象
            if let Some(obj) = json_val.as_object() {
                let mut modified = false;
                let mut new_obj = serde_json::Map::new();
                let underscore_str = String::from("_");
                let field_with_underscore = format!("{}{}", underscore_str, field);

                for (k, v) in obj {
                    // 检查字段名匹配
                    if k == field || k.contains(&field_with_underscore) {
                        let redacted_key = format!("{}{}redacted", k, underscore_str);
                        new_obj.insert(redacted_key, serde_json::Value::String(replacement.to_string()));
                        modified = true;
                    } else if v.is_string() {
                        let s = v.as_str().unwrap_or("");
                        // 检测并脱敏 Base64 编码
                        if Self::is_base64(s) {
                            new_obj.insert(k.clone(), serde_json::Value::String(replacement.to_string()));
                            modified = true;
                        } else {
                            new_obj.insert(k.clone(), v.clone());
                        }
                    } else {
                        new_obj.insert(k.clone(), v.clone());
                    }
                }

                if modified {
                    result = serde_json::to_string(&new_obj).unwrap_or(result);
                }
            }
            // 如果是数组，处理数组中的每个元素
            else if let Some(arr) = json_val.as_array() {
                let mut modified = false;
                let mut new_arr = Vec::new();

                for item in arr {
                    if let Some(obj) = item.as_object() {
                        let mut new_obj = serde_json::Map::new();
                        for (k, v) in obj {
                            let should_mask = k == field
                                || k.contains(field)
                                || (v.is_string() && Self::is_base64(v.as_str().unwrap_or("")));

                            if should_mask {
                                new_obj.insert(k.clone(), serde_json::Value::String(replacement.to_string()));
                                modified = true;
                            } else {
                                new_obj.insert(k.clone(), v.clone());
                            }
                        }
                        new_arr.push(serde_json::Value::Object(new_obj));
                    } else {
                        new_arr.push(item.clone());
                    }
                }

                if modified {
                    result = serde_json::to_string(&new_arr).unwrap_or(result);
                }
            }
        }

        result
    }

    /// 脱敏 JSON 数组中的敏感字段
    fn sanitize_json_arrays(value: &str, field: &str, replacement: &str) -> String {
        // 检测数组模式 [ {"field": "value"}, ... ]
        let array_pattern = format!(r#"{{"{}","#, field);
        if !value.contains(&array_pattern) {
            return value.to_string();
        }

        // 尝试解析并脱敏
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(value) {
            if let Some(arr) = json_val.as_array() {
                let mut modified = false;
                let mut new_arr = Vec::new();

                for item in arr {
                    if let Some(obj) = item.as_object() {
                        let mut new_obj = serde_json::Map::new();
                        for (k, v) in obj {
                            if k == field {
                                new_obj.insert(k.clone(), serde_json::Value::String(replacement.to_string()));
                                modified = true;
                            } else {
                                new_obj.insert(k.clone(), v.clone());
                            }
                        }
                        new_arr.push(serde_json::Value::Object(new_obj));
                    } else {
                        new_arr.push(item.clone());
                    }
                }

                if modified {
                    return serde_json::to_string(&new_arr).unwrap_or(value.to_string());
                }
            }
        }

        value.to_string()
    }

    /// 检测字符串是否为有效的 Base64 编码
    fn is_base64(s: &str) -> bool {
        if s.len() % 4 != 0 || s.is_empty() {
            return false;
        }
        let valid_chars: std::collections::HashSet<char> =
            "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
                .chars()
                .collect();
        s.chars().all(|c| valid_chars.contains(&c) || c == '=')
    }

    /// 检查是否需要告警
    fn should_alert(&self, event: &AuditEvent) -> bool {
        if !self.config.enabled {
            return false;
        }

        self.config.alert_operations.contains(&event.operation)
    }

    /// 触发告警
    fn trigger_alert(&self, event: &AuditEvent) {
        if let Some(callback) = &self.alert_callback {
            callback(event);
        }
    }
}

/// 审计上下文（用于在请求中传递审计信息）
#[derive(Debug, Default, Clone)]
pub struct AuditContext {
    /// 用户 ID
    pub user_id: String,
    /// 用户角色
    pub user_role: String,
    /// 客户端 IP
    pub client_ip: String,
    /// 请求 ID
    pub request_id: String,
    /// 会话 ID
    pub session_id: String,
}

impl AuditContext {
    /// 创建审计上下文
    pub fn new(user_id: &str, role: &str, client_ip: &str) -> Self {
        Self {
            user_id: user_id.to_string(),
            user_role: role.to_string(),
            client_ip: client_ip.to_string(),
            request_id: Uuid::new_v4().to_string(),
            session_id: String::new(),
        }
    }

    /// 设置请求 ID
    pub fn with_request_id(mut self, request_id: &str) -> Self {
        self.request_id = request_id.to_string();
        self
    }

    /// 设置会话 ID
    pub fn with_session_id(mut self, session_id: &str) -> Self {
        self.session_id = session_id.to_string();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[tokio::test]
    async fn test_audit_event_creation() {
        let event = AuditEvent::create("users", "1", "admin");
        assert_eq!(event.operation, AuditOperation::Create);
        assert_eq!(event.entity_type, "users");
        assert_eq!(event.entity_id, "1");
        assert_eq!(event.user_id, "admin");
    }

    #[tokio::test]
    async fn test_audit_event_update() {
        let before = r#"{"name": "old"}"#;
        let after = r#"{"name": "new"}"#;
        let event = AuditEvent::update("users", "1", "admin", Some(before.to_string()), Some(after.to_string()));

        assert_eq!(event.operation, AuditOperation::Update);
        assert_eq!(event.before_value, Some(before.to_string()));
        assert_eq!(event.after_value, Some(after.to_string()));
    }

    #[tokio::test]
    async fn test_audit_event_setters_and_default_storage() {
        let event = AuditEvent::create("users", "1", "admin")
            .with_user("role", "127.0.0.1")
            .with_result(AuditStatus::Failure)
            .with_severity(AuditSeverity::High)
            .with_extra("x")
            .with_before_value("b")
            .with_after_value("a")
            .with_request_id("r")
            .with_session_id("s");

        assert_eq!(event.user_role, "role");
        assert_eq!(event.client_ip, "127.0.0.1");
        assert_eq!(event.result, AuditStatus::Failure);
        assert_eq!(event.severity, AuditSeverity::High);
        assert_eq!(event.extra.as_deref(), Some("x"));
        assert_eq!(event.before_value.as_deref(), Some("b"));
        assert_eq!(event.after_value.as_deref(), Some("a"));
        assert_eq!(event.request_id, "r");
        assert_eq!(event.session_id, "s");

        let storage = MemoryAuditStorage::default();
        storage.store(&event).await.expect("Storage operation should succeed");
        assert_eq!(storage.event_count().await, 1);
    }

    #[test]
    fn test_audit_event_json_roundtrip() {
        let event = AuditEvent::create("users", "1", "admin")
            .with_user("role", "127.0.0.1")
            .with_result(AuditStatus::Success)
            .with_severity(AuditSeverity::Medium)
            .with_extra("x")
            .with_before_value("b")
            .with_after_value("a")
            .with_request_id("r")
            .with_session_id("s");

        let json = event.to_json().unwrap();
        let parsed = AuditEvent::from_json(&json).unwrap();

        assert_eq!(parsed.operation, event.operation);
        assert_eq!(parsed.entity_type, event.entity_type);
        assert_eq!(parsed.entity_id, event.entity_id);
        assert_eq!(parsed.user_id, event.user_id);
        assert_eq!(parsed.user_role, event.user_role);
        assert_eq!(parsed.client_ip, event.client_ip);
        assert_eq!(parsed.result, event.result);
        assert_eq!(parsed.severity, event.severity);
        assert_eq!(parsed.extra, event.extra);
        assert_eq!(parsed.before_value, event.before_value);
        assert_eq!(parsed.after_value, event.after_value);
        assert_eq!(parsed.request_id, event.request_id);
        assert_eq!(parsed.session_id, event.session_id);
    }

    #[tokio::test]
    async fn test_audit_logger_helpers_and_alert_disabled() {
        let storage = Arc::new(MemoryAuditStorage::new(10));

        let logger = AuditLogger::with_config(
            AuditConfig {
                enabled: false,
                alert_operations: vec![AuditOperation::Delete],
                ..Default::default()
            },
            storage.clone(),
        );

        logger.log_create("t", "1", "u", Some("v".to_string())).await.unwrap();
        logger.log_read("t", "1", "u").await.unwrap();
        logger
            .log_update("t", "1", "u", Some("b".to_string()), Some("a".to_string()))
            .await
            .unwrap();
        logger.log_delete("t", "1", "u", None).await.unwrap();

        assert_eq!(storage.event_count().await, 0);
        assert!(!logger.should_alert(&AuditEvent::delete("t", "1", "u")));
    }

    #[tokio::test]
    async fn test_audit_log_create_none_branch_and_cleanup_success() {
        let storage = Arc::new(MemoryAuditStorage::new(10));
        let logger = AuditLogger::with_config(AuditConfig::default(), storage.clone());

        logger.log_create("t", "1", "u", None).await.unwrap();

        let mut old = AuditEvent::create("t", "2", "u");
        old.timestamp = Utc::now() - chrono::Duration::days(2);
        logger.log(old).await.unwrap();

        let removed = logger.cleanup(1).await.unwrap();
        assert_eq!(removed, 1);
        assert_eq!(storage.event_count().await, 1);
    }

    #[tokio::test]
    async fn test_audit_sanitize_base64_non_string_values() {
        let storage = Arc::new(MemoryAuditStorage::new(10));
        let logger = AuditLogger::with_config(AuditConfig::default(), storage);

        let event = AuditEvent::create("t", "1", "u").with_after_value(r#"{"count":1,"name":"x"}"#);
        logger.log(event).await.unwrap();

        let results = logger.query(&AuditQueryFilters::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].after_value.as_ref().unwrap().contains("count"));
    }

    #[tokio::test]
    async fn test_audit_logger() {
        let storage = Arc::new(MemoryAuditStorage::new(100));
        let config = AuditConfig::default();
        let logger = AuditLogger::with_config(config, storage);

        let event = AuditEvent::create("users", "1", "admin");
        logger.log(event).await.unwrap();

        let filters = AuditQueryFilters {
            entity_type: Some("users".to_string()),
            ..Default::default()
        };
        let results = logger.query(&filters).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entity_type, "users");
    }

    #[tokio::test]
    async fn test_audit_sanitization() {
        let storage = Arc::new(MemoryAuditStorage::new(100));
        let config = AuditConfig::default();
        let logger = AuditLogger::with_config(config, storage);

        let event =
            AuditEvent::create("users", "1", "admin").with_after_value(r#"{"password": "secret123", "name": "test"}"#);

        logger.log(event).await.unwrap();

        let filters = AuditQueryFilters::default();
        let results = logger.query(&filters).await.unwrap();
        let after_value = results[0].after_value.as_ref().unwrap();

        // 密码应该被脱敏
        assert!(after_value.contains("***REDACTED_PASSWORD***"));
        assert!(after_value.contains("name"));
    }

    #[tokio::test]
    async fn test_audit_context() {
        let ctx = AuditContext::new("user123", "admin", "192.168.1.1");
        assert_eq!(ctx.user_id, "user123");
        assert_eq!(ctx.user_role, "admin");
        assert_eq!(ctx.client_ip, "192.168.1.1");
        assert!(!ctx.request_id.is_empty());
    }

    #[test]
    fn test_audit_enum_display_and_defaults() {
        assert_eq!(AuditOperation::Create.to_string(), "CREATE");
        assert_eq!(AuditOperation::Read.to_string(), "READ");
        assert_eq!(AuditOperation::Update.to_string(), "UPDATE");
        assert_eq!(AuditOperation::Delete.to_string(), "DELETE");
        assert_eq!(AuditOperation::Login.to_string(), "LOGIN");
        assert_eq!(AuditOperation::Logout.to_string(), "LOGOUT");
        assert_eq!(AuditOperation::PermissionChange.to_string(), "PERMISSION_CHANGE");
        assert_eq!(AuditOperation::ConfigChange.to_string(), "CONFIG_CHANGE");
        assert_eq!(AuditOperation::Other("custom_op".to_string()).to_string(), "CUSTOM_OP");
        assert_eq!(AuditOperation::default().to_string(), "UNKNOWN");

        assert_eq!(AuditSeverity::Info.to_string(), "INFO");
        assert_eq!(AuditSeverity::Low.to_string(), "LOW");
        assert_eq!(AuditSeverity::Medium.to_string(), "MEDIUM");
        assert_eq!(AuditSeverity::High.to_string(), "HIGH");
        assert_eq!(AuditSeverity::Critical.to_string(), "CRITICAL");

        assert_eq!(AuditStatus::Success.to_string(), "SUCCESS");
        assert_eq!(AuditStatus::Failure.to_string(), "FAILURE");
        assert_eq!(AuditStatus::Partial.to_string(), "PARTIAL");
        assert_eq!(AuditStatus::Unknown.to_string(), "UNKNOWN");
    }

    #[tokio::test]
    async fn test_memory_storage_overflow_and_dropped_count() {
        let storage = MemoryAuditStorage::new(1);
        assert_eq!(storage.dropped_count(), 0);

        let event1 = AuditEvent::create("users", "1", "admin");
        let event2 = AuditEvent::create("users", "2", "admin");

        storage.store(&event1).await.unwrap();
        storage.store(&event2).await.unwrap();

        assert_eq!(storage.event_count().await, 1);
        assert_eq!(storage.dropped_count(), 1);

        let results = storage.query(&AuditQueryFilters::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entity_id, "2");
    }

    #[tokio::test]
    async fn test_audit_query_filters_all_fields_and_cleanup() {
        let storage = Arc::new(MemoryAuditStorage::new(100));
        let logger = AuditLogger::with_config(AuditConfig::default(), storage.clone());

        let now = Utc::now();
        let mut e1 = AuditEvent::create("users", "1", "u1")
            .with_user("admin", "10.0.0.1")
            .with_severity(AuditSeverity::Low)
            .with_result(AuditStatus::Success)
            .with_request_id("r1")
            .with_session_id("s1");
        e1.timestamp = now - chrono::Duration::minutes(10);

        let mut e2 = AuditEvent::delete("orders", "9", "u2")
            .with_user("system", "10.0.0.2")
            .with_severity(AuditSeverity::High)
            .with_result(AuditStatus::Failure);
        e2.timestamp = now;

        logger.log(e1.clone()).await.unwrap();
        logger.log(e2.clone()).await.unwrap();

        let filters = AuditQueryFilters {
            user_id: Some("u2".to_string()),
            entity_type: Some("orders".to_string()),
            operation: Some(AuditOperation::Delete),
            start_time: Some(now - chrono::Duration::minutes(5)),
            end_time: Some(now + chrono::Duration::minutes(1)),
            severity: Some(AuditSeverity::High),
            result: Some(AuditStatus::Failure),
        };

        let filtered = logger.query(&filters).await.unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].entity_id, "9");

        let removed = storage.cleanup(&(now - chrono::Duration::minutes(1))).await.unwrap();
        assert_eq!(removed, 1);
        assert_eq!(storage.event_count().await, 1);
    }

    #[tokio::test]
    async fn test_audit_logger_disabled_and_alert_callback() {
        let storage = Arc::new(MemoryAuditStorage::new(100));

        let disabled_logger = AuditLogger::with_config(
            AuditConfig {
                enabled: false,
                ..Default::default()
            },
            storage.clone(),
        );

        disabled_logger
            .log(AuditEvent::create("users", "1", "admin"))
            .await
            .unwrap();
        assert_eq!(storage.event_count().await, 0);

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let mut logger = AuditLogger::with_default_storage();
        logger.set_alert_callback(move |_event| {
            called_clone.store(true, Ordering::SeqCst);
        });

        logger
            .log_delete("users", "2", "admin", Some(r#"{\"password\":\"x\"}"#.to_string()))
            .await
            .unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_audit_sanitization_base64_and_nested_field() {
        let storage = Arc::new(MemoryAuditStorage::new(100));
        let mut config = AuditConfig::default();
        config.sensitive_fields.push("user.password".to_string());
        let logger = AuditLogger::with_config(config, storage);

        let after_value = r#"{"password":"p","_password":"p2","data":"c2VjcmV0","user.password":"v"}"#;
        let event = AuditEvent::create("users", "1", "admin").with_after_value(after_value);
        logger.log(event).await.unwrap();

        let results = logger.query(&AuditQueryFilters::default()).await.unwrap();
        let stored = results[0].after_value.as_ref().unwrap();
        assert!(stored.contains("***REDACTED_PASSWORD***"));
        assert!(stored.contains("_password_redacted"));
        assert!(stored.contains(r#""data":"***REDACTED_PASSWORD***""#));
        assert!(stored.contains("***REDACTED_USER.PASSWORD***"));

        assert!(!AuditLogger::is_base64(""));
        assert!(!AuditLogger::is_base64("abc"));
        assert!(!AuditLogger::is_base64("!!!!"));
        assert!(AuditLogger::is_base64("c2VjcmV0"));
    }

    #[tokio::test]
    async fn test_audit_logger_cleanup_invalid_date_calculation() {
        let storage = Arc::new(MemoryAuditStorage::new(100));
        let logger = AuditLogger::with_config(AuditConfig::default(), storage);
        let result = logger.cleanup(i64::MAX).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_audit_context_setters() {
        let ctx = AuditContext::new("u", "r", "ip")
            .with_request_id("req")
            .with_session_id("sess");
        assert_eq!(ctx.request_id, "req");
        assert_eq!(ctx.session_id, "sess");
    }

    #[test]
    fn test_sanitize_value_nested_objects() {
        // 测试嵌套对象中的敏感字段脱敏
        let nested_json = r#"{
            "name": "test",
            "password": "secret123",
            "user": {
                "name": "john",
                "password": "nested_secret",
                "profile": {
                    "token": "deep_token",
                    "age": 30
                }
            }
        }"#;

        let sanitized = AuditEvent::sanitize_value(nested_json, None);
        let parsed: serde_json::Value = serde_json::from_str(&sanitized).unwrap();

        // 验证第一层敏感字段被脱敏
        assert_eq!(parsed["password"], "[REDACTED]");
        // 验证嵌套对象中的敏感字段被脱敏
        assert_eq!(parsed["user"]["password"], "[REDACTED]");
        // 验证深层嵌套的敏感字段被脱敏
        assert_eq!(parsed["user"]["profile"]["token"], "[REDACTED]");
        // 验证非敏感字段保持不变
        assert_eq!(parsed["name"], "test");
        assert_eq!(parsed["user"]["name"], "john");
        assert_eq!(parsed["user"]["profile"]["age"], 30);
    }

    #[test]
    fn test_sanitize_value_nested_arrays() {
        // 测试数组中的嵌套对象敏感字段脱敏
        let array_json = r#"{
            "users": [
                {"name": "user1", "password": "pass1"},
                {"name": "user2", "password": "pass2", "token": "tok2"}
            ],
            "count": 2
        }"#;

        let sanitized = AuditEvent::sanitize_value(array_json, None);
        let parsed: serde_json::Value = serde_json::from_str(&sanitized).unwrap();

        // 验证数组中每个对象的敏感字段都被脱敏
        assert_eq!(parsed["users"][0]["password"], "[REDACTED]");
        assert_eq!(parsed["users"][1]["password"], "[REDACTED]");
        assert_eq!(parsed["users"][1]["token"], "[REDACTED]");
        // 验证非敏感字段保持不变
        assert_eq!(parsed["users"][0]["name"], "user1");
        assert_eq!(parsed["users"][1]["name"], "user2");
        assert_eq!(parsed["count"], 2);
    }

    #[test]
    fn test_sanitize_value_max_depth() {
        // 构建一个超过最大深度的嵌套 JSON（使用非敏感字段名）
        let mut deep_value = serde_json::Value::Object(serde_json::Map::new());
        deep_value
            .as_object_mut()
            .unwrap()
            .insert("deep_data".to_string(), serde_json::Value::String("value".to_string()));

        for i in 0..12 {
            let mut new_obj = serde_json::Map::new();
            new_obj.insert(format!("level{}", i), deep_value);
            deep_value = serde_json::Value::Object(new_obj);
        }
        let json_str = serde_json::to_string(&deep_value).unwrap();

        let sanitized = AuditEvent::sanitize_value(&json_str, None);

        // 验证超过最大深度时返回占位符
        assert!(
            sanitized.contains("[MAX_DEPTH_EXCEEDED]"),
            "应该包含 MAX_DEPTH_EXCEEDED 标记"
        );
    }

    #[test]
    fn test_sanitize_value_case_insensitive() {
        // 测试字段名不区分大小写
        let json = r#"{
            "PASSWORD": "upper",
            "Password": "mixed",
            "api_key": "key1",
            "API_KEY": "key2",
            "MySecretToken": "token1"
        }"#;

        let sanitized = AuditEvent::sanitize_value(json, None);
        let parsed: serde_json::Value = serde_json::from_str(&sanitized).unwrap();

        // 验证所有大小写变体都被脱敏
        assert_eq!(parsed["PASSWORD"], "[REDACTED]");
        assert_eq!(parsed["Password"], "[REDACTED]");
        assert_eq!(parsed["api_key"], "[REDACTED]");
        assert_eq!(parsed["API_KEY"], "[REDACTED]");
        assert_eq!(parsed["MySecretToken"], "[REDACTED]");
    }

    #[test]
    fn test_sanitize_value_custom_fields() {
        // 测试自定义敏感字段
        let json = r#"{
            "name": "test",
            "custom_sensitive": "should_be_redacted",
            "password": "also_redacted"
        }"#;

        let custom_fields = vec!["custom_sensitive".to_string()];
        let sanitized = AuditEvent::sanitize_value(json, Some(custom_fields));
        let parsed: serde_json::Value = serde_json::from_str(&sanitized).unwrap();

        // 验证自定义字段被脱敏
        assert_eq!(parsed["custom_sensitive"], "[REDACTED]");
        // 默认字段不在自定义列表中，不会被脱敏
        assert_eq!(parsed["password"], "also_redacted");
        // 非敏感字段保持不变
        assert_eq!(parsed["name"], "test");
    }

    #[test]
    fn test_sanitize_value_complex_nested_structure() {
        // 测试复杂的嵌套结构
        // 注意: 使用不包含敏感词的字段名（如 credentials 包含 credential，api_keys 包含 key）
        let complex_json = r#"{
            "user": {
                "auth_data": {
                    "password": "user_pass",
                    "auth_list": [
                        {"access_token": "at1", "auth_type": "bearer"},
                        {"access_token": "at2", "auth_type": "bearer"}
                    ]
                },
                "settings": {
                    "config": {
                        "secret": "api_secret",
                        "name": "production"
                    }
                }
            },
            "metadata": {
                "count": 10,
                "tags": ["tag1", "tag2"]
            }
        }"#;

        let sanitized = AuditEvent::sanitize_value(complex_json, None);
        let parsed: serde_json::Value = serde_json::from_str(&sanitized).unwrap();

        // 验证多层嵌套中的敏感字段都被脱敏
        assert_eq!(parsed["user"]["auth_data"]["password"], "[REDACTED]");
        assert_eq!(
            parsed["user"]["auth_data"]["auth_list"][0]["access_token"],
            "[REDACTED]"
        );
        assert_eq!(
            parsed["user"]["auth_data"]["auth_list"][1]["access_token"],
            "[REDACTED]"
        );
        assert_eq!(parsed["user"]["settings"]["config"]["secret"], "[REDACTED]");
        // 验证非敏感字段保持不变
        assert_eq!(parsed["user"]["auth_data"]["auth_list"][0]["auth_type"], "bearer");
        assert_eq!(parsed["user"]["settings"]["config"]["name"], "production");
        assert_eq!(parsed["metadata"]["count"], 10);
        assert_eq!(parsed["metadata"]["tags"], serde_json::json!(["tag1", "tag2"]));
    }

    #[test]
    fn test_sanitize_value_non_json() {
        // 测试非 JSON 字符串
        let non_json = "This is just a string with \"password\": value";
        let sanitized = AuditEvent::sanitize_value(non_json, None);
        assert_eq!(sanitized, "***REDACTED***");

        // 测试不包含敏感关键字的非 JSON 字符串
        let safe_string = "This is a safe string";
        let sanitized = AuditEvent::sanitize_value(safe_string, None);
        assert_eq!(sanitized, safe_string);
    }

    #[test]
    fn test_sanitize_value_empty_and_null() {
        // 测试空对象
        let empty_obj = "{}";
        let sanitized = AuditEvent::sanitize_value(empty_obj, None);
        assert_eq!(sanitized, "{}");

        // 测试空数组
        let empty_arr = "[]";
        let sanitized = AuditEvent::sanitize_value(empty_arr, None);
        assert_eq!(sanitized, "[]");

        // 测试 null 值
        let null_val = "null";
        let sanitized = AuditEvent::sanitize_value(null_val, None);
        assert_eq!(sanitized, "null");

        // 测试包含 null 值的对象
        let with_null = r#"{"password": null, "name": "test"}"#;
        let sanitized = AuditEvent::sanitize_value(with_null, None);
        let parsed: serde_json::Value = serde_json::from_str(&sanitized).unwrap();
        assert_eq!(parsed["password"], "[REDACTED]");
        assert_eq!(parsed["name"], "test");
    }
}

/// 审计事件构建器
///
/// 提供链式 API 来构建 `AuditEvent`，避免大量参数：
/// ```rust
/// use dbnexus::audit::{AuditEvent, AuditOperation, AuditSeverity};
///
/// # fn example() -> Result<(), dbnexus::audit::BuildError> {
/// let event = AuditEvent::builder()
///     .operation(AuditOperation::Create)
///     .entity_type("users")
///     .entity_id("123")
///     .user_id("admin")
///     .user_role("administrator")
///     .client_ip("192.168.1.1")
///     .severity(AuditSeverity::High)
///     .result(dbnexus::audit::AuditStatus::Success)
///     .before_value(r#"{"name":"old"}"#)
///     .after_value(r#"{"name":"new"}"#)
///     .extra(r#"{"reason":"update request"}"#)
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Default)]
pub struct AuditEventBuilder {
    operation: Option<AuditOperation>,
    entity_type: Option<String>,
    entity_id: Option<String>,
    user_id: Option<String>,
    user_role: Option<String>,
    client_ip: Option<String>,
    severity: AuditSeverity,
    result: AuditStatus,
    before_value: Option<String>,
    after_value: Option<String>,
    extra: Option<String>,
    request_id: Option<String>,
    session_id: Option<String>,
}

impl AuditEventBuilder {
    /// 创建新构建器
    pub fn new() -> Self {
        Self {
            operation: None,
            entity_type: None,
            entity_id: None,
            user_id: None,
            user_role: None,
            client_ip: None,
            severity: AuditSeverity::Info,
            result: AuditStatus::Success,
            before_value: None,
            after_value: None,
            extra: None,
            request_id: None,
            session_id: None,
        }
    }

    /// 设置操作类型
    pub fn operation(mut self, operation: AuditOperation) -> Self {
        self.operation = Some(operation);
        self
    }

    /// 设置实体类型
    pub fn entity_type(mut self, entity_type: &str) -> Self {
        self.entity_type = Some(entity_type.to_string());
        self
    }

    /// 设置实体 ID
    pub fn entity_id(mut self, entity_id: &str) -> Self {
        self.entity_id = Some(entity_id.to_string());
        self
    }

    /// 设置用户 ID
    pub fn user_id(mut self, user_id: &str) -> Self {
        self.user_id = Some(user_id.to_string());
        self
    }

    /// 设置用户角色
    pub fn user_role(mut self, user_role: &str) -> Self {
        self.user_role = Some(user_role.to_string());
        self
    }

    /// 设置客户端 IP
    pub fn client_ip(mut self, client_ip: &str) -> Self {
        self.client_ip = Some(client_ip.to_string());
        self
    }

    /// 设置严重级别
    pub fn severity(mut self, severity: AuditSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// 设置操作结果
    pub fn result(mut self, result: AuditStatus) -> Self {
        self.result = result;
        self
    }

    /// 设置变更前值（JSON）
    pub fn before_value(mut self, value: &str) -> Self {
        self.before_value = Some(value.to_string());
        self
    }

    /// 设置变更后值（JSON）
    pub fn after_value(mut self, value: &str) -> Self {
        self.after_value = Some(value.to_string());
        self
    }

    /// 设置附加信息（JSON）
    pub fn extra(mut self, value: &str) -> Self {
        self.extra = Some(value.to_string());
        self
    }

    /// 设置请求 ID
    pub fn request_id(mut self, request_id: &str) -> Self {
        self.request_id = Some(request_id.to_string());
        self
    }

    /// 设置会话 ID
    pub fn session_id(mut self, session_id: &str) -> Self {
        self.session_id = Some(session_id.to_string());
        self
    }

    /// 构建 AuditEvent
    ///
    /// # Errors
    /// 如果必需字段（operation, entity_type, entity_id）未设置则返回错误
    pub fn build(self) -> Result<AuditEvent, BuildError> {
        if self.operation.is_none() {
            return Err(BuildError::OperationRequired);
        }
        if self.entity_type.is_none() {
            return Err(BuildError::EntityTypeRequired);
        }
        if self.entity_id.is_none() {
            return Err(BuildError::EntityIdRequired);
        }

        Ok(AuditEvent {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            operation: self.operation.unwrap(),
            entity_type: self.entity_type.unwrap(),
            entity_id: self.entity_id.unwrap(),
            user_id: self.user_id.unwrap_or_default(),
            user_role: self.user_role.unwrap_or_default(),
            client_ip: self.client_ip.unwrap_or_default(),
            severity: self.severity,
            result: self.result,
            error_message: None,
            before_value: self.before_value,
            after_value: self.after_value,
            extra: self.extra,
            request_id: self.request_id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            session_id: self.session_id.unwrap_or_default(),
            trace_context: None,
        })
    }
}

/// 审计日志器构建器
///
/// 提供链式 API 来构建 `AuditLogger`：
/// ```rust
/// use std::sync::Arc;
/// use dbnexus::audit::{AuditLogger, AuditLoggerBuilder, MemoryAuditStorage, AuditConfig};
///
/// let storage = Arc::new(MemoryAuditStorage::new(1000));
/// let logger = AuditLogger::builder()
///     .storage(storage)
///     .config(AuditConfig::default())
///     .build();
/// ```
pub struct AuditLoggerBuilder {
    config: AuditConfig,
    storage: Option<Arc<dyn AuditStorage>>,
}

impl fmt::Debug for AuditLoggerBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditLoggerBuilder")
            .field("config", &self.config)
            .field("storage", &self.storage.as_ref().map(|_| "Arc<dyn AuditStorage>"))
            .finish()
    }
}

impl Default for AuditLoggerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditLoggerBuilder {
    /// 创建新构建器
    pub fn new() -> Self {
        Self {
            config: AuditConfig::default(),
            storage: None,
        }
    }

    /// 设置存储后端
    pub fn storage(mut self, storage: Arc<dyn AuditStorage>) -> Self {
        self.storage = Some(storage);
        self
    }

    /// 设置配置
    pub fn config(mut self, config: AuditConfig) -> Self {
        self.config = config;
        self
    }

    /// 构建 AuditLogger
    pub fn build(self) -> AuditLogger {
        let storage = self.storage.unwrap_or_else(|| Arc::new(MemoryAuditStorage::new(10000)));
        AuditLogger::with_config(self.config, storage)
    }
}
