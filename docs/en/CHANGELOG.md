# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/lang/zh-CN/spec/v2.0.0.html).

<details open>
<summary>📑 Version Index</summary>

- [0.6.0 - 2026-11-14](#060---2026-11-14)
- [0.6.0-rc.10 - 2026-10-19](#060-rc10---2026-10-19)
- [0.6.0-rc.9 - 2026-10-11](#060-rc9---2026-10-11)
- [0.6.0-rc.8 - 2026-10-11](#060-rc8---2026-10-11)
- [0.6.0-rc.7 - 2026-10-11](#060-rc7---2026-10-11)
- [0.6.0-rc.6 - 2026-10-06](#060-rc6---2026-10-06)
- [0.6.0-rc.5 - 2026-09-21](#060-rc5---2026-09-21)
- [0.6.0-rc.4 - 2026-09-14](#060-rc4---2026-09-14)
- [0.6.0-rc.2 - 2026-09-03](#060-rc2---2026-09-03)
- [0.5.1 - 2026-08-06](#051---2026-08-06)
- [0.5.0 - 2026-08-04](#050---2026-08-04)
- [0.4.4 - 2026-07-23](#044---2026-07-23)
- [0.4.3 - 2026-07-22](#043---2026-07-22)
- [0.4.2 - 2026-07-22](#042---2026-07-22)
- [0.4.0 - 2026-07-13](#040---2026-07-13)
- [0.3.4 - 2026-07-12](#034---2026-07-12)
- [0.3.3 - 2026-07-11](#033---2026-07-11)
- [0.3.0 - 2026-07-03](#030---2026-07-03)
- [0.2.0 - 2026-06-27](#020---2026-06-27)
- [0.1.2 - Previous Release](#012---previous-release)

</details>

## [0.6.1] - 2026-11-15

### Added

- `embedded-runtime` feature: runtime (tokio + rustls) + config-env **without** any
  database backend. `embedded` is unchanged and still includes `sqlite`. Consumers
  that only need the embedded runtime should switch to `embedded-runtime`.

## [0.6.0] - 2026-11-14

### Added

- **External connection accounting `DbPool::track_external_connection()`**: a RAII API for registering physical connections outside the pool (returns `ExternalConnectionGuard`; dropping deregisters, with no manual release path — count drift is impossible by construction). `PoolStatus` gains an `external` field, and `health_snapshot().pool` gains an `external` key. Accounting notes: ① accounting does not occupy a semaphore permit and does not change the `max_connections` quota; the out-of-pool connection cap is enforced on the database side; ② `external` is not merged into `total`/`active`, and the semantics of `idle` and saturation are unchanged; the real physical footprint = outer metrics + external (still not the total number of PG sessions — the idle connections held by the inner sqlx pool are likewise not counted). Typical scenario: the caller takes a SeaORM connection via `Session.connection()` and runs a long-lived `begin()` transaction holding locks itself (e.g. a PG advisory lock); that connection is outside the outer pool's counting.

### Changed

- **i18n stack migrated to trait-kit**: the catalog switched to trait-kit's `I18nManager` registration surface and delegates formatting to `I18nFormatter`, retiring the self-maintained fluent runtime; the direct ICU4X/fluent dependencies were removed with the switch (`icu_casemap`/`icu_list`/`icu_segmenter`/`icu_experimental` leave the dependency graph)
- **In-house dependency finalization (0.6.0 stable)**: `trait-kit` 0.5.0-rc.14 → **0.5.0**, `confers` 0.6.0-rc.13 → **0.6.0**, `oxcache` 0.5.0-rc.12 → **0.5.0** (all three published to crates.io); manifest version precision converged from rc pins to minor (`"0.5"`/`"0.6"`); sub-crates `dbnexus-macros`/`dbnexus-limiter-port` synced to 0.6.0
- **Dependency refresh**: `libduckdb-sys` pinned 1.20000.0 → 1.20000.2, `lbug` 0.22.1 → 0.22.3, `neo4rs` 0.9.0-rc.10 → 0.9.0-rc.11, `prost` 0.14.4 → 0.14.6 (`itertools` 0.15 enters via `prost-derive`; the BSD-2-Clause license is already allowed in `deny.toml`)
- **rustdoc**: private-item intra-doc links in `src/i18n/catalog.rs` changed to plain text (fixing `private_intra_doc_links` blocking the CI Documentation leg); examples password seed literals changed to byte-string form (clippy)
- **CI**: CodeQL action upgraded (init/analyze)

## [0.6.0-rc.11] - 2026-10-31

### Changed

- **`QueryFragment::to_sql` identifier validation changed from panic to returning an error (breaking)**: signature `fn to_sql(&self) -> String` → `fn to_sql(&self) -> DbResult<String>` (the `query-dsl` feature). When a table name/column name/condition column/sort column fails the allowlist, `DbError::Config` is returned instead of a thread panic (a public API must not express input-validation failure as a panic). Migration: add `?` (or `expect`/`unwrap_err`) at former `fragment.to_sql()` call sites
- **Dependency refresh (landed after rc.10 finalization)**: `duckdb`/`libduckdb-sys` pinned 1.10506.1 → 1.20000.0 (the manifest accordingly converged from `1.10505` to `1.20000`, with `default-features = false` tightened); `confers` 0.6.0-rc.11 → 0.6.0-rc.12, `oxcache` 0.5.0-rc.8 → 0.5.0-rc.11, `trait-kit` 0.5.0-rc.10 → 0.5.0-rc.11; `lbug` 0.21 → 0.22 with `default-features = false` tightened; `neo4rs` stays at 0.9.0-rc.10 with `default-features = false` tightened. The README feature-status section has been updated to reflect `duckdb` 1.20000.0
- **In-house dependency alignment with the rc.12 wave (release finalization)**: `oxcache` 0.5.0-rc.11 → 0.5.0-rc.12, `trait-kit` 0.5.0-rc.11 → 0.5.0-rc.12 (both published to crates.io); sub-crates `dbnexus-macros`/`dbnexus-limiter-port` self-pinned in sync at 0.6.0-rc.11

## [0.6.0-rc.10] - 2026-10-19

### Added

- **SQLite PRAGMA configuration surface and BUSY-aware write retry**: a new PRAGMA configuration interface; write transactions hitting SQLITE_BUSY retry with backoff, reducing concurrent write-lock conflict failures.

### Changed

- **YAML backend migration**: `serde_yaml_ng` → `serde-saphyr` (pure Rust, eliminating the `RUSTSEC-2024-0449` unsafe-libyaml root cause).
- **i18n**: CLI help and error text fully migrated to the dual-track i18n, with `set_locale_from_system` added; `build_bundle` gains an EN fallback for unknown locales.
- **Release dependency propagation**: `confers` rc.10 → rc.11, `trait-kit` rc.9 → rc.10 (sub-crates `dbnexus-macros`/`dbnexus-limiter-port` self-pinned in sync at rc.10); `oxcache` stays at rc.8 (a reverse optional edge, per the cycle-breaking convention).
- **Dependency upgrades (landed 2026-10-19)**: `rustls` 0.23.45 → 0.23.46 — fixes RUSTSEC-2026-0346/0347/0348/0349 (`Debug` leaking TLS session keys, buffered application data sent after ECH rejection, a 1 GiB allocation in Brotli certificate decompression, unauthenticated ServerHello downgrade to classic key exchange); `duckdb` 1.10506.0 → 1.10506.1 (`libduckdb-sys` upgraded in sync; its build-time `zip` requirement ^6 → ^9) — fixes the zip 6.0.0 RUSTSEC-2026-0367/0368/0369 (symlink escape / Unix permission manipulation via symlinks / SUID·SGID bit preservation, critical 9.1 at the top). No new exemptions in `deny.toml` / `.cargo/audit.toml` (all 7 advisories are fixed by the upgrades)

## [0.6.0-rc.9] - 2026-10-11

### Fixed

- **The `sql-parser` feature implies `dep:oxcache` + `oxcache/memory`**: `prepare_cache` (synchronous LRU) is backed by the oxcache `DashMapMemoryBackend`; enabling `sql-parser` alone must activate oxcache — reverting the rc.6 "sql-parser decoupled from cache/oxcache" entry (that wording did not match the implementation, see the note at the end of the rc.6 entry), and fixing the compile failure of the `sql-parser` combination missing `DashMapMemoryBackend` after rc.8's default tightening

## [0.6.0-rc.8] - 2026-10-11

### Fixed

- **The `oxcache` dependency declaration adds the `memory` feature**: the oxcache optional dependency declaration was tightened to `default-features = false` with an explicit `features = ["memory"]`, fixing the `sql-parser` combination missing `DashMapMemoryBackend` after the default tightening (further converged by rc.9's feature implication)

## [0.6.0-rc.7] - 2026-10-11

### Removed

- **The hand-written `PermissionCache` implementation absorbed into oxcache (breaking)**: `PermissionCache` migrated from a hand-written DashMap dual-table TTL/SWR/throttling to oxcache's `Cache` (the stale + offload features). Removed the `len`/`is_empty`/`is_expired` public methods that had no oxcache equivalent and no production consumers; the read/write surface (`get`/`insert`/`invalidate`/`clear`/`refresh`) became async methods; `with_refresh_interval` maps to the stale window length (the original time-window throttling semantics are now carried by the stale window + offload in-flight dedup); the miss path gains provider backfill (when the provider returns None, no path writes the cache and the old value is kept). The `permission` feature adds the `oxcache/stale` dependency

### Added

- **`DbConfig::for_sqlite_path`**: a convenient bootstrap constructor for sqlite

### Fixed

- **Connection pool return made synchronous**: `release_connection` used to defer the connection push via `tokio::spawn` when `try_lock` failed — the return was postponed to the next scheduling point, and an immediately following `acquire_connection` would see an empty idle queue before the push landed and create a new connection (for backends like `duckdb::memory:` where "a new connection is a new database", the new connection cannot see data created earlier). Now the return is synchronous via a std Mutex; the critical section contains only the queue push/len, notify, and semaphore-permit return, with no await
- **No more redundant connection creation within the idle connection validation window**: `clean_invalid_connections` / `validate_and_recreate_connections` no longer hold the idle lock across awaits (the old implementation blocked the return side); changed to "take away for validation + backfill", with an `idle_check_in_progress` flag marking validation in flight; `acquire_connection` waits for the backfill and retries when the idle queue is empty and the flag is set. The wait budget is the smaller of the remaining `acquire_timeout` and 500ms, counted in the waiter metrics; if the validator stalls past the deadline, it degrades to creating a new connection and logs a warn. The flag is cleared via a guard when the function returns (including error paths), waking the waiters
- **Validation backfill no longer over-creates**: `validate_and_recreate_connections` checks the `max_connections` cap when rebuilding up to the minimum connection count, avoiding idle-queue over-provisioning combined with concurrent acquisitions

### Changed

- **i18n sweep**: CLI user-facing text defaults to English and is wired into the catalog
- **`prepare_cache` reuses the oxcache `CacheBuilder`**: the prepared statement LRU and the permission cache share the cache construction path
- **Redundant feature cleanup**: removed the empty dead feature `config-toml` and the unreferenced aggregate `dev-full`
- **CI/release pipeline**: GitHub Action references pinned to 40-character SHAs; the release workflow marks pre-releases by tag suffix (rc/beta/alpha); the coverage gate went 90% → 95%

## [0.6.0-rc.6] - 2026-10-06

### Added

- **Single source of truth for sharding strategies**: `database::sharding` adds the `is_known_strategy` export; the alias registry `STRATEGY_ALIASES` is shared by `is_known_strategy`/`create_strategy`
- **Bounded audit queries**: `AuditQueryFilters` gains a `limit` field (`DbAuditStorage` is made thread-safe into a SQL `LIMIT`; `MemoryAuditStorage` truncates synchronously); downstream literal constructions need `..Default::default()`
- **YAML permission provider loads on first query**: fixes the behavioral defect where a newly created `EngineYamlPermissionProvider` silently returned empty rules (deny-everything, with no signal) within the 60s lazy threshold window
- **Remaining ops CLI commands**: `dbnexus-cli` adds `pool-status` (a structured `health_snapshot` pool snapshot, exiting 1 on the unhealthy branch) / `audit-query` (filtered queries via `DbAuditStorage` with user/entity/operation/severity/status/time-range parameters; idempotent table creation, an empty set is success) / `shard-info` (strategy and shard listing output, `--route-key` demonstrating hash routing; unknown strategies are rejected explicitly, silent fallback forbidden) / `permission-check` (a permission config file + PDP decisions: allow→0 / deny→1 / config error→2), keeping the JSON output + exit-code 0/1/2 contract; `--database-url` became on-demand (shard-info/permission-check need no database)

- **Structured logging wiring tests**: `tests/observability_inklog_wiring_tests.rs` asserts the record levels and content of the four wiring points via an in-process TestLogger (including a log-injection sanitization case for malformed identifiers)
- **duckdb capability extensions**: a serialized write gate `with_serialized_writes` (not enabled by default; read concurrency is unlimited); a generic transaction closure `with_transaction` (readable inside the transaction, the connection is returned on success or failure); the pool mode gains `execute_batch` multi-statement batch execution
- **migration customization**: `with_history_table` makes the migration history table name customizable (the default-path SQL is byte-for-byte unchanged); `with_markers` makes the UP/DOWN migration marker sets customizable (the default semantics unchanged)
- **cache-guard triple protection**: penetration/stampede/avalanche protection and scatter cross-shard global merging
- **replica read-after-write sticky window**: probe result caching and parallel probing
- **session current-read and isolation levels**: `query_rows_for_update` current read, `begin_transaction_with_isolation` isolation-level control
- **Bound-parameterized execution API**: session/repository bound-parameterized execution and JsonRepository full-chain value binding
- **Business shard-key routing**: `calculate_for_key` / `route_for_key` (sharding)
- **Rate-limiting unification (limiteron port extraction)**: the permission-check rate limiter is extracted as the `Limiter` port (a new workspace member `dbnexus-limiter-port`, depending only on `async-trait`, on neither dbnexus nor limiteron) — limiteron already optionally depends on dbnexus, Cargo forbids package-level circular dependencies (optional included), and a reverse integration from dbnexus would inevitably form a cycle; hence the port became a standalone crate that both sides depend on one-way, assembled at the application composition root (zero dependency cycles in full-graph resolution; the design is in `docs/en/RATE_LIMITING.md`); the legacy token bucket remains the default backend (behavior unchanged), and the new `PermissionContext::with_cache_size_and_backend` provides dual-backend switching (`TokenBucket`/`External(Arc<dyn Limiter>)`); **429 response semantics**: the new `TableAccessDecision` (`Allowed`/`Denied`/`RateLimited{retry_after}`) and `DbError::RateLimited{retry_after_secs}` (`ErrorCode::RateLimited = 2002`); all session execution paths distinguish a policy denial (403) from a rate-limit denial (429, carrying `Retry-After`), with rate-limiting backend failures fail-closed and alerted explicitly; **audit events**: under the `audit` feature, once attached via `set_audit_logger`, a rate-limit denial produces the `rate_limit_exceeded` audit event (table name/SQL action/Retry-After go into extra; write failures do not block the decision but are alerted explicitly); the legacy boolean API `check_table_access` is behavior-compatible; migration guide at `docs/en/RATE_LIMITING.md`
- **HTTP health endpoint generator**: the new `http-health` feature (axum `0.8` optional, `default-features = false`); `dbnexus::integrations::http_health::HealthRouterBuilder` takes the pool (required)/circuit breaker/metrics collector (optional) via chaining and produces an axum Router mounting `/healthz` (liveness, always 200) / `/readyz` (readiness: judged by `health_snapshot`, healthy/degraded → 200, unhealthy and unknown statuses fail-closed → 503, a breaker `Open` overrides to not-ready and reports the `circuit_breaker` field) / `/metrics` (Prometheus text, an explicit 404 when no collector is injected); generates rather than serves (listening/graceful shutdown are orchestrated by the consumer via `axum::serve`), and with the feature disabled the library has zero HTTP dependencies; example `examples/src/observability/http_health.rs`
- **Batch write (`copy`)**: the `BatchInsertStatement::chunk_rows_owned` by-value variant — the row set is consumed by value and parameters are moved at the pointer level via `Vec::append`, sparing callers that own the row set the per-value deep copies of the borrowing version (the biggest gains are for JSON object/array columns); the validation and chunking contracts match the borrowing version
- **Metrics (`metrics`)**: the `MetricsCollector::slow_queries_count()` zero-clone count accessor (count-only scenarios such as health snapshots avoid deep-copying the whole ring)
- **First-class oxcache query cache integration (`oxcache-integration`)**: the new `OxcacheQueryCache` query cache decorator — `query_cached` derives the key (SHA-256, length-prefix encoding is injective) from the **role/namespace security context + data-protection policy epoch + SQL + bound parameters + table version raw bytes** (row-set contents depend on the execution-time RLS/masking/permission context; multiple instances sharing a backend must distinguish with `with_role`/`with_namespace`, or a cross-context hit is an unauthorized read); a hit returns the cached row set (marked `from_cache`), a miss executes through and backfills; **policy-swap invalidation**: under the `data-protection` feature the pool-side epoch counter participates in key derivation, so `set_data_protection` swapping at runtime immediately invalidates entries backfilled under the old policy (old RLS predicates/masking exits do not produce dirty reads; tightening/loosening takes effect immediately, no manual invalidation by the caller); `invalidate_table` stamps the version with a per-table nanosecond timestamp as a decimal string after the write path (injective, idempotent, free of read-modify-write races; repeated invalidations do not collide back to the old key), letting cache entries mentioning that table expire naturally; empty `tables` sets and illegal table names are rejected fail-closed (omitting them would mean never being affected by invalidation); version read failures and cache backfill failures both error explicitly; the feature dependencies gain `sha2` (key derivation)

### Changed

- **sql-parser decoupled from cache/oxcache**: `sql-parser` no longer implies `cache`→oxcache — the review confirmed the only coupling point was `SqlParser`'s parse-result cache (formerly via oxcache `Cache`'s moka async backend), now carried by an in-library synchronous LRU (reusing `prepare_cache`'s `PreparedStatementCache` port, adding probe-style `get`/`insert`/`clear` methods, with entry and hit/miss/eviction statistics semantics unchanged; failed parses still do not enter the cache); `permission`'s real consumption of oxcache (the DbPool permission policy cache) now explicitly carries `cache`, with the `permission`/`default-no-db` closures unchanged overall; enabling `sql-parser` alone (e.g. with `retry`) no longer drags in oxcache/moka; `cargo tree` verified that the `--features sql-parser` closure contains zero oxcache; the `prepare_cache` module itself lost its feature gate (external re-exports remain gated by `prepare-cache`); the `SqlParser` public API signatures are unchanged (construction/clearing keep async signatures, and `Default` no longer depends on a tokio runtime); the contract test `tests/feature_dependency_contract.rs` pins the decoupling invariant via `cargo metadata` (**note**: from rc.7, `prepare_cache` switched to oxcache as its cache backend, and rc.8/rc.9 re-made `sql-parser` imply `dep:oxcache` + `oxcache/memory` — the "zero oxcache in the `--features sql-parser` closure" wording of this entry is obsolete; the current state is governed by the `sql-parser` feature declaration in `Cargo.toml` and the rc.9 entry)

- **Driver mutual-exclusion guard (`duckdb` × `ladybug`)**: the mbedtls duplicate-symbol link conflict escalated to a compile-time mutual exclusion — the root cause was located in both drivers bundling their own vendored mbedtls static library (libduckdb-sys's `duckdb/third_party/mbedtls` and lbug's `lbug-src/third_party/mbedtls`, both linked whole as a same-named `mbedtls` static library, with the duckdb side additionally providing symbol-replacement implementations such as rsa_alt/sha256_alt), unresolvable at the dependency level; enabling `duckdb` and `ladybug` together now triggers `compile_error!` (with grouped-enable guidance), and the `ladybug` + `sqlite` combination is unaffected; the guard contract test `tests/feature_mutex_guards.rs` pins the guards' existence/scope (all guards uniformly gated on `not(clippy)`)

- **MySQL integration test regularization verified**: the roadmap item "restore regular MySQL integration test runs" is now complete — the CI `test` job runs the regular `db: [sqlite, postgres, mysql]` matrix (a mysql:8.0 service container + mysqladmin health check + `DATABASE_URL` injection + full tests with `--features mysql,default-no-db,all-optional`); the README record now matches the actual state

- **HTTP health endpoint (`http-health`) deployment contract**: the module and API documentation now state explicitly that the three endpoints have no authentication/no rate limiting — `/readyz` returns the full pool snapshot and `/metrics` returns the full metrics document; the example listen address changed to loopback, requiring placement behind an internal network/reverse-proxy authentication or nesting into an authenticated admin Router; the full `/metrics` export is offloaded via `spawn_blocking` (avoiding blocking executor threads); the `ReplicaHealthProvider` contract is made explicit (must be cheap/purely synchronous/free of blocking IO)

- **Batch write (`copy`) error contract**: `copy_in` rejections on non-COPY backends are unified as a `DbError::Query` base message (the postgres/duckdb support scope + guidance to switch non-COPY backends); when a driver group is enabled but the pool connection type mismatches, the original downcast error passes through as a footnote ("got SeaOrm" etc.), with remediation guidance to append the driver feature when no driver group is present — the observable variant of the duckdb mismatch path changed from `DbError::Connection` to `DbError::Query`

- **Removing the `dbnexus → inklog` dependency edge (cycle prevention)**: during rc.6 development an `inklog` feature and an `init_inklog_logger()` convenience entry had been added. Because inklog itself already optionally depends on dbnexus (`^0.6.0-rc.5`; the caret would match rc.6), enabling both `dbnexus/inklog` and `inklog/{sqlite,postgres,mysql,duckdb,dbnexus-audit,kit}` triggered cargo's hard `cyclic package dependency` failure (reproduced in practice). This version removes that feature, `src/integrations/inklog.rs`, and the optional dependency; the logging backend is installed by the consumer as the global `log` backend — the ability of the four wiring points (pool timeout/permission denied/circuit breaker/slow query) to emit records via the `log` facade and its tests are unchanged, as is the default build behavior

- **Third-party dependencies refreshed to the latest stable versions** (a unified `cargo update` before release; gates and coverage re-run on the refreshed lock set): cross-repo pins converged to `confers 0.6.0-rc.6` / `oxcache 0.5.0-rc.6` / `trait-kit 0.5.0-rc.7` (the `inklog` dependency edge has been removed, see the previous entry); key direct dependencies `sea-orm 2.0.2 → 2.0.4`, `libduckdb-sys/duckdb 1.10505 → 1.10506` (a bundled engine minor version), `tokio 1.53.1 → 1.53.2`, dev dependency `testcontainers 0.27.3 → 0.28.0` (replacing dependabot PR #29), `cc 1.4.6 → 1.6.0`, `cxx 1.0.199 → 1.0.202`, `lbug 0.20.2 → 0.20.4`; the `trybuild` dependency moved from `target-triple` to `target-tuple 1.0.2` (the upstream's official rename, verified on crates.io as dtolnay's official crate); `cargo deny check` (advisories/bans/licenses/sources) and `cargo audit` both pass with 0 vulnerabilities

### Fixed

- **[Security] backend-aware `sql_literal` escaping fixes MySQL backslash injection**: string literals on the column projection/cursor pagination/optimistic-lock paths are escaped per backend rules; the MySQL backend (backslash as an escape character) previously had an injection surface
- **duckdb connection pool return**: pool connections now return via RAII, the connection is returned on both success and failure of statement execution, and the write gate reuses idempotently
- **saga persistence made explicit**: persistence failures are no longer silent; replay skips already-compensated steps; compensation session failures are reported explicitly
- **global-index / health**: a secondary index for the global index table's query column; the preconditions of the zero-connection health test made explicit
- The permission audit chain module's header links changed to full paths, fixing rustdoc `-D warnings` false reds
- **admin executing table-free statements was wrongly rejected**: in the `execute_raw`/`query_rows` permission pipeline, the table-name validity check (empty `all_table_names`/illegal table names) used to come before the admin bypass, so admin executing FROM-less statements (`SELECT 1` connectivity probes / health check warmups, legitimate scenarios) was wrongly rejected; now the admin bypass is hoisted in front as a whole (the table-name check is part of the permission check and is skipped with the bypass), while non-admin fail-closed rejection and per-table validation are unchanged; the regression test `test_admin_bypasses_table_name_validity_check` asserts both directions (reproduced at HEAD 2861e3c, not introduced by this round's feature changes)
- **Restricted-role sessions in query cache tests were denied by the safe default**: the two cases of `oxcache_query_cache_tests` (cross-role isolation/hash field injection) used pools without a permission file, so `get_session` on `with_role` instances was denied by the safe default (admin/system only) — the test pools now explicitly carry a permission configuration (defining the restricted roles and table permissions, with newline-containing role names carried via JSON escaping), 12/12 green (reproduced at HEAD 2861e3c)
- **Ops CLI end-to-end test accounting adaptation**: the whole `ops_cli_tests` suite gained explicit `#![cfg(...)]` gating (the subcommands are gated by cli features such as `migration`/`health-check`/`permission-engine`, which do not exist under the workspace `--no-default-features` accounting), so that accounting skips them at compile time; `cargo test -p dbnexus-cli` under the default accounting is 25/25 green (adapting the workspace accounting to the cli's default features is a pending CI feature-engineering item, recorded in reviews/WS_R14_REVIEW.md)
- **`global-index` compile crack in no-driver combinations**: the `global-index` feature now implies `entity-macros` (the sea-orm derive forwarding) — the `DeriveEntityModel`/`DeriveRelation` in `src/storage/global_index.rs` depend on sea-orm/macros, so no-driver combinations (`all-optional`/`data-management`/`global-index`) previously failed to compile (23 errors reproduced at HEAD; CI never exposed it because it always carries `sqlite`); a contract test pins the implication
- **Batch write (`copy`)**: multi-row INSERT chunking shrinks by `min(500, bind_param_limit/column count)` (the placeholder count per statement never exceeds the backend bind limit for wide tables; a column count above the limit errors at build time); the DuckDB CSV encoding adds quote escaping for JSON object/array values (values containing commas/quotes no longer break the column structure); the COPY payload temp file's permissions tightened to owner-only 0600 and synchronous file I/O offloaded via `spawn_blocking`
- **Metrics (`metrics`)**: the `type` label value of the Prometheus export is escaped per the text exposition rules (`\` `"` newline) — free-form strings in the public `record_query` API containing injection payloads can no longer escape the label or forge metric lines
- **rustdoc broken links (under `RUSTDOCFLAGS=-D warnings`)**: 7 documentation links fixed — `is_safe_identifier`/`UP_MARKERS`/`DOWN_MARKERS` are `pub(crate)` private items changed to plain code font (executor.rs/repository.rs); the short links in the module-level docs of `oxcache_query_cache` and `http_health` (`CacheBackend`/`DbPool`/`DbError::Cache`/`Router`) resolve under the parent module's scope when the module declaration carries outer documentation and are invisible to in-module `use`s, changed to fully qualified paths
- **session module feature-combination compile cracks**: `json_to_sea_value`/`build_statement` gained `sql-parser` gating (all call sites are on the raw query path, so combinations like neo4j-only no longer produce dead-code warnings); `apply_masking` gained the `data-protection × (postgres|sqlite)` gate (call sites exist only in the postgres/sqlite raw branches, so duckdb-only combinations no longer produce dead-code warnings); the early return of the no-sql-parser branch of `query_rows_impl` gained a `needless_return` exemption (inside a cfg-mutex block that return is exactly the function tail, a false positive under clippy's single-combination accounting); the session test module's `use super::*` glob is uniformly exempted across the feature matrix (unused warnings under bare/partial combinations)
- **db_pool dead-connection tests not simulatable (disclosed truthfully)**: two `db_pool` cases (cleanup/validate killing connections) remain `#[ignore]` — sea-orm 2.0 sqlite goes through `RusqliteSharedConnection`, an in-process embedded database, where `SELECT 1` always succeeds and `close` cannot really kill a connection, so a "dead connection" cannot be simulated; the restoration condition is cleanup/validate supporting an injectable health probe (the reason is inlined in the ignore attribute); the Neo4j server-dependent cases keep their environment-variable gating

## [0.6.0-rc.5] - 2026-09-21

### Changed

- **dbnexus-macros enhancement**: the new `entity-macros` feature forwards the sea-orm derives — consumers can define entities via the `dbnexus::sea_orm` re-export without depending on sea-orm directly; the `sea_orm` re-export is no longer bound to driver features
- **i18n overhaul**: unified management of error and message text
- **Dependency upgrades**: sha2 0.10.9 → 0.11.0 and a patch-group refresh; cross-repo path dependencies moved to crates.io
- **Engineering hardening**: the detect-secrets baseline, pre-commit gates, the typos false-positive allowlist, and a clippy legacy cleanup

---

## [0.6.0-rc.4] - 2026-09-14

> Note: 0.6.0-rc.3 was not released separately (no tag, never on crates.io); this section includes the original rc.3 development batch and shipped together with 0.6.0-rc.4.

### Added

- **JWT revocation TTL**: `JwtManager` supports an optional `DbCacheProvider` distributed revocation cache (the `oxcache-integration` feature); the revocation entry TTL equals the token's remaining lifetime, and the local HashMap keeps the synchronous fast path
- **Slow-query detection wiring**: `Session::execute_raw` inserts timing before every return path, recording to `MetricsCollector` automatically when above the `SlowQueryConfig` threshold
- **Cache DI closed loop**: `Session` gains `query_cache_get`/`query_cache_set` methods, driving read-only query result caching through the injected `DbCacheProvider`; the `cache_provider` field moved into `DbPoolInner` and is read lock-free via `ArcSwapOption`
- **kit observer fix**: the `kit` feature explicitly includes `trait-kit/observer`; the gated integration test `tests/kit_feature_gate.rs` was added
- **Unified row query API**:
- **Saga persistent recovery**:
- **Field-level automatic masking and RLS**:
- **confers hot reload**:
- **Structured health export**:
- **COPY batch writes**:
- **DB audit storage**:
- **Permission-change audit chain**:
- **OTel export bridge**:
- **Kit full-capability registration**:
- **End-to-end performance benchmark**:
- **Ops CLI**:
- **Unified injection detection engine**:
- **Generic repository**:
- **Data API gateway**:
- **Prepare cache**:
- **Unified permission facade**:
- **Query DSL macro**:
- **Entity event bus + Outbox**:

### Changed

- **Saga compensation no longer silent**: compensation failures are recorded in `SagaStepLog`, a new `CompensationFailed` terminal state was added, and compensation errors are no longer swallowed
- The `cache_provider` storage changed from `Option<Arc<...>>` to `ArcSwapOption<Arc<...>>`, eliminating unsafe writes (compatible with `#![forbid(unsafe_code)]`)
- The `arc-swap` dependency extended to the `cache` and `oxcache-integration` features
- **Retry auto-wiring**:
- **Replica load balancing**:
- **DDL guard normalization**:
- **Unified error system**:
- **Large-file splitting**:
- The `DbNexusModule` capability type changed to `Arc<DbPool>` (prerequisite), still usable as a `ConnectionPool` trait object

### Fixed

- `verify_refresh_token` / `refresh_access_token` became async, cascading updates to all call sites (tests, examples, documentation)

---

## [0.6.0-rc.2] - 2026-09-03

### BREAKING CHANGES

- **Removed the `tracing` feature**: the OpenTelemetry distributed tracing module and the related `TracingGuard`/`TracingError` APIs have been removed from the code. Requestors should instrument directly with the `tracing` / `tracing-opentelemetry` / `opentelemetry-otlp` crates, avoiding lexical conflicts with same-named upstream sub-dependencies

### Changed

- Dependency path localization: `oxcache` / `trait-kit` in `[workspace.dependencies]` and `examples/` now use dual `path` + `version` declarations, shared by local integration debugging and the CI release mode
- `Cargo.lock` added to version control (the library crate now generates and commits it)
- Version bumped to `0.6.0-rc.2` (the next minor pre-release)

### Fixed

- **Pool driver mutual exclusion strictified**: removed the escape hatch for enabling all 4 drivers together; compile-time `compile_error!` guarantees `sqlite` / `postgres` / `mysql` / `duckdb` are mutually exclusive
- **Health check semantics made real**: `permission::health_check` validates capacity for the memory policy table and readability for the YAML policy file
- **JOIN / subquery permission enforcement**: completed permission checks on cross-table JOIN paths
- **Global index test isolation**: each case gets an independent in-memory DB handle, eradicating parallel upsert races
- **h2 security upgrade**: the `h2` crate upgraded to a version fixing the DoS vulnerability
- **macros dependency slimming**: the `proc-macro-error2` dependency removed (no longer needed), reducing compile overhead
- **Connection pool acceptance fixes**: cross-feature-gate consistency, failover cleanup, and Snowflake ID test stability

### Documentation

- Fixed the stale breaking-change descriptions of `v0.2` / `v0.4` left in the README
- Synced the README preset feature list with the actual `Cargo.toml` values
- Fixed leftover version numbers, dependency names, and the `AsyncMutex` description errors in USER_GUIDE / ARCHITECTURE
- Synced documentation version numbers to `0.6.0-rc.2`
- Removed all descriptions and examples of the `tracing` feature from USER_GUIDE / API_REFERENCE / ARCHITECTURE
- Added a multilingual identifier helper under the `docs/` subdirectory

## [0.5.1] - 2026-08-06

### Performance

- **SQL injection detection**: the pattern table changed from a stack array to `static &[&str]`, eliminating the per-call array rebuild overhead
- **Saga compensation lookup**: a pre-indexed `HashMap<&str, usize>` replaces the O(n²) linear scan
- **Masking**: `sanitize_json_object` changed to an owned-value signature, eliminating recursive clones
- **Session concurrency**: `tokio::sync::Mutex` → `tokio::sync::RwLock`, allowing concurrent access to pure-read operations (`is_in_transaction`, `get_graph_transaction`, etc.)
- **Connection pool config sharing**: `DbConfig` is shared via `Arc<DbConfig>`, avoiding deep copies during warmup
- **Connection acquisition slimming**: the `acquire_connection` hot path extracted into an `#[inline]` helper; feature-gated branches cost nothing
- **Atomic ordering downgrade**: `SeqCst` → `AcqRel`, reducing unnecessary global synchronization
- **Lock-held await fix**: the `begin_transaction` concurrent-conflict path changed to the extract-check-operate-writeback pattern, avoiding `.await` while holding the lock

### Removed

- **Deprecated builder method cleanup**: removed setters marked `#[deprecated]` such as `DbPoolBuilder::metrics_collector()`, `permission_config()`, `with_oxcache()`
- **CLI dead-code cleanup**: removed unused helper functions and imports from `src/tools/cli/src/main.rs`

### Changed

- `docs/CHANGELOG.md`: moved from the project root into `docs/` for unified documentation organization
- Removed `.semgrepignore` (the project does not use Semgrep)

## [0.5.0] - 2026-08-04

### Added

- **Distributed capability examples (7 new examples)**: completing the missing core capability module examples in `examples/`
  - `distributed_id`: the Snowflake ID generator — batch generation, ID parsing, multi-threaded concurrency, error scenarios
  - `saga`: Saga distributed transaction orchestration — custom SagaAction, success/failure + compensation scenarios, SagaLog
  - `scatter_gather`: cross-shard Scatter-Gather queries — aggregate functions, PartialFailurePolicy
  - `replica_routing`: replica routing read/write splitting — ReplicaConfig, FailoverConfig coordination
  - `shard_migration`: the shard migration orchestrator — parallel/serial modes, OrchestratedMigrationResult
  - `retry`: runtime retry + exponential backoff — idempotency judgment, custom policies, retry exhaustion
  - `failover`: the connection failover chain — the CircuitBreaker state machine, DbConfig integration
- Added examples passthrough of the `distributed-capabilities` aggregate feature

### Fixed

- **Connection pool test assertion sync**: `test_pool_status_and_config` and `test_connection_pool_trait_methods` updated their assertions to match the pool's pre-created min_connections behavior
- **DuckDB test feature guard**: `test_create_connection_duckdb_not_enabled` gained `#[cfg(not(feature = "duckdb"))` to avoid running when the feature is enabled
- **Kit health check test sync**: `health_check_unhealthy_before_first_use` and `full_kit_with_lifecycle_health_observer` updated their expectations to match the Healthy state after the pool pre-creates connections
- **Documentation ghost-entry cleanup**: removed the nonexistent `tracing_otlp` entry from `examples/README.md`

### Changed

- `docs/API_REFERENCE.md`: added distributed capability module API documentation
- `docs/ARCHITECTURE.md`: updated the architecture diagram to reflect the distributed capability module

## [0.4.4] - 2026-07-23

### Changed

- Removed the tilde (`~`) from dependency version constraints: `dashmap ~6.2`→`6.2`, `jsonwebtoken ~10`→`10`, `bcrypt ~0.19`→`0.19`, unified into the Major.Minor exact format
- Dependency refresh: `cargo update` pinned the lockfile to the latest compatible versions (including oxcache 0.3.9→0.3.12, trait-kit 0.3.0→0.3.1, tokio 1.52.4→1.53.1, uuid 1.23.5→1.24.0, duckdb 1.10504→1.10505, arrow 58.3→58.4, etc.)

### Security

- `cargo deny check advisories bans` passed with no known vulnerabilities (the unmaintained warnings are pre-existing transitive dependencies, already registered in deny.toml)

### Notes

- The regression tests used the feature combination supported by the project CI, `--no-default-features --features sqlite,default-no-db,all-optional` (covering the cache/security features where dashmap/jsonwebtoken/bcrypt live), all passing; `--all-features` hits the duplicate-symbol link conflict because duckdb and lbug each bundle mbedtls — a pre-existing architectural limitation (CI never uses `--all-features`), not introduced by this round's dependency changes

## [0.4.3] - 2026-07-22

### Fixed

- **[fix-cache-key]** The `cache_key(id: i64)` generated by the `#[db_entity]` macro changed from a hard-coded i64 to the generic `cache_key<PK: Display>(id: PK)`, fixing the compile error when calling `cache_key` on Uuid/String primary-key entities with `cache(...)` enabled (the known limitation noted in 0.4.2). Added 3 regression tests covering cache_key calls for the Uuid/i64/String primary key types

### Testing

- Added `tests/e2e_advanced.rs` (89 tests): covering boundary and error scenarios of 8 modules — SensitiveMasker(11), CircuitBreaker(12), ShardRouter(13), GlobalIndex(10), Authentication(14), i18n(16), Tracing(5), PoolHealthMetrics(13). 2 tests marked `#[ignore]` (requiring an OTLP collector)

### Maintenance

- Removed unused dependencies: toml (main), serde (macros), serde/time/toml (cli), etc.

## [0.4.2] - 2026-07-22

### Fixed

- **[fix-pk-types]** The primary key parameters generated by the `#[db_entity]` macro changed from a hard-coded `i64` to generics, fixing the `the trait bound 'uuid::Uuid: From<i64>' is not satisfied` compile error when Uuid/String primary-key entities call them. Specific constraints: `find_by_id` / `delete` (the soft_delete=false branch) became `PK: Into<<<Entity as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType>` (mapping to `Entity::find_by_id`); `find_by_ids` / `delete` (the soft_delete=true branch) / `force_delete` became `PK: Into<sea_orm::Value>` (mapping to `Column::eq` / `is_in`). Fully backward compatible (i64 primary keys need no call-site changes). Added `tests/entity/unit/db_entity_pk_types_test.rs` with 11 regression tests covering i64/Uuid/String primary keys × compile-time/signature/ActiveModel/soft_delete=true branch verification
- **[known-limitation]** `cache_key(id: i64)` still hard-codes i64 (inconsistent with this round's genericization direction); with `cache(...)` enabled on Uuid/String primary-key entities, calling `cache_key` fails to compile. This issue is beyond this round's fix scope (rule 6, surgical changes) and is planned for 0.4.3

### Security Fixes

- **[vuln-0001]** Added audit logging for admin bypass operations, with a warning for the default `admin_role`
- **[vuln-0002]** `add_user` validates the bcrypt hash format, rejecting malformed password hashes
- **[vuln-0003]** Table-name extraction replaced naive string matching with `SqlParser`, eliminating the injection surface
- **[vuln-0004]** Password policy enhanced: a blacklist + complexity requirements
- **[vuln-0005]** Added `execute_cypher_with_params` and deprecated raw Cypher execution, adding injection protection
- **[vuln-0006]** Relaxed the `jsonwebtoken` constraint to `~10`, accepting the CVE-2026-25537 patched version

### Refactoring

- graph/pool: `HD-1` the `execute_cypher_with_params` default implementation returns an error instead of silently falling back (eliminating the Liskov injection risk)
- auth: `HD-3` strengthened the `pub(crate)` constraint documentation of `add_user_unchecked` + responsibility-separation tests; `HD-4` made `PasswordPolicy` configurable + a custom blacklist
- pool: `HD-5`/`MD-4` extracted `audit.rs` to separate security audit logic; `MD-1` extracted `execute_cypher_in_transaction` to reuse graph transaction dispatch; `MD-3`/`LD-4` simplified `batch_execute_in_transaction`; `LD-2` made `execute_with_operation` reuse `record_metrics_and_mark_write`
- `HD-2`/`MD-2`/`LD-1` false-positive documentation notes (trait decoupling / the necessity of `Send+Sync` / `Mutex` protection timing)
- examples/tests/benches: extended L1 re-export isolation + added `tokio`
- lib.rs: documented the L1/L2/L3 re-export scopes

### Performance

- Removed `eprintln!` from the audit_admin_bypass hot path

## [0.4.0] - 2026-07-13

### Added

- **Graph database support**: added `LadybugConnection` (embedded graph DB, based on lbug 0.18) and `Neo4jConnection` (server-side graph DB, based on neo4rs 0.8), unified through the `GraphConnection` trait
- **Graph transactions**: `Session` gains `execute_cypher`, supporting graph transaction dispatch (begin/commit/rollback), with graph transaction handles abstracted by the `GraphTransaction` trait
- **Macro extensions**: `#[db_entity]` gains the `exists()` and `find_by_ids()` methods; the new `#[db_graph]` macro generates graph entity code; macro parameter type validation enhanced (non-string literals error)
- **Permission model**: `PermissionAction` gains the `Traverse` and `Match` variants for graph operation permission control
- **DatabaseType**: gains the `Ladybug` and `Neo4j` variants; `from_url` recognizes the `ladybug:`/`neo4j:`/`neo4j+s:`/`neo4j+ssc:` schemes
- **DbConnection**: gains the `Ladybug(Arc<LadybugConnection>)` and `Neo4j(Arc<Neo4jConnection>)` variants, with the `as_graph()`/`is_graph()` methods
- **feature gate**: the new `ladybug` and `neo4j` features (not mutually exclusive with relational DBs, mixed use allowed)

### Changed

- The `trait-kit` dependency upgraded `0.2` → `0.3`
- The `oxcache` dependency upgraded `0.3.4` → `0.3.8` (required by trait-kit 0.3)
- `parse_url` (Neo4j) returns a `Result`, reading credentials from environment variables or returning a clear error when absent
- `LadybugConnection` uses `Arc<Semaphore>` to limit concurrency (replacing the `Vec<Connection>` pool, due to lbug lifetime constraints)

### Fixed

- Graph transaction concurrency race — the take → await outside the lock → put back pattern of `execute_cypher` bypassed transaction isolation under concurrency; the new `graph_op_mutex` serializes graph operations
- **FM-3.1** (RPN=216): transaction isolation bypassed when `execute_cypher` panics — the PoisonGuard RAII pattern sets the poisoned flag during unwinding
- **FM-3.6** (RPN=294): `Session::Drop` did not handle graph transactions — solved via cascading Drop (the LadybugTransaction actor automatically ROLLBACKs; Neo4jTransaction::Drop spawns a rollback)
- **FM-2.2** (RPN=168): `Neo4jTransaction` had no Drop impl — implemented Drop with `try_lock` + a spawned rollback task
- **FM-1.6** (RPN=144): `begin_graph_txn` had no concurrency limit — `LadybugTransaction` holds an `OwnedSemaphorePermit` until commit/rollback
- **M-48**: `LadybugConnection::parse_url` path traversal validation — rejects paths containing `..`
- **M-49**: `Neo4jConnection::parse_url` error messages do not echo the original URL — avoiding credential leakage
- `LadybugConnection::execute_cypher` row-fetching logic duplicated — deduplicated into the shared `execute_cypher_on_conn` function
- Macro parameter type mismatches silently ignored — 4 parameters + hooks gained `else` branches that error

### ⚠️ BREAKING CHANGES

- The `default` feature became an empty array `[]` (it previously contained the 7 features runtime-tokio-rustls/sqlite/permission/sql-parser/macros/config-env/with-time)
- The new `default-no-db` aggregate feature provides the common feature set without database drivers
- Users must explicitly enable runtime + driver + features; `default-features = false, features = ["default-no-db", "sqlite"]` is recommended
- The pre-commit scripts and CI uniformly use the `--no-default-features --features sqlite,default-no-db,all-optional` command

## [0.3.4] - 2026-07-12

### ⚠️ BREAKING CHANGES

- The `error` module moved from `src/common/error.rs` to `src/error.rs`; the import path `crate::common::error::` → `crate::error::`

## [0.3.3] - 2026-07-11

### Changed

- **Permission export dedup**: eliminated duplicate permission module exports
- **RUSTSEC-2023-0071 documented**: recorded the `rsa` crate security advisory (no upstream fix yet; declared as ignored in `deny.toml`)

### Changed

- **edition 2024 upgrade**: the workspace moved from edition 2021 to edition 2024
- **rust-version 1.85**: the minimum supported Rust version raised to 1.85 (required by edition 2024)
- **MIT license unification**: the workspace uniformly adopts the MIT license; all sub-crates (dbnexus, dbnexus-macros, dbnexus-examples) use the same license

## [0.3.0] - 2026-07-03

### ⚠️ BREAKING CHANGES

- **Version bump**: the workspace version `0.2.0 → 0.3.0` (including `Cargo.toml` + `macros/Cargo.toml` + `examples/Cargo.toml`)
- **README rename**: `README.md → README_EN.md`, `README_zh.md → README.md` (Chinese becomes the primary README)
- **Compile-time mutual exclusion rule fix**: `sqlite`/`postgres`/`mysql` are now truly mutually exclusive (the old rules allowed enabling them together, contradicting the documentation's "Must enable exactly one")
- **`observability` preset correction**: changed from `["metrics", "health-check"]` to `["metrics", "health-check", "tracing"]` (consistent with the README description)
- **`dbentity-v2-rewrite` change archived**: 88/88 tasks completed, archived to `openspec/changes/archive/`

### Added

#### New Features

- **The `duckdb` feature**: DuckDB OLAP backend support, based on the `duckdb` crate (synchronous) + a `tokio::task::spawn_blocking` bridge, integrated as an analytical read-only bypass (bypassing sea-orm, because sea-orm 2.0.0-rc.37 does not support DuckDB)
  - Added the `DbConnection::DuckDb` enum variant
  - Added the `DuckDbConnection` connection wrapper, supporting the `execute()`/`query()` async interfaces
  - `DatabaseType::DuckDb` supports parsing `duckdb:` URL prefixes
- **The `tracing` feature**: OpenTelemetry distributed tracing, based on `tracing` + `tracing-opentelemetry` + `opentelemetry-otlp`
  - Added the `TracingGuard::init_with_otlp()` API
  - Integrated the `tracing` module exports
- **Structured error report**: `QueryErrorReport` categorizes by "permission denied / injection risk / syntax error / shard conflict", with fix suggestions
- **Smart connection-string parsing**: the `url` crate replaces the hard-coded `starts_with("sqlite:")` prefix matching, supporting standard URI formats
- **Permission cache TTL + dynamic hot reload**: `PermissionCache` gains `expire_after` + a background async refresh task
- **Shard routing integrated into Session**: `pool.get_session_for_shard(key, role)` routes to the corresponding shard automatically

#### Engineering Guardrails

- `.github/dependabot.yml`: cargo + github-actions automatic dependency update PRs
- `.github/codeql.yml`: the semantic security scanning workflow
- `.editorconfig`: cross-editor consistency (Rust 4-space indent, UTF-8, LF line endings)
- `.pre-commit-config.yaml`: integrates the cargo-fmt / cargo-clippy / cargo-deny Rust-specific hooks

#### Test Coverage

- `tests/authentication/`: external tests for AuthenticationManager/JwtManager/PasswordHasher
- `tests/security/sensitive_masker_tests.rs`: external tests for SensitiveMasker/MaskType (including Unicode safety tests)
- `tests/kit/kit_integration_tests.rs`: external tests for DbNexusKit
- `benches/permission_engine_bench.rs`: the permission engine benchmark
- `benches/sharding_bench.rs`: the shard routing benchmark
- `benches/metrics_bench.rs`: the metrics collection benchmark

### Changed

- **`cache` feature description corrected**: from "LRU" to "oxcache (internal moka L1 backend)"
- **Protocol-compatible databases documented**: CockroachDB/YugabyteDB/TiDB/MariaDB/Aurora work without code changes, stated explicitly in the README

### Performance

- **DuckDB pooled**: replaced DuckDB's single Mutex with a connection pool (`DuckDbConnection`), supporting concurrent queries via `with_pool_size(url, pool_size)`
- **Session short-lock mode**: avoids running async DB calls while `Session` holds a lock, reducing lock contention
- **MetricsCollector atomized**: removed the unnecessary `RwLock` in favor of atomic operations
- **SqlParser globally shared singleton**: `SqlParser::shared().await` returns an `Arc<SqlParser>`, avoiding repeated parser instance creation

### Fixed

#### Correctness Fixes

- **`mask_email` UTF-8 byte slicing panic**: non-ASCII local parts (Chinese/emoji) panicked under byte slicing; switched to safe `chars()` handling
- **`ShardRouter::default()` division-by-zero panic**: the default `total_shards=0` caused `% total_shards` to divide by zero; changed to `total_shards=1` (single-shard semantics) + a defensive check
- **`shard_id_for_key` hasher misuse**: `shard_key.hash(&hasher)` should be `&mut hasher` (a pre-existing compile error)
- **DuckDB `permit` dropped early**: in `execute()`/`query()` the permit dropped before `handle.await`, releasing the semaphore before the spawn_blocking task finished, losing the concurrency limit
- **`warmup_connections` silently dropped failures**: it returned `Ok(())` even when everything failed; now all failures return `Err` and partial failures log a warn (rule 12)
- **`validate_role_name` silently fail-open**: without a permission configuration it silently used the safe default policy; a warn log was added to make it explicit (rule 12, a conservative fix for a CRITICAL risk)

#### Security Fixes

- **`verify_token` did not check `token_type`**: a refresh token could be used as an access token (privilege escalation risk); the new `verify_access_token`/`verify_refresh_token` methods additionally validate `token_type`
- **`add_user` could not validate password strength**: it received an already-hashed `password_hash`; the new `register_user(username, password, role)` runs the full `validate_strength → hash → insert` flow

#### Security Review Fixes

- **DuckDB SQL injection protection**: `execute_duckdb_raw` adds admin role + DdlGuard AST validation (aligned with `execute_raw_ddl`); DDL is denied for non-admin roles
- **Token-bucket race fix**: RateLimiter internal state updates changed to atomic operations
- **`try_from` panic fix**: `DbPool::try_from` returns an error instead of panicking when the `permission` feature is enabled
- **singleflight violation fix**: the permission cache's singleflight coordination logic corrected
- **`eprintln!` replacement**: all `eprintln!` replaced with `tracing::warn!`/`tracing::error!`
- **`sql-parser` dependency hardening**: the sql-parser dependency declared explicitly to prevent permission bypass
- **Fake DI setter fix**: removed the misleading dependency injection setter
- **`Engine*` double export fix**: eliminated duplicate type exports
- **Empty `impl_` directory cleanup**: removed the empty implementation directory
- **Redundant preload removal**: optimized away useless startup preload logic
- **`deny.toml` adds RUSTSEC-2023-0071**: the security advisory added to the cargo-deny policy

#### Final Regression Fixes

- **DuckDB DDL design bug**: `execute_duckdb_raw` documentation claimed DDL support but the code rejected it unconditionally; fixed to execute after admin role + DdlGuard AST validation (aligned with `execute_raw_ddl`)
- **DuckDB health check query failure**: `SELECT 1 AS health` was rejected because `parse_operation_async` returned `Ok(None)`; fixed to allow execution for the admin role (aligned with `execute`'s None path)
- **Missing `sqlite3://` scheme**: `DatabaseType::from_url` did not support the `sqlite3://` format; support added
- **sharding flaky test fix**: `test_shard_router_key_consistency` assumed two specific keys must map to different shards (a 10% collision probability); changed to verify determinism properties + statistical distribution properties

### Maintenance (pre-0.3.0 release hardening)

#### Dependency Upgrades (9 Cargo dependencies)

- **sqlparser**: 0.47 → 0.62 (breaking API: `Statement` enum variants became tuple structs, updating the pattern matches of `classify_statement` and `statement_type_name`)
- **opentelemetry**: 0.27 → 0.32 (`TracerProvider`/`Span` and other API refactors)
- **opentelemetry_sdk**: 0.27 → 0.32
- **opentelemetry-otlp**: 0.27 → 0.32
- **tracing-opentelemetry**: 0.28 → 0.33
- **criterion**: 0.5 → 0.8 (fixing the `BenchmarkGroup` deprecation)
- **toml**: 1.0 → 1.1
- **parking_lot**: a 0.12 patch upgrade
- **url**: a 2.5 patch upgrade

#### CI Hardening (9 GitHub Actions upgrades + 2 nonexistent-action fixes)

- **actions/checkout**: v4 → v7 (5 places)
- **actions/cache**: v4 → v6 (3 places)
- **codecov/codecov-action**: v4 → v7
- **github/codeql-action/init**: v3 → v4
- **github/codeql-action/analyze**: v3 → v4
- **softprops/action-gh-release**: v2 → v3 (2 places)
- **swatinmurthy/cache-for-rust@v1** → **Swatinem/rust-cache@v2** (4 places; the original action repository did not exist, GitHub API returned 404)
- **actions/attest-release-assets@v1** → **actions/attest-build-provenance@v3** (the original action did not exist, GitHub API returned 404)
- **Swatinem/rust-cache parameter fix**: removed the unsupported `path`/`restore-keys` parameters, using `shared-key` to distinguish cache namespaces

#### CI Validation Rule Enhancements

- **clippy `--all-features` fix**: the lint job lacked the `--all-features` argument, so feature-gated code was never linted
- **cargo doc check added**: the lint job gained `cargo doc --all-features --no-deps --workspace`, catching rustdoc warnings early

#### Documentation Warning Fixes (28 rustdoc warnings)

- **`src/domain/migration/converter.rs`**: fixed 28 "unclosed HTML tag" warnings. Generic types in markdown tables (such as `Option<TableRef>`, `Vec<ColumnDef>`, `Char(Option<u32>)`) were not wrapped in backticks, so rustdoc mistook them for HTML tag starts. All wrapped in backticks now.

## [0.2.0] - 2026-06-27

### ⚠️ BREAKING CHANGES - ALL USERS MUST UPDATE

This is a **major breaking change** that affects **100% of users**. There is **NO backward compatibility** and **NO automatic migration path**.

#### Core Changes

- **Removed from default features**: `cache` is no longer enabled by default
- **Feature dependencies**: `permission`, `permission-engine`, and `sql-parser` now **require** `cache` feature
- **Compilation failure**: Code will not compile without correct feature flags
- **`foundation::pool` deprecated**: All database operations must use `database::pool` for Session management
- **Legacy `db_crud` macro removed**: Replaced by the unified `#[db_entity]` macro with explicit `primary_key` specification

#### Required Action for All Users

**Before (v0.1.x):**
```toml
dbnexus = "0.1"
# or
dbnexus = { version = "0.1", features = ["postgres"] }
```

**After (v0.2.0) - Choose ONE:**

```toml
# Option 1: Use presets (RECOMMENDED)
dbnexus = { version = "0.2", features = ["microservice"] }

# Option 2: Explicit features
dbnexus = { version = "0.2", features = [
    "postgres",
    "permission",
    "cache",
    "observability"
] }

# Option 3: Ultra-minimal (embedded)
dbnexus = { version = "0.2", features = ["embedded"] }
```

### Removed

- **`confers` configuration framework**: Removed the optional `confers` dependency and the `confers` feature flag entirely. The project now relies on `serde` / `serde_yaml_ng` / `serde_json` for direct deserialization.
  - Removed `confers = ["dep:confers"]` feature and the `confers` path dependency from `Cargo.toml`.
  - Removed `confers/yaml` and `confers/json` from the `config-yaml` feature; the `yaml` feature now drives YAML support directly.
  - Removed `confers/yaml` from the `permission-engine` feature.
  - Removed `confers` from the `required-features` of the `core_session_transaction` and `cross_cutting_benchmarks` test targets.
  - Removed `CacheConfig::from_confers`, `DbConfig::from_confers`, and `PermissionConfig::from_confers`.
  - Added `CacheConfig::from_yaml_str` / `CacheConfig::from_json_value`, `DbConfig::from_yaml_str` / `DbConfig::from_json_str`, and `PermissionConfig::from_yaml_str` / `PermissionConfig::from_json_str` as serde-based replacements.
  - Removed `#[cfg(feature = "confers")]` gates from `DbPool::load_permission_config` / `parse_permission_yaml` / `parse_permission_json`, `YamlPermissionProvider::new` / `parse_yaml_content` / `refresh`, `parse_permission_yaml_async`, and `permission_engine::YamlPermissionProvider::load_config`. The corresponding `#[cfg(not(feature = "confers"))]` Err branches were deleted.
  - Deleted `tests/confers_oxcache_integration.rs` (all tests were `#[ignore]`d and referenced an unimplemented `with_confers()` method).
  - Updated `enterprise` preset to no longer include `confers`.

- **`foundation::pool`**: Deprecated; all database operations must use `database::pool` for Session management.

- **Legacy `db_crud` macro**: Removed; replaced by the unified `#[db_entity]` macro with explicit `primary_key` specification.

- **`minimal` preset**: Replaced by `embedded` (different feature set).

- **Implicit cache dependency**: Cache is now optional and must be explicitly enabled.

- **Fallback behaviors**: No fallback or no-op implementations — compilation fails if required features are missing.

### Added

- **`#[db_entity]` unified macro** (`macros`): Single attribute macro replacing 5 separate macros (`db_entity`, `db_crud`, `db_audit`, `db_cache`, soft_delete). Supports `table_name`, `primary_key`, `timestamps`, `soft_delete`, `validate`, `hooks(...)` parameters.
  - **`schema(backend)`**: Generates `migration::schema::Table` from Entity via `sea_orm::Schema::create_table_from_entity` with custom converter.
  - **`query(session)`**: Returns Sea-ORM native `Select<E>` for chaining `filter/order_by/limit`.
  - **`paginate(session, page_size)`**: Returns Sea-ORM paginator with `num_items/num_pages/fetch_page`.
  - **`insert_many(session, models)`**: Batch insert returning `InsertResult` with `last_insert_id`.
  - **`update_many(session, filter, updates)`**: Conditional batch update returning affected row count.
  - **`find_with_deleted(session)`**: Find including soft-deleted records.
  - **`find_only_deleted(session)`**: Find only soft-deleted records.
  - **`force_delete(session, id)`**: Hard delete bypassing soft delete.
  - **`timestamps = true`**: Auto-manages `created_at` / `updated_at` via `ActiveModelBehavior::before_save` — insert sets both, update sets only `updated_at`.
  - **`validate`**: Integrates `validator` crate with `#[derive(Validate)]` and field-level `#[validate(...)]` attributes. Validation runs before timestamps in `before_save`.
  - **`hooks(...)`**: 6 event hooks — `before_insert`/`after_insert`/`before_update`/`after_update`/`before_delete`/`after_delete`. Hook orchestration order: `validate → timestamps → user_hooks` (any failure short-circuits).
  - **`soft_delete = true`**: Rewrites `find*` and `delete` semantics to auto-filter/set `deleted_at` column. Auto-injects `deleted_at: Option<time::OffsetDateTime>` field.

- **`with-time` feature**: Enables `sea-orm/with-time` dependency for `OffsetDateTime` timestamp support. Included in default features.

- **`validation` feature**: Gates `validator` crate with `features = ["derive"]` for `#[derive(Validate)]` macro support.

- **New combined features**:
  - **`cache`**: Independent cache feature (was implicitly enabled, now explicit)
  - **`observability`**: Combined feature for metrics + tracing + health-check
  - **`data-management`**: Combined feature for migration + auto-migrate + sharding + global-index
  - **`security`**: Combined feature for audit + permission-engine
  - **`bench`**: Performance testing dependencies (criterion)
  - **`test-utils`**: Testing utilities (tempfile, assert_cmd)

- **New presets**:
  - **`embedded`**: Ultra-minimal (runtime-tokio-rustls, sqlite, config-env) for embedded/edge devices
  - **`microservice`**: Optimized for microservice deployment (postgres, permission, sql-parser, config-env, observability)
  - **`monolith`**: Complete for monolithic applications (postgres, permission, sql-parser, config-yaml, data-management, security, observability)
  - **`enterprise`**: Full enterprise features (postgres + monolith + permission-engine)

- **Cache stampede protection** (`permission`): `PermissionContext` now uses singleflight request coalescing to prevent thundering herd when multiple requests hit an uncached role simultaneously. Concurrent cache-miss requests are collapsed into a single load; followers wait for the leader's result. A `stampede_events` counter tracks how many times coalescing occurred. New `get_cache_metrics()` method returns `(hit_rate, miss_count, stampede_count, cache_size)`.

- **Connection pool alerting** (`pool`): `acquire_connection` now tracks `wait_count` (current waiters) and `max_waiters` (historical peak) with CAS-safe updates. Timeout paths trigger tiered log alerts: warn ≥3s, error ≥5s, critical ≥10s.

- **Acquire duration histogram** (`metrics` feature): `MetricsCollector` now records `connection_acquire_duration` histograms with 100ms/500ms/1s/3s/5s/10s buckets. Slow acquires (>1s) increment `slow_acquires` counter. Timeout events are classified by level and counted separately.

- **Prometheus metrics export** (`metrics` feature): `MetricsCollector` now exports Prometheus-format metrics via `export_prometheus()`:
  - `dbnexus_pool_connections_total / active / idle` (gauges)
  - `dbnexus_connection_acquire_slow_total` (counter)
  - `dbnexus_connection_timeout_total{level="warn|error|critical"}` (counter)
  - `dbnexus_pool_acquire_duration_seconds` (histogram)

- **Rate limiter burst capacity**: `RateLimiter::new()` now accepts a `burst_capacity: u32` parameter (default = `max_requests`) to allow initial token count to exceed the steady-state refill rate. New `update_config(max_requests, window_duration)` method for runtime reconfiguration. New `with_defaults(max_requests, window_duration, max_buckets)` convenience constructor.

### Changed

- **Performance**: All metrics recording in `acquire_connection` is gated behind `#[cfg(feature = "metrics")]` — zero overhead when the feature is disabled.

- **Macro validation logic**: Fixed bug where `validate` + `timestamps = true` combination destroyed `ActiveValue::Set` state during `before_save`. Validation now clones `ActiveModel` for read-only validation, preserving original `Set/Unchanged/NotSet` states so UPDATE operations correctly persist changed fields.

- **Sea-ORM 2.0 `ActiveModelBehavior` signatures**: `after_save` receives `Model` (not `ActiveModel`); `after_delete` takes `self` by value and returns `Result<Self, DbErr>`.

- **Hook function signatures**: `before_*` must be `fn(&mut ActiveModel) -> Result<(), E>`, `after_*` must be `fn(&Model) -> Result<(), E>`.

- **Feature reorganization**:
  - **`permission`**: Now requires `cache` feature
  - **`sql-parser`**: Now requires `cache` feature
  - **`permission-engine`**: Now requires `cache` feature
  - **`minimal` preset**: Removed and replaced with `embedded` (truly minimal, no caching)

- **Dependency updates**:
  - **`oxcache`**: Changed to optional dependency (was required)
  - **`regex`**: Removed duplicate declarations across features
  - **`once_cell`**: Removed duplicate declarations across features

### Migration Guide

#### Step 1: Update Version

```toml
# In your Cargo.toml
[dependencies]
dbnexus = "0.2"  # Update from 0.1.x
```

#### Step 2: Choose a Configuration

**For most users:**
```toml
dbnexus = { version = "0.2", features = ["microservice"] }
```

**For embedded/edge devices:**
```toml
dbnexus = { version = "0.2", features = ["embedded"] }
```

**For full enterprise features:**
```toml
dbnexus = { version = "0.2", features = ["enterprise"] }
```

**For custom configuration:**
```toml
dbnexus = { version = "0.2", features = [
    "postgres",     # or mysql/sqlite
    "permission",    # requires cache
    "cache",         # REQUIRED by permission
    "observability"
] }
```

#### Step 3: Build and Test

```bash
cargo clean
cargo build
# If you get compilation errors about missing features,
# add the required features to your Cargo.toml
```

### Important Notes

- **No automatic migration**: You must manually update Cargo.toml
- **No compatibility layers**: v0.1.x and v0.2.0 are completely incompatible
- **Feature combinations**: Ensure `cache` is enabled if using `permission`, `permission-engine`, or `sql-parser`
- **Compilation errors**: If compilation fails, the error message will indicate which feature is required

### Performance Impact

**Without cache feature:**
- Binary size: Reduced by 5-10%
- Compile time: Reduced by 15-20%
- Runtime performance: May be significantly slower (100x for permission checks, 10x for SQL parsing)

**Recommendation**: Enable `cache` feature for production use unless targeting embedded devices.

## [0.1.2] - Previous Release

### Features
- Connection pooling with RAII lifecycle management
- Permission control (RBAC)
- Procedural macros for CRUD and permission checks
- SQL parser
- Transaction support
- Multi-database support (SQLite, PostgreSQL, MySQL)
- Enterprise features (metrics, tracing, audit, migration, sharding, etc.)

### Known Issues
- Cache feature was implicitly enabled, could not be disabled
- Documentation inconsistencies with Cargo.toml
- No practical presets for common use cases

[0.6.0]: https://github.com/Kirky-X/dbnexus/compare/v0.6.0-rc.11...v0.6.0
[0.6.0-rc.11]: https://github.com/Kirky-X/dbnexus/compare/v0.6.0-rc.10...v0.6.0-rc.11
[0.6.0-rc.10]: https://github.com/Kirky-X/dbnexus/compare/v0.6.0-rc.9...v0.6.0-rc.10
[0.6.0-rc.9]: https://github.com/Kirky-X/dbnexus/compare/v0.6.0-rc.8...v0.6.0-rc.9
[0.6.0-rc.8]: https://github.com/Kirky-X/dbnexus/compare/v0.6.0-rc.7...v0.6.0-rc.8
[0.6.0-rc.7]: https://github.com/Kirky-X/dbnexus/compare/v0.6.0-rc.6...v0.6.0-rc.7
[0.6.0-rc.6]: https://github.com/Kirky-X/dbnexus/compare/v0.6.0-rc.5...v0.6.0-rc.6
[0.6.0-rc.5]: https://github.com/Kirky-X/dbnexus/compare/v0.6.0-rc.4...v0.6.0-rc.5
[0.6.0-rc.4]: https://github.com/Kirky-X/dbnexus/compare/v0.6.0-rc.2...v0.6.0-rc.4
[0.6.0-rc.2]: https://github.com/Kirky-X/dbnexus/compare/v0.5.1...v0.6.0-rc.2
[0.5.1]: https://github.com/Kirky-X/dbnexus/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/Kirky-X/dbnexus/compare/v0.4.4...v0.5.0
[0.4.4]: https://github.com/Kirky-X/dbnexus/compare/v0.4.3...v0.4.4
[0.4.3]: https://github.com/Kirky-X/dbnexus/compare/v0.4.2...v0.4.3
[0.4.2]: https://github.com/Kirky-X/dbnexus/compare/v0.4.0...v0.4.2
[0.4.0]: https://github.com/Kirky-X/dbnexus/compare/v0.3.4...v0.4.0
[0.3.4]: https://github.com/Kirky-X/dbnexus/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/Kirky-X/dbnexus/compare/v0.3.0...v0.3.3
[0.3.0]: https://github.com/Kirky-X/dbnexus/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/Kirky-X/dbnexus/compare/v0.1.2...v0.2.0
