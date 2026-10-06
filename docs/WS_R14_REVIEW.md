# ws-R14 复核记录（dbnexus-HK1）

> 对应变更：`base-roadmap-full-completion`（T034）。复核对象：FEATURE_AUDIT_REPORT.md §2.3/§9.2-1
> dbnexus 命名迁移项与 sql-parser→cache/oxcache 解耦项；附 cargo-hack 主要 feature 组合抽查。
> 复核日期：2026-09-30。基线：0.6.0-rc.6（2861e3c 之后的工作树）。

## 一、§9.2-1 解耦项

| 项 | 复核结论 | 证据 |
|---|---|---|
| sql-parser→cache/oxcache 解耦 | **成立，已实施**（独立任务交付） | 耦合面复核确认唯一触点为 `SqlParser` 解析结果缓存（原 `src/access/sql_parser.rs` 直接 `use oxcache::Cache`）；已改由库内同步 LRU（`prepare_cache` 的 `PreparedStatementCache` 端口，新增 `get`/`insert`/`clear`）承接；`sql-parser` feature 去 `cache`、`permission` 显式补 `cache`（DbPool 权限策略缓存的真实消费）。契约测试 `tests/feature_dependency_contract.rs`（`cargo metadata` 结构断言）；`cargo tree --no-default-features --features sql-parser` 闭包零 oxcache/moka（实测 NO_OXCACHE_IN_CLOSURE）；`cargo check --no-default-features --features sql-parser` 编译通过 |

## 二、§9.2 / §2.3 命名迁移项

| 项 | 复核结论 | 证据 |
|---|---|---|
| `json` vs `with-json` 近义歧义（`json` 为空死 feature） | **已解决**（前轮治理删除死 feature `json`/`dev`） | `Cargo.toml` [features] 无 `json =`/`dev =` 定义；`with-json = ["sea-orm/with-json"]` 唯一正名 |
| `runtime-async-std` 名不符实 | **已解决**（保留 feature，注释显性化语义） | `Cargo.toml` `runtime-async-std` 定义处注明：tokio 为非可选核心依赖恒开，本 feature 仅转发 sea-orm 的 async-std 运行时支持，dbnexus 自身仍构建于 tokio 之上 |
| `pool-health-check` → `pool-probe`（与 `health-check` 易混） | **成立，本轮不改，登记待迁移** | 两 feature 并存且语义相近的混淆仍实存（`pool-health-check = ["futures"]` 连接池周期探测 vs `health-check = []` 健康快照模块，`Cargo.toml:157/163`）。不改理由：feature 名属公开 API 面，正名迁移须按工作区「正名 + 兼容别名一个版本」先例（limiteron `cache-redis`、oxcache `redlock` 同型）在 0.7.0 变更窗口执行：新增 `pool-probe` 正名、`pool-health-check` 降级为兼容别名，docs/示例同步后一个版本移除别名 |
| `kit` → `trait-kit-integration`（名称不可解） | **成立，本轮不改，登记待迁移** | 同上：`kit = ["dep:trait-kit", "oxcache-integration", ...]`（`Cargo.toml:149`）名称不表达 trait-kit 集成语义。迁移方案同型：0.7.0 新增 `trait-kit-integration` 正名 + `kit` 兼容别名；迁移前 `kit` 在 README 双语 feature 表与架构文档中的表述保持一致 |

> 两条待迁移项均不构成本轮验收缺口：验收口径为「每项有结论」，且更名属破坏面变更，须与版本窗口绑定（与 confers T050 显式迁移任务分属不同仓不同授权）。

## 三、cargo-hack 主要 feature 组合抽查

抽查口径：`cargo check -p dbnexus --no-default-features --features <组合>`，2026-09-30 实测（cargo-hack 0.6.45 在位，多组合经逐条 `cargo check` 执行——该版本将多个 `--features` 参数合并为单次运行，故未用其多参模式）。

**全绿组合（22 条）**：

- 零 feature（default = []，池抽象层）
- `sql-parser`（解耦证明：无 cache/oxcache 编译通过）
- `cache`、`oxcache-integration`、`permission`、`sql-parser,cache`（解耦后两 feature 独立正交共存）
- `sql-parser,permission`、`retry`、`prepare-cache`、`prepare-cache,sql-parser`
- `kit`、`data-protection`、`audit`、`observability`、`http-health`
- `neo4j`、`config-confers`、`default-no-db`、`copy`、`distributed-capabilities`
- `runtime-async-std`、`validation`、`authentication`、`with-chrono`、`entity-macros`、`mock`

**抽查发现并已修复的裂缝**：

- `all-optional`、`data-management`（含 `global-index`）无驱动组合编译失败：`src/storage/global_index.rs` 的 `DeriveEntityModel`/`DeriveRelation` derive 依赖 sea-orm/macros，而 `global-index` feature 未蕴含。**既有裂缝实证**：HEAD（2861e3c）worktree 同组合复现 23 个编译错误（E0433 `Column` 缺失等），CI 未暴露因 CI 口径恒带 `sqlite`（驱动 feature 均隐含 sea-orm/macros）。**修复**：`global-index = ["chrono", "with-json", "entity-macros"]`（复用既有 sea-orm 派生宏转发 feature），修复后 `all-optional`/`data-management`/`global-index`/`shard-migration` 四组合全绿；契约测试 `global_index_implies_entity_macros` 固化蕴含防回归。

**测试面连带发现并已修复的裂缝（全量测试复跑暴露）**：

- `http_health_tests` 7/9 失败（admin 预热查询 `SELECT 1` 被权限管道误拒）：`execute_raw`/`query_rows` 的表名有效性检查排在 admin 绕过之前，无 FROM 语句对 admin 一律拒绝，与「admin 绕过权限检查」语义相悖。**既有缺陷实证**：HEAD（2861e3c）worktree 同测试同样 7 failed，非本轮 feature 变更引入。**修复**：两分支的 admin 绕过整体前置（表名检查随绕过跳过），非 admin fail-closed 与逐表校验不变；回归测试 `test_admin_bypasses_table_name_validity_check` 双向断言（admin 放行 + 非 admin 拒绝），修复后 `core_permission_integration` 11/11、`http_health_tests` 9/9 全绿。
- `core_session_transaction` 3 条用例（`test_execute_raw_denies_when_sql_parse_fails`/`test_execute_denies_when_no_table_in_statement`/`test_execute_raw_rejects_effectively_empty_table_name` + 同型 `test_execute_rejects_effectively_empty_table_name`）系对上述缺陷行为的固化断言（用 admin 无表语句断言拒绝，与测试名所述「解析失败拒绝」意图亦不符——`SELECT 1` 解析并不失败）。按修正后语义同步改写：解析失败与无表/坏表名的 fail-closed 断言改由非 admin 角色承载，admin 侧补放行断言；修正后 `core_session_transaction` 34/34 全绿。**冲突裁决依据**：README「解析失败时 admin 角色放行、非 admin 角色拒绝」与 admin-bypass 设计语义优先，测试固化的是修复前的实现顺序缺陷。
- `oxcache_query_cache_tests` 2 条失败（`test_cross_role_instances_never_share_entries`/`test_hash_field_injection_cannot_confuse_entries`，HEAD 同败）：`with_role` 实例的 `query_cached` 经 `get_session(role)` 触发角色校验，测试池未配权限文件，受限角色被安全默认（仅 admin/system）拒绝。**修复**：两测试的池显式携带权限配置（定义受限角色及其表权限；换行角色名经 JSON 转义序列承载），修正后 12/12 全绿。
- `ops_cli_tests` 5 条失败（HEAD 同败 8 条）：运维子命令（pool-status/audit-query/shard-info/permission-check）经 cli 的 `migration`/`health-check`/`permission-engine` 等 features 门控（均属 cli default 集合），workspace `--no-default-features` 口径关闭它们使子命令不存在。**处置**：测试套件加显性 `#![cfg(...)]` 门控（该口径下编译期跳过而非失败），`cargo test -p dbnexus-cli` 默认口径 25/25 全绿；**登记待办**：workspace 口径与 cli default features 的适配属 CI feature 工程问题（`--no-default-features` 对所有 workspace 成员生效，cli 被连带裁剪），修复需 cli 转发 feature 设计，超出本任务范围。

**沙箱边界（未抽查组合与理由）**：

- `duckdb`/`ladybug`/`sqlite`/`postgres`/`mysql` 及含驱动的组合：bundled 驱动（duckdb/lbug）需 C++ 全量编译，沙箱不可行；驱动组合由 CI `check`/`check-bare`/`test` 矩阵覆盖。
- `duckdb` + `ladybug` 互斥守卫的触发验证：由 `tests/feature_mutex_guards.rs` 以源码契约固化守卫 cfg（rustc `cfg` + `compile_error!` 语义为确定性编译行为），沙箱不实际编译 bundled 驱动。

## 四、附带交付物索引

- `tests/feature_mutex_guards.rs`：duckdb/ladybug 互斥守卫契约（T042 交付，本记录第三节引用）
- `tests/feature_dependency_contract.rs`：sql-parser 解耦 + permission 显式 cache + global-index 蕴含 entity-macros 三契约
- `docs/CHANGELOG.md` Unreleased：解耦、互斥守卫、MySQL 核验、global-index 蕴含修复条目
