# 🧪 Dbnexus Test Scenario Baseline

> Records the test pyramid baseline, the functional-domain mapping of the 87 test targets, E2E scenario definitions, real-behavior corrections and findings, the driver-group combination matrix, and the static gates. All verification is measured with `cargo test`.

## 📋 Table of Contents

<details open>
<summary>📑 Table of Contents</summary>

- [Test Pyramid Baseline](#test-pyramid-baseline)
- [Test Target Functional-Domain Mapping](#test-target-functional-domain-mapping)
- [E2E Scenario Definitions](#e2e-scenario-definitions)
- [Real-Behavior Corrections and Findings](#real-behavior-corrections-and-findings)
- [Driver-Group Combination Matrix](#driver-group-combination-matrix)
- [Static Gates](#static-gates)

</details>

---

## Test Pyramid Baseline

| Layer | Carrier | Count baseline |
|------|------|----------|
| L1 lib unit tests | `#[cfg(test)]` inside `src/**` | self-tests of error/common_types/config/domain and other modules, run with `--lib` |
| L2 integration tests | deep paths under `tests/**`, 87 explicitly registered `[[test]]` | unit/integration layered directories (unit/integration/) |
| L3 E2E scenarios | `tests/e2e/` (e2e_advanced 86 tests + e2e_distributed 10 tests) | 96 tests, isolated via cfg(feature) |
| L4 container-level | `postgres_testcontainers` + `mysql_testcontainers` | 30 tests (postgres 16 + mysql 14), per-test isolated containers |

Driver-group full results (CI command `--no-default-features --features <db>,default-no-db,all-optional --workspace --exclude dbnexus-examples --exclude dbnexus-macros --no-fail-fast`):

| Driver group | Result | Notes |
|--------|------|------|
| sqlite | 1712 passed / 0 failed | includes 2 DdlGuard outdated-assertion corrections |
| postgres | 1276 passed / 0 failed | re-verified after gating edge_cases to sqlite and switching tc to self-started isolation (env -u DATABASE_URL -u TEST_DATABASE_URL) |
| mysql | 1276 passed / 0 failed | separately verified 14/14 (self-started) after the tc fix |
| duckdb | 1300 passed / 0 failed | absent from the CI matrix, verified locally |

## Test Target Functional-Domain Mapping

The 87 test targets map to functional domains as follows:

- **core**: entity_integration / sql_parser_integration / session_transaction / permission_integration / dbnexus_integration / config_integration / health_integration / config_unit / common_types_unit / error_unit / edge_cases
- **entity**: sea_orm_type / entity_unit / db_entity_pk_types (primary-key genericization regression)
- **migration**: integration ×8 (including auto_migrate / schema_macro / query_builder / timestamps / soft_delete / validation / hooks) + unit
- **pool**: session_unit / db_pool_unit / pool_unit / pool_integration / duckdb_pool / pool_semaphore
- **permission**: rbac_unit / advanced_rbac_unit / pdp_unit / rate_limiter_unit / permission_cache_ttl / permission_test(domain) / permission_engine_integration / permission_integration
- **security**: sql_injection_tests / sensitive_masker_tests
- **audit**: audit_integration / audit_unit
- **sharding**: sharding_integration / shard_session_routing / sharding_unit
- **observability**: metrics_integration / metrics_unit / health_unit
- **Distributed capabilities**: retry_unit / failover_unit / replica_unit / scatter_unit / saga_unit / distributed_id_unit
- **global_index**: global_index_unit / global_index_integration
- **Container-level**: postgres_testcontainers / mysql_testcontainers
- **Cross-cutting**: cross_cutting (benchmarks / cli / concurrency / multi_db) / repository_macro / authentication_integration / cache_integration / macros-related (the dbnexus-macros crate has its own separate tests)
- **E2E**: tests/e2e/e2e_advanced + tests/e2e/e2e_distributed (directory-carried, not placed bare at the top level)

## E2E Scenario Definitions

### tests/e2e/e2e_advanced.rs (86 tests, domains split by cfg(feature))

| Domain | feature gate | Scenario highlights |
|----|------------|----------|
| SensitiveMasker boundaries | none | empty string / shorter than reserved digits / custom-length out of range / Unicode / single character / over-long input → InvalidInput or safe output |
| CircuitBreaker state machine | health-check | failure threshold opens / half-open recovery threshold / open-state failure noop / open-state success noop / closed-state success reset / Display / Error construction |
| ShardRouter | sharding | default routing / unregistered shard / strategy name listing / month-year strategy shard_id validation / empty key falls back to strategy / deterministic |
| GlobalIndex batch_sync | global-index + sqlite | exactly one chunk / just over one chunk / two full chunks / large shard_id / Unicode / mixed tables / duplicate upsert |
| Authentication JWT flow | authentication | register → authenticate → refresh token flow / token-type mismatch / custom expiry / special characters / password strength / concurrent authentication / empty hash rejected / plaintext hash rejected |
| i18n formatting | none | invalid locale / multiple formatters / plural categories / timestamp boundaries (leap-year 2.29 / 2.30 / 12.31) / number formatting (extremes / negatives / non-finite) |
| Pool metrics | runtime | connection created / failed / closed recorded / active increments and decrements / should_create decision / health (idle / busy / capacity) |
| Migration messages | none | format_row_count zero value / migration message zero value |

### tests/e2e/e2e_distributed.rs (10 tests, member-level cfg)

Covers retry×DbConfig / failover multi-URL / replica lag threshold / scatter aggregate types and PartialFailurePolicy / saga lifecycle and error types / distributed_id parse round-trip + Snowflake E2E. Under the CI command, all-optional transitively enables all members → fully activated.

## Real-Behavior Corrections and Findings

The following findings were recorded during driver-group matrix verification; all were corrected to match real behavior or supplemented with gating:

1. **DdlGuard allowlist outdated assertions ×2**: `test_ddl_guard_vs_sql_injection` in the session unit tests and sql_injection_tests once asserted "DROP TABLE should be rejected". Since 0540954, `DropTable` has been in the `ALLOWED_DDL_STATEMENTS` allowlist (a deliberate decision for migration scenarios); the actual interception layer is the string patterns of the unified injection engine ("DROP DATABASE" / "DROP ALL"). The tests were corrected to the real behavior: DropTable allowed + DROP DATABASE rejected.
2. **sqlite hard-coded targets without gating**: 26 test files depend on `sqlite::memory:` via `common::make_sqlite_memory_pool` (embedded-only). 6 targets (core_session_transaction / migration_integration / migration_auto_migrate / cross_cutting_concurrency / cross_cutting_multi_db / edge_cases) have been given `required-features = "sqlite"`; otherwise they would always panic in driver groups without sqlite.
3. **common's multi-database redirection intent disconnected from CI reality**: common reads `DATABASE_URL` with the intent of "one test suite across a database matrix", but CI never went fully green (sqlite dialect / table-name conflicts exposed under the postgres/mysql jobs); CI actually sets `DATABASE_URL`, while the local driver-group runs mistakenly used `TEST_DATABASE_URL` (which common does not read). Making multi-database truly viable requires table-name uniquification + dialect adaptation; recorded as a follow-up improvement item.
4. **testcontainers reuse branch violated the isolation design**: the setup of postgres/mysql_testcontainers used to prefer reusing `TEST_DATABASE_URL` / `DATABASE_URL`; the shared database had no pre-cleanup, so cross-run residual tables inevitably hit 42P07 already exists (postgres verified 10 failed; mysql going green that run was luck — the database was being used for the first time with no residue). The reuse branch has been removed, restoring the primary design of per-test isolated containers; both files verified 14/14 ×2 in self-started mode.
5. **e2e targets' required-features mismatch**: e2e_advanced once carried `authentication`, and e2e_distributed once carried the aggregate name `distributed-capabilities`; neither is inside the feature closure of the CI command (`<db>,default-no-db,all-optional`), so the targets were never compiled or run. Removed per the "files isolated by cfg(feature)" design; the activation verification of the authentication module moved into the combination matrix (86 passed).
6. **e2e directory carrying**: tests/e2e_advanced.rs and tests/e2e_distributed.rs were moved into tests/e2e/ (git rename) with the `[[test]]` paths updated; the two files have no mod common/include_str relative references, so the move introduced no lookup-root problem.
7. **duckdb_params_security_test.rs self-gating confirmed**: top-level auto-discovery plus the file-level gate `#![cfg(all(feature = "duckdb", feature = "sql-parser"))]`; the duckdb group really runs it (parameterized injection safety / sqlparser DuckDB dialect gate / transaction atomicity / file-pool sequential sessions) — not a gap.
8. **edge_cases multi-database follow-up**: the concurrency tests inside the target share table names; multi-database runs require table-name uniquification first (otherwise shared databases hit CREATE TABLE same-name conflicts); currently gated to the sqlite group.

## Driver-Group Combination Matrix

| Combination | Coverage | Result |
|------|------|------|
| `<db>,default-no-db,all-optional` (`<db>` ∈ sqlite / postgres / mysql / duckdb) | full run of the four driver groups | see the driver-group full results in [Test Pyramid Baseline](#test-pyramid-baseline) |
| sqlite+all-optional+authentication | all modules of e2e_advanced activated | 86/0 |
| Driver mutual exclusion | mixing embedded and server-side fails to compile (no escape hatch) | combination verification does not apply to --all-features |

## Static Gates

| Gate | Command | Result |
|------|----------|------|
| fmt | `cargo fmt --all -- --check` | clean |
| clippy | `cargo clippy --no-default-features --features <db>,default-no-db,all-optional --workspace --exclude dbnexus-examples --exclude dbnexus-macros -- -D warnings` (×4 driver groups) + `-p dbnexus-macros -- -D warnings` | all zero warnings |
| doc | `RUSTFLAGS="-D warnings" cargo doc --no-deps --features sqlite,default-no-db,all-optional` | zero warnings |
| deny | `cargo deny check` | 4 items ok (licenses bound to the LICENSE hash via clarify, see the deny.toml comments) |
| audit | `cargo audit --stale` | rc=0 (RUSTSEC-2025-0134 in allowed, a neo4rs transitive dependency) |
| examples | `examples/test_all_examples.sh` (the project's own script, 52 [[bin]] targets, graph_ladybug excluded) | 52/52 passed, 0 warnings 0 failures |
| macros | `cargo test -p dbnexus-macros` (excluded in CI, verified locally) | db_entity_tests 2 aggregates all passed (pass×6 + compile_fail×3) |

---

## Test Scale (as of 0.6.1)

| Metric | Value | Source |
|------|------|------|
| Total test functions | 2873 `#[test]` / `#[tokio::test]` | grep count (`grep -rEc '#\[(tokio::)?test\b' --include='*.rs' src tests macros`: src 1437 + tests 1434 + macros 2) |
| Registered test targets | 87 `[[test]]` | `Cargo.toml` |
| Driver-group full runs | sqlite 1712 / postgres 1276 / mysql 1276 / duckdb 1300 passed | see [Test Pyramid Baseline](#test-pyramid-baseline) above (0.6.0 run baseline) |
| Coverage gate | ≥ 95% line coverage | `.github/workflows/ci.yml` (llvm-cov) |
