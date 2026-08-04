// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Message catalog for i18n translations powered by Fluent.
//!
//! All translations are defined as Fluent (FTL) resource strings,
//! parsed into `FluentBundle` instances at first access and cached
//! via `OnceLock` for the lifetime of the process.
//!
//! Each message key maps to an FTL pattern with `{ $variable }`
//! placeholders resolved at format time by the Fluent engine.

use std::sync::OnceLock;

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

use super::locale::current_locale;
/// Translate a message key to the current locale.
///
/// Looks up `key` in the Fluent message catalog for the current locale,
/// formats any `{ $var }` placeholders with values from `args`,
/// and returns the resolved string.
///
/// Falls back to English if the key is not found in the current locale.
/// Falls back to the key itself if not found in English (should not happen).
pub fn translate(key: &str, args: &[(&str, String)]) -> String {
    let locale = current_locale();
    let lang = locale.id.language.as_str();

    // Try current locale, then English fallback
    format_from_bundle(lang, key, args)
        .or_else(|| format_from_bundle("en", key, args))
        .unwrap_or_else(|| key.to_string())
}

/// Translate a message key to English specifically, regardless of current locale.
pub fn translate_en(key: &str, args: &[(&str, String)]) -> String {
    format_from_bundle("en", key, args).unwrap_or_else(|| key.to_string())
}

/// Shorthand for [`translate()`].
pub fn t(key: &str, args: &[(&str, String)]) -> String {
    translate(key, args)
}

/// Convenience: translate with no dynamic arguments.
pub fn t_simple(key: &str) -> String {
    translate(key, &[])
}

// ============================================================================
// Fluent bundle management
// ============================================================================

/// Cached concurrent Fluent bundles (thread-safe, built once on first access).
static EN_BUNDLE: OnceLock<FluentBundle<FluentResource>> = OnceLock::new();
static ZH_BUNDLE: OnceLock<FluentBundle<FluentResource>> = OnceLock::new();

/// Format a message from the Fluent catalog for the given language.
fn format_from_bundle(lang: &str, key: &str, args: &[(&str, String)]) -> Option<String> {
    let bundle = match lang {
        "zh" => ZH_BUNDLE.get_or_init(build_zh_bundle),
        _ => EN_BUNDLE.get_or_init(build_en_bundle),
    };

    let msg = bundle.get_message(key)?;
    let pattern = msg.value()?;

    let mut fluent_args = FluentArgs::new();
    for (name, value) in args {
        fluent_args.set(*name, FluentValue::from(value.clone()));
    }

    let mut errors = vec![];
    let result = bundle.format_pattern(pattern, Some(&fluent_args), &mut errors);
    Some(result.to_string())
}

fn build_en_bundle() -> FluentBundle<FluentResource> {
    let resource = FluentResource::try_new(EN_FTL.to_string()).unwrap_or_else(|e| e.0);
    let langid: LanguageIdentifier = "en".parse().expect("'en' is a valid language identifier");
    let mut bundle = FluentBundle::new_concurrent(vec![langid]);
    bundle.set_use_isolating(false);
    bundle
        .add_resource(resource)
        .expect("EN resources should add without conflict");
    bundle
}

fn build_zh_bundle() -> FluentBundle<FluentResource> {
    let resource = FluentResource::try_new(ZH_FTL.to_string()).unwrap_or_else(|e| e.0);
    let langid: LanguageIdentifier = "zh".parse().expect("'zh' is a valid language identifier");
    let mut bundle = FluentBundle::new_concurrent(vec![langid]);
    bundle.set_use_isolating(false);
    bundle
        .add_resource(resource)
        .expect("ZH resources should add without conflict");
    bundle
}

// ============================================================================
// English (en) Fluent resources
// ============================================================================

const EN_FTL: &str = r#"
config-missing-field = Missing required configuration: { $field }
config-missing-url = Missing required configuration: dbnexus.url
config-invalid-cache-capacity = Invalid cache capacity: { $reason }
config-invalid-value = Invalid configuration value for '{ $key }': { $message }
config-invalid-format = Invalid configuration format: { $reason }
config-file-not-found = Configuration file not found: { $path }
config-io-error = IO error: { $reason }
config-invalid-url = Invalid URL: { $url }
config-unsupported-protocol = Unsupported database protocol: { $protocol }
config-parse-error = Parse error: { $reason }
config-validation-error = Validation error: { $reason }

db-connection = Database connection error: { $error }
db-config = Configuration error: { $message }
db-permission = Permission denied: { $message }
db-transaction = Transaction error: { $message }
db-migration = Migration error: { $message }
db-cache = Cache error: { $message }
db-query = Query error: { $message }
db-validation = Validation error: { $message }

pool-acquire-timeout = Failed to acquire connection within timeout
pool-exhausted = Connection pool exhausted
pool-connection-failed = Failed to create connection: { $reason }
pool-health-check-failed = Health check failed: { $reason }

migration-file-not-found = Migration file not found: { $path }
migration-parse-error = Failed to parse migration file: { $reason }
migration-execution-error = Migration execution failed: { $reason }
migration-version-conflict = Migration version conflict: { $reason }
migration-rollback-error = Migration rollback failed: { $reason }

audit-write-error = Failed to write audit log: { $reason }
audit-serialization-error = Failed to serialize audit data: { $reason }
audit-config-error = Invalid audit configuration: { $reason }

perm-config-missing-field = missing required field: { $field }
perm-config-invalid-value = invalid value for field '{ $field }': { $reason }
perm-config-policy-not-found = policy file not found: { $path }

perm-denied = permission denied for { $operation } on { $resource }
perm-role-not-found = role not found: { $role }
perm-invalid-policy = invalid policy configuration: { $reason }
perm-rate-limited = rate limit exceeded
perm-parse-error = policy parse error: { $reason }

perm-provider-role-not-found = Role '{ $role }' not found
perm-provider-load-error = Failed to load config: { $reason }
perm-provider-check-error = Permission check failed: { $reason }
perm-provider-unknown = Unknown error: { $reason }

sql-parse-error = Failed to parse SQL: { $reason }
sql-unsupported-statement = Unsupported SQL statement type: { $stmt_type }
sql-empty-statement = Empty SQL statement
sql-multiple-statements = Multiple statements not allowed
sql-contains-variables = SQL statement contains variables: { $details }

sensitive-masking-failed = Masking failed: { $reason }
sensitive-encryption-failed = Encryption failed: { $reason }
sensitive-decryption-failed = Decryption failed: { $reason }
sensitive-invalid-key = Invalid key: { $reason }
sensitive-invalid-input = Invalid input: { $reason }

auth-invalid-credentials = Invalid credentials
auth-token-generation = Token generation failed: { $reason }
auth-invalid-token = Invalid token
auth-token-expired = Token expired
auth-user-not-found = User not found: { $user }
auth-password-hash = Password hash failed: { $reason }
auth-user-limit-reached = User storage limit reached: { $reason }

metrics-export-error = Export failed: { $reason }
metrics-not-initialized = Collector not initialized
metrics-unknown = Unknown metrics error: { $reason }

circuit-breaker = Circuit breaker is { $state }

audit-builder-operation-required = operation is required
audit-builder-entity-type-required = entity_type is required
audit-builder-entity-id-required = entity_id is required

retry-exhausted = Retry exhausted after { $attempts } attempts: { $last_error }
retry-non-retryable = Non-retryable operation: { $error }
retry-timeout = Retry timed out after { $timeout_ms }ms: { $last_error }

saga-execution-failed = Saga execution failed: { $reason }
saga-compensation-failed = Saga compensation failed: { $reason }
saga-timeout = Saga timeout: { $reason }

snowflake-clock-backtrack = Clock backtrack: waited timestamp { $waited_ts } still behind last used { $last_ts }
snowflake-timestamp-overflow = Timestamp overflow: { $timestamp } exceeds 41-bit capacity

nexus-unsupported-database = Unsupported database scheme in URL: { $scheme }

error-category-permission = Permission
error-category-injection-risk = InjectionRisk
error-category-syntax-error = SyntaxError
error-category-shard-conflict = ShardConflict

query-error-report = [{ $category }] { $message } — Suggestion: { $suggestion }
query-error-suggestion = Suggestion: { $suggestion }
query-error-table = Table: { $table }
query-error-operation = Operation: { $operation }

migration = { $count } migrations applied
hello-world = Hello, World!

cli-migration-created = ✓ Migration file created: { $path }
cli-status-title = Migration Status
cli-db-connect-failed = ❌ Database connection failed: { $error }
cli-db-type = 📊 Database type: { $type }
cli-migrations-dir = 📁 Migrations directory: { $path }
cli-session-failed = ❌ Failed to get database session: { $error }
cli-history-load-failed = ⚠️  Failed to load migration history: { $error }
cli-history-table-missing = Migration history table may not exist
cli-applied-count = ✅ Applied migrations: { $count }
cli-latest-migration = Latest migration:
cli-version = - Version: { $version }
cli-description = - Description: { $description }
cli-applied-at = - Applied at: { $time }
cli-history-details = Migration history details:
cli-local-files = 📦 Local migration files: { $count }
cli-pending-count = ⏳ Pending migrations: { $count }
cli-pending-list = Pending migration list:
cli-all-applied = ✓ All migrations have been applied
cli-db-connected = 🔗 Database connection: Connected
cli-db-url = URL: { $url }
cli-dir-create-failed = Failed to create directory: { $error }
cli-timestamp-parse-failed = Failed to parse timestamp: { $error }
cli-desc-special-chars-only = Migration description must not contain only special characters
cli-desc-too-long = Migration description too long (max 100 characters)
cli-file-write-failed = Failed to write migration file: { $error }
cli-db-type-detect-failed = Database type detection failed: { $error }
cli-test-connection-title = Database Connection Test
cli-testing-connection = Testing database connection...
cli-connection-failed = Connection failed: { $error }
cli-connection-success = Connection successful!
cli-connection-time = Connection time: { $duration }
cli-connection-url = Connection URL: { $url }
cli-pool-status = Connection pool status:
cli-total-connections = Total connections: { $count }
cli-active-connections = Active connections: { $count }
cli-idle-connections = Idle connections: { $count }
cli-connection-verify-failed = Connection verification failed: { $error }
cli-apply-title = Apply Migrations
cli-no-migration-files = No migration files found in directory
cli-no-pending = No pending migrations to apply
cli-found-pending = Found { $count } pending migrations
cli-target-version = Target version: { $version }
cli-starting-apply = Starting to apply migrations...
cli-applying = Applying v{ $version } - { $description } ...
cli-apply-success = Successfully applied { $success } / { $total } migrations
cli-rollback-title = Rollback Migrations
cli-no-applied-rollback = No applied migrations to rollback
cli-to-rollback-count = Need to rollback { $count } migrations
cli-mode-rollback-all = Mode: Rollback all migrations
cli-mode-rollback-version = Mode: Rollback to version { $version }
cli-mode-rollback-last = Mode: Rollback last version
cli-starting-rollback = Starting to rollback migrations...
cli-rolling-back = Rolling back v{ $version } - { $description } ...
cli-rollback-error-stop = Error occurred during rollback, stopping execution
cli-rollback-success = Successfully rolled back { $success } / { $total } migrations
cli-generate-title = Generate Migration File
cli-parsing-schema = Parsing schema files...
cli-schema-read-source-failed = Failed to read source schema file: { $error }
cli-schema-read-target-failed = Failed to read target schema file: { $error }
cli-schema-diff-generated = Generated schema diff SQL
cli-no-schema-template = No schema file provided, generated blank template
cli-output-dir-create-failed = Failed to create output directory: { $error }
cli-check-edit-file = Please review and edit the generated migration file for correctness
cli-list-title = Migration File List
cli-list-directory = Directory: { $path }
cli-list-total-count = Total { $count } migration files

pool-invalid-config = Invalid configuration: { $error }
pool-read-config-failed = Failed to read permission config file '{ $path }': { $error }
pool-parse-config-failed = Failed to parse permission config file '{ $path }': { $error }
pool-yaml-parse-error = YAML parse error in '{ $source }': { $error }
pool-invalid-db-url = Invalid database URL: { $error }
pool-recreate-failed = Failed to recreate connections: { $error }
session-txn-begin-failed = Failed to begin transaction: { $error }
session-txn-begin-graph-failed = Failed to begin graph transaction: { $error }
session-txn-commit-failed = Failed to commit graph transaction: { $error }
session-txn-rollback-failed = Failed to rollback transaction: { $error }
session-txn-rollback-graph-failed = Failed to rollback graph transaction: { $error }
session-ddl-not-allowed = DDL operation not allowed: { $reason }
session-ddl-parse-failed = Failed to parse DDL SQL: { $error }
session-ddl-validation-error = DDL validation error: { $error }
session-unknown-operation = Unknown operation: { $operation }
session-permission-denied = Permission denied for { $action } on { $table }
"#;

// ============================================================================
// Chinese (zh) Fluent resources
// ============================================================================

const ZH_FTL: &str = r#"
config-missing-field = 缺少必填配置项: { $field }
config-missing-url = 缺少必填配置项: dbnexus.url
config-invalid-cache-capacity = 无效的缓存容量: { $reason }
config-invalid-value = 配置项 '{ $key }' 的值无效: { $message }
config-invalid-format = 配置格式无效: { $reason }
config-file-not-found = 配置文件未找到: { $path }
config-io-error = IO 错误: { $reason }
config-invalid-url = 无效的 URL: { $url }
config-unsupported-protocol = 不支持的数据库协议: { $protocol }
config-parse-error = 解析错误: { $reason }
config-validation-error = 验证错误: { $reason }

db-connection = 数据库连接错误: { $error }
db-config = 配置错误: { $message }
db-permission = 权限被拒绝: { $message }
db-transaction = 事务错误: { $message }
db-migration = 迁移错误: { $message }
db-cache = 缓存错误: { $message }
db-query = 查询错误: { $message }
db-validation = 验证错误: { $message }

pool-acquire-timeout = 无法在超时时间内获取连接
pool-exhausted = 连接池已耗尽
pool-connection-failed = 创建连接失败: { $reason }
pool-health-check-failed = 健康检查失败: { $reason }

migration-file-not-found = 迁移文件未找到: { $path }
migration-parse-error = 迁移文件解析失败: { $reason }
migration-execution-error = 迁移执行失败: { $reason }
migration-version-conflict = 迁移版本冲突: { $reason }
migration-rollback-error = 迁移回滚失败: { $reason }

audit-write-error = 审计日志写入失败: { $reason }
audit-serialization-error = 审计数据序列化失败: { $reason }
audit-config-error = 审计配置无效: { $reason }

perm-config-missing-field = 缺少必填字段: { $field }
perm-config-invalid-value = 字段 '{ $field }' 的值无效: { $reason }
perm-config-policy-not-found = 策略文件未找到: { $path }

perm-denied = 对 { $resource } 的 { $operation } 操作权限被拒绝
perm-role-not-found = 角色未找到: { $role }
perm-invalid-policy = 策略配置无效: { $reason }
perm-rate-limited = 速率限制已超出
perm-parse-error = 策略解析错误: { $reason }

perm-provider-role-not-found = 角色 '{ $role }' 未找到
perm-provider-load-error = 加载配置失败: { $reason }
perm-provider-check-error = 权限检查失败: { $reason }
perm-provider-unknown = 未知错误: { $reason }

sql-parse-error = SQL 解析失败: { $reason }
sql-unsupported-statement = 不支持的 SQL 语句类型: { $stmt_type }
sql-empty-statement = 空的 SQL 语句
sql-multiple-statements = 不允许多条语句
sql-contains-variables = SQL 语句包含变量: { $details }

sensitive-masking-failed = 脱敏失败: { $reason }
sensitive-encryption-failed = 加密失败: { $reason }
sensitive-decryption-failed = 解密失败: { $reason }
sensitive-invalid-key = 无效的密钥: { $reason }
sensitive-invalid-input = 无效的输入: { $reason }

auth-invalid-credentials = 无效的凭据
auth-token-generation = 令牌生成失败: { $reason }
auth-invalid-token = 无效的令牌
auth-token-expired = 令牌已过期
auth-user-not-found = 用户未找到: { $user }
auth-password-hash = 密码哈希失败: { $reason }
auth-user-limit-reached = 用户存储已达上限: { $reason }

metrics-export-error = 导出失败: { $reason }
metrics-not-initialized = 收集器未初始化
metrics-unknown = 未知指标错误: { $reason }

circuit-breaker = 断路器处于 { $state } 状态

audit-builder-operation-required = 操作类型为必填项
audit-builder-entity-type-required = 实体类型为必填项
audit-builder-entity-id-required = 实体 ID 为必填项

retry-exhausted = 重试 { $attempts } 次后仍失败: { $last_error }
retry-non-retryable = 不可重试的操作: { $error }
retry-timeout = 重试在 { $timeout_ms }ms 后超时: { $last_error }

saga-execution-failed = Saga 执行失败: { $reason }
saga-compensation-failed = Saga 补偿失败: { $reason }
saga-timeout = Saga 超时: { $reason }

snowflake-clock-backtrack = 时钟回拨: 等待后的时间戳 { $waited_ts } 仍落后于上次使用的 { $last_ts }
snowflake-timestamp-overflow = 时间戳溢出: { $timestamp } 超出 41 位容量

nexus-unsupported-database = 不支持的数据库 URL 方案: { $scheme }

error-category-permission = 权限
error-category-injection-risk = 注入风险
error-category-syntax-error = 语法错误
error-category-shard-conflict = 分片冲突

query-error-report = [{ $category }] { $message } — 建议: { $suggestion }
query-error-suggestion = 建议: { $suggestion }
query-error-table = 表: { $table }
query-error-operation = 操作: { $operation }

migration = 已应用 { $count } 个迁移
hello-world = 你好，世界！

cli-migration-created = ✓ 迁移文件已创建: { $path }
cli-status-title = 迁移状态查看
cli-db-connect-failed = ❌ 数据库连接失败: { $error }
cli-db-type = 📊 数据库类型: { $type }
cli-migrations-dir = 📁 迁移目录: { $path }
cli-session-failed = ❌ 无法获取数据库会话: { $error }
cli-history-load-failed = ⚠️  无法加载迁移历史: { $error }
cli-history-table-missing = 迁移历史表可能不存在
cli-applied-count = ✅ 已应用的迁移: { $count } 个
cli-latest-migration = 最新迁移:
cli-version = - 版本: { $version }
cli-description = - 描述: { $description }
cli-applied-at = - 应用时间: { $time }
cli-history-details = 迁移历史详情:
cli-local-files = 📦 本地迁移文件: { $count } 个
cli-pending-count = ⏳ 待应用的迁移: { $count } 个
cli-pending-list = 待应用迁移列表:
cli-all-applied = ✓ 所有迁移都已应用
cli-db-connected = 🔗 数据库连接: 已连接
cli-db-url = URL: { $url }
cli-dir-create-failed = 无法创建目录: { $error }
cli-timestamp-parse-failed = 无法解析时间戳: { $error }
cli-desc-special-chars-only = 迁移描述不能只包含特殊字符
cli-desc-too-long = 迁移描述过长（最大 100 字符）
cli-file-write-failed = 无法写入迁移文件: { $error }
cli-db-type-detect-failed = 数据库类型检测失败: { $error }
cli-test-connection-title = 数据库连接测试
cli-testing-connection = 正在测试数据库连接...
cli-connection-failed = 连接失败: { $error }
cli-connection-success = 连接成功!
cli-connection-time = 连接耗时: { $duration }
cli-connection-url = 连接 URL: { $url }
cli-pool-status = 连接池状态:
cli-total-connections = 总连接数: { $count }
cli-active-connections = 活跃连接: { $count }
cli-idle-connections = 空闲连接: { $count }
cli-connection-verify-failed = 连接验证失败: { $error }
cli-apply-title = 应用迁移
cli-no-migration-files = 迁移目录中没有找到迁移文件
cli-no-pending = 没有待应用的迁移
cli-found-pending = 找到 { $count } 个待应用迁移
cli-target-version = 目标版本: { $version }
cli-starting-apply = 开始应用迁移...
cli-applying = 正在应用 v{ $version } - { $description } ...
cli-apply-success = 成功应用 { $success } / { $total } 个迁移
cli-rollback-title = 回滚迁移
cli-no-applied-rollback = 没有已应用的迁移可以回滚
cli-to-rollback-count = 需要回滚 { $count } 个迁移
cli-mode-rollback-all = 模式: 回滚所有迁移
cli-mode-rollback-version = 模式: 回滚到版本 { $version }
cli-mode-rollback-last = 模式: 回滚上一个版本
cli-starting-rollback = 开始回滚迁移...
cli-rolling-back = 正在回滚 v{ $version } - { $description } ...
cli-rollback-error-stop = 回滚过程中发生错误，停止执行
cli-rollback-success = 成功回滚 { $success } / { $total } 个迁移
cli-generate-title = 生成迁移文件
cli-parsing-schema = 解析 Schema 文件...
cli-schema-read-source-failed = 无法读取源 schema 文件: { $error }
cli-schema-read-target-failed = 无法读取目标 schema 文件: { $error }
cli-schema-diff-generated = 已生成 schema 差异 SQL
cli-no-schema-template = 未提供 schema 文件，已生成空白模板
cli-output-dir-create-failed = 无法创建输出目录: { $error }
cli-check-edit-file = 请检查并编辑生成的迁移文件以确保正确性
cli-list-title = 迁移文件列表
cli-list-directory = 目录: { $path }
cli-list-total-count = 共 { $count } 个迁移文件

pool-invalid-config = 无效的配置: { $error }
pool-read-config-failed = 读取权限配置文件 '{ $path }' 失败: { $error }
pool-parse-config-failed = 解析权限配置文件 '{ $path }' 失败: { $error }
pool-yaml-parse-error = '{ $source }' 中的 YAML 解析错误: { $error }
pool-invalid-db-url = 无效的数据库 URL: { $error }
pool-recreate-failed = 重新创建连接失败: { $error }
session-txn-begin-failed = 开始事务失败: { $error }
session-txn-begin-graph-failed = 开始图事务失败: { $error }
session-txn-commit-failed = 提交图事务失败: { $error }
session-txn-rollback-failed = 回滚事务失败: { $error }
session-txn-rollback-graph-failed = 回滚图事务失败: { $error }
session-ddl-not-allowed = DDL 操作不允许: { $reason }
session-ddl-parse-failed = DDL SQL 解析失败: { $error }
session-ddl-validation-error = DDL 验证错误: { $error }
session-unknown-operation = 未知操作: { $operation }
session-permission-denied = 对 { $table } 的 { $action } 权限被拒绝
"#;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Direct bundle tests (no global locale state — safe for parallel execution)

    #[test]
    fn test_fluent_en_simple() {
        assert_eq!(
            format_from_bundle("en", "hello-world", &[]),
            Some("Hello, World!".to_string())
        );
        assert_eq!(
            format_from_bundle("en", "pool-acquire-timeout", &[]),
            Some("Failed to acquire connection within timeout".to_string())
        );
    }

    #[test]
    fn test_fluent_zh_simple() {
        assert_eq!(
            format_from_bundle("zh", "hello-world", &[]),
            Some("你好，世界！".to_string())
        );
        assert_eq!(
            format_from_bundle("zh", "pool-acquire-timeout", &[]),
            Some("无法在超时时间内获取连接".to_string())
        );
    }

    #[test]
    fn test_fluent_en_with_args() {
        let result = format_from_bundle("en", "config-missing-field", &[("field", "dbnexus.url".to_string())]);
        assert_eq!(result, Some("Missing required configuration: dbnexus.url".to_string()));
    }

    #[test]
    fn test_fluent_zh_with_args() {
        let result = format_from_bundle(
            "zh",
            "perm-denied",
            &[("operation", "DELETE".to_string()), ("resource", "users".to_string())],
        );
        assert_eq!(result, Some("对 users 的 DELETE 操作权限被拒绝".to_string()));
    }

    #[test]
    fn test_fluent_unknown_key_returns_none() {
        assert_eq!(format_from_bundle("en", "nonexistent-key", &[]), None);
    }

    #[test]
    fn test_fluent_fallback_lang_uses_en() {
        // Unknown language falls back to EN bundle
        assert_eq!(
            format_from_bundle("ar", "hello-world", &[]),
            Some("Hello, World!".to_string())
        );
    }

    #[test]
    fn test_fluent_multiple_vars() {
        let result = format_from_bundle(
            "en",
            "session-permission-denied",
            &[("action", "SELECT".to_string()), ("table", "users".to_string())],
        );
        assert_eq!(result, Some("Permission denied for SELECT on users".to_string()));
    }

    #[test]
    fn test_fluent_zh_pool_exhausted() {
        assert_eq!(
            format_from_bundle("zh", "pool-exhausted", &[]),
            Some("连接池已耗尽".to_string())
        );
    }
}
