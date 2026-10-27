// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Message catalog for i18n translations powered by Fluent.
//!
//! All translations are defined as Fluent (FTL) resource strings,
//! parsed into `FluentBundle` instances at first access and cached
//! via `OnceLock` for the lifetime of the process.
//!
//! Each message key maps to an FTL pattern with `{ $variable }`
//! placeholders resolved at format time by the Fluent engine.

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

/// Translate a message key with no arguments — the canonical entry point.
///
/// Same contract as [`translate()`]: the current locale's bundle first, the
/// `en` bundle second, the bare key as the last resort. The default locale is
/// English unless the user explicitly selects another language
/// (see [`super::locale`]).
pub fn tr(key: &str) -> String {
    translate(key, &[])
}

/// Translate a message key with `{ $name }` arguments — the canonical entry
/// point for parameterized messages.
///
/// `args` pairs carry the bare placeholder name (no `$`), e.g.
/// `tr_args("cli-invalid-limit", &[("limit", limit.to_string())])`.
pub fn tr_args(key: &str, args: &[(&str, String)]) -> String {
    translate(key, args)
}

// Fluent bundle management

/// Cached concurrent Fluent bundles (thread-safe, built once on first access).
/// Format a message from the Fluent catalog for the given language.
///
/// Visible to the crate so `DbI18nFormatter` can resolve messages for an
/// explicit locale instead of the process-global current locale.
pub(crate) fn format_from_bundle(lang: &str, key: &str, args: &[(&str, String)]) -> Option<String> {
    let bundle = build_bundle(lang);

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

// Embedded Fluent resources (compile-time mirror of the locales/ directory)

/// Language packs embedded at compile time, as `(locales-relative path, FTL source)`.
///
/// The translated text lives only in the `.ftl` files; the table is kept in
/// lockstep with the on-disk `locales/` directory by the
/// `test_embedded_locales_match_disk` sync guard.
pub const EMBEDDED_LOCALES: &[(&str, &str)] = &[
    (
        "locales/en/errors.ftl",
        include_str!("../../locales/en/errors.ftl"),
    ),
    (
        "locales/en/logs.ftl",
        include_str!("../../locales/en/logs.ftl"),
    ),
    (
        "locales/en/cli.ftl",
        include_str!("../../locales/en/cli.ftl"),
    ),
    (
        "locales/en/examples.ftl",
        include_str!("../../locales/en/examples.ftl"),
    ),
    (
        "locales/zh/errors.ftl",
        include_str!("../../locales/zh/errors.ftl"),
    ),
    (
        "locales/zh/logs.ftl",
        include_str!("../../locales/zh/logs.ftl"),
    ),
    (
        "locales/zh/cli.ftl",
        include_str!("../../locales/zh/cli.ftl"),
    ),
    (
        "locales/zh/examples.ftl",
        include_str!("../../locales/zh/examples.ftl"),
    ),
];

/// Build the Fluent bundle of one language from its embedded language packs.
///
/// Resources are added in [`EMBEDDED_LOCALES`] order. A key defined twice in
/// one language panics here: a conflicting catalog must never ship silently.
fn build_bundle(lang: &str) -> FluentBundle<FluentResource> {
    // 未知 locale 回退 EN（既有契约）：请求语言无内嵌资源时用 EN 包，
    // 避免空 bundle 让所有查询静默返回 None。
    let requested = if EMBEDDED_LOCALES
        .iter()
        .any(|(p, _)| p.starts_with(&format!("locales/{lang}/")))
    {
        lang
    } else {
        "en"
    };
    let prefix = format!("locales/{requested}/");
    let langid: LanguageIdentifier = lang
        .parse()
        .expect("language pack directories use valid language identifiers");
    let mut bundle = FluentBundle::new_concurrent(vec![langid]);
    bundle.set_use_isolating(false);
    for &(path, ftl) in EMBEDDED_LOCALES {
        if !path.starts_with(&prefix) {
            continue;
        }
        let resource = FluentResource::try_new(ftl.to_string()).unwrap_or_else(|e| e.0);
        if let Err(errors) = bundle.add_resource(resource) {
            panic!("FTL resource {path} conflicts with an earlier key: {errors:?}");
        }
    }
    bundle
}

// Tests

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
        let result = format_from_bundle(
            "en",
            "config-missing-field",
            &[("field", "dbnexus.url".to_string())],
        );
        assert_eq!(
            result,
            Some("Missing required configuration: dbnexus.url".to_string())
        );
    }

    #[test]
    fn test_fluent_zh_with_args() {
        let result = format_from_bundle(
            "zh",
            "perm-denied",
            &[
                ("operation", "DELETE".to_string()),
                ("resource", "users".to_string()),
            ],
        );
        assert_eq!(
            result,
            Some("对 users 的 DELETE 操作权限被拒绝".to_string())
        );
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
            &[
                ("action", "SELECT".to_string()),
                ("table", "users".to_string()),
            ],
        );
        assert_eq!(
            result,
            Some("Permission denied for SELECT on users".to_string())
        );
    }

    #[test]
    fn test_fluent_zh_pool_exhausted() {
        assert_eq!(
            format_from_bundle("zh", "pool-exhausted", &[]),
            Some("连接池已耗尽".to_string())
        );
    }

    /// db-rate-limited 双语模板：携带建议秒数走 -retry 键插值
    /// retry_after_secs（防「死参数」回归），无建议走无秒数键
    #[test]
    fn test_fluent_rate_limited_retry_after_interpolation() {
        assert_eq!(
            format_from_bundle(
                "en",
                "db-rate-limited-retry",
                &[("retry_after_secs", "7".to_string())]
            ),
            Some("Rate limit exceeded: too many requests; retry after 7s".to_string())
        );
        assert_eq!(
            format_from_bundle("en", "db-rate-limited", &[]),
            Some("Rate limit exceeded: too many requests, please retry later".to_string())
        );
        assert_eq!(
            format_from_bundle(
                "zh",
                "db-rate-limited-retry",
                &[("retry_after_secs", "7".to_string())]
            ),
            Some("请求过于频繁，已被速率限制，7 秒后重试".to_string())
        );
        assert_eq!(
            format_from_bundle("zh", "db-rate-limited", &[]),
            Some("请求过于频繁，已被速率限制，请稍后重试".to_string())
        );
    }

    /// 守卫:error-code 与 query-error-suggestion-code-table 在 en/zh 双束均存在
    /// （缺键时 to_localized_string() 会退化为裸键）
    #[test]
    fn test_error_code_and_suggestion_keys_present_both_bundles() {
        let args = &[
            ("code", "3001".to_string()),
            ("name", "SqlSyntax".to_string()),
        ];
        assert_eq!(
            format_from_bundle("en", "error-code", args),
            Some("Error code: 3001 (SqlSyntax)".to_string())
        );
        assert_eq!(
            format_from_bundle("zh", "error-code", args),
            Some("错误码: 3001（SqlSyntax）".to_string())
        );
        assert_eq!(
            format_from_bundle("en", "query-error-suggestion-code-table", &[]),
            Some("Refer to the error code table to determine the handling strategy".to_string())
        );
        assert_eq!(
            format_from_bundle("zh", "query-error-suggestion-code-table", &[]),
            Some("参考错误码表定位处理策略".to_string())
        );
    }

    /// 守卫：错误消息与日志接线的新键在 en/zh 双束均存在
    /// （缺键时 to_localized_string() / t() 会退化为裸键）
    #[test]
    fn test_new_wired_keys_present_both_bundles() {
        const NEW_KEYS: &[&str] = &[
            "db-unsupported",
            "db-version-conflict",
            "circuit-breaker-closed",
            "circuit-breaker-opened",
            "circuit-breaker-reopened",
            "circuit-breaker-half-open",
            "metrics-collector-not-configured",
            "metrics-export-join-failed",
            "metrics-slow-query",
            "metrics-feature-not-enabled",
            "pool-log-acquire-timeout",
            "saga-compensation-no-pool",
            "saga-compensation-session-unavailable",
            "session-for-update-txn-required",
            "session-for-update-relational-required",
            "session-isolation-relational-required",
            "session-log-sql-parse-denied",
            "session-log-permission-denied",
            "session-log-rate-limited",
            "cli-invalid-limit",
            "cli-invalid-operation",
            "cli-invalid-severity",
            "cli-invalid-status",
            "cli-invalid-since",
            "cli-invalid-until",
            "cli-invalid-total-shards",
            "cli-invalid-strategy",
            "cli-invalid-route-key",
            "cli-empty-role-or-table",
            "cli-invalid-action",
            "cli-permissions-file-unavailable",
            "cli-permissions-file-load-failed",
            "cli-permissions-file-invalid",
            "cli-unsupported-db-url-protocol",
            "perm-rate-limiter-error",
            "perm-rate-limit-audit-dropped",
            "copy-path-empty",
            "copy-insert-column-required",
            "copy-insert-row-required",
            "copy-insert-table-invalid",
            "copy-insert-column-invalid",
            "copy-in-incompatible-connection",
            "copy-payload-path-utf8",
            "copy-payload-cleanup-failed",
            "copy-driver-feature-missing",
            "copy-payload-create-failed",
            "copy-payload-create-io-failed",
            "copy-payload-write-join-failed",
            "duckdb-execute-batch-failed",
            "duckdb-copy-prepare-failed",
            "duckdb-copy-execute-failed",
            "duckdb-copy-fetch-failed",
            "duckdb-copy-count-read-failed",
            "duckdb-pool-exhausted",
            "duckdb-txn-begin-failed",
            "duckdb-txn-commit-failed",
            "duckdb-spawn-blocking-join-failed",
            "repository-version-column-unsafe",
            "repository-version-field-missing",
            "repository-version-column-integer",
            "repository-column-unsafe",
            "repository-entity-deserialize-failed",
            "session-already-in-transaction",
            "session-already-in-transaction-concurrent",
            "session-already-in-graph-transaction",
            "session-connection-not-available",
            "shard-route-unregistered",
            "migration-history-table-invalid",
            "query-cache-get-failed",
            "query-cache-decode-failed",
            "query-cache-encode-failed",
            "query-cache-fill-failed",
            "query-cache-clock-before-epoch",
            "query-cache-invalidate-failed",
            "query-cache-tables-required",
            "query-cache-version-read-failed",
            "query-cache-params-encode-failed",
            "query-cache-table-name-invalid",
            "example-replica-routing-api-replica-pool-new",
            // i18n 审计接线批次（zh 硬编码英文化 + 硬编码英文字符串入目录）
            "migration-rollback-no-down-section",
            "migration-rollback-down-not-executable",
            "migration-identifier-empty",
            "migration-identifier-too-long",
            "migration-identifier-invalid-chars",
            "migration-identifier-reserved-keyword",
            "migration-identifier-type-table",
            "migration-identifier-type-primary-key-column",
            "migration-identifier-type-column",
            "migration-identifier-type-index",
            "migration-identifier-type-index-column",
            "migration-identifier-type-new-column",
            "migration-identifier-type-fk-table",
            "migration-identifier-type-fk-constraint",
            "migration-identifier-type-fk-column",
            "migration-identifier-type-fk-referenced-table",
            "migration-identifier-type-fk-referenced-column",
            "migration-sql-comment-create-table",
            "migration-sql-comment-drop-table",
            "migration-sql-comment-alter-table",
            "migration-sql-comment-add-column",
            "migration-sql-comment-drop-column",
            "migration-sql-comment-add-index",
            "migration-sql-comment-drop-index",
            "migration-sql-comment-add-foreign-key",
            "migration-sql-comment-drop-foreign-key",
            "migration-sqlite-drop-column-note",
            "migration-sqlite-modify-column-note",
            "migration-sqlite-modify-type-note",
            "migration-sqlite-modify-nullability-note",
            "migration-sqlite-modify-default-note",
            "migration-scan-read-dir-failed",
            "migration-scan-read-entry-failed",
            "migration-scan-read-file-failed",
            "config-confers-subscribe-failed",
            "config-confers-hot-reload-failed",
            "config-confers-skip-invalid-event",
            "config-confers-log-hot-reload",
            "config-confers-log-ack-failed",
            "perm-config-path-empty",
            "perm-config-path-parent-reference",
            "perm-config-path-safety-validation",
            "perm-config-yaml-feature-not-enabled",
            "perm-provider-memory-policy-table-uninitialized",
            "perm-provider-yaml-file-unreadable",
            "perm-provider-config-read-error",
            "perm-provider-lock-error",
            "cli-migration-file-not-found-for-rollback",
            "cli-migration-rollback-failed",
            "cli-schema-diff-up-placeholder",
            "cli-schema-diff-down-placeholder",
            "cli-invalid-database-url-format",
            "cli-database-not-supported",
            "cli-unsupported-database-protocol",
            "session-already-in-graph-transaction-concurrent",
            "session-not-in-transaction",
            "session-no-active-transaction-commit",
            "session-no-active-transaction-rollback",
            "session-commit-transaction-in-use",
            "session-rollback-transaction-in-use",
            "session-duckdb-ddl-admin-only",
            "session-ddl-admin-only",
            "health-check-timeout",
            "health-check-timeout-recommendation",
            "health-recommendation-check-connection-config",
            "health-recommendation-check-network",
            "health-recommendation-ensure-pool-initialized",
            "health-recommendation-wait-circuit-breaker",
            "health-recommendation-increase-pool-size",
            "health-recommendation-check-connection-leak",
            "health-recommendation-increase-min-connections",
            "health-status-failure-rate-too-high",
            "health-status-no-connections",
            "health-status-circuit-breaker-open",
            "health-status-pool-full",
            "health-status-no-idle-connections",
            "health-details-pool",
            "health-details-stats",
            "health-details-circuit-breaker",
            "shard-migration-orchestration-not-implemented",
            "neo4j-connect-failed",
            "neo4j-url-no-credentials-env",
            "neo4j-url-invalid-and-env-missing",
            "neo4j-not-connected",
            "neo4j-execute-failed",
            "neo4j-execute-with-params-failed",
            "neo4j-row-fetch-failed",
            "neo4j-row-deserialize-failed",
            "neo4j-health-check-no-rows",
            "neo4j-start-txn-failed",
            "neo4j-transaction-consumed",
            "neo4j-commit-failed",
            "neo4j-rollback-failed",
            "neo4j-txn-execute-failed",
            "neo4j-txn-execute-with-params-failed",
            "neo4j-txn-row-fetch-failed",
            "neo4j-txn-row-deserialize-failed",
            "otel-serialize-failed",
            "otel-http-task-failed",
            "otel-endpoint-not-http",
            "otel-connect-failed",
            "otel-write-failed",
            "otel-read-failed",
            "otel-collector-non-2xx",
            "otel-export-failed",
            "kit-health-no-connections",
            "kit-health-waiting",
            "kit-health-no-idle",
            "perm-cache-log-insert-failed",
            "perm-cache-log-invalidate-failed",
            "perm-cache-log-clear-failed",
            "prepare-cache-log-read-miss",
            "prepare-cache-log-write-failed",
            "prepare-cache-log-clear-failed",
            // i18n 接线批次（权限缓存错误文案 + 示例标题行）
            "perm-cache-provider-no-policy",
            "perm-cache-provider-not-configured",
            // i18n 复核批次：YAML 权限提供器解析错误补接线（上一轮建键漏改调用点）
            "perm-provider-yaml-parse-error",
            "example-basic-crud-title",
            "example-audit-logging-title",
            "example-rate-limiter-title",
            "example-kit-usage-title",
            "example-health-check-title",
            // 散射查询 / 审计清理 / 迁移差异 / 权限配置的错误载荷接线
            "scatter-query-timed-out",
            "scatter-partial-failure",
            "audit-cleanup-invalid-date",
            "migration-graph-alter-unsupported",
            "perm-config-role-not-found",
            // 会话层硬编码错误载荷接线（execute/duckdb/mod 同文件口径不一致）
            "session-ddl-not-allowed-context",
            "session-ddl-not-allowed-duckdb",
            "session-permission-table-name-required",
            "session-permission-table-extract-failed",
            "session-permission-parse-failed",
            "session-sql-parser-feature-required",
            "session-sql-parser-feature-required-secure",
            "session-rls-parse-failed",
            "session-rls-not-applicable",
            "session-query-rows-select-only",
            "session-query-rows-sqlite-single-table",
            "session-query-rows-sqlite-postgres-only",
            "session-batch-rollback-failed",
            "session-cypher-length-exceeded",
            "session-cypher-multiple-statements",
            "session-cypher-line-comment",
            "session-cypher-block-comment",
            "session-cypher-dangerous-procedure",
            // i18n 审计接线批次：口令 bcrypt 上限 / RLS 策略错误与日志 /
            // 图操作权限门 / data-api 与查询片段 fail-closed / URL 解析
            "auth-password-too-long-bcrypt",
            "rls-unsafe-predicate-column",
            "rls-set-operations-unsupported",
            "graph-op-denied-admin-required",
            "data-api-owner-column-unsafe",
            "data-api-query-role-not-configured",
            "repository-query-role-not-configured",
            "query-fragment-column-unsafe",
            "query-fragment-table-unsafe",
            "query-fragment-condition-column-unsafe",
            "query-fragment-order-column-unsafe",
            "session-rls-multi-table-unsafe",
            "session-rls-single-table-required",
            "nexus-url-parse-failed",
            "auth-log-policy-max-len-exceeds-bcrypt",
            "auth-log-legacy-verify-exceeds-limit",
            "auth-log-legacy-verify-split-multibyte",
            "session-log-rls-rejected",
        ];
        for key in NEW_KEYS {
            assert!(
                format_from_bundle("en", key, &[]).is_some(),
                "en bundle missing key: {key}"
            );
            assert!(
                format_from_bundle("zh", key, &[]).is_some(),
                "zh bundle missing key: {key}"
            );
        }
    }

    /// 单条精确值断言用例：(键, 格式化参数, en 期望, zh 期望)
    type ExactValueCase = (
        &'static str,
        &'static [(&'static str, &'static str)],
        &'static str,
        &'static str,
    );

    /// 守卫：本批接线键的 en/zh 模板精确值——en 为代码基准文案，
    /// zh 术语对齐既有资源（连接池/join/标识符等），双侧任一漂移即失败
    #[test]
    fn test_batch_wired_keys_en_zh_exact_values() {
        let cases: &[ExactValueCase] = &[
            (
                "copy-insert-table-invalid",
                &[("table", "t 9")],
                "INSERT target table identifier is invalid: 't 9' (allowed: letters/digits/underscore/dot)",
                "INSERT 目标表标识符无效: 't 9'（允许字母/数字/下划线/点）",
            ),
            (
                "copy-insert-column-invalid",
                &[("column", "c1")],
                "INSERT column identifier is invalid: 'c1' (allowed: letters/digits/underscore/dot)",
                "INSERT 列标识符无效: 'c1'（允许字母/数字/下划线/点）",
            ),
            (
                "copy-payload-write-join-failed",
                &[("error", "E")],
                "DuckDB COPY payload write task join failed: E",
                "DuckDB COPY 载荷写入任务 join 失败: E",
            ),
            (
                "copy-payload-create-io-failed",
                &[("error", "E")],
                "DuckDB COPY payload temp file creation failed: E",
                "DuckDB COPY 载荷临时文件创建失败: E",
            ),
            (
                "repository-column-unsafe",
                &[("column", "c")],
                "repository column must be a safe identifier: 'c'",
                "repository 列必须是安全标识符: 'c'",
            ),
            (
                "duckdb-pool-exhausted",
                &[],
                "DuckDB pool exhausted: no connection available",
                "DuckDB 连接池已耗尽: 无可用连接",
            ),
            (
                "duckdb-txn-begin-failed",
                &[("error", "E")],
                "DuckDB begin transaction failed: E",
                "DuckDB 开始事务失败: E",
            ),
            (
                "duckdb-txn-commit-failed",
                &[("error", "E")],
                "DuckDB commit failed: E",
                "DuckDB 提交事务失败: E",
            ),
            (
                "duckdb-spawn-blocking-join-failed",
                &[("error", "E")],
                "spawn_blocking join failed: E",
                "spawn_blocking 任务 join 失败: E",
            ),
            (
                "session-already-in-transaction",
                &[],
                "Already in transaction",
                "已处于事务中",
            ),
            (
                "session-already-in-transaction-concurrent",
                &[],
                "Already in transaction (concurrent begin detected)",
                "已处于事务中（检测到并发开启）",
            ),
            (
                "session-already-in-graph-transaction",
                &[],
                "Already in graph transaction",
                "已处于图事务中",
            ),
            (
                "session-connection-not-available",
                &[],
                "Connection not available - Session may have been invalidated",
                "连接不可用 - 会话可能已失效",
            ),
            (
                "cli-unsupported-db-url-protocol",
                &[],
                "unsupported database URL protocol",
                "不支持的数据库 URL 协议",
            ),
            (
                "repository-entity-deserialize-failed",
                &[("error", "E")],
                "entity deserialize failed: E",
                "实体反序列化失败: E",
            ),
            (
                "example-replica-routing-api-replica-pool-new",
                &[],
                "- ReplicaPool::new(pool, lag_detector)",
                "- ReplicaPool::new(pool, lag_detector)",
            ),
            (
                "perm-cache-provider-no-policy",
                &[("role", "admin")],
                "permission provider has no policy for role 'admin'",
                "权限提供器没有角色 'admin' 的策略",
            ),
            (
                "perm-cache-provider-not-configured",
                &[("role", "admin")],
                "no permission provider configured for role 'admin'",
                "未为角色 'admin' 配置权限提供器",
            ),
            (
                "example-basic-crud-title",
                &[],
                "📝 DBNexus basic CRUD example",
                "📝 DBNexus 基础 CRUD 示例",
            ),
            (
                "example-audit-logging-title",
                &[],
                "📝 DBNexus audit logging example",
                "📝 DBNexus 审计日志示例",
            ),
            (
                "example-rate-limiter-title",
                &[],
                "⏱  DBNexus rate limiter example",
                "⏱  DBNexus 速率限制器示例",
            ),
            (
                "example-kit-usage-title",
                &[],
                "🧰 DBNexus AsyncKit + DbNexusModule dependency injection example",
                "🧰 DBNexus AsyncKit + DbNexusModule 依赖注入示例",
            ),
            (
                "example-health-check-title",
                &[],
                "🩺 DBNexus health check and circuit breaker example",
                "🩺 DBNexus 健康检查与熔断器示例",
            ),
            (
                "perm-provider-yaml-parse-error",
                &[("path", "p.yaml"), ("error", "E")],
                "YAML parse error (path='p.yaml'): E",
                "YAML 解析错误（path='p.yaml'）: E",
            ),
            (
                "cli-help-migrate",
                &[],
                "Apply all pending migrations in the migrations directory (machine-readable output, exit codes 0/1/2)",
                "应用迁移目录中的所有待应用迁移（机器可读输出，退出码 0/1/2）",
            ),
            (
                "cli-help-audit-query-operation",
                &[],
                "Filter by operation type (create/read/update/delete/login/logout/permission-change/config-change/other:<text>)",
                "按操作类型过滤（create/read/update/delete/login/logout/permission-change/config-change/other:<text>）",
            ),
            (
                "cli-help-shard-info-template",
                &[],
                "Connection string template (shard placeholder)",
                "连接字符串模板（shard 占位符）",
            ),
            (
                "scatter-query-timed-out",
                &[],
                "Scatter-gather query timed out",
                "Scatter-gather 查询超时",
            ),
            (
                "scatter-partial-failure",
                &[("shards", "2")],
                "Scatter-gather failed: 2 shard(s) failed",
                "Scatter-gather 失败：2 个分片失败",
            ),
            (
                "audit-cleanup-invalid-date",
                &[],
                "Invalid date calculation",
                "日期计算无效",
            ),
            (
                "migration-graph-alter-unsupported",
                &[],
                "Graph databases do not support relational ALTER TABLE operations",
                "图数据库不支持关系型 ALTER TABLE 操作",
            ),
            (
                "perm-config-role-not-found",
                &[],
                "Role not found in permission config",
                "权限配置中未找到该角色",
            ),
            (
                "session-ddl-not-allowed-context",
                &[],
                "DDL operations are not allowed in this context",
                "当前上下文不允许 DDL 操作",
            ),
            (
                "session-ddl-not-allowed-duckdb",
                &[],
                "DDL operations are not allowed in DuckDB query context",
                "DuckDB 查询上下文不允许 DDL 操作",
            ),
            (
                "session-permission-table-name-required",
                &[],
                "SQL statement requires a valid table name for permission checking",
                "SQL 语句缺少用于权限检查的有效表名",
            ),
            (
                "session-permission-table-extract-failed",
                &[],
                "Failed to extract table name for permission checking",
                "提取权限检查所需表名失败",
            ),
            (
                "session-permission-parse-failed",
                &[],
                "Failed to parse SQL statement for permission checking",
                "解析权限检查所需 SQL 语句失败",
            ),
            (
                "session-sql-parser-feature-required",
                &[("function", "execute_raw")],
                "execute_raw requires the sql-parser feature to be enabled",
                "execute_raw 需要启用 sql-parser 特性",
            ),
            (
                "session-sql-parser-feature-required-secure",
                &[("function", "execute_duckdb")],
                "execute_duckdb requires the sql-parser feature to be enabled for security checks",
                "execute_duckdb 需要启用 sql-parser 特性以执行安全检查",
            ),
            (
                "session-rls-parse-failed",
                &[],
                "failed to parse SQL statement for row-level security",
                "解析行级安全所需的 SQL 语句失败",
            ),
            (
                "session-rls-not-applicable",
                &[],
                "row-level security policy cannot be applied to this statement",
                "行级安全策略无法应用于该语句",
            ),
            (
                "session-query-rows-select-only",
                &[],
                "query_rows only allows SELECT statements",
                "query_rows 只允许 SELECT 语句",
            ),
            (
                "session-query-rows-sqlite-single-table",
                &[],
                "query_rows on sqlite (MVP) requires a single-table SELECT statement",
                "sqlite 上的 query_rows（MVP）要求单表 SELECT 语句",
            ),
            (
                "session-query-rows-sqlite-postgres-only",
                &[],
                "query_rows MVP supports postgres/sqlite backends only",
                "query_rows 的 MVP 仅支持 postgres/sqlite 后端",
            ),
            (
                "session-batch-rollback-failed",
                &[("error", "E"), ("rollback_error", "R")],
                "batch failed: E; rollback also failed: R",
                "批量执行失败: E; 回滚也失败: R",
            ),
            (
                "session-cypher-length-exceeded",
                &[("max", "10240"), ("actual", "20480")],
                "Cypher query exceeds maximum length (10240 bytes, got 20480 bytes) - potential DoS payload",
                "Cypher 查询超过最大长度（10240 字节，实际 20480 字节）- 疑似 DoS 载荷",
            ),
            (
                "session-cypher-multiple-statements",
                &[],
                "Cypher query contains multiple statements (';' inside query) - potential injection",
                "Cypher 查询包含多条语句（查询内含 ';'）- 疑似注入",
            ),
            (
                "session-cypher-line-comment",
                &[],
                "Cypher query contains line comment '//' - potential injection",
                "Cypher 查询包含行注释 '//' - 疑似注入",
            ),
            (
                "session-cypher-block-comment",
                &[],
                "Cypher query contains block comment '/* */' - potential injection",
                "Cypher 查询包含块注释 '/* */' - 疑似注入",
            ),
            (
                "session-cypher-dangerous-procedure",
                &[("procedure", "apoc.load")],
                "Cypher query calls dangerous procedure ('apoc.load') - potential privilege escalation",
                "Cypher 查询调用了危险过程（'apoc.load'）- 疑似提权",
            ),
            (
                "auth-password-too-long-bcrypt",
                &[("max_len", "72")],
                "Password must not exceed 72 bytes (bcrypt truncation limit)",
                "密码长度不得超过 72 字节（bcrypt 截断上限）",
            ),
            (
                "rls-unsafe-predicate-column",
                &[("table", "t"), ("column", "c")],
                "RLS policy for table 't' has an unsafe predicate column 'c'",
                "表 't' 的行级安全策略包含不安全的谓词列 'c'",
            ),
            (
                "rls-set-operations-unsupported",
                &[],
                "RLS policy does not support set operations (UNION/INTERSECT/EXCEPT)",
                "行级安全策略不支持集合操作（UNION/INTERSECT/EXCEPT）",
            ),
            (
                "graph-op-denied-admin-required",
                &[("role", "analyst")],
                "Graph operation denied for role 'analyst': administrator role required",
                "角色 'analyst' 的图操作被拒绝: 需要 admin 角色",
            ),
            (
                "data-api-owner-column-unsafe",
                &[("column", "c")],
                "data-api owner column must be a safe identifier: 'c'",
                "data-api 归属列必须是安全标识符: 'c'",
            ),
            (
                "data-api-query-role-not-configured",
                &[],
                "data-api query role is not configured; call with_role() before querying",
                "data-api 查询角色未配置；查询前请先调用 with_role()",
            ),
            (
                "repository-query-role-not-configured",
                &[],
                "repository query role is not configured; call with_role() before querying",
                "repository 查询角色未配置；查询前请先调用 with_role()",
            ),
            (
                "query-fragment-column-unsafe",
                &[("column", "c")],
                "query fragment column name must be a safe identifier: 'c'",
                "查询片段列名必须是安全标识符: 'c'",
            ),
            (
                "query-fragment-table-unsafe",
                &[("table", "t")],
                "query fragment table name must be a safe identifier: 't'",
                "查询片段表名必须是安全标识符: 't'",
            ),
            (
                "query-fragment-condition-column-unsafe",
                &[("column", "c")],
                "query fragment condition column must be a safe identifier: 'c'",
                "查询片段条件列必须是安全标识符: 'c'",
            ),
            (
                "query-fragment-order-column-unsafe",
                &[("column", "o")],
                "query fragment order column must be a safe identifier: 'o'",
                "查询片段排序列必须是安全标识符: 'o'",
            ),
            (
                "session-rls-multi-table-unsafe",
                &[],
                "row-level security does not support statements referencing the protected table more than once (self-join or subquery)",
                "行级安全不支持多次引用受保护表的语句（自连接或子查询）",
            ),
            (
                "session-rls-single-table-required",
                &[],
                "row-level security requires a single-table statement",
                "行级安全要求单表语句",
            ),
            (
                "nexus-url-parse-failed",
                &[("scheme", "<unknown>"), ("error", "E")],
                "failed to parse URL (scheme=<unknown>): E",
                "URL 解析失败（scheme=<unknown>）: E",
            ),
            (
                "auth-log-policy-max-len-exceeds-bcrypt",
                &[("max_len", "100"), ("limit", "72")],
                "password policy max_len 100 exceeds the bcrypt limit of 72 bytes; the effective limit is 72 bytes",
                "密码策略 max_len 100 超过 bcrypt 上限 72 字节；实际生效上限为 72 字节",
            ),
            (
                "auth-log-legacy-verify-exceeds-limit",
                &[],
                "legacy password verification: password exceeds the bcrypt limit, verifying against the legacy 72-byte prefix",
                "存量口令校验: 口令超过 bcrypt 上限，按存量 72 字节前缀比对",
            ),
            (
                "auth-log-legacy-verify-split-multibyte",
                &[],
                "legacy password verification failed and the 72-byte cut splits a multi-byte character: the stored hash covers a byte sequence that is not valid UTF-8 and cannot be re-verified — this account requires a password reset",
                "存量口令校验失败，72 字节截断切开了多字节字符: 存量哈希覆盖的字节序列不是合法 UTF-8，无法重新校验 —— 该账号需要重设口令",
            ),
            (
                "session-log-rls-rejected",
                &[("error", "E")],
                "row-level security policy rejected: E",
                "行级安全策略被拒绝: E",
            ),
        ];
        for (key, args, en, zh) in cases {
            let owned: Vec<(&str, String)> =
                args.iter().map(|(k, v)| (*k, v.to_string())).collect();
            assert_eq!(
                format_from_bundle("en", key, &owned),
                Some(en.to_string()),
                "en value drifted for {key}"
            );
            assert_eq!(
                format_from_bundle("zh", key, &owned),
                Some(zh.to_string()),
                "zh value drifted for {key}"
            );
        }
    }

    /// 守卫：新键模板的占位符名全部可解析——提供超集参数格式化后，
    /// 输出不得残留 `{ $…}` 字面（Fluent 对缺失/拼错的占位符会原样保留）
    #[test]
    fn test_new_wired_keys_placeholders_resolve_both_bundles() {
        let superset_args = [
            ("error", "E".to_string()),
            ("message", "M".to_string()),
            ("table", "T".to_string()),
            ("id", "1".to_string()),
            ("column", "C".to_string()),
            ("key", "K".to_string()),
            ("shard_id", "2".to_string()),
            ("name", "N".to_string()),
            ("role", "R".to_string()),
            ("action", "A".to_string()),
            ("retry_after", "RA".to_string()),
            ("retry_after_secs", "3".to_string()),
            ("waited_ms", "4".to_string()),
            ("in_progress", "1".to_string()),
            ("timeout_ms", "5".to_string()),
            ("waiters", "6".to_string()),
            ("successes", "7".to_string()),
            ("failures", "8".to_string()),
            ("threshold", "9".to_string()),
            ("query_type", "Q".to_string()),
            ("duration_ms", "10".to_string()),
            ("threshold_ms", "11".to_string()),
            ("identifier_type", "IT".to_string()),
            ("identifier", "I".to_string()),
            ("database", "Sqlite".to_string()),
            ("version", "V".to_string()),
            ("description", "D".to_string()),
            ("path", "P".to_string()),
            ("admin_role", "admin".to_string()),
            ("failed", "1".to_string()),
            ("created", "2".to_string()),
            ("closed", "3".to_string()),
            ("percent", "10.0".to_string()),
            ("active", "4".to_string()),
            ("total", "5".to_string()),
            ("waiting", "6".to_string()),
            ("idle", "7".to_string()),
            ("state", "Open".to_string()),
            ("variable", "NEO4J_USER".to_string()),
            ("endpoint", "http://localhost:4318".to_string()),
            ("status", "HTTP/1.1 500".to_string()),
            ("protocol", "ftp".to_string()),
            ("op", "get_or_prepare".to_string()),
            ("max_len", "12".to_string()),
            ("limit", "72".to_string()),
            ("scheme", "ftp".to_string()),
        ];
        const NEW_KEYS: &[&str] = &[
            "db-unsupported",
            "db-version-conflict",
            "circuit-breaker-closed",
            "circuit-breaker-opened",
            "circuit-breaker-half-open",
            "metrics-export-join-failed",
            "metrics-slow-query",
            "pool-log-acquire-timeout",
            "pool-log-idle-check-wait-timeout",
            "saga-compensation-no-pool",
            "saga-compensation-session-unavailable",
            "session-log-sql-parse-denied",
            "session-log-permission-denied",
            "session-log-rate-limited",
            "cli-permissions-file-load-failed",
            "perm-rate-limiter-error",
            "perm-rate-limit-audit-dropped",
            "copy-in-incompatible-connection",
            "copy-payload-cleanup-failed",
            "copy-insert-table-invalid",
            "copy-insert-column-invalid",
            "copy-payload-create-io-failed",
            "copy-payload-write-join-failed",
            "duckdb-execute-batch-failed",
            "duckdb-copy-prepare-failed",
            "duckdb-copy-execute-failed",
            "duckdb-copy-fetch-failed",
            "duckdb-copy-count-read-failed",
            "duckdb-txn-begin-failed",
            "duckdb-txn-commit-failed",
            "duckdb-spawn-blocking-join-failed",
            "repository-version-column-unsafe",
            "repository-column-unsafe",
            "repository-version-field-missing",
            "repository-version-column-integer",
            "shard-route-unregistered",
            "migration-history-table-invalid",
            "query-cache-get-failed",
            "query-cache-decode-failed",
            "query-cache-encode-failed",
            "query-cache-fill-failed",
            "query-cache-clock-before-epoch",
            "query-cache-invalidate-failed",
            "query-cache-version-read-failed",
            "query-cache-params-encode-failed",
            "query-cache-table-name-invalid",
            "migration-rollback-no-down-section",
            "migration-rollback-down-not-executable",
            "migration-identifier-empty",
            "migration-identifier-too-long",
            "migration-identifier-invalid-chars",
            "migration-identifier-reserved-keyword",
            "migration-sql-comment-create-table",
            "migration-sql-comment-drop-table",
            "migration-sql-comment-alter-table",
            "migration-sql-comment-add-column",
            "migration-sql-comment-drop-column",
            "migration-sql-comment-add-index",
            "migration-sql-comment-drop-index",
            "migration-sql-comment-add-foreign-key",
            "migration-sql-comment-drop-foreign-key",
            "migration-sqlite-drop-column-note",
            "migration-sqlite-modify-column-note",
            "migration-sqlite-modify-type-note",
            "migration-sqlite-modify-nullability-note",
            "migration-sqlite-modify-default-note",
            "migration-scan-read-dir-failed",
            "migration-scan-read-entry-failed",
            "migration-scan-read-file-failed",
            "config-confers-subscribe-failed",
            "config-confers-hot-reload-failed",
            "config-confers-skip-invalid-event",
            "config-confers-log-hot-reload",
            "config-confers-log-ack-failed",
            "perm-config-path-empty",
            "perm-config-path-parent-reference",
            "perm-config-path-safety-validation",
            "perm-config-yaml-feature-not-enabled",
            "perm-provider-memory-policy-table-uninitialized",
            "perm-provider-yaml-file-unreadable",
            "perm-provider-config-read-error",
            "perm-provider-lock-error",
            "cli-migration-file-not-found-for-rollback",
            "cli-migration-rollback-failed",
            "cli-schema-diff-up-placeholder",
            "cli-schema-diff-down-placeholder",
            "cli-invalid-database-url-format",
            "cli-database-not-supported",
            "cli-unsupported-database-protocol",
            "session-already-in-graph-transaction-concurrent",
            "session-not-in-transaction",
            "session-no-active-transaction-commit",
            "session-no-active-transaction-rollback",
            "session-commit-transaction-in-use",
            "session-rollback-transaction-in-use",
            "session-duckdb-ddl-admin-only",
            "session-ddl-admin-only",
            "health-check-timeout",
            "health-check-timeout-recommendation",
            "health-recommendation-check-connection-config",
            "health-recommendation-check-network",
            "health-recommendation-ensure-pool-initialized",
            "health-recommendation-wait-circuit-breaker",
            "health-recommendation-increase-pool-size",
            "health-recommendation-check-connection-leak",
            "health-recommendation-increase-min-connections",
            "health-status-failure-rate-too-high",
            "health-status-no-connections",
            "health-status-circuit-breaker-open",
            "health-status-pool-full",
            "health-status-no-idle-connections",
            "health-details-pool",
            "health-details-stats",
            "health-details-circuit-breaker",
            "shard-migration-orchestration-not-implemented",
            "neo4j-connect-failed",
            "neo4j-url-no-credentials-env",
            "neo4j-url-invalid-and-env-missing",
            "neo4j-not-connected",
            "neo4j-execute-failed",
            "neo4j-execute-with-params-failed",
            "neo4j-row-fetch-failed",
            "neo4j-row-deserialize-failed",
            "neo4j-health-check-no-rows",
            "neo4j-start-txn-failed",
            "neo4j-transaction-consumed",
            "neo4j-commit-failed",
            "neo4j-rollback-failed",
            "neo4j-txn-execute-failed",
            "neo4j-txn-execute-with-params-failed",
            "neo4j-txn-row-fetch-failed",
            "neo4j-txn-row-deserialize-failed",
            "otel-serialize-failed",
            "otel-http-task-failed",
            "otel-endpoint-not-http",
            "otel-connect-failed",
            "otel-write-failed",
            "otel-read-failed",
            "otel-collector-non-2xx",
            "otel-export-failed",
            "kit-health-no-connections",
            "kit-health-waiting",
            "kit-health-no-idle",
            "perm-cache-log-insert-failed",
            "perm-cache-log-invalidate-failed",
            "perm-cache-log-clear-failed",
            "prepare-cache-log-read-miss",
            "prepare-cache-log-write-failed",
            "prepare-cache-log-clear-failed",
            "perm-cache-provider-no-policy",
            "perm-cache-provider-not-configured",
            "perm-provider-yaml-parse-error",
            "auth-password-too-long-bcrypt",
            "rls-unsafe-predicate-column",
            "rls-set-operations-unsupported",
            "graph-op-denied-admin-required",
            "data-api-owner-column-unsafe",
            "data-api-query-role-not-configured",
            "repository-query-role-not-configured",
            "query-fragment-column-unsafe",
            "query-fragment-table-unsafe",
            "query-fragment-condition-column-unsafe",
            "query-fragment-order-column-unsafe",
            "session-rls-multi-table-unsafe",
            "session-rls-single-table-required",
            "nexus-url-parse-failed",
            "auth-log-policy-max-len-exceeds-bcrypt",
            "auth-log-legacy-verify-exceeds-limit",
            "auth-log-legacy-verify-split-multibyte",
            "session-log-rls-rejected",
        ];
        for key in NEW_KEYS {
            for lang in ["en", "zh"] {
                let formatted = format_from_bundle(lang, key, &superset_args)
                    .unwrap_or_else(|| panic!("{lang} bundle missing key: {key}"));
                assert!(
                    !formatted.contains("{ $"),
                    "{lang}/{key} left an unresolved placeholder: {formatted}"
                );
            }
        }
    }

    /// 拼接某语言的嵌入 FTL 源（复用 `EMBEDDED_LOCALES`，与运行时 bundle 同源）
    fn catalog_ftl(lang: &str) -> String {
        let prefix = format!("locales/{lang}/");
        EMBEDDED_LOCALES
            .iter()
            .filter(|(path, _)| path.starts_with(&prefix))
            .map(|(_, ftl)| *ftl)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 从 FTL 源解析 (键, 占位符名集合)，供目录级守卫使用
    fn catalog_entries(ftl: &str) -> Vec<(String, Vec<String>)> {
        let mut out = Vec::new();
        for line in ftl.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                if key.is_empty()
                    || !key
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                {
                    continue;
                }
                let mut names: Vec<String> = value
                    .match_indices("{ $")
                    .filter_map(|(idx, _)| {
                        let rest = &value[idx + 3..];
                        rest.find('}').map(|end| rest[..end].trim().to_string())
                    })
                    .collect();
                names.sort();
                names.dedup();
                out.push((key.to_string(), names));
            }
        }
        out
    }

    /// 守卫：en/zh 两份目录的键集合与逐键占位符集合完全一致
    ///
    /// 单侧缺键会让 `translate()` 退化为裸键，占位符名不一致则让某一语言
    /// 残留 `{ $…}` 字面量。此前该一致性只由仓外脚本比对，此处把它变成
    /// 编译进测试的不变式，任何新增键漏登记即刻失败。
    #[test]
    fn test_en_zh_catalogs_share_key_and_placeholder_sets() {
        let en = catalog_entries(&catalog_ftl("en"));
        let zh = catalog_entries(&catalog_ftl("zh"));

        let en_keys: Vec<&str> = en.iter().map(|(k, _)| k.as_str()).collect();
        let zh_keys: Vec<&str> = zh.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            en_keys.len(),
            zh_keys.len(),
            "en/zh 键数量不一致（可能存在重复键或单侧缺失）"
        );

        for (key, names) in &en {
            let zh_entry = zh
                .iter()
                .find(|(k, _)| k == key)
                .unwrap_or_else(|| panic!("zh bundle missing key: {key}"));
            assert_eq!(
                &zh_entry.1, names,
                "placeholder set drifted for key: {key} (en={names:?}, zh={:?})",
                zh_entry.1
            );
        }
    }

    /// `EMBEDDED_LOCALES` 与磁盘 `locales/` 目录必须逐文件一致（路径集合相等），
    /// 且每个语言包都必须能被 Fluent 无错解析——新增/改名语言包漏登记即刻失败。
    #[test]
    fn test_embedded_locales_match_disk() {
        let locales_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("locales");

        let mut on_disk: Vec<String> = Vec::new();
        for lang in ["en", "zh"] {
            let dir = locales_dir.join(lang);
            for entry in std::fs::read_dir(&dir)
                .unwrap_or_else(|e| panic!("locales/{lang}/ must be readable: {e}"))
            {
                let entry = entry.expect("locales directory entry");
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("ftl") {
                    continue;
                }
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .expect("locale file name is UTF-8");
                on_disk.push(format!("locales/{lang}/{name}"));
            }
        }
        on_disk.sort();

        let mut embedded: Vec<String> = EMBEDDED_LOCALES
            .iter()
            .map(|(path, _)| (*path).to_string())
            .collect();
        embedded.sort();
        assert_eq!(
            embedded, on_disk,
            "EMBEDDED_LOCALES and the locales/ directory must list exactly the same files"
        );

        for (path, ftl) in EMBEDDED_LOCALES {
            assert!(
                FluentResource::try_new((*ftl).to_string()).is_ok(),
                "{path} must parse as Fluent without errors"
            );
        }
    }

    /// 守卫：`cli-help-*` 键全部在 CLI 二进制源码里有字面量引用。
    ///
    /// 防「语言包建了键、调用点漏改」——本轮就发现了
    /// `perm-provider-config-read-error` 这类上一轮遗留的孤儿键（调用点
    /// 是硬编码 format!）。该测试把它变成自动防线：cli.ftl 每新增一个
    /// help 键，build_cli() 必须出现同名字符量。
    #[test]
    fn test_cli_help_keys_all_wired_in_cli_binary() {
        let cli_main = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tools/cli/src/main.rs"),
        )
        .unwrap_or_else(|e| panic!("CLI binary source must be readable: {e}"));

        let en_ftl = catalog_ftl("en");
        for (key, _) in catalog_entries(&en_ftl) {
            if !key.starts_with("cli-help-") {
                continue;
            }
            assert!(
                cli_main.contains(&format!("\"{key}\"")),
                "cli.ftl key '{key}' is not referenced by the CLI binary \
                 (src/tools/cli/src/main.rs, build_cli())"
            );
        }
    }
}
