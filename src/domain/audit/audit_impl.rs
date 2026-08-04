// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Audit module implementation details.
//!
//! Contains function implementations and impl blocks extracted from [`super`].

use super::*;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use tokio::sync::Mutex;
use uuid::Uuid;

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

impl AuditEvent {
    /// 创建审计事件（推荐使用构建器模式）
    ///
    /// # 推荐方式
    /// 使用 `AuditEventBuilder` 进行链式构建：
    /// ```rust
    /// # use dbnexus::{AuditEvent, AuditOperation, AuditSeverity};
    /// # fn example() -> Result<(), dbnexus::domain::BuildError> {
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
    /// # use dbnexus::{AuditEvent, AuditSeverity};
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
    /// - 将敏感字段的值替换为 "\[REDACTED\]"
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

impl AuditLogger {
    /// 创建带默认配置的审计日志器
    pub fn new() -> Self {
        Self::with_default_storage()
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
                    result = result.replace(&format!(r#""{}":"#, field), &format!(r#""{}":"#, replacement));

                    // 2. 非 JSON 格式: field:
                    result = result.replace(&format!(r#"{}:"#, field), &format!(r#"{}:"#, replacement));

                    // 3. 嵌套字段 (如 user.password)
                    if field.contains('.') {
                        let parts: Vec<&str> = field.split('.').collect();
                        if parts.len() >= 2 {
                            let nested_pattern = format!(r#""{}""#, field);
                            result = result.replace(&nested_pattern, &format!(r#""{}""#, replacement));
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
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(value)
            && let Some(arr) = json_val.as_array()
        {
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

        value.to_string()
    }

    /// 检测字符串是否为有效的 Base64 编码
    pub(super) fn is_base64(s: &str) -> bool {
        if !s.len().is_multiple_of(4) || s.is_empty() {
            return false;
        }
        let valid_chars: std::collections::HashSet<char> =
            "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
                .chars()
                .collect();
        s.chars().all(|c| valid_chars.contains(&c) || c == '=')
    }

    /// 检查是否需要告警
    pub(super) fn should_alert(&self, event: &AuditEvent) -> bool {
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
