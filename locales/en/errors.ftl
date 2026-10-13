# dbnexus error catalog (en).
# One key per error/validation/status message surfaced to users;
# keys are `<domain>-<semantic>` in kebab-case. Placeholders are
# written `{ $name }` and must match the zh pack key for key.
# Operational log lines live in locales/en/logs.ftl.

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
perm-provider-yaml-parse-error = YAML parse error (path='{ $path }'): { $error }
perm-provider-lock-error = Lock error: permission provider internal state is poisoned
perm-rate-limiter-error = rate limiter backend error: { $error }
perm-rate-limit-audit-dropped = rate limit audit event dropped: { $error }
perm-cache-log-insert-failed = PermissionCache insert failed (error omits the raw key): { $error }
perm-cache-log-invalidate-failed = PermissionCache invalidate failed (error omits the raw key): { $error }
perm-cache-log-clear-failed = PermissionCache clear failed: { $error }
perm-cache-provider-no-policy = permission provider has no policy for role '{ $role }'
perm-cache-provider-not-configured = no permission provider configured for role '{ $role }'

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

pool-invalid-config = Invalid configuration: { $error }
pool-read-config-failed = Failed to read permission config file '{ $path }': { $error }
pool-parse-config-failed = Failed to parse permission config file '{ $path }': { $error }
pool-yaml-parse-error = YAML parse error in '{ $source }': { $error }
pool-invalid-db-url = Invalid database URL: { $error }
pool-recreate-failed = Failed to recreate connections: { $error }

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

kit-health-no-connections = no connections established
kit-health-waiting = { $waiting } waiting, { $active }/{ $total } active/total
kit-health-no-idle = no idle connections, { $active }/{ $total } active/total
