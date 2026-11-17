# 🤝 Dbnexus Contributing Guide

Thank you for your interest in the Dbnexus project! This document describes the environment, process, and conventions required for contributing to development.

## 📋 Table of Contents

<details open>
<summary>📑 Table of Contents</summary>

- [🧰 Development Environment](#-development-environment)
- [🔄 TDD Workflow](#-tdd-workflow)
- [🪝 Pre-commit Hooks](#-pre-commit-hooks)
- [🧪 Code Quality](#-code-quality)
- [🔀 Pull Request Process](#-pull-request-process)
- [✍️ Code Style](#️-code-style)
- [📏 Testing Conventions](#-testing-conventions)
- [📄 License](#-license)

</details>

---

## 🧰 Development Environment

| Item | Requirement |
|----|------|
| Rust | **1.97.1+** (pinned by `rust-toolchain.toml`, edition 2024) |
| Git | configured with pre-commit hooks |
| Docker | optional, for PostgreSQL / MySQL integration tests |

### Initialization

```bash
git clone https://github.com/Kirky-X/dbnexus.git
cd dbnexus

# Install Git hooks (choose one)
lefthook install
# or: pip install pre-commit && pre-commit install

# Verify the environment (embedded/server-side drivers are mutually exclusive; --all-features is forbidden: the compile_error! guard in src/lib.rs)
cargo build --no-default-features --features sqlite,default-no-db,all-optional
cargo test --no-default-features --features sqlite,default-no-db,all-optional --lib
```

---

## 🔄 TDD Workflow

This project uses test-driven development (TDD); every development task group must follow this loop:

1. **Define the interface**: first define the trait / API signatures (`trait Xxx { ... }`), without writing the implementation
2. **Write tests**: write unit tests based on the interface (`#[cfg(test)] mod tests { ... }`); the tests should fail at this point (red)
3. **Write code**: implement the interface so the tests pass (green)
4. **Run tests**: `cargo test --features <corresponding-features> --lib`, make sure all tests pass
5. **commit**: after passing, explicitly stage the changed files and commit `git commit -m "feat(<module>): <description>"`
6. **Impact analysis**: analyze the impact of this change on other modules and identify code that needs coordinated modification
7. **Continue to the next one**: adjust subsequent tasks based on the analysis, then start the next loop iteration

---

## 🪝 Pre-commit Hooks

The project uses pre-commit hooks (lefthook / pre-commit, functionally equivalent) to guarantee code quality:

```bash
# Install (choose one)
lefthook install
# or: pip install pre-commit && pre-commit install

# Run manually
pre-commit run --all-files
```

### Prohibitions

- **Do not skip pre-commit hooks with `--no-verify`**
- **Do not use `git commit -n`** to bypass checks
- Violating this rule is treated as a quality red-line incident

---

## 🧪 Code Quality

Before contributing, make sure the following checks pass (identical to the CI gates):

| Gate | Command |
|------|------|
| Format | `cargo fmt --all -- --check` |
| Lint | `cargo clippy --all-targets -- -D warnings` |
| Supply chain | `cargo deny check`, `cargo audit` |
| Coverage | line coverage ≥ 95% (measured by llvm-cov in CI) |
| Documentation | `cargo doc` zero warnings |

### Automated Review Tools

The project integrates automated review tools into the development process:

| Tool | Purpose | Typical scenarios |
|------|------|----------|
| Code quality review | simplification, architecture optimization, performance review | review / audit / tech debt / over-engineering / simplify; any write / add / refactor / fix task |
| SAST security scanning | vulnerability and injection detection | security review / vulnerability scanning; hardcoded secrets / SQL injection / unsafe eval; pre-release security checks |
| kueiku | methodology framework: root-cause analysis and structured thinking | analysis / strategy / decision / root-cause analysis |

---

## 🔀 Pull Request Process

1. **Create a feature branch**

   ```bash
   git checkout -b feat/<your-feature>
   # or
   git checkout -b fix/<bug-description>
   ```

2. **Make sure pre-commit hooks pass**

   ```bash
   pre-commit run --all-files
   ```

3. **Make sure tests pass**

   ```bash
   cargo test --no-default-features --features sqlite,default-no-db,all-optional
   cargo clippy --no-default-features --features sqlite,default-no-db,all-optional -- -D warnings
   cargo fmt --all -- --check
   ```

   > Embedded and server-side drivers are mutually exclusive; `--all-features` is forbidden (the `compile_error!` guard in `src/lib.rs` forces the build to fail).

4. **Submit the PR**
   - Title format: Conventional Commits (`feat(<module>): <description>` or `fix(<module>): <description>`, validated by the commit-msg hook)
   - Describe what changed and why
   - Link the relevant issues

---

## ✍️ Code Style

### General Conventions

- **Follow the existing codebase's naming and architecture conventions** (even if you believe your approach is better)
- **Dependencies must be feature-gated**; do not introduce ungated dependencies
- **edition 2024**, Rust 1.97.1
- **MIT License**

### Rust Conventions

- Naming: `snake_case` (functions/variables/modules), `CamelCase` (types/traits)
- Error handling: return errors with `Result<T, E>`; **swallowing errors is strictly forbidden**
- **No `unsafe` code** (`#![forbid(unsafe_code)]`)
- **Documentation comments are mandatory** (`#![deny(missing_docs)]`)

### Feature Gating Conventions

```toml
# Correct gating example in Cargo.toml
[features]
my-feature = ["dep:some-crate"]

[dependencies]
some-crate = { version = "1.0", optional = true }
```

```rust
// Correct gating example in src/lib.rs
#[cfg(feature = "my-feature")]
pub mod my_module;
```

---

## 📏 Testing Conventions

- Tests must verify **meaningful properties of correct behavior** (values, structures, side effects, error types)
- Do not merely verify "the function returns something" or "does not error"
- "All tests pass" is a necessary but not sufficient condition
- When tests are too weak, say so explicitly

For scenario definitions, the driver-group matrix, and the static gates, see [🧪 Test Scenario Baseline](TEST_SCENARIOS.md).

### Running Tests

```bash
# Unit tests
cargo test --features sqlite --lib

# Integration tests
cargo test --features sqlite

# All tests (CI standard feature combination)
cargo test --no-default-features --features sqlite,default-no-db,all-optional

# Specific database backends
cargo test --features postgres
cargo test --features mysql
```

> Embedded and server-side drivers are strictly mutually exclusive at compile time; for multi-driver verification, run grouped feature combinations (see [Test Scenario Baseline](TEST_SCENARIOS.md#driver-group-combination-matrix)).

---

## 📄 License

Submitted code will be licensed under the MIT license, consistent with the project.

---

## 🗺️ Development Roadmap

### Short Term (0.6.0 stable release)

- [x] Release 0.6.0 stable — released (tag v0.6.0, 2026-11-14; trait-kit 0.5 / oxcache 0.5 dependency requirements updated per the stable-version propagation table); 0.6.1 followed on 2026-11-15
- [x] Align the declared MSRV with actual dependency requirements — unified to 1.97.1 per workspace CONFIG_BASELINE (2026-09-06), covering the 1.94 transitive requirement
- [x] Restore regular MySQL integration test runs — the CI `test` job now runs a regular `db: [sqlite, postgres, mysql]` matrix: a MySQL 8.0 service container (readiness via mysqladmin health check, then `DATABASE_URL` injection), full test + doc test runs with `--features mysql,default-no-db,all-optional` (2026-09-30)

### Mid Term

- [x] Resolve the mbedtls duplicate-symbol link conflict when `duckdb` and `ladybug` are enabled together — root cause is each driver bundling its own vendored mbedtls static library (unresolvable at the dependency level); a compile-time mutex guard now fails the combination with grouping guidance, `ladybug` + `sqlite` stays allowed (2026-09-30)
- [x] Follow up on Medium items filed during code quality reviews — query-cache policy-change stale reads fixed: key derivation includes the data-protection epoch, `set_data_protection` invalidates old entries on swap (2026-09-30)

> Items compiled from the [CHANGELOG.md](CHANGELOG.md) and repository acceptance records.
