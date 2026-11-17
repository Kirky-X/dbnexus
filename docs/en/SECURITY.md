# 🔒 Dbnexus Security Document

This document describes Dbnexus's security support policy, vulnerability reporting process, security design overview, and security best practices for using the project. For the full design details of the security architecture (including the defense-in-depth diagram and the permission model), see the [Architecture document](ARCHITECTURE.md).

## 📋 Table of Contents

<details open>
<summary>📑 Table of Contents</summary>

- [📌 Supported Versions & Maintenance Policy](#-supported-versions--maintenance-policy)
- [📮 Vulnerability Reporting Process](#-vulnerability-reporting-process)
- [🏰 Security Design Overview](#-security-design-overview)
- [✅ Security Best Practices](#-security-best-practices)

</details>

---

## 📌 Supported Versions & Maintenance Policy

This project is in the 0.x pre-release stage and **only the current minor version line is maintained** (`0.6.x`, including rc pre-releases within that line).

- **Current line**: receives bug fixes and security fixes; fixes ship with the latest release of that line.
- **Older minor lines**: reach EOL as soon as the first version of the next minor line is released; no fixes of any kind are provided, and security patches are not backported.
- **No LTS**: before 1.0, no cross-minor API compatibility is promised; minor-version upgrades may contain breaking changes. Read the [CHANGELOG](CHANGELOG.md) before upgrading.
- **crates.io visibility ≠ supported**: old versions remain downloadable (unless yanked for a severe defect) but are not within the supported scope.

| Version line | Status | Notes |
|--------|------|------|
| `0.6.x` (current) | ✅ Supported | The only line receiving fixes |
| `≤ 0.5.x` | ❌ EOL | Superseded by the current line, no longer maintained, please upgrade |

- The minimum supported Rust version (MSRV) is **1.97.1** (see `rust-version` in `Cargo.toml`).

---

## 📮 Vulnerability Reporting Process

**Please do not report security vulnerabilities through public Issues / Discussions.**

1. **Preferred channel**: submit a report privately via GitHub's **"Report a vulnerability"** (GitHub Security Advisories, the private vulnerability reporting entry on the repository's Security tab).
2. **Fallback channel**: if the preferred channel is unavailable, send an email to **Kirky-X@outlook.com**.
3. Please include as much of the following in the report as possible:
   - Affected versions and the enabled feature combination (e.g. `sqlite` + `permission`);
   - Vulnerability type and impact (e.g. permission bypass, injection, information disclosure);
   - Reproduction steps or a minimal reproducing snippet;
   - Possible mitigations (if any).
4. Kirky.X will acknowledge the report as soon as possible and follow up with you on the assessment and the fix plan; please do not disclose details publicly before the fix is released (responsible coordinated disclosure).
5. After the fix is released, if you wish, we will credit the reporter in the changelog.

For non-sensitive general questions (documentation errors, ordinary bugs), feel free to use [Issues](https://github.com/Kirky-X/dbnexus/issues) as usual.

### ⏱️ Response Commitment

- Acknowledgment **within 48 hours**
- Initial assessment (accepted / rejected / more information needed) **within 7 days**
- Before the fix ships, the reporter gets a chance to preview and verify the patch
- After the fix ships, the reporter is credited (anonymously if preferred) in the CHANGELOG and the advisory

---

## 🏰 Security Design Overview

Dbnexus adopts a defense-in-depth design with five layers from the bottom up. All mechanisms below really exist in the code:

### Layer 1: Compile-time Guarantees

- **Unsafe code forbidden**: crate-wide `#![forbid(unsafe_code)]` (`src/lib.rs`).
- **Compile-time driver mutual exclusion**: mixing embedded (`sqlite`/`duckdb`) and server-side (`postgres`/`mysql`) drivers fails the build outright via `compile_error!` — no escape hatch, preventing misconfiguration that leads to multi-backend inconsistency.
- **Compile-time validation of feature dependencies**: `permission`, `sql-parser`, and `permission-engine` fail the build when required dependencies such as `cache` are not enabled (fail-loud, no silent degradation).

### Layer 2: Runtime Permission Checks

- **Role-based table-level access control (RBAC)**: a role → table → action (SELECT/INSERT/UPDATE/DELETE, etc.) policy model, supporting wildcard tables (`*`) and action-level control.
- **Cross-table permission checks**: target tables on JOIN / subquery paths are permission-checked as well (completed in 0.6.0-rc.2).
- **Permission cache**: a TTL-based permission cache plus singleflight request coalescing prevents cache stampede; background hot-reload is supported.
- **Rate limiting**: permission checks embed token-bucket rate limiting (`RateLimiter`) to prevent abuse and brute-force attempts.
- **Permission health checks**: `health_check` really validates the memory policy table capacity and the readability of the YAML policy file; an empty table or an unreadable file is reported as unhealthy.

### Layer 3: SQL Injection Defense

- **Parameterized queries**: everything uses parameterized queries by default.
- **SQL parser validation**: table-name extraction is based on `SqlParser` (replacing naive string matching since 0.4.2, eliminating injection risks at the parsing layer); `contains_sql_injection` provides injection-pattern detection (with Unicode normalization protection).
- **Unified injection detection engine**: the relational / DDL / graph rule tables were merged into a single registry (`InjectionEngine`, 0.6.0-rc.4).
- **DDL defense**: raw DDL execution (including DuckDB's safe channel) requires the admin role and passes `DdlGuard` AST validation; dangerous patterns (e.g. `DROP DATABASE`) are hard-blocked by the injection detection engine.
- **Graph query defense**: raw Cypher execution on graph databases is deprecated; the unified `execute_cypher_with_params` (with injection protection; the default implementation refuses silent fallback) is used instead.

### Layer 4: Authentication & Configuration Security

- **JWT authentication** (`authentication` feature): access/refresh tokens are validated distinctly (`verify_access_token` / `verify_refresh_token` additionally check `token_type`, preventing a refresh token from being reused as an access token); JWT keys have a minimum-length requirement; since 0.6.0-rc.4 the revocation cache is TTL-based (equal to the token's remaining lifetime).
- **Password security**: bcrypt hash storage; `PasswordPolicy` supports a password blacklist + complexity requirements (maximum length is counted in **bytes**: bcrypt only consumes the first 72 bytes, over-limit passwords are rejected outright, not silently truncated, to avoid password collisions); a configured `max_len` beyond 72 bytes has no effect and the effective cap is `min(max_len, 72)` (`PasswordPolicy::effective_max_len`, with an explicit warning when the hasher is created); legacy over-long passwords accepted under the old character-count policy can be migrated via `PasswordHasher::verify_legacy` using the first-72-bytes semantics (the password should be reset immediately after a successful verification); `register_user` provides the full registration flow of strength validation → hashing → persistence; `add_user` validates the incoming hash format and rejects malformed hashes.
- **Paths and credentials**: connection URLs / policy file paths are traversal-validated (paths containing `..` are rejected); when URL parsing fails, the original URL is never echoed back, avoiding credential leakage.

### Layer 5: Audit & Masking

- **Audit logging** (`audit` feature): full operation logging with user-context tracking; admin permission-bypass operations are audited as well (since 0.4.2). Permission changes additionally have an HMAC-SHA256 chain-signed audit chain with tamper detection (0.6.0-rc.4).
- **Sensitive data masking**: the built-in `SensitiveMasker` supports email, phone, ID number and other masking types (with Unicode-safe handling); the `data-protection` feature provides query-exit field-level masking and row-level security predicate injection (the admin bypass channel is preserved). Row-level predicate injection covers **single-table** statements: both the read exit (`query_rows`) and the raw write exits (UPDATE/DELETE of `execute_raw` / `execute_with_params` / `execute`) are subject to row-level isolation — table-level permissions do not replace row-level predicates; multi-table statements and set operations (UNION/INTERSECT/EXCEPT) cannot be fully covered by a single-table predicate and are rejected with a permission error when RLS is in effect (fail-closed), and unparsable statements are rejected as well; predicate column names are validated against an identifier allowlist, and predicate values are backend-escaped and written as string literals (in PostgreSQL the literal type is determined by the column type and works for text/uuid/int and other column types); the tenant column of an INSERT is assigned by the caller and is not covered by row-level predicates; both the PostgreSQL and SQLite backends consume the injected statement.

### Defense-in-depth Supplements

- **Graph transaction safety**: `execute_cypher` is serialized by `graph_op_mutex` to avoid concurrency bypassing transaction isolation; when a transaction handle panics, the RAII `PoisonGuard` marks it poisoned; uncommitted graph transactions are automatically rolled back when the `Session` is dropped.
- **Dependency and supply-chain security**: CI integrates `cargo deny` (licenses/advisories/bans) and `cargo audit` checks; registered and exempted advisories are recorded in `deny.toml` / `audit.toml`; CodeQL semantic scanning and Dependabot automatic dependency updates are enabled; security upgrades of transitive dependencies such as `h2` ship with releases.

---

## ✅ Security Best Practices

When using Dbnexus, follow these practices:

1. **Enable the security feature combination explicitly**: enable `permission` (which hard-depends on `sql-parser`) and `audit` in production; do not bypass permission checks by disabling SQL parsing.
2. **Least privilege**: grant business roles permissions at table/action granularity, avoid the wildcard `*` in production policies; do not let everyday business code use the admin role (admin bypass operations are audited).
3. **Use parameterized queries**: always use the entity-macro-generated CRUD and parameterized interfaces; avoid concatenating SQL strings, and only when necessary use the restricted channel guarded by `DdlGuard`.
4. **Use the parameterized interface for graph queries**: use `execute_cypher_with_params`; do not use the deprecated raw Cypher execution.
5. **Use the authentication system correctly**:
   - Use a sufficiently long, high-entropy random JWT key (satisfy the minimum length requirement, avoid hard-coding);
   - Use `register_user` for user registration (full strength validation + hashing); do not bypass the password policy by writing hashes directly;
   - Validate access tokens and refresh tokens separately.
6. **Protect policies and configuration**: put the permission policy file (YAML) and connection configuration under least-privilege deployment management; do not commit database credentials from environment variables into version control.
7. **Keep dependencies updated**: run `cargo audit` / `cargo deny` regularly; watch for new releases on crates.io and upgrade promptly.
8. **Enable auditing and monitoring**: enable the `audit` feature in production and plug in log collection; use the `metrics` feature to monitor connection-pool and permission-check metrics and spot abnormal access patterns early.

---

Related documents: [📖 User Guide](USER_GUIDE.md) ｜ [📘 API Reference](API_REFERENCE.md) ｜ [🏗️ Architecture](ARCHITECTURE.md)
