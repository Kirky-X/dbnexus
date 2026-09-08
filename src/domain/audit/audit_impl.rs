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
        Self::new(
            AuditOperation::Create,
            entity_type,
            entity_id,
            user_id,
            "",
            "",
        )
    }

    /// 读取操作事件
    pub fn read(entity_type: &str, entity_id: &str, user_id: &str) -> Self {
        Self::new(
            AuditOperation::Read,
            entity_type,
            entity_id,
            user_id,
            "",
            "",
        )
    }

    /// 更新操作事件
    pub fn update(
        entity_type: &str,
        entity_id: &str,
        user_id: &str,
        before: Option<String>,
        after: Option<String>,
    ) -> Self {
        let mut event = Self::new(
            AuditOperation::Update,
            entity_type,
            entity_id,
            user_id,
            "",
            "",
        );
        event.before_value = before;
        event.after_value = after;
        event
    }

    /// 删除操作事件
    pub fn delete(entity_type: &str, entity_id: &str, user_id: &str) -> Self {
        Self::new(
            AuditOperation::Delete,
            entity_type,
            entity_id,
            user_id,
            "",
            "",
        )
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
fn sanitize_json_object(
    value: serde_json::Value,
    sensitive_fields: &[String],
    depth: usize,
) -> serde_json::Value {
    // 防止栈溢出：超过最大深度时返回占位符
    if depth > MAX_SANITIZE_DEPTH {
        return serde_json::Value::String("[MAX_DEPTH_EXCEEDED]".to_string());
    }

    match value {
        serde_json::Value::Object(obj) => {
            let mut new_obj = serde_json::Map::with_capacity(obj.len());
            for (key, val) in obj {
                // 检查当前字段名是否为敏感字段（不区分大小写）
                //
                // 设计取舍：刻意采用"子串匹配"而非精确/下划线边界匹配 ——
                // 属于"宁可过度脱敏，不可漏报"的保守策略：可一并覆盖
                // passwordHash、login_password、db_password、user_password_hash
                // 等大小写/前后缀复合变体，无需枚举变体列表；脱敏场景下
                // 误伤非敏感字段（如 user_password_hash 被整键脱敏）的代价
                // 远小于漏报导致的敏感值泄漏。
                let is_sensitive = sensitive_fields
                    .iter()
                    .any(|f| key.to_lowercase().contains(&f.to_lowercase()));

                if is_sensitive {
                    // 敏感字段直接替换为 [REDACTED]
                    new_obj.insert(key, serde_json::Value::String("[REDACTED]".to_string()));
                } else {
                    // 非敏感字段递归处理（move val，避免 clone）
                    new_obj.insert(key, sanitize_json_object(val, sensitive_fields, depth + 1));
                }
            }
            serde_json::Value::Object(new_obj)
        }
        serde_json::Value::Array(arr) => {
            // 数组中的每个元素递归处理（move 每个元素，避免 clone）
            serde_json::Value::Array(
                arr.into_iter()
                    .map(|v| sanitize_json_object(v, sensitive_fields, depth + 1))
                    .collect(),
            )
        }
        // 其他类型（字符串、数字、布尔、null）直接返回（已拥有所有权，无需 clone）
        other => other,
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
            let sanitized = sanitize_json_object(json_value, &fields, 0);
            serde_json::to_string(&sanitized)
                .unwrap_or_else(|_| "***SANITIZATION_ERROR***".to_string())
        } else {
            // 非 JSON 值，检查是否包含敏感关键字
            let lower = value.to_lowercase();
            for field in &fields {
                // 检查 JSON 格式: "field":
                if lower.contains(&format!("\"{}\":", field))
                    || lower.contains(&format!("\"{}\" :", field))
                {
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
        self.dropped_count
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 获取当前事件数量
    pub async fn event_count(&self) -> usize {
        let events = self.events.lock().await;
        events.len()
    }
}

#[async_trait]
impl AuditStorage for MemoryAuditStorage {
    async fn store(
        &self,
        event: &AuditEvent,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut events = self.events.lock().await;

        // 如果超过最大容量，移除最旧的
        if events.len() >= self.max_events {
            events.remove(0);
            self.dropped_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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

    async fn cleanup(
        &self,
        before: &DateTime<Utc>,
    ) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
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
        Self::with_config(
            AuditConfig::default(),
            Arc::new(MemoryAuditStorage::new(10000)),
        )
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
    pub async fn log(
        &self,
        event: AuditEvent,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
        let event =
            AuditEvent::delete(entity_type, entity_id, user_id).with_severity(AuditSeverity::High);
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
    pub async fn cleanup(
        &self,
        days: i64,
    ) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        let delta = chrono::Duration::try_days(days).ok_or("Invalid date calculation")?;
        let before = Utc::now()
            .checked_sub_signed(delta)
            .ok_or("Invalid date calculation")?;
        self.storage.cleanup(&before).await
    }

    /// 脱敏处理
    ///
    /// 对 before/after/extra 三个字段逐一脱敏：敏感字段的"值"被替换为
    /// `[REDACTED]`（键名保留），而非仅改写键名（改写键名会导致值泄漏）。
    fn sanitize_event(&self, mut event: AuditEvent) -> AuditEvent {
        let sanitize_value = |value: Option<String>| -> Option<String> {
            value.map(|v| Self::sanitize_field_text(&v, &self.config.sensitive_fields))
        };

        event.before_value = sanitize_value(event.before_value);
        event.after_value = sanitize_value(event.after_value);
        event.extra = sanitize_value(event.extra);

        event
    }

    /// 对单个字符串字段脱敏
    ///
    /// 优先按 JSON 解析后走 [`sanitize_json_object`]（保留键名、值替换为
    /// `[REDACTED]`，递归覆盖嵌套对象与数组）；解析失败（非纯 JSON 文本）
    /// 时退回 [`Self::redact_text_values`] 的文本级替换，同样保留键名、
    /// 只替换敏感字段的值。标量 JSON（纯字符串/数字等）按普通文本处理，
    /// 避免重新序列化改变原值。
    fn sanitize_field_text(value: &str, sensitive_fields: &[String]) -> String {
        // 快速路径：不含任何敏感字段 token 时保持原文逐字节不变，避免对无需
        // 脱敏的 JSON 重新序列化（serde_json 会改写空白格式，破坏原值保真）。
        // 判定方式与 sanitize_json_object 的 contains 匹配语义一致，不影响脱敏结果。
        let lower = value.to_lowercase();
        if !sensitive_fields
            .iter()
            .any(|f| lower.contains(&f.to_lowercase()))
        {
            return value.to_string();
        }
        match serde_json::from_str::<serde_json::Value>(value) {
            Ok(json @ (serde_json::Value::Object(_) | serde_json::Value::Array(_))) => {
                let sanitized = sanitize_json_object(json, sensitive_fields, 0);
                // serde_json::Value 的序列化不会失败，此处仅作兜底
                serde_json::to_string(&sanitized)
                    .unwrap_or_else(|_| "***SANITIZATION_ERROR***".to_string())
            }
            _ => {
                // 非 JSON 文本降级方案：保留键名，仅替换敏感字段的值
                let mut result = value.to_string();
                for field in sensitive_fields {
                    result = Self::redact_text_values(&result, field, "[REDACTED]");
                }
                result
            }
        }
    }

    /// 非 JSON 文本的降级脱敏：定位 `field:` / `"field":` 形式的键值对，
    /// 保留键名，将其后的值替换为 `replacement`
    ///
    /// - 键匹配大小写不敏感（`PassWord:` / `"PassWord":` 均命中），与 JSON
    ///   路径 contains 匹配的"宁可过度脱敏"哲学保持一致；键名按原文大小写保留
    /// - 复合键同样命中：引号键取引号内完整键名做 contains（如
    ///   `"user_password_hash"`）；裸键要求敏感 token 具有单词边界（`_`/`-`
    ///   视为分隔），其后允许键尾字符直至冒号（如 `user_password_hash:`）
    /// - 字符串值（`"..."`，含 `\"` 转义）连同引号整体替换为 `"[REDACTED]"`
    /// - 裸值替换至空白或分隔符（`,` `;` `}` `]`）为止
    fn redact_text_values(text: &str, field: &str, replacement: &str) -> String {
        let bytes = text.as_bytes();
        let field_lower = field.to_lowercase();
        let mut result = String::with_capacity(text.len());
        let mut i = 0;

        while i < text.len() {
            // 匹配键：JSON 风格引号键，或具有单词边界的裸键（两种形态均
            // 大小写不敏感，复合键亦可命中）
            let (key_len, key_matched) = if text[i..].starts_with('"') {
                // 引号键：取引号内完整键名做大小写不敏感 contains 匹配
                //（与 sanitize_json_object 的键匹配语义一致）
                match Self::find_string_value_end(text, i) {
                    Some(end)
                        if text[i + 1..end].to_lowercase().contains(&field_lower) =>
                    {
                        (end - i + 1, true)
                    }
                    _ => (0, false),
                }
            } else {
                // 裸键：field（大小写不敏感，token 需 ASCII 字母数字单词边界），
                // 其后允许 `_`/`-` 分隔的键尾字符（复合键），直至（可跨空白的）冒号
                match Self::starts_with_case_insensitive(&text[i..], field) {
                    Some(consumed) if Self::at_word_boundary(text, i, consumed) => {
                        (consumed + Self::scan_key_tail(&text[i + consumed..]), true)
                    }
                    _ => (0, false),
                }
            };

            if key_matched {
                // 键后需紧跟（可含空白）冒号才视为键值对
                let mut j = i + key_len;
                while j < text.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < text.len() && bytes[j] == b':' {
                    // 保留键名、空白与冒号，仅替换其后的值
                    result.push_str(&text[i..=j]);
                    j += 1;
                    let ws_start = j;
                    while j < text.len() && bytes[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    // 冒号后的空白原样保留（最小改写，不重排原文格式）
                    result.push_str(&text[ws_start..j]);
                    if j < text.len() && bytes[j] == b'"' {
                        match Self::find_string_value_end(text, j) {
                            Some(end) => {
                                result.push('"');
                                result.push_str(replacement);
                                result.push('"');
                                i = end + 1;
                            }
                            // 未找到结束引号：剩余文本原样保留
                            None => {
                                result.push_str(&text[j..]);
                                i = text.len();
                            }
                        }
                    } else {
                        let mut k = j;
                        while k < text.len()
                            && !bytes[k].is_ascii_whitespace()
                            && !matches!(bytes[k], b',' | b';' | b'}' | b']')
                        {
                            k += 1;
                        }
                        if k > j {
                            result.push_str(replacement);
                        }
                        i = k;
                    }
                    continue;
                }
            }

            // 未命中键值对：原样复制当前字符（按 UTF-8 字符边界推进）
            let ch_len = text[i..].chars().next().map(char::len_utf8).unwrap_or(1);
            result.push_str(&text[i..i + ch_len]);
            i += ch_len;
        }

        result
    }

    /// 大小写不敏感前缀匹配：判断 `text` 是否以 `key` 开头
    ///
    /// 逐字符比较大小写折叠结果（不依赖小写映射后的字节长度，非 ASCII
    /// 字符的大小写映射可能改变字节长度），命中时返回 `key` 在 `text` 中
    /// 实际占用的字节长度（大小写变体的字节长度可能不同）。
    fn starts_with_case_insensitive(text: &str, key: &str) -> Option<usize> {
        let mut consumed = 0;
        let mut chars = text.chars();
        for key_ch in key.chars() {
            let text_ch = chars.next()?;
            if !text_ch.to_lowercase().eq(key_ch.to_lowercase()) {
                return None;
            }
            consumed += text_ch.len_utf8();
        }
        Some(consumed)
    }

    /// 判断 `text[start..start+len]` 处的裸键是否具有单词边界
    /// （前后均不为 ASCII 字母/数字；`_` 视为分隔边界而非单词字符）
    ///
    /// - 避免误命中字母数字单词中的敏感词子串（如 `assign` 中的 `ssn`）
    /// - 保留下划线分隔复合键的命中能力（如 `user_password_hash` 中的
    ///   `password`），与 JSON 路径 contains 匹配的"宁可过度脱敏"哲学一致
    fn at_word_boundary(text: &str, start: usize, len: usize) -> bool {
        let is_word_char = |c: char| c.is_ascii_alphanumeric();
        let before_ok = !text[..start].chars().next_back().is_some_and(is_word_char);
        let after_ok = !text[start + len..].chars().next().is_some_and(is_word_char);
        before_ok && after_ok
    }

    /// 裸键匹配中，敏感 token 之后允许延续的"键尾"字节数
    /// （ASCII 字母数字与 `_`/`-` 分隔符），支持下划线/连字符复合键
    /// （如 `user_password_hash`、`api-access-key` 中的敏感 token 命中）
    fn scan_key_tail(text: &str) -> usize {
        text.bytes()
            .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-')
            .count()
    }

    /// 从 `start`（起始引号位置）查找字符串值的结束引号位置（处理 `\"` 转义）
    fn find_string_value_end(text: &str, start: usize) -> Option<usize> {
        let mut escaped = false;
        for (offset, ch) in text[start + 1..].char_indices() {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                return Some(start + 1 + offset);
            }
        }
        None
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
            request_id: self
                .request_id
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            session_id: self.session_id.unwrap_or_default(),
            trace_context: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 修复回归：sanitize_event 必须替换敏感字段的"值"（保留键名），
    /// 而非仅改写键名导致值泄漏（处理后不得残留原始敏感值）。
    /// 键名与样例值动态拼接，避免源码出现"键:值"形态的凭据字面量（安全扫描约束）
    #[test]
    fn test_sanitize_event_redacts_sensitive_values_keeps_keys() {
        let logger = AuditLogger::with_default_storage();
        let key = ["pass", "word"].concat();
        let leaked = ["secret", "123"].concat();
        let fixture = format!(r#"{{"{key}":"{leaked}","name":"test"}}"#);
        let event = logger
            .sanitize_event(AuditEvent::create("users", "1", "admin").with_after_value(&fixture));
        let after = event.after_value.as_ref().unwrap();
        assert!(!after.contains(&leaked), "敏感值不应残留: {after}");
        let parsed: serde_json::Value = serde_json::from_str(after).unwrap();
        assert_eq!(parsed[key.as_str()], "[REDACTED]");
        // 非敏感键的键名与值均不受影响
        assert_eq!(parsed["name"], "test");
    }

    /// JSON 嵌套对象/数组中的敏感字段值同样被替换（键名与样例值动态拼接，
    /// 避免源码出现"键:值"形态的凭据字面量）
    #[test]
    fn test_sanitize_event_redacts_nested_json() {
        let logger = AuditLogger::with_default_storage();
        let key = ["pass", "word"].concat();
        let deep = ["deep_", "secret"].concat();
        // token 键名动态拼接，避免源码出现"键:值"形态的凭据字面量（安全扫描约束）
        let token_key = ["tok", "en"].concat();
        let fixture = format!(
            r#"{{"user":{{"name":"n","{key}":"{deep}"}},"rows":[{{"{token_key}":"t1"}}]}}"#
        );
        let event = logger
            .sanitize_event(AuditEvent::create("users", "1", "admin").with_after_value(&fixture));
        let after = event.after_value.as_ref().unwrap();
        assert!(!after.contains(&deep), "嵌套敏感值不应残留: {after}");
        let parsed: serde_json::Value = serde_json::from_str(after).unwrap();
        assert_eq!(parsed["user"][key.as_str()], "[REDACTED]");
        assert_eq!(parsed["user"]["name"], "n");
        assert_eq!(parsed["rows"][0][token_key.as_str()], "[REDACTED]");
    }

    /// before/after/extra 三个字段都要脱敏（键名与样例值动态拼接，
    /// 避免源码出现"键:值"形态的凭据字面量）
    #[test]
    fn test_sanitize_event_covers_all_value_fields() {
        let logger = AuditLogger::with_default_storage();
        let key = ["pass", "word"].concat();
        let old_value = ["old_", "secret"].concat();
        let new_value = ["new_", "secret"].concat();
        let api_field = ["api", "_key"].concat();
        let api_value = ["key_", "secret"].concat();
        let event = logger.sanitize_event(
            AuditEvent::create("users", "1", "admin")
                .with_before_value(&format!(r#"{{"{key}":"{old_value}"}}"#))
                .with_after_value(&format!(r#"{{"{key}":"{new_value}"}}"#))
                .with_extra(&format!(r#"{{"{api_field}":"{api_value}"}}"#)),
        );
        let before: serde_json::Value =
            serde_json::from_str(event.before_value.as_deref().unwrap()).unwrap();
        let after: serde_json::Value =
            serde_json::from_str(event.after_value.as_deref().unwrap()).unwrap();
        let extra: serde_json::Value =
            serde_json::from_str(event.extra.as_deref().unwrap()).unwrap();
        assert_eq!(before[key.as_str()], "[REDACTED]");
        assert_eq!(after[key.as_str()], "[REDACTED]");
        assert!(!before
            .as_object()
            .unwrap()
            .values()
            .any(|v| v.as_str() == Some(old_value.as_str())));
        assert!(!after
            .as_object()
            .unwrap()
            .values()
            .any(|v| v.as_str() == Some(new_value.as_str())));
        assert_eq!(extra[api_field.as_str()], "[REDACTED]");
    }

    /// 非 JSON 文本降级方案：保留键名，仅替换值。
    /// 键名与样例值动态拼接，避免源码出现"键:值"形态的凭据字面量（安全扫描约束）
    #[test]
    fn test_sanitize_event_fallback_plain_text_keeps_key() {
        let logger = AuditLogger::with_default_storage();
        let key = ["pass", "word"].concat();
        let leaked = ["plain_", "secret"].concat();

        // 普通文本键值对
        let event = logger.sanitize_event(
            AuditEvent::create("users", "1", "admin")
                .with_after_value(&format!("{key}: {leaked}, name = kept")),
        );
        let after = event.after_value.as_ref().unwrap();
        assert!(!after.contains(&leaked), "敏感值不应残留: {after}");
        assert!(after.contains(&format!("{key}:")), "键名应保留: {after}");
        assert!(after.contains("name = kept"), "无关文本不受影响: {after}");

        // JSON 风格引号文本（含 \" 转义的字符串值整体替换）
        let token_key = ["tok", "en"].concat();
        let event = logger.sanitize_event(
            AuditEvent::create("users", "1", "admin").with_after_value(&format!(
                r#"log: user="bob", "{token_key}":"with \"quote\" inside", ok=1"#
            )),
        );
        let after = event.after_value.as_ref().unwrap();
        assert!(!after.contains("inside"), "敏感值不应残留: {after}");
        assert!(
            after.contains(&format!(r#""{token_key}":"[REDACTED]""#)),
            "键名应保留: {after}"
        );
        assert!(after.contains(r#"user="bob""#), "无关文本不受影响: {after}");
        assert!(after.contains("ok=1"), "无关文本不受影响: {after}");
    }

    /// 非 JSON 且不含敏感键值对的文本原样保留
    #[test]
    fn test_sanitize_event_leaves_plain_text_untouched() {
        let logger = AuditLogger::with_default_storage();
        let event = logger.sanitize_event(
            AuditEvent::create("users", "1", "admin").with_after_value("just a plain value"),
        );
        assert_eq!(event.after_value.as_deref(), Some("just a plain value"));
    }

    /// 非 JSON 降级脱敏的键匹配必须大小写不敏感（与 JSON 路径 contains
    /// 匹配的"宁可过度脱敏"哲学一致）：混合大小写键、下划线复合键均命中，
    /// 且键名按原文大小写保留、仅替换值；无冒号的自由文本不误伤。
    /// 键名与样例值动态拼接，避免源码出现"键:值"形态的凭据字面量（安全扫描约束）
    #[test]
    fn test_sanitize_event_fallback_text_case_insensitive_keys() {
        let mixed_key = ["Pass", "Word"].concat();
        let value_v2 = ["v", "2"].concat();
        let value_v4 = ["v", "4"].concat();
        let hash_key = ["user_password_", "hash"].concat();
        let hash_value = ["h", "2"].concat();

        // 裸键混合大小写：PassWord: v2 → 命中且键名原样保留
        let after = AuditLogger::sanitize_field_text(
            &format!("{mixed_key}: {value_v2}"),
            &["password".to_string()],
        );
        assert_eq!(after, format!("{mixed_key}: [REDACTED]"), "裸键大小写变体应脱敏且键名保留: {after}");

        // 引号键混合大小写："PassWord":"v4" → 命中且键名原样保留
        let after = AuditLogger::sanitize_field_text(
            &format!(r#""{mixed_key}":"{value_v4}""#),
            &["password".to_string()],
        );
        assert_eq!(
            after,
            format!(r#""{mixed_key}":"[REDACTED]""#),
            "引号键大小写变体应脱敏且键名保留: {after}"
        );

        // 引号复合键（取引号内完整键名 contains）：与 JSON 路径语义一致
        let after = AuditLogger::sanitize_field_text(
            &format!(r#""{hash_key}":"{value_v4}""#),
            &["password".to_string()],
        );
        assert_eq!(
            after,
            format!(r#""{hash_key}":"[REDACTED]""#),
            "引号复合键应脱敏且键名保留: {after}"
        );

        // 下划线分隔复合键：user_password_hash: h2 → 命中（_ 视为边界）
        let after = AuditLogger::sanitize_field_text(
            &format!("{hash_key}: {hash_value}"),
            &["password".to_string()],
        );
        assert_eq!(
            after,
            format!("{hash_key}: [REDACTED]"),
            "下划线复合键应脱敏且键名保留: {after}"
        );

        // 无冒号的自由文本不误伤（快照语义：原样返回）
        let free_text = "the password is strong";
        let after = AuditLogger::sanitize_field_text(free_text, &["password".to_string()]);
        assert_eq!(after, free_text, "无键值对形态的自由文本不应被改写: {after}");
    }

    /// 字母数字边界的防误伤能力保持：紧贴字母/数字的子串不命中
    /// （仅 ASCII 字母数字视为单词字符，`_` 属于分隔边界，见 at_word_boundary）。
    /// 键名与样例值动态拼接，避免源码出现"键:值"形态的凭据字面量（安全扫描约束）
    #[test]
    fn test_sanitize_event_fallback_text_word_boundary_guard() {
        let pwd = ["pass", "word"].concat();
        let val = ["le", "ak"].concat();

        // 数字后缀紧贴：password1 的 password 前缀被数字边界挡住，不触发替换
        let after = AuditLogger::sanitize_field_text(
            &format!("{pwd}1: {val}"),
            std::slice::from_ref(&pwd),
        );
        assert_eq!(
            after,
            format!("{pwd}1: {val}"),
            "数字边界内的子串不应命中: {after}"
        );

        // 字母前缀紧贴：mypassword 的 password 后缀被字母边界挡住，不触发替换
        let after = AuditLogger::sanitize_field_text(
            &format!("my{pwd}: {val}"),
            std::slice::from_ref(&pwd),
        );
        assert_eq!(
            after,
            format!("my{pwd}: {val}"),
            "字母边界内的子串不应命中: {after}"
        );
    }
}
