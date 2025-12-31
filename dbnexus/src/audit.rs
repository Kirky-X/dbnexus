// Copyright (c) 2025 Kirky.X
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
//! ```ignore
//! use dbnexus::audit::{AuditLogger, AuditEvent, AuditConfig};
//!
//! let logger = AuditLogger::new(AuditConfig::default());
//! logger.log(AuditEvent::create("users", "1", "admin")).await;
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use tokio::sync::Mutex;
use uuid::Uuid;

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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AuditSeverity {
    /// 信息
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

/// 审计结果
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AuditResult {
    /// 成功
    Success,
    /// 失败
    Failure,
    /// 部分成功
    Partial,
    /// 未知
    Unknown,
}

impl fmt::Display for AuditResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuditResult::Success => write!(f, "SUCCESS"),
            AuditResult::Failure => write!(f, "FAILURE"),
            AuditResult::Partial => write!(f, "PARTIAL"),
            AuditResult::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

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
    /// 操作结果
    pub result: AuditResult,
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
}

impl AuditEvent {
    /// 创建审计事件
    #[allow(clippy::too_many_arguments)]
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
            result: AuditResult::Success,
            before_value: None,
            after_value: None,
            extra: None,
            request_id: Uuid::new_v4().to_string(),
            session_id: String::new(),
        }
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
    pub fn with_result(mut self, result: AuditResult) -> Self {
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
    pub result: Option<AuditResult>,
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
    /// 创建审计日志器
    pub fn new(config: AuditConfig, storage: Arc<dyn AuditStorage>) -> Self {
        Self {
            config,
            storage,
            alert_callback: None,
        }
    }

    /// 创建带默认配置的审计日志器
    pub fn with_default_storage() -> Self {
        Self::new(AuditConfig::default(), Arc::new(MemoryAuditStorage::new(10000)))
    }

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
        let before = Utc::now().checked_sub_signed(chrono::Duration::days(days)).unwrap();
        self.storage.cleanup(&before).await
    }

    /// 脱敏处理
    fn sanitize_event(&self, mut event: AuditEvent) -> AuditEvent {
        let sanitize_value = |value: Option<String>| -> Option<String> {
            if let Some(v) = value {
                let mut result = v;
                for field in &self.config.sensitive_fields {
                    let replacement = format!("***REDACTED_{}***", field.to_uppercase());
                    result = result.replace(&format!(r#""{}":"#, field), &format!(r#""{}":"#, &replacement));
                    result = result.replace(&format!(r#"{}:"#, field), &format!(r#"{}:"#, &replacement));
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

        // 默认实现：打印到stderr
        eprintln!(
            "[AUDIT ALERT] {} - {} {} on {} by user {}",
            event.severity, event.operation, event.entity_id, event.entity_type, event.user_id
        );
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
    async fn test_audit_logger() {
        let storage = Arc::new(MemoryAuditStorage::new(100));
        let config = AuditConfig::default();
        let logger = AuditLogger::new(config, storage);

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
        let logger = AuditLogger::new(config, storage);

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
}
