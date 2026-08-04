// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Message catalog for i18n translations.
//!
//! All translations are embedded at compile time as Rust `match` expressions.
//! This avoids runtime I/O and external dependencies while maintaining
//! full type safety.
//!
//! Each message key maps to a template string with `{ $variable }` placeholders
//! that are substituted at runtime by [`translate()`].

use super::locale::current_locale;

/// Translate a message key to the current locale.
///
/// Looks up `key` in the message catalog for the current locale,
/// substitutes any `{ $var }` placeholders with values from `args`,
/// and returns the formatted string.
///
/// Falls back to English if the key is not found in the current locale.
/// Falls back to the key itself if not found in English (should not happen).
pub fn translate(key: &str, args: &[(&str, String)]) -> String {
    let locale = current_locale();
    let lang = locale.id.language.as_str();

    // Try current locale, then English fallback
    let template = lookup(lang, key).or_else(|| lookup("en", key)).unwrap_or(key);

    substitute(template, args)
}

/// Translate a message key to English specifically, regardless of current locale.
pub fn translate_en(key: &str, args: &[(&str, String)]) -> String {
    let template = lookup_en(key).unwrap_or(key);
    substitute(template, args)
}

/// Shorthand for [`translate()`].
pub fn t(key: &str, args: &[(&str, String)]) -> String {
    translate(key, args)
}

/// Convenience: translate with no dynamic arguments.
pub fn t_simple(key: &str) -> String {
    translate(key, &[])
}

/// Look up a message key for a given language.
fn lookup(lang: &str, key: &str) -> Option<&'static str> {
    match lang {
        "en" => lookup_en(key),
        "zh" => lookup_zh(key),
        _ => None,
    }
}

/// Substitute `{ $var }` placeholders in a template.
fn substitute(template: &str, args: &[(&str, String)]) -> String {
    let mut result = template.to_string();
    for (name, value) in args {
        let placeholder = format!("{{ ${name} }}");
        result = result.replace(&placeholder, value);
    }
    result
}

// ============================================================================
// English messages
// ============================================================================

fn lookup_en(key: &str) -> Option<&'static str> {
    match key {
        // --- Config errors ---
        "config-missing-field" => Some("Missing required configuration: { $field }"),
        "config-missing-url" => Some("Missing required configuration: dbnexus.url"),
        "config-invalid-cache-capacity" => Some("Invalid cache capacity: { $reason }"),
        "config-invalid-value" => Some("Invalid configuration value for '{ $key }': { $message }"),
        "config-invalid-format" => Some("Invalid configuration format: { $reason }"),
        "config-file-not-found" => Some("Configuration file not found: { $path }"),
        "config-io-error" => Some("IO error: { $reason }"),
        "config-invalid-url" => Some("Invalid URL: { $url }"),
        "config-unsupported-protocol" => Some("Unsupported database protocol: { $protocol }"),
        "config-parse-error" => Some("Parse error: { $reason }"),
        "config-validation-error" => Some("Validation error: { $reason }"),

        // --- DbError ---
        "db-connection" => Some("Database connection error: { $error }"),
        "db-config" => Some("Configuration error: { $message }"),
        "db-permission" => Some("Permission denied: { $message }"),
        "db-transaction" => Some("Transaction error: { $message }"),
        "db-migration" => Some("Migration error: { $message }"),
        "db-cache" => Some("Cache error: { $message }"),
        "db-query" => Some("Query error: { $message }"),
        "db-validation" => Some("Validation error: { $message }"),

        // --- PoolError ---
        "pool-acquire-timeout" => Some("Failed to acquire connection within timeout"),
        "pool-exhausted" => Some("Connection pool exhausted"),
        "pool-connection-failed" => Some("Failed to create connection: { $reason }"),
        "pool-health-check-failed" => Some("Health check failed: { $reason }"),

        // --- MigrationError ---
        "migration-file-not-found" => Some("Migration file not found: { $path }"),
        "migration-parse-error" => Some("Failed to parse migration file: { $reason }"),
        "migration-execution-error" => Some("Migration execution failed: { $reason }"),
        "migration-version-conflict" => Some("Migration version conflict: { $reason }"),
        "migration-rollback-error" => Some("Migration rollback failed: { $reason }"),

        // --- AuditError (foundation) ---
        "audit-write-error" => Some("Failed to write audit log: { $reason }"),
        "audit-serialization-error" => Some("Failed to serialize audit data: { $reason }"),
        "audit-config-error" => Some("Invalid audit configuration: { $reason }"),

        // --- PermissionConfigError ---
        "perm-config-missing-field" => Some("missing required field: { $field }"),
        "perm-config-invalid-value" => Some("invalid value for field '{ $field }': { $reason }"),
        "perm-config-policy-not-found" => Some("policy file not found: { $path }"),

        // --- PermissionError ---
        "perm-denied" => Some("permission denied for { $operation } on { $resource }"),
        "perm-role-not-found" => Some("role not found: { $role }"),
        "perm-invalid-policy" => Some("invalid policy configuration: { $reason }"),
        "perm-rate-limited" => Some("rate limit exceeded"),
        "perm-parse-error" => Some("policy parse error: { $reason }"),

        // --- PermissionProviderError ---
        "perm-provider-role-not-found" => Some("Role '{ $role }' not found"),
        "perm-provider-load-error" => Some("Failed to load config: { $reason }"),
        "perm-provider-check-error" => Some("Permission check failed: { $reason }"),
        "perm-provider-unknown" => Some("Unknown error: { $reason }"),

        // --- SqlParserError ---
        "sql-parse-error" => Some("Failed to parse SQL: { $reason }"),
        "sql-unsupported-statement" => Some("Unsupported SQL statement type: { $stmt_type }"),
        "sql-empty-statement" => Some("Empty SQL statement"),
        "sql-multiple-statements" => Some("Multiple statements not allowed"),
        "sql-contains-variables" => Some("SQL statement contains variables: { $details }"),

        // --- SensitiveError ---
        "sensitive-masking-failed" => Some("Masking failed: { $reason }"),
        "sensitive-encryption-failed" => Some("Encryption failed: { $reason }"),
        "sensitive-decryption-failed" => Some("Decryption failed: { $reason }"),
        "sensitive-invalid-key" => Some("Invalid key: { $reason }"),
        "sensitive-invalid-input" => Some("Invalid input: { $reason }"),

        // --- AuthError ---
        "auth-invalid-credentials" => Some("Invalid credentials"),
        "auth-token-generation" => Some("Token generation failed: { $reason }"),
        "auth-invalid-token" => Some("Invalid token"),
        "auth-token-expired" => Some("Token expired"),
        "auth-user-not-found" => Some("User not found: { $user }"),
        "auth-password-hash" => Some("Password hash failed: { $reason }"),
        "auth-user-limit-reached" => Some("User storage limit reached: { $reason }"),

        // --- MetricsError ---
        "metrics-export-error" => Some("Export failed: { $reason }"),
        "metrics-not-initialized" => Some("Collector not initialized"),
        "metrics-unknown" => Some("Unknown metrics error: { $reason }"),

        // --- CircuitBreakerError ---
        "circuit-breaker" => Some("Circuit breaker is { $state }"),

        // --- AuditBuilderError ---
        "audit-builder-operation-required" => Some("operation is required"),
        "audit-builder-entity-type-required" => Some("entity_type is required"),
        "audit-builder-entity-id-required" => Some("entity_id is required"),

        // --- RetryError ---
        "retry-exhausted" => Some("Retry exhausted after { $attempts } attempts: { $last_error }"),
        "retry-non-retryable" => Some("Non-retryable operation: { $error }"),
        "retry-timeout" => Some("Retry timed out after { $timeout_ms }ms: { $last_error }"),

        // --- SagaError ---
        "saga-execution-failed" => Some("Saga execution failed: { $reason }"),
        "saga-compensation-failed" => Some("Saga compensation failed: { $reason }"),
        "saga-timeout" => Some("Saga timeout: { $reason }"),

        // --- SnowflakeError ---
        "snowflake-clock-backtrack" => {
            Some("Clock backtrack: waited timestamp { $waited_ts } still behind last used { $last_ts }")
        }
        "snowflake-timestamp-overflow" => Some("Timestamp overflow: { $timestamp } exceeds 41-bit capacity"),

        // --- DbNexusError ---
        "nexus-unsupported-database" => Some("Unsupported database scheme in URL: { $scheme }"),

        // --- ErrorCategory ---
        "error-category-permission" => Some("Permission"),
        "error-category-injection-risk" => Some("InjectionRisk"),
        "error-category-syntax-error" => Some("SyntaxError"),
        "error-category-shard-conflict" => Some("ShardConflict"),

        // --- QueryErrorReport ---
        "query-error-report" => Some("[{ $category }] { $message }\nSuggestion: { $suggestion }"),
        "query-error-suggestion" => Some("Suggestion: { $suggestion }"),
        "query-error-table" => Some("Table: { $table }"),
        "query-error-operation" => Some("Operation: { $operation }"),

        // --- Migration messages (existing) ---
        "migration" => Some("{ $count } migrations applied"),
        "hello-world" => Some("Hello, World!"),

        // --- CLI messages ---
        "cli-migration-created" => Some("✓ Migration file created: { $path }"),
        "cli-status-title" => Some("Migration Status"),
        "cli-db-connect-failed" => Some("❌ Database connection failed: { $error }"),
        "cli-db-type" => Some("📊 Database type: { $type }"),
        "cli-migrations-dir" => Some("📁 Migrations directory: { $path }"),
        "cli-session-failed" => Some("❌ Failed to get database session: { $error }"),
        "cli-history-load-failed" => Some("⚠️  Failed to load migration history: { $error }"),
        "cli-history-table-missing" => Some("Migration history table may not exist"),
        "cli-applied-count" => Some("✅ Applied migrations: { $count }"),
        "cli-latest-migration" => Some("Latest migration:"),
        "cli-version" => Some("  - Version: { $version }"),
        "cli-description" => Some("  - Description: { $description }"),
        "cli-applied-at" => Some("  - Applied at: { $time }"),
        "cli-history-details" => Some("Migration history details:"),
        "cli-local-files" => Some("📦 Local migration files: { $count }"),
        "cli-pending-count" => Some("⏳ Pending migrations: { $count }"),
        "cli-pending-list" => Some("Pending migration list:"),
        "cli-all-applied" => Some("✓ All migrations have been applied"),
        "cli-db-connected" => Some("🔗 Database connection: Connected"),
        "cli-db-url" => Some("   URL: { $url }"),
        "cli-dir-create-failed" => Some("Failed to create directory: { $error }"),
        "cli-timestamp-parse-failed" => Some("Failed to parse timestamp: { $error }"),
        "cli-desc-special-chars-only" => Some("Migration description must not contain only special characters"),
        "cli-desc-too-long" => Some("Migration description too long (max 100 characters)"),
        "cli-file-write-failed" => Some("Failed to write migration file: { $error }"),
        "cli-db-type-detect-failed" => Some("Database type detection failed: { $error }"),
        "cli-test-connection-title" => Some("Database Connection Test"),
        "cli-testing-connection" => Some("Testing database connection..."),
        "cli-connection-failed" => Some("Connection failed: { $error }"),
        "cli-connection-success" => Some("Connection successful!"),
        "cli-connection-time" => Some("Connection time: { $duration }"),
        "cli-connection-url" => Some("Connection URL: { $url }"),
        "cli-pool-status" => Some("Connection pool status:"),
        "cli-total-connections" => Some("Total connections: { $count }"),
        "cli-active-connections" => Some("Active connections: { $count }"),
        "cli-idle-connections" => Some("Idle connections: { $count }"),
        "cli-connection-verify-failed" => Some("Connection verification failed: { $error }"),
        "cli-apply-title" => Some("Apply Migrations"),
        "cli-no-migration-files" => Some("No migration files found in directory"),
        "cli-no-pending" => Some("No pending migrations to apply"),
        "cli-found-pending" => Some("Found { $count } pending migrations"),
        "cli-target-version" => Some("Target version: { $version }"),
        "cli-starting-apply" => Some("Starting to apply migrations..."),
        "cli-applying" => Some("Applying v{ $version } - { $description } ... "),
        "cli-apply-success" => Some("Successfully applied { $success } / { $total } migrations"),
        "cli-rollback-title" => Some("Rollback Migrations"),
        "cli-no-applied-rollback" => Some("No applied migrations to rollback"),
        "cli-to-rollback-count" => Some("Need to rollback { $count } migrations"),
        "cli-mode-rollback-all" => Some("Mode: Rollback all migrations"),
        "cli-mode-rollback-version" => Some("Mode: Rollback to version { $version }"),
        "cli-mode-rollback-last" => Some("Mode: Rollback last version"),
        "cli-starting-rollback" => Some("Starting to rollback migrations..."),
        "cli-rolling-back" => Some("Rolling back v{ $version } - { $description } ... "),
        "cli-rollback-error-stop" => Some("Error occurred during rollback, stopping execution"),
        "cli-rollback-success" => Some("Successfully rolled back { $success } / { $total } migrations"),
        "cli-generate-title" => Some("Generate Migration File"),
        "cli-parsing-schema" => Some("Parsing schema files..."),
        "cli-schema-read-source-failed" => Some("Failed to read source schema file: { $error }"),
        "cli-schema-read-target-failed" => Some("Failed to read target schema file: { $error }"),
        "cli-schema-diff-generated" => Some("Generated schema diff SQL"),
        "cli-no-schema-template" => Some("No schema file provided, generated blank template"),
        "cli-output-dir-create-failed" => Some("Failed to create output directory: { $error }"),
        "cli-check-edit-file" => Some("Please review and edit the generated migration file for correctness"),
        "cli-list-title" => Some("Migration File List"),
        "cli-list-directory" => Some("Directory: { $path }"),
        "cli-list-total-count" => Some("Total { $count } migration files"),

        // --- Pool/Session messages ---
        "pool-invalid-config" => Some("Invalid configuration: { $error }"),
        "pool-read-config-failed" => Some("Failed to read permission config file '{ $path }': { $error }"),
        "pool-parse-config-failed" => Some("Failed to parse permission config file '{ $path }': { $error }"),
        "pool-yaml-parse-error" => Some("YAML parse error in '{ $source }': { $error }"),
        "pool-invalid-db-url" => Some("Invalid database URL: { $error }"),
        "pool-recreate-failed" => Some("Failed to recreate connections: { $error }"),
        "session-txn-begin-failed" => Some("Failed to begin transaction: { $error }"),
        "session-txn-begin-graph-failed" => Some("Failed to begin graph transaction: { $error }"),
        "session-txn-commit-failed" => Some("Failed to commit graph transaction: { $error }"),
        "session-txn-rollback-failed" => Some("Failed to rollback transaction: { $error }"),
        "session-txn-rollback-graph-failed" => Some("Failed to rollback graph transaction: { $error }"),
        "session-ddl-not-allowed" => Some("DDL operation not allowed: { $reason }"),
        "session-ddl-parse-failed" => Some("Failed to parse DDL SQL: { $error }"),
        "session-ddl-validation-error" => Some("DDL validation error: { $error }"),
        "session-unknown-operation" => Some("Unknown operation: { $operation }"),
        "session-permission-denied" => Some("Permission denied for { $action } on { $table }"),

        _ => None,
    }
}

// ============================================================================
// Chinese (zh) messages
// ============================================================================

fn lookup_zh(key: &str) -> Option<&'static str> {
    match key {
        "config-missing-field" => Some("缺少必填配置项: { $field }"),
        "config-missing-url" => Some("缺少必填配置项: dbnexus.url"),
        "config-invalid-cache-capacity" => Some("无效的缓存容量: { $reason }"),
        "config-invalid-value" => Some("配置项 '{ $key }' 的值无效: { $message }"),
        "config-invalid-format" => Some("配置格式无效: { $reason }"),
        "config-file-not-found" => Some("配置文件未找到: { $path }"),
        "config-io-error" => Some("IO 错误: { $reason }"),
        "config-invalid-url" => Some("无效的 URL: { $url }"),
        "config-unsupported-protocol" => Some("不支持的数据库协议: { $protocol }"),
        "config-parse-error" => Some("解析错误: { $reason }"),
        "config-validation-error" => Some("验证错误: { $reason }"),

        "db-connection" => Some("数据库连接错误: { $error }"),
        "db-config" => Some("配置错误: { $message }"),
        "db-permission" => Some("权限被拒绝: { $message }"),
        "db-transaction" => Some("事务错误: { $message }"),
        "db-migration" => Some("迁移错误: { $message }"),
        "db-cache" => Some("缓存错误: { $message }"),
        "db-query" => Some("查询错误: { $message }"),
        "db-validation" => Some("验证错误: { $message }"),

        "pool-acquire-timeout" => Some("无法在超时时间内获取连接"),
        "pool-exhausted" => Some("连接池已耗尽"),
        "pool-connection-failed" => Some("创建连接失败: { $reason }"),
        "pool-health-check-failed" => Some("健康检查失败: { $reason }"),

        "migration-file-not-found" => Some("迁移文件未找到: { $path }"),
        "migration-parse-error" => Some("迁移文件解析失败: { $reason }"),
        "migration-execution-error" => Some("迁移执行失败: { $reason }"),
        "migration-version-conflict" => Some("迁移版本冲突: { $reason }"),
        "migration-rollback-error" => Some("迁移回滚失败: { $reason }"),

        "audit-write-error" => Some("审计日志写入失败: { $reason }"),
        "audit-serialization-error" => Some("审计数据序列化失败: { $reason }"),
        "audit-config-error" => Some("审计配置无效: { $reason }"),

        "perm-config-missing-field" => Some("缺少必填字段: { $field }"),
        "perm-config-invalid-value" => Some("字段 '{ $field }' 的值无效: { $reason }"),
        "perm-config-policy-not-found" => Some("策略文件未找到: { $path }"),

        "perm-denied" => Some("对 { $resource } 的 { $operation } 操作权限被拒绝"),
        "perm-role-not-found" => Some("角色未找到: { $role }"),
        "perm-invalid-policy" => Some("策略配置无效: { $reason }"),
        "perm-rate-limited" => Some("速率限制已超出"),
        "perm-parse-error" => Some("策略解析错误: { $reason }"),

        "perm-provider-role-not-found" => Some("角色 '{ $role }' 未找到"),
        "perm-provider-load-error" => Some("加载配置失败: { $reason }"),
        "perm-provider-check-error" => Some("权限检查失败: { $reason }"),
        "perm-provider-unknown" => Some("未知错误: { $reason }"),

        "sql-parse-error" => Some("SQL 解析失败: { $reason }"),
        "sql-unsupported-statement" => Some("不支持的 SQL 语句类型: { $stmt_type }"),
        "sql-empty-statement" => Some("空的 SQL 语句"),
        "sql-multiple-statements" => Some("不允许多条语句"),
        "sql-contains-variables" => Some("SQL 语句包含变量: { $details }"),

        "sensitive-masking-failed" => Some("脱敏失败: { $reason }"),
        "sensitive-encryption-failed" => Some("加密失败: { $reason }"),
        "sensitive-decryption-failed" => Some("解密失败: { $reason }"),
        "sensitive-invalid-key" => Some("无效的密钥: { $reason }"),
        "sensitive-invalid-input" => Some("无效的输入: { $reason }"),

        "auth-invalid-credentials" => Some("无效的凭据"),
        "auth-token-generation" => Some("令牌生成失败: { $reason }"),
        "auth-invalid-token" => Some("无效的令牌"),
        "auth-token-expired" => Some("令牌已过期"),
        "auth-user-not-found" => Some("用户未找到: { $user }"),
        "auth-password-hash" => Some("密码哈希失败: { $reason }"),
        "auth-user-limit-reached" => Some("用户存储已达上限: { $reason }"),

        "metrics-export-error" => Some("导出失败: { $reason }"),
        "metrics-not-initialized" => Some("收集器未初始化"),
        "metrics-unknown" => Some("未知指标错误: { $reason }"),

        "circuit-breaker" => Some("断路器处于 { $state } 状态"),

        "audit-builder-operation-required" => Some("操作类型为必填项"),
        "audit-builder-entity-type-required" => Some("实体类型为必填项"),
        "audit-builder-entity-id-required" => Some("实体 ID 为必填项"),

        "retry-exhausted" => Some("重试 { $attempts } 次后仍失败: { $last_error }"),
        "retry-non-retryable" => Some("不可重试的操作: { $error }"),
        "retry-timeout" => Some("重试在 { $timeout_ms }ms 后超时: { $last_error }"),

        "saga-execution-failed" => Some("Saga 执行失败: { $reason }"),
        "saga-compensation-failed" => Some("Saga 补偿失败: { $reason }"),
        "saga-timeout" => Some("Saga 超时: { $reason }"),

        "snowflake-clock-backtrack" => Some("时钟回拨: 等待后的时间戳 { $waited_ts } 仍落后于上次使用的 { $last_ts }"),
        "snowflake-timestamp-overflow" => Some("时间戳溢出: { $timestamp } 超出 41 位容量"),

        "nexus-unsupported-database" => Some("不支持的数据库 URL 方案: { $scheme }"),

        "error-category-permission" => Some("权限"),
        "error-category-injection-risk" => Some("注入风险"),
        "error-category-syntax-error" => Some("语法错误"),
        "error-category-shard-conflict" => Some("分片冲突"),

        "query-error-report" => Some("[{ $category }] { $message }\n建议: { $suggestion }"),
        "query-error-suggestion" => Some("建议: { $suggestion }"),
        "query-error-table" => Some("表: { $table }"),
        "query-error-operation" => Some("操作: { $operation }"),

        "migration" => Some("已应用 { $count } 个迁移"),
        "hello-world" => Some("你好，世界！"),

        "cli-migration-created" => Some("✓ 迁移文件已创建: { $path }"),
        "cli-status-title" => Some("迁移状态查看"),
        "cli-db-connect-failed" => Some("❌ 数据库连接失败: { $error }"),
        "cli-db-type" => Some("📊 数据库类型: { $type }"),
        "cli-migrations-dir" => Some("📁 迁移目录: { $path }"),
        "cli-session-failed" => Some("❌ 无法获取数据库会话: { $error }"),
        "cli-history-load-failed" => Some("⚠️  无法加载迁移历史: { $error }"),
        "cli-history-table-missing" => Some("迁移历史表可能不存在"),
        "cli-applied-count" => Some("✅ 已应用的迁移: { $count } 个"),
        "cli-latest-migration" => Some("最新迁移:"),
        "cli-version" => Some("  - 版本: { $version }"),
        "cli-description" => Some("  - 描述: { $description }"),
        "cli-applied-at" => Some("  - 应用时间: { $time }"),
        "cli-history-details" => Some("迁移历史详情:"),
        "cli-local-files" => Some("📦 本地迁移文件: { $count } 个"),
        "cli-pending-count" => Some("⏳ 待应用的迁移: { $count } 个"),
        "cli-pending-list" => Some("待应用迁移列表:"),
        "cli-all-applied" => Some("✓ 所有迁移都已应用"),
        "cli-db-connected" => Some("🔗 数据库连接: 已连接"),
        "cli-db-url" => Some("   URL: { $url }"),
        "cli-dir-create-failed" => Some("无法创建目录: { $error }"),
        "cli-timestamp-parse-failed" => Some("无法解析时间戳: { $error }"),
        "cli-desc-special-chars-only" => Some("迁移描述不能只包含特殊字符"),
        "cli-desc-too-long" => Some("迁移描述过长（最大 100 字符）"),
        "cli-file-write-failed" => Some("无法写入迁移文件: { $error }"),
        "cli-db-type-detect-failed" => Some("数据库类型检测失败: { $error }"),
        "cli-test-connection-title" => Some("数据库连接测试"),
        "cli-testing-connection" => Some("正在测试数据库连接..."),
        "cli-connection-failed" => Some("连接失败: { $error }"),
        "cli-connection-success" => Some("连接成功!"),
        "cli-connection-time" => Some("连接耗时: { $duration }"),
        "cli-connection-url" => Some("连接 URL: { $url }"),
        "cli-pool-status" => Some("连接池状态:"),
        "cli-total-connections" => Some("总连接数: { $count }"),
        "cli-active-connections" => Some("活跃连接: { $count }"),
        "cli-idle-connections" => Some("空闲连接: { $count }"),
        "cli-connection-verify-failed" => Some("连接验证失败: { $error }"),
        "cli-apply-title" => Some("应用迁移"),
        "cli-no-migration-files" => Some("迁移目录中没有找到迁移文件"),
        "cli-no-pending" => Some("没有待应用的迁移"),
        "cli-found-pending" => Some("找到 { $count } 个待应用迁移"),
        "cli-target-version" => Some("目标版本: { $version }"),
        "cli-starting-apply" => Some("开始应用迁移..."),
        "cli-applying" => Some("正在应用 v{ $version } - { $description } ... "),
        "cli-apply-success" => Some("成功应用 { $success } / { $total } 个迁移"),
        "cli-rollback-title" => Some("回滚迁移"),
        "cli-no-applied-rollback" => Some("没有已应用的迁移可以回滚"),
        "cli-to-rollback-count" => Some("需要回滚 { $count } 个迁移"),
        "cli-mode-rollback-all" => Some("模式: 回滚所有迁移"),
        "cli-mode-rollback-version" => Some("模式: 回滚到版本 { $version }"),
        "cli-mode-rollback-last" => Some("模式: 回滚上一个版本"),
        "cli-starting-rollback" => Some("开始回滚迁移..."),
        "cli-rolling-back" => Some("正在回滚 v{ $version } - { $description } ... "),
        "cli-rollback-error-stop" => Some("回滚过程中发生错误，停止执行"),
        "cli-rollback-success" => Some("成功回滚 { $success } / { $total } 个迁移"),
        "cli-generate-title" => Some("生成迁移文件"),
        "cli-parsing-schema" => Some("解析 Schema 文件..."),
        "cli-schema-read-source-failed" => Some("无法读取源 schema 文件: { $error }"),
        "cli-schema-read-target-failed" => Some("无法读取目标 schema 文件: { $error }"),
        "cli-schema-diff-generated" => Some("已生成 schema 差异 SQL"),
        "cli-no-schema-template" => Some("未提供 schema 文件，已生成空白模板"),
        "cli-output-dir-create-failed" => Some("无法创建输出目录: { $error }"),
        "cli-check-edit-file" => Some("请检查并编辑生成的迁移文件以确保正确性"),
        "cli-list-title" => Some("迁移文件列表"),
        "cli-list-directory" => Some("目录: { $path }"),
        "cli-list-total-count" => Some("共 { $count } 个迁移文件"),

        "pool-invalid-config" => Some("无效的配置: { $error }"),
        "pool-read-config-failed" => Some("读取权限配置文件 '{ $path }' 失败: { $error }"),
        "pool-parse-config-failed" => Some("解析权限配置文件 '{ $path }' 失败: { $error }"),
        "pool-yaml-parse-error" => Some("'{ $source }' 中的 YAML 解析错误: { $error }"),
        "pool-invalid-db-url" => Some("无效的数据库 URL: { $error }"),
        "pool-recreate-failed" => Some("重新创建连接失败: { $error }"),
        "session-txn-begin-failed" => Some("开始事务失败: { $error }"),
        "session-txn-begin-graph-failed" => Some("开始图事务失败: { $error }"),
        "session-txn-commit-failed" => Some("提交图事务失败: { $error }"),
        "session-txn-rollback-failed" => Some("回滚事务失败: { $error }"),
        "session-txn-rollback-graph-failed" => Some("回滚图事务失败: { $error }"),
        "session-ddl-not-allowed" => Some("DDL 操作不允许: { $reason }"),
        "session-ddl-parse-failed" => Some("DDL SQL 解析失败: { $error }"),
        "session-ddl-validation-error" => Some("DDL 验证错误: { $error }"),
        "session-unknown-operation" => Some("未知操作: { $operation }"),
        "session-permission-denied" => Some("对 { $table } 的 { $action } 权限被拒绝"),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::locale::{clear_locale_override, set_locale};

    #[test]
    fn test_translate_en_simple() {
        clear_locale_override();
        set_locale("en").unwrap();
        assert_eq!(t_simple("hello-world"), "Hello, World!");
        assert_eq!(
            t_simple("pool-acquire-timeout"),
            "Failed to acquire connection within timeout"
        );
    }

    #[test]
    fn test_translate_zh_simple() {
        set_locale("zh-CN").unwrap();
        assert_eq!(t_simple("hello-world"), "你好，世界！");
        assert_eq!(t_simple("pool-acquire-timeout"), "无法在超时时间内获取连接");
    }
    #[test]
    fn test_translate_with_args() {
        set_locale("en").unwrap();
        let result = t("config-missing-field", &[("field", "dbnexus.url".to_string())]);
        assert_eq!(result, "Missing required configuration: dbnexus.url");
    }

    #[test]
    fn test_translate_with_args_zh() {
        set_locale("zh-CN").unwrap();
        let result = t(
            "perm-denied",
            &[("operation", "DELETE".to_string()), ("resource", "users".to_string())],
        );
        assert_eq!(result, "对 users 的 DELETE 操作权限被拒绝");
    }

    #[test]
    fn test_translate_unknown_key_returns_key() {
        set_locale("en").unwrap();
        assert_eq!(t_simple("nonexistent-key"), "nonexistent-key");
    }

    #[test]
    fn test_translate_fallback_to_en() {
        // Use a locale that has no translations (e.g. "ar")
        set_locale("ar").unwrap();
        // Should fall back to English
        assert_eq!(t_simple("hello-world"), "Hello, World!");
        clear_locale_override();
    }

    #[test]
    fn test_substitute_multiple_vars() {
        let template = "{ $a } and { $b }";
        let args = [("a", "X".to_string()), ("b", "Y".to_string())];
        assert_eq!(substitute(template, &args), "X and Y");
    }

    #[test]
    fn test_substitute_no_vars() {
        let template = "no placeholders";
        assert_eq!(substitute(template, &[]), "no placeholders");
    }

    #[test]
    fn test_substitute_repeated_var() {
        let template = "{ $x } + { $x } = { $x }";
        let args = [("x", "1".to_string())];
        assert_eq!(substitute(template, &args), "1 + 1 = 1");
    }
}
