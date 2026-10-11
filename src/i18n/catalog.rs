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
///
/// Visible to the crate so `DbI18nFormatter` can resolve messages for an
/// explicit locale instead of the process-global current locale.
pub(crate) fn format_from_bundle(lang: &str, key: &str, args: &[(&str, String)]) -> Option<String> {
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
config-confers-subscribe-failed = confers subscribe failed: { $error }
config-confers-hot-reload-failed = permission hot reload failed: { $error }
config-confers-skip-invalid-event = skip invalid permission event: { $error }
config-confers-log-hot-reload = permission hot reload: { $error }
config-confers-log-ack-failed = confers ack failed: { $error }

db-connection = Database connection error: { $error }
db-config = Configuration error: { $message }
db-permission = Permission denied: { $message }
db-transaction = Transaction error: { $message }
db-migration = Migration error: { $message }
db-cache = Cache error: { $message }
db-query = Query error: { $message }
db-validation = Validation error: { $message }
db-unsupported = Unsupported operation: { $message }
db-version-conflict = Version conflict on table '{ $table }' for id { $id }
db-rate-limited = Rate limit exceeded: too many requests, please retry later
db-rate-limited-retry = Rate limit exceeded: too many requests; retry after { $retry_after_secs }s

pool-acquire-timeout = Failed to acquire connection within timeout
pool-exhausted = Connection pool exhausted
pool-connection-failed = Failed to create connection: { $reason }
pool-health-check-failed = Health check failed: { $reason }

migration-file-not-found = Migration file not found: { $path }
migration-parse-error = Failed to parse migration file: { $reason }
migration-execution-error = Migration execution failed: { $reason }
migration-version-conflict = Migration version conflict: { $reason }
migration-rollback-error = Migration rollback failed: { $reason }
migration-rollback-no-down-section = Migration v{ $version } ({ $description }) has no rollable-back DOWN section
migration-rollback-down-not-executable = The DOWN section exists but contains no executable statement; rollback is rejected to avoid a false rollback
migration-identifier-empty = { $identifier_type } must not be empty
migration-identifier-too-long = { $identifier_type } must not exceed 64 characters
migration-identifier-invalid-chars = { $identifier_type } '{ $identifier }' contains invalid characters; only letters, digits and underscore are allowed, and it must not start with a digit
migration-identifier-reserved-keyword = { $identifier_type } '{ $identifier }' is a reserved SQL keyword and is not allowed
migration-identifier-type-table = table name
migration-identifier-type-primary-key-column = primary key column name
migration-identifier-type-column = column name
migration-identifier-type-index = index name
migration-identifier-type-index-column = index column name
migration-identifier-type-new-column = new column name
migration-identifier-type-fk-table = foreign key table name
migration-identifier-type-fk-constraint = foreign key constraint name
migration-identifier-type-fk-column = foreign key column name
migration-identifier-type-fk-referenced-table = foreign key referenced table name
migration-identifier-type-fk-referenced-column = foreign key referenced column name
migration-sql-comment-create-table = -- Create table: { $name }
migration-sql-comment-drop-table = -- Drop table: { $name }
migration-sql-comment-alter-table = -- Alter table: { $name }
migration-sql-comment-add-column = -- Add column: { $name }
migration-sql-comment-drop-column = -- Drop column: { $name }
migration-sql-comment-add-index = -- Add index: { $name }
migration-sql-comment-drop-index = -- Drop index: { $name }
migration-sql-comment-add-foreign-key = -- Add foreign key: { $name }
migration-sql-comment-drop-foreign-key = -- Drop foreign key: { $name }
migration-sqlite-drop-column-note = -- SQLite does not support dropping a column directly; rebuild table { $table } manually
migration-sqlite-modify-column-note = -- SQLite does not support ALTER COLUMN; rebuild table { $table } manually to adjust column { $column }
migration-sqlite-modify-type-note = -- SQLite does not support ALTER COLUMN; rebuild table { $table } manually to change the type of column { $column }
migration-sqlite-modify-nullability-note = -- { $database } does not support altering column nullability directly; rebuild table { $table } manually to adjust the NOT NULL constraint of column { $column }
migration-sqlite-modify-default-note = -- SQLite does not support ALTER COLUMN; rebuild table { $table } manually to change the default of column { $column }
migration-scan-read-dir-failed = Failed to read migration directory: { $error }
migration-scan-read-entry-failed = Failed to read migration entry: { $error }
migration-scan-read-file-failed = Failed to read migration file: { $error }

audit-write-error = Failed to write audit log: { $reason }
audit-serialization-error = Failed to serialize audit data: { $reason }
audit-config-error = Invalid audit configuration: { $reason }

perm-config-missing-field = missing required field: { $field }
perm-config-invalid-value = invalid value for field '{ $field }': { $reason }
perm-config-policy-not-found = policy file not found: { $path }
perm-config-path-empty = Config path cannot be empty
perm-config-path-parent-reference = Config path contains an invalid parent directory reference
perm-config-path-safety-validation = Config path failed safety validation
perm-config-yaml-feature-not-enabled = Cannot parse permission config: the 'yaml' feature is not enabled

perm-denied = permission denied for { $operation } on { $resource }
perm-role-not-found = role not found: { $role }
perm-invalid-policy = invalid policy configuration: { $reason }
perm-rate-limited = rate limit exceeded
perm-parse-error = policy parse error: { $reason }

perm-provider-role-not-found = Role '{ $role }' not found
perm-provider-load-error = Failed to load config: { $reason }
perm-provider-check-error = Permission check failed: { $reason }
perm-provider-unknown = Unknown error: { $reason }
perm-provider-memory-policy-table-uninitialized = MemoryPermissionProvider policy table is not initialized: no role policies at all (capacity 0)
perm-provider-yaml-file-unreadable = YamlPermissionProvider policy file is not readable ({ $path }): { $error }
perm-provider-config-read-error = Permission config file read error (path='{ $path }'): { $error }
perm-provider-lock-error = Lock error: permission provider internal state is poisoned
perm-rate-limiter-error = rate limiter backend error: { $error }
perm-rate-limit-audit-dropped = rate limit audit event dropped: { $error }
perm-cache-log-insert-failed = PermissionCache insert failed (error omits the raw key): { $error }
perm-cache-log-invalidate-failed = PermissionCache invalidate failed (error omits the raw key): { $error }
perm-cache-log-clear-failed = PermissionCache clear failed: { $error }
prepare-cache-log-read-miss = PreparedStatementCache { $op } read failed, treated as a cache miss: { $error }
prepare-cache-log-write-failed = PreparedStatementCache { $op } write failed: { $error }
prepare-cache-log-clear-failed = PreparedStatementCache clear failed: { $error }

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
metrics-collector-not-configured = metrics collector not configured (use with_metrics_collector)
metrics-export-join-failed = metrics export task join failed: { $error }
metrics-slow-query = slow query: query_type={ $query_type } duration_ms={ $duration_ms } threshold_ms={ $threshold_ms }
metrics-feature-not-enabled = metrics feature not enabled (rebuild with the metrics feature to expose the collector endpoint)

circuit-breaker = Circuit breaker is { $state }
circuit-breaker-closed = circuit breaker closed: successes={ $successes } success_threshold={ $threshold }
circuit-breaker-opened = circuit breaker opened: failures={ $failures } failure_threshold={ $threshold }
circuit-breaker-reopened = circuit breaker reopened: half-open probe failed
circuit-breaker-half-open = circuit breaker half-open: timeout_ms={ $timeout_ms }

audit-builder-operation-required = operation is required
audit-builder-entity-type-required = entity_type is required
audit-builder-entity-id-required = entity_id is required
audit-builder-id-empty = event id must not be empty

retry-exhausted = Retry exhausted after { $attempts } attempts: { $last_error }
retry-non-retryable = Non-retryable operation: { $error }
retry-timeout = Retry timed out after { $timeout_ms }ms: { $last_error }

saga-execution-failed = Saga execution failed: { $reason }
saga-compensation-failed = Saga compensation failed: { $reason }
saga-timeout = Saga timeout: { $reason }
saga-compensation-no-pool = compensation session unavailable: no pool for shard { $shard_id }
saga-compensation-session-unavailable = compensation session unavailable: { $error }

snowflake-clock-backtrack = Clock backtrack: waited timestamp { $waited_ts } still behind last used { $last_ts }
snowflake-timestamp-overflow = Timestamp overflow: { $timestamp } exceeds 41-bit capacity

nexus-unsupported-database = Unsupported database scheme in URL: { $scheme }

error-category-permission = Permission
error-category-injection-risk = InjectionRisk
error-category-syntax-error = SyntaxError
error-category-shard-conflict = ShardConflict

query-error-report = [{ $category }] { $message } — Suggestion: { $suggestion }
query-error-suggestion-code-table = Refer to the error code table to determine the handling strategy

error-code = Error code: { $code } ({ $name })

migration = { $count } migrations applied
hello-world = Hello, World!

cli-migration-created = ✓ Migration file created: { $path }
cli-status-title = Migration Status
cli-help-about = DBNexus database migration tool
cli-help-database-url = Database connection string (global: may appear before or after any subcommand)
cli-help-config = Configuration file path
cli-help-migrations-dir = Migrations directory (global: may appear before or after any subcommand)
cli-help-lang = Manually specify language (en, zh)
cli-database-url-required = error: --database-url is required (or set DATABASE_URL)
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
cli-invalid-limit = limit must not exceed 1000000 (0 means unlimited)
cli-invalid-operation = operation must be one of create/read/update/delete/login/logout/permission-change/config-change
cli-invalid-severity = severity must be one of info/low/medium/high/critical
cli-invalid-status = status must be one of success/failure/partial/unknown
cli-invalid-since = since must be an RFC 3339 timestamp (e.g. 2026-01-01T00:00:00Z)
cli-invalid-until = until must be an RFC 3339 timestamp (e.g. 2026-01-01T00:00:00Z)
cli-invalid-total-shards = total_shards must be between 1 and 10000
cli-invalid-strategy = strategy must be one of yearly/monthly/daily/hash/consistent-hash
cli-invalid-route-key = route_key must not be empty
cli-empty-role-or-table = role and table must not be empty
cli-invalid-action = action must be one of select/insert/update/delete
cli-permissions-file-unavailable = permissions file does not exist or is not readable
cli-permissions-file-load-failed = permissions file could not be loaded: { $error }
cli-permissions-file-invalid = permissions file could not be evaluated (malformed config)
cli-unsupported-db-url-protocol = unsupported database URL protocol
cli-migration-file-not-found-for-rollback = Migration file for v{ $version } ({ $description }) was not found; cannot run the DOWN rollback
cli-migration-rollback-failed = Migration rollback failed for v{ $version }: { $error }
cli-schema-diff-up-placeholder = -- Auto-generated UP SQL, please edit manually
cli-schema-diff-down-placeholder = -- Auto-generated DOWN SQL, please edit manually
cli-invalid-database-url-format = Invalid database URL format: { $error }
cli-database-not-supported = { $database } is not supported
cli-unsupported-database-protocol = Unsupported database protocol: '{ $protocol }'. Supported protocols: sqlite, postgres, mysql

pool-invalid-config = Invalid configuration: { $error }
pool-read-config-failed = Failed to read permission config file '{ $path }': { $error }
pool-parse-config-failed = Failed to parse permission config file '{ $path }': { $error }
pool-yaml-parse-error = YAML parse error in '{ $source }': { $error }
pool-invalid-db-url = Invalid database URL: { $error }
pool-recreate-failed = Failed to recreate connections: { $error }
pool-log-acquire-timeout = pool acquire timeout: waited_ms={ $waited_ms } timeout_ms={ $timeout_ms } waiters={ $waiters }
pool-log-idle-check-wait-timeout = pool acquire waited for idle-connection check: waited_ms={ $waited_ms } in_progress={ $in_progress }

health-check-timeout = Health check timed out
health-check-timeout-recommendation = Health check timed out; optimize the check logic
health-recommendation-check-connection-config = Check that the database connection configuration is correct
health-recommendation-check-network = Verify that the network connection is stable
health-recommendation-ensure-pool-initialized = Ensure that the database connection pool is initialized correctly
health-recommendation-wait-circuit-breaker = Wait for the circuit breaker to recover
health-recommendation-increase-pool-size = Consider increasing the connection pool size
health-recommendation-check-connection-leak = Check for connection leaks
health-recommendation-increase-min-connections = Consider increasing the minimum number of connections
health-status-failure-rate-too-high = Connection failure rate too high: { $failed }/{ $created } ({ $percent }%)
health-status-no-connections = No connections available
health-status-circuit-breaker-open = Circuit breaker is open; requests are rejected
health-status-pool-full = Connection pool full: { $active }/{ $total } active, { $waiting } waiting
health-status-no-idle-connections = No idle connections
health-details-pool = Connection pool: total={ $total }, active={ $active }, idle={ $idle }, waiting={ $waiting }
health-details-stats = Statistics: created={ $created }, failed={ $failed }, closed={ $closed }
health-details-circuit-breaker = Circuit breaker: { $state } ({ $failures } failures, { $successes } successes)
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
session-for-update-txn-required = query_rows_for_update requires an active transaction (call begin_transaction first)
session-for-update-relational-required = query_rows_for_update requires a relational (SeaORM) backend
session-isolation-relational-required = begin_transaction_with_isolation requires a relational (SeaORM) backend
session-log-sql-parse-denied = permission denied: role={ $role } reason=sql-parse-failure (fail-closed)
session-log-permission-denied = permission denied: action={ $action } table={ $table }
session-log-rate-limited = rate limited: table access throttled (retry_after={ $retry_after })

session-already-in-transaction = Already in transaction
session-already-in-transaction-concurrent = Already in transaction (concurrent begin detected)
session-already-in-graph-transaction = Already in graph transaction
session-already-in-graph-transaction-concurrent = Already in graph transaction (concurrent begin detected)
session-connection-not-available = Connection not available - Session may have been invalidated
session-not-in-transaction = Not in transaction
session-no-active-transaction-commit = No active transaction to commit
session-no-active-transaction-rollback = No active transaction to rollback
session-commit-transaction-in-use = Cannot commit: transaction is in use by a concurrent query
session-rollback-transaction-in-use = Cannot rollback: transaction is in use by a concurrent query
session-duckdb-ddl-admin-only = DDL operations are only allowed for the admin role in DuckDB context. Current role: '{ $role }', Admin role: '{ $admin_role }'
session-ddl-admin-only = DDL operations are only allowed for the admin role. Current role: '{ $role }', Admin role: '{ $admin_role }'

copy-path-empty = COPY file path must not be empty
copy-insert-column-required = INSERT requires at least one column
copy-insert-row-required = multi-row INSERT requires at least one row (empty VALUES is meaningless)
copy-insert-table-invalid = INSERT target table identifier is invalid: '{ $table }' (allowed: letters/digits/underscore/dot)
copy-insert-column-invalid = INSERT column identifier is invalid: '{ $column }' (allowed: letters/digits/underscore/dot)
copy-in-incompatible-connection = (pool returned an incompatible connection: { $error })
copy-payload-path-utf8 = COPY payload temp path is not valid UTF-8
copy-payload-cleanup-failed = DuckDB COPY payload temp file cleanup failed: { $error }
copy-driver-feature-missing = — enable the postgres or duckdb driver feature to use COPY
copy-payload-create-failed = could not create COPY payload temp file
copy-payload-create-io-failed = DuckDB COPY payload temp file creation failed: { $error }
copy-payload-write-join-failed = DuckDB COPY payload write task join failed: { $error }

duckdb-execute-batch-failed = DuckDB execute_batch failed: { $error }
duckdb-copy-prepare-failed = DuckDB COPY prepare failed: { $error }
duckdb-copy-execute-failed = DuckDB COPY execute failed: { $error }
duckdb-copy-fetch-failed = DuckDB COPY result fetch failed: { $error }
duckdb-copy-count-read-failed = DuckDB COPY count column read failed: { $error }

duckdb-pool-exhausted = DuckDB pool exhausted: no connection available
duckdb-txn-begin-failed = DuckDB begin transaction failed: { $error }
duckdb-txn-commit-failed = DuckDB commit failed: { $error }
duckdb-spawn-blocking-join-failed = spawn_blocking join failed: { $error }

neo4j-connect-failed = neo4j connect: { $error }
neo4j-url-no-credentials-env = neo4j URL has no credentials and the { $variable } environment variable is not set
neo4j-url-invalid-and-env-missing = neo4j URL is not a valid URL and the { $variable } environment variable is not set
neo4j-not-connected = Neo4jConnection not connected (placeholder)
neo4j-execute-failed = neo4j execute: { $error }
neo4j-execute-with-params-failed = neo4j execute (params): { $error }
neo4j-row-fetch-failed = neo4j row fetch: { $error }
neo4j-row-deserialize-failed = neo4j row deserialize: { $error }
neo4j-health-check-no-rows = neo4j health check returned no rows
neo4j-start-txn-failed = neo4j start_txn: { $error }
neo4j-transaction-consumed = neo4j transaction already consumed
neo4j-commit-failed = neo4j commit: { $error }
neo4j-rollback-failed = neo4j rollback: { $error }
neo4j-txn-execute-failed = neo4j txn execute: { $error }
neo4j-txn-execute-with-params-failed = neo4j txn execute (params): { $error }
neo4j-txn-row-fetch-failed = neo4j txn row fetch: { $error }
neo4j-txn-row-deserialize-failed = neo4j txn row deserialize: { $error }

repository-version-column-unsafe = repository version column must be a safe identifier: '{ $column }'
repository-version-field-missing = update_if_version requires field '{ $column }' on the entity
repository-version-column-integer = version column '{ $column }' must be an integer
repository-column-unsafe = repository column must be a safe identifier: '{ $column }'
repository-entity-deserialize-failed = entity deserialize failed: { $error }

shard-route-unregistered = shard key '{ $key }' routed to unregistered shard { $shard_id }
shard-migration-orchestration-not-implemented = Shard migration orchestration is not implemented yet: the MigrationExecutor is not wired in, so this shard ran no migrations

migration-history-table-invalid = illegal migration history table name: { $name }

query-cache-get-failed = query cache get failed: { $error }
query-cache-decode-failed = query cache decode failed: { $error }
query-cache-encode-failed = query cache encode failed: { $error }
query-cache-fill-failed = query cache fill failed: { $error }
query-cache-clock-before-epoch = system clock before epoch: { $error }
query-cache-invalidate-failed = query cache invalidate failed: { $error }
query-cache-tables-required = query cache requires at least one table in `tables`; a query with no declared tables can never be invalidated (fail-closed)
query-cache-version-read-failed = query cache table version read failed: { $error }
query-cache-params-encode-failed = query cache params encode failed: { $error }
query-cache-table-name-invalid = query cache table name is invalid: '{ $table }' (allowed: letters/digits/underscore/dot, must not start with a digit)

otel-serialize-failed = serialize failed: { $error }
otel-http-task-failed = otlp http task failed: { $error }
otel-endpoint-not-http = otlp endpoint must be http:// (got: { $endpoint })
otel-connect-failed = otlp connect failed: { $error }
otel-write-failed = otlp write failed: { $error }
otel-read-failed = otlp read failed: { $error }
otel-collector-non-2xx = otlp collector responded non-2xx: { $status }
otel-export-failed = otlp export failed: { $error }

kit-health-no-connections = no connections established
kit-health-waiting = { $waiting } waiting, { $active }/{ $total } active/total
kit-health-no-idle = no idle connections, { $active }/{ $total } active/total

example-replica-routing-api-replica-pool-new = - ReplicaPool::new(pool, lag_detector)
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
config-confers-subscribe-failed = confers 订阅失败: { $error }
config-confers-hot-reload-failed = 权限热重载失败: { $error }
config-confers-skip-invalid-event = 跳过无效的权限变更事件: { $error }
config-confers-log-hot-reload = 权限热重载: { $error }
config-confers-log-ack-failed = confers 确认失败: { $error }

db-connection = 数据库连接错误: { $error }
db-config = 配置错误: { $message }
db-permission = 权限被拒绝: { $message }
db-transaction = 事务错误: { $message }
db-migration = 迁移错误: { $message }
db-cache = 缓存错误: { $message }
db-query = 查询错误: { $message }
db-validation = 验证错误: { $message }
db-unsupported = 不支持的操作: { $message }
db-version-conflict = 表 '{ $table }' 中 ID 为 { $id } 的记录版本冲突
db-rate-limited = 请求过于频繁，已被速率限制，请稍后重试
db-rate-limited-retry = 请求过于频繁，已被速率限制，{ $retry_after_secs } 秒后重试

pool-acquire-timeout = 无法在超时时间内获取连接
pool-exhausted = 连接池已耗尽
pool-connection-failed = 创建连接失败: { $reason }
pool-health-check-failed = 健康检查失败: { $reason }

migration-file-not-found = 迁移文件未找到: { $path }
migration-parse-error = 迁移文件解析失败: { $reason }
migration-execution-error = 迁移执行失败: { $reason }
migration-version-conflict = 迁移版本冲突: { $reason }
migration-rollback-error = 迁移回滚失败: { $reason }
migration-rollback-no-down-section = 迁移 v{ $version } ({ $description }) 无可回滚的 DOWN 部分
migration-rollback-down-not-executable = DOWN 段存在但不含可执行语句，拒绝回滚以避免假回滚
migration-identifier-empty = { $identifier_type }不能为空
migration-identifier-too-long = { $identifier_type }长度不能超过 64 个字符
migration-identifier-invalid-chars = { $identifier_type } '{ $identifier }' 包含无效字符，只允许字母、数字和下划线，且不能以数字开头
migration-identifier-reserved-keyword = { $identifier_type } '{ $identifier }' 是 SQL 保留关键字，不允许使用
migration-identifier-type-table = 表名
migration-identifier-type-primary-key-column = 主键列名
migration-identifier-type-column = 列名
migration-identifier-type-index = 索引名
migration-identifier-type-index-column = 索引列名
migration-identifier-type-new-column = 新列名
migration-identifier-type-fk-table = 外键表名
migration-identifier-type-fk-constraint = 外键约束名
migration-identifier-type-fk-column = 外键列名
migration-identifier-type-fk-referenced-table = 外键引用表名
migration-identifier-type-fk-referenced-column = 外键引用列名
migration-sql-comment-create-table = -- 创建表: { $name }
migration-sql-comment-drop-table = -- 删除表: { $name }
migration-sql-comment-alter-table = -- 修改表: { $name }
migration-sql-comment-add-column = -- 添加列: { $name }
migration-sql-comment-drop-column = -- 删除列: { $name }
migration-sql-comment-add-index = -- 添加索引: { $name }
migration-sql-comment-drop-index = -- 删除索引: { $name }
migration-sql-comment-add-foreign-key = -- 添加外键: { $name }
migration-sql-comment-drop-foreign-key = -- 删除外键: { $name }
migration-sqlite-drop-column-note = -- SQLite 不支持直接删除列，请手动重建表 { $table }
migration-sqlite-modify-column-note = -- SQLite 不支持修改列定义，请手动重建表 { $table } 以调整列 { $column }
migration-sqlite-modify-type-note = -- SQLite 不支持修改列类型，请手动重建表 { $table } 以调整列 { $column } 的类型
migration-sqlite-modify-nullability-note = -- { $database } 不支持直接修改列可空性，请手动重建表 { $table } 以调整列 { $column } 的 NOT NULL 约束
migration-sqlite-modify-default-note = -- SQLite 不支持修改列默认值，请手动重建表 { $table } 以调整列 { $column } 的默认值
migration-scan-read-dir-failed = 读取迁移目录失败: { $error }
migration-scan-read-entry-failed = 读取迁移目录条目失败: { $error }
migration-scan-read-file-failed = 读取迁移文件失败: { $error }

audit-write-error = 审计日志写入失败: { $reason }
audit-serialization-error = 审计数据序列化失败: { $reason }
audit-config-error = 审计配置无效: { $reason }

perm-config-missing-field = 缺少必填字段: { $field }
perm-config-invalid-value = 字段 '{ $field }' 的值无效: { $reason }
perm-config-policy-not-found = 策略文件未找到: { $path }
perm-config-path-empty = 配置路径不能为空
perm-config-path-parent-reference = 配置路径包含无效的父目录引用
perm-config-path-safety-validation = 配置路径未通过安全性校验
perm-config-yaml-feature-not-enabled = 无法解析权限配置：未启用 'yaml' 特性

perm-denied = 对 { $resource } 的 { $operation } 操作权限被拒绝
perm-role-not-found = 角色未找到: { $role }
perm-invalid-policy = 策略配置无效: { $reason }
perm-rate-limited = 速率限制已超出
perm-parse-error = 策略解析错误: { $reason }

perm-provider-role-not-found = 角色 '{ $role }' 未找到
perm-provider-load-error = 加载配置失败: { $reason }
perm-provider-check-error = 权限检查失败: { $reason }
perm-provider-unknown = 未知错误: { $reason }
perm-provider-memory-policy-table-uninitialized = MemoryPermissionProvider 策略表未初始化：无任何角色策略（容量 0）
perm-provider-yaml-file-unreadable = YamlPermissionProvider 策略文件不可读（{ $path }）: { $error }
perm-provider-config-read-error = 权限配置文件读取错误（path='{ $path }'）: { $error }
perm-provider-lock-error = 锁错误: 权限提供器内部状态已中毒
perm-rate-limiter-error = 限流后端故障: { $error }
perm-rate-limit-audit-dropped = 限流审计事件已丢弃: { $error }
perm-cache-log-insert-failed = PermissionCache 插入失败（错误不含键原文）: { $error }
perm-cache-log-invalidate-failed = PermissionCache invalidate 失败（错误不含键原文）: { $error }
perm-cache-log-clear-failed = PermissionCache clear 失败: { $error }
prepare-cache-log-read-miss = PreparedStatementCache { $op } 读取失败，按未命中处理: { $error }
prepare-cache-log-write-failed = PreparedStatementCache { $op } 写入失败: { $error }
prepare-cache-log-clear-failed = PreparedStatementCache clear 失败: { $error }

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
metrics-collector-not-configured = 指标收集器未配置（请使用 with_metrics_collector）
metrics-export-join-failed = 指标导出任务 join 失败: { $error }
metrics-slow-query = 慢查询: query_type={ $query_type } duration_ms={ $duration_ms } threshold_ms={ $threshold_ms }
metrics-feature-not-enabled = metrics 特性未启用（重新编译时启用 metrics 特性以暴露收集器端点）

circuit-breaker = 断路器处于 { $state } 状态
circuit-breaker-closed = 断路器闭合: successes={ $successes } success_threshold={ $threshold }
circuit-breaker-opened = 断路器打开: failures={ $failures } failure_threshold={ $threshold }
circuit-breaker-reopened = 断路器重新打开: half-open 探测失败
circuit-breaker-half-open = 断路器半开: timeout_ms={ $timeout_ms }

audit-builder-operation-required = 操作类型为必填项
audit-builder-entity-type-required = 实体类型为必填项
audit-builder-entity-id-required = 实体 ID 为必填项
audit-builder-id-empty = 事件 ID 不能为空

retry-exhausted = 重试 { $attempts } 次后仍失败: { $last_error }
retry-non-retryable = 不可重试的操作: { $error }
retry-timeout = 重试在 { $timeout_ms }ms 后超时: { $last_error }

saga-execution-failed = Saga 执行失败: { $reason }
saga-compensation-failed = Saga 补偿失败: { $reason }
saga-timeout = Saga 超时: { $reason }
saga-compensation-no-pool = 补偿会话不可用: 分片 { $shard_id } 无可用连接池
saga-compensation-session-unavailable = 补偿会话不可用: { $error }

snowflake-clock-backtrack = 时钟回拨: 等待后的时间戳 { $waited_ts } 仍落后于上次使用的 { $last_ts }
snowflake-timestamp-overflow = 时间戳溢出: { $timestamp } 超出 41 位容量

nexus-unsupported-database = 不支持的数据库 URL 方案: { $scheme }

error-category-permission = 权限
error-category-injection-risk = 注入风险
error-category-syntax-error = 语法错误
error-category-shard-conflict = 分片冲突

query-error-report = [{ $category }] { $message } — 建议: { $suggestion }
query-error-suggestion-code-table = 参考错误码表定位处理策略

error-code = 错误码: { $code }（{ $name }）

migration = 已应用 { $count } 个迁移
hello-world = 你好，世界！

cli-migration-created = ✓ 迁移文件已创建: { $path }
cli-status-title = 迁移状态查看
cli-help-about = DBNexus 数据库迁移工具
cli-help-database-url = 数据库连接字符串（global：可置于子命令前后任意位置）
cli-help-config = 配置文件路径
cli-help-migrations-dir = 迁移文件目录（global：可置于子命令前后任意位置）
cli-help-lang = 手动指定语言 (en, zh)
cli-database-url-required = 错误: 必须提供 --database-url（或设置 DATABASE_URL 环境变量）
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
cli-invalid-limit = limit 不能超过 1000000（0 表示不限制）
cli-invalid-operation = operation 必须是 create/read/update/delete/login/logout/permission-change/config-change 之一
cli-invalid-severity = severity 必须是 info/low/medium/high/critical 之一
cli-invalid-status = status 必须是 success/failure/partial/unknown 之一
cli-invalid-since = since 必须是 RFC 3339 时间戳（例如 2026-01-01T00:00:00Z）
cli-invalid-until = until 必须是 RFC 3339 时间戳（例如 2026-01-01T00:00:00Z）
cli-invalid-total-shards = total_shards 必须在 1 到 10000 之间
cli-invalid-strategy = strategy 必须是 yearly/monthly/daily/hash/consistent-hash 之一
cli-invalid-route-key = route_key 不能为空
cli-empty-role-or-table = role 与 table 不能为空
cli-invalid-action = action 必须是 select/insert/update/delete 之一
cli-permissions-file-unavailable = 权限配置文件不存在或不可读
cli-permissions-file-load-failed = 权限配置文件加载失败: { $error }
cli-permissions-file-invalid = 权限配置文件无法求值（配置格式错误）
cli-unsupported-db-url-protocol = 不支持的数据库 URL 协议
cli-migration-file-not-found-for-rollback = 未找到迁移 v{ $version } ({ $description }) 的迁移文件，无法执行 DOWN 回滚
cli-migration-rollback-failed = 迁移 v{ $version } 回滚失败: { $error }
cli-schema-diff-up-placeholder = -- 自动生成的 UP SQL 请手动编辑
cli-schema-diff-down-placeholder = -- 自动生成的 DOWN SQL 请手动编辑
cli-invalid-database-url-format = 无效的数据库 URL 格式: { $error }
cli-database-not-supported = 不支持 { $database }
cli-unsupported-database-protocol = 不支持的数据库协议: '{ $protocol }'。支持的协议: sqlite、postgres、mysql

pool-invalid-config = 无效的配置: { $error }
pool-read-config-failed = 读取权限配置文件 '{ $path }' 失败: { $error }
pool-parse-config-failed = 解析权限配置文件 '{ $path }' 失败: { $error }
pool-yaml-parse-error = '{ $source }' 中的 YAML 解析错误: { $error }
pool-invalid-db-url = 无效的数据库 URL: { $error }
pool-recreate-failed = 重新创建连接失败: { $error }
pool-log-acquire-timeout = 连接池获取超时: waited_ms={ $waited_ms } timeout_ms={ $timeout_ms } waiters={ $waiters }
pool-log-idle-check-wait-timeout = 连接池获取等待空闲连接校验回填超时: waited_ms={ $waited_ms } in_progress={ $in_progress }

health-check-timeout = 健康检查超时
health-check-timeout-recommendation = 健康检查超时，建议优化检查逻辑
health-recommendation-check-connection-config = 检查数据库连接配置是否正确
health-recommendation-check-network = 验证网络连接是否稳定
health-recommendation-ensure-pool-initialized = 请确保数据库连接池已正确初始化
health-recommendation-wait-circuit-breaker = 等待熔断器恢复
health-recommendation-increase-pool-size = 考虑增加连接池大小
health-recommendation-check-connection-leak = 检查是否有连接泄露
health-recommendation-increase-min-connections = 考虑增加最小连接数
health-status-failure-rate-too-high = 连接失败率过高: { $failed }/{ $created } ({ $percent }%)
health-status-no-connections = 无可用连接
health-status-circuit-breaker-open = 熔断器已打开，请求被拒绝
health-status-pool-full = 连接池已满: { $active }/{ $total } 活跃, { $waiting } 等待
health-status-no-idle-connections = 无空闲连接
health-details-pool = 连接池: total={ $total }, active={ $active }, idle={ $idle }, waiting={ $waiting }
health-details-stats = 统计: created={ $created }, failed={ $failed }, closed={ $closed }
health-details-circuit-breaker = 熔断器: { $state } ({ $failures } 次失败, { $successes } 次成功)
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
session-for-update-txn-required = query_rows_for_update 需要活动事务（请先调用 begin_transaction）
session-for-update-relational-required = query_rows_for_update 需要关系型（SeaORM）后端
session-isolation-relational-required = begin_transaction_with_isolation 需要关系型（SeaORM）后端
session-log-sql-parse-denied = 权限被拒绝: role={ $role } reason=sql-parse-failure (fail-closed)
session-log-permission-denied = 权限被拒绝: action={ $action } table={ $table }
session-log-rate-limited = 速率受限: 表访问被限流 (retry_after={ $retry_after })

session-already-in-transaction = 已处于事务中
session-already-in-transaction-concurrent = 已处于事务中（检测到并发开启）
session-already-in-graph-transaction = 已处于图事务中
session-already-in-graph-transaction-concurrent = 已处于图事务中（检测到并发开启）
session-connection-not-available = 连接不可用 - 会话可能已失效
session-not-in-transaction = 不在事务中
session-no-active-transaction-commit = 没有活动事务可提交
session-no-active-transaction-rollback = 没有活动事务可回滚
session-commit-transaction-in-use = 无法提交：事务正被并发查询使用
session-rollback-transaction-in-use = 无法回滚：事务正被并发查询使用
session-duckdb-ddl-admin-only = DuckDB 上下文中 DDL 操作仅允许 admin 角色。当前角色: '{ $role }'，admin 角色: '{ $admin_role }'
session-ddl-admin-only = DDL 操作仅允许 admin 角色。当前角色: '{ $role }'，admin 角色: '{ $admin_role }'

copy-path-empty = COPY 文件路径不能为空
copy-insert-column-required = INSERT 至少需要一列
copy-insert-row-required = 多行 INSERT 至少需要一行（空 VALUES 无意义）
copy-insert-table-invalid = INSERT 目标表标识符无效: '{ $table }'（允许字母/数字/下划线/点）
copy-insert-column-invalid = INSERT 列标识符无效: '{ $column }'（允许字母/数字/下划线/点）
copy-in-incompatible-connection = （连接池返回了不兼容的连接: { $error }）
copy-payload-path-utf8 = COPY 载荷临时路径不是有效的 UTF-8
copy-payload-cleanup-failed = DuckDB COPY 载荷临时文件清理失败: { $error }
copy-driver-feature-missing = ——启用 postgres 或 duckdb 驱动特性后方可使用 COPY
copy-payload-create-failed = 无法创建 COPY 载荷临时文件
copy-payload-create-io-failed = DuckDB COPY 载荷临时文件创建失败: { $error }
copy-payload-write-join-failed = DuckDB COPY 载荷写入任务 join 失败: { $error }

duckdb-execute-batch-failed = DuckDB execute_batch 失败: { $error }
duckdb-copy-prepare-failed = DuckDB COPY 预处理失败: { $error }
duckdb-copy-execute-failed = DuckDB COPY 执行失败: { $error }
duckdb-copy-fetch-failed = DuckDB COPY 结果读取失败: { $error }
duckdb-copy-count-read-failed = DuckDB COPY 计数列读取失败: { $error }

duckdb-pool-exhausted = DuckDB 连接池已耗尽: 无可用连接
duckdb-txn-begin-failed = DuckDB 开始事务失败: { $error }
duckdb-txn-commit-failed = DuckDB 提交事务失败: { $error }
duckdb-spawn-blocking-join-failed = spawn_blocking 任务 join 失败: { $error }

neo4j-connect-failed = neo4j 连接失败: { $error }
neo4j-url-no-credentials-env = neo4j URL 无凭据，且未设置 { $variable } 环境变量
neo4j-url-invalid-and-env-missing = neo4j URL 不是合法 URL，且未设置 { $variable } 环境变量
neo4j-not-connected = Neo4jConnection 未连接（占位连接）
neo4j-execute-failed = neo4j 执行失败: { $error }
neo4j-execute-with-params-failed = neo4j 参数化执行失败: { $error }
neo4j-row-fetch-failed = neo4j 行读取失败: { $error }
neo4j-row-deserialize-failed = neo4j 行反序列化失败: { $error }
neo4j-health-check-no-rows = neo4j 健康检查未返回任何行
neo4j-start-txn-failed = neo4j 开启事务失败: { $error }
neo4j-transaction-consumed = neo4j 事务已被消耗
neo4j-commit-failed = neo4j 提交失败: { $error }
neo4j-rollback-failed = neo4j 回滚失败: { $error }
neo4j-txn-execute-failed = neo4j 事务内执行失败: { $error }
neo4j-txn-execute-with-params-failed = neo4j 事务内参数化执行失败: { $error }
neo4j-txn-row-fetch-failed = neo4j 事务内行读取失败: { $error }
neo4j-txn-row-deserialize-failed = neo4j 事务内行反序列化失败: { $error }

repository-version-column-unsafe = repository 版本列必须是安全标识符: '{ $column }'
repository-version-field-missing = update_if_version 要求实体包含字段 '{ $column }'
repository-version-column-integer = 版本列 '{ $column }' 必须是整数
repository-column-unsafe = repository 列必须是安全标识符: '{ $column }'
repository-entity-deserialize-failed = 实体反序列化失败: { $error }

shard-route-unregistered = 分片键 '{ $key }' 路由到未注册的分片 { $shard_id }
shard-migration-orchestration-not-implemented = 分片迁移编排尚未实现：未接入 MigrationExecutor，该分片未执行任何迁移

migration-history-table-invalid = 非法的迁移历史表名: { $name }

query-cache-get-failed = 查询缓存读取失败: { $error }
query-cache-decode-failed = 查询缓存解码失败: { $error }
query-cache-encode-failed = 查询缓存编码失败: { $error }
query-cache-fill-failed = 查询缓存回填失败: { $error }
query-cache-clock-before-epoch = 系统时钟早于 Unix 纪元: { $error }
query-cache-invalidate-failed = 查询缓存失效操作失败: { $error }
query-cache-tables-required = 查询缓存要求 tables 中至少声明一个表；未声明表的查询永远无法失效（fail-closed）
query-cache-version-read-failed = 查询缓存表版本读取失败: { $error }
query-cache-params-encode-failed = 查询缓存参数编码失败: { $error }
query-cache-table-name-invalid = 查询缓存表名无效: '{ $table }'（允许字母/数字/下划线/点，不得以数字开头）

otel-serialize-failed = 序列化失败: { $error }
otel-http-task-failed = otlp HTTP 任务失败: { $error }
otel-endpoint-not-http = otlp 端点必须为 http://（当前: { $endpoint }）
otel-connect-failed = otlp 连接失败: { $error }
otel-write-failed = otlp 写入失败: { $error }
otel-read-failed = otlp 读取失败: { $error }
otel-collector-non-2xx = otlp collector 返回非 2xx: { $status }
otel-export-failed = otlp 导出失败: { $error }

kit-health-no-connections = 尚未建立任何连接
kit-health-waiting = { $waiting } 个等待，{ $active }/{ $total } 活跃/总数
kit-health-no-idle = 无空闲连接，{ $active }/{ $total } 活跃/总数

example-replica-routing-api-replica-pool-new = - ReplicaPool::new(pool, lag_detector)
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

    /// 从 FTL 常量体解析 (键, 占位符名集合)，供目录级守卫使用
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
        let en = catalog_entries(EN_FTL);
        let zh = catalog_entries(ZH_FTL);

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
}
