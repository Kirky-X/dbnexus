// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
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
//! use dbnexus::{AuditEvent, AuditLogger};
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

mod audit_impl;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use thiserror::Error;
use tokio::sync::Mutex;

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
/// use dbnexus::{AuditEvent, AuditOperation, AuditSeverity};
///
/// # fn example() -> Result<(), dbnexus::domain::audit::BuildError> {
/// let event = AuditEvent::builder()
///     .operation(AuditOperation::Create)
///     .entity_type("users")
///     .entity_id("123")
///     .user_id("admin")
///     .user_role("administrator")
///     .client_ip("192.168.1.1")
///     .severity(AuditSeverity::High)
///     .result(dbnexus::AuditStatus::Success)
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
