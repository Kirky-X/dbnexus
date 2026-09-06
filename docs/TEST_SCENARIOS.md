# dbnexus 测试场景固化（TEST_SCENARIOS）

> 阶段 2 验收产物。记录测试金字塔基线、66 个测试目标的功能域落点、E2E 场景定义、
> 本轮真实行为核正发现、驱动组组合矩阵与静态门槛。验证口径全部为 `cargo test`。

## §1 测试金字塔基线

| 层级 | 承载 | 数量基线 |
| --- | --- | --- |
| L1 lib 单元测试 | `src/**` 内 `#[cfg(test)]` | error/common_types/config/domain 等模块自测，随 `--lib` 运行 |
| L2 集成测试 | `tests/**` 深路径，66 个 `[[test]]` 显式注册 | 单元/集成分层目录（unit/integration/） |
| L3 E2E 场景 | `tests/e2e/`（e2e_advanced 88 测试 + e2e_distributed 10 测试） | 98 测试，按 cfg(feature) 隔离 |
| L4 容器级 | `postgres_testcontainers` + `mysql_testcontainers` | 各 14 测试，每测试独立容器隔离 |

驱动组全量结果（CI 口径 `--no-default-features --features <db>,default-no-db,all-optional --workspace --exclude dbnexus-examples --exclude dbnexus-macros --no-fail-fast`）：

| 驱动组 | 结果 | 备注 |
| --- | --- | --- |
| sqlite | 1712 passed / 0 failed | 含 2 处 DdlGuard 过时断言核正 |
| postgres | 1276 passed / 0 failed | edge_cases 门控至 sqlite、tc 改自起隔离后重跑实证（env -u DATABASE_URL -u TEST_DATABASE_URL） |
| mysql | 1276 passed / 0 failed | tc 修正后单独实证 14/14（自起） |
| duckdb | 1300 passed / 0 failed | CI 矩阵缺席，本地补验 |

## §2 测试目标功能域落点（66 目标 → 模块域）

- core：entity_integration / sql_parser_integration / session_transaction / permission_integration / dbnexus_integration / config_integration / health_integration / config_unit / common_types_unit / error_unit / edge_cases
- entity：sea_orm_type / entity_unit / db_entity_pk_types（0.4.2 主键泛型回归）
- migration：integration ×8（含 auto_migrate / schema_macro / query_builder / timestamps / soft_delete / validation / hooks）+ unit
- pool：session_unit / db_pool_unit / pool_unit / pool_integration / duckdb_pool / pool_semaphore
- permission：rbac_unit / advanced_rbac_unit / pdp_unit / rate_limiter_unit / permission_cache_ttl / permission_test(domain) / permission_engine_integration / permission_integration
- security：sql_injection_tests / sensitive_masker_tests
- audit：audit_integration / audit_unit
- sharding：sharding_integration / shard_session_routing / sharding_unit
- observability：metrics_integration / metrics_unit / health_unit
- 分布式能力：retry_unit / failover_unit / replica_unit / scatter_unit / saga_unit / distributed_id_unit
- global_index：global_index_unit / global_index_integration
- 容器级：postgres_testcontainers / mysql_testcontainers
- 交叉：cross_cutting（benchmarks / cli / concurrency / multi_db）/ repository_macro / authentication_integration / cache_integration / macros 相关（dbnexus-macros crate 另有独立测试）
- E2E：tests/e2e/e2e_advanced + tests/e2e/e2e_distributed（目录承载，不裸放顶层）

## §3 E2E 场景定义

### tests/e2e/e2e_advanced.rs（88 测试，cfg(feature) 分域）

边界编号沿用分析文档 B 系列：B07/B08/B09/B22/B25/B26/B33/B34 已归属本文件。

| 域 | feature 门 | 场景要点 |
| --- | --- | --- |
| SensitiveMasker 边界 | 无 | 空串（B33）/短于保留位（B34）/自定义长度越界/Unicode/单字符/超长输入 → InvalidInput 或安全输出 |
| CircuitBreaker 状态机 | health-check | 失败阈值开/半开恢复阈值/开态失败 noop/开态成功 noop/闭态成功重置/Display/Error 构造 |
| ShardRouter | sharding | 默认路由/未注册分片/策略名清单/月年策略 shard_id 校验/空键走策略/deterministic |
| GlobalIndex batch_sync | global-index + sqlite | 恰好 chunk/刚超 chunk/两个满 chunk/大 shard_id/Unicode/混合表/重复 upsert |
| 认证 JWT 流 | authentication | 注册→认证→刷新令牌流/令牌类型错配/自定义过期/特殊字符/密码强度/并发认证/空 hash 拒绝/明文 hash 拒绝 |
| i18n 格式化 | 无 | 无效 locale/多 formatter/复数类目/时间戳边界（闰年 2.29/2.30/12.31）/数字格式（极值/负数/非有限） |
| Pool 指标 | runtime | 连接创建/失败/关闭记录/active 增减/should_create 判定/健康度（idle/busy/capacity）|
| Migration 消息 | 无 | format_row_count 零值/migration message 零值 |

### tests/e2e/e2e_distributed.rs（10 测试，成员级 cfg）

retry×DbConfig / failover 多 URL / replica lag 阈值 / scatter 聚合类型与 PartialFailurePolicy / saga 生命周期与错误类型 / distributed_id 解析回环 + Snowflake E2E。CI 口径下 all-optional 传递启用全部成员 → 全激活。

## §4 真实行为核正与发现（阶段 2）

1. **DdlGuard 白名单过时断言 ×2**：session_unit_tests TEST-U-SESS-012 与 sql_injection_tests test_ddl_guard_vs_sql_injection 断言"DROP TABLE 应被拒"已过时——自 0540954 起 DropTable 入 ALLOWED_DDL_STATEMENTS 白名单（迁移场景设计决策）；实际拦截层为 FORBIDDEN_PATTERNS 字符串层（"DROP DATABASE"/"DROP ALL"）。按真实行为核正为 DropTable 放行 + DROP DATABASE 拒绝。
2. **sqlite 硬编码目标缺门控**：26 个测试文件经 common::make_sqlite_memory_pool 依赖 sqlite::memory:（embedded 专属），6 个目标（core_session_transaction / migration_integration / migration_auto_migrate / cross_cutting_concurrency / cross_cutting_multi_db / edge_cases）required-features 补 "sqlite"，否则无 sqlite 驱动组必 panic。
3. **common 多库重定向意图与 CI 实况脱节**：common 读 DATABASE_URL 意图实现"同一测试跨库矩阵"，但 CI 从未全绿（postgres/mysql job 下 sqlite 方言/表名冲突暴露）；CI 实际设 DATABASE_URL，本地组跑曾误用 TEST_DATABASE_URL（common 不读）。多库真正可行需表名唯一化+方言适配改造，记为后续改进项。
4. **testcontainers 复用分支违背隔离设计**：postgres/mysql_testcontainers 的 setup 优先复用 TEST_DATABASE_URL/DATABASE_URL——共享库无预清理，跨轮残留表 42P07 already exists 必现（postgres 实证 10 failed；mysql 当轮全绿系库为首次使用无残留的侥幸）。移除复用分支，恢复每测试独立容器隔离主设计，两文件自起模式 14/14 ×2 实证。
5. **e2e 目标 required-features 口径不匹配**：e2e_advanced 原含 authentication、e2e_distributed 原含聚合名 distributed-capabilities——均不在 CI 口径（<db>,default-no-db,all-optional）的 feature 闭包内，目标从未编译运行。按"文件按 cfg(feature) 隔离"设计移除；authentication 模块激活验证归入组合矩阵（86 passed）。
6. **e2e 目录承载**：tests/e2e_advanced.rs、tests/e2e_distributed.rs 迁入 tests/e2e/（git rename），[[test]] path 同步；两文件无 mod common/include_str 相对引用，迁移无查找根问题。
7. **duckdb_params_security_test.rs 自门控确认**：顶层自动发现 + `#![cfg(all(feature = "duckdb", feature = "sql-parser"))]` 文件级门控，duckdb 组真实运行（参数化注入安全/sqlparser DuckDB 方言门/事务原子性/文件池顺序 session），非缺口。
8. **edge_cases 多库改造后续项**：目标内并发测试共用表名，多库运行需先做表名唯一化（否则共享库 CREATE TABLE 同名冲突），当前门控至 sqlite 组。

## §5 驱动组组合矩阵

| 组合 | 覆盖 | 结果 |
| --- | --- | --- |
| sqlite,default-no-db,all-optional | sqlite 组全量 | 1712/0 |
| postgres,default-no-db,all-optional | postgres 组全量（无 URL，tc 自起） | 见台账 |
| mysql,default-no-db,all-optional | mysql 组全量（tc 自起单独实证） | 1276/0 |
| duckdb,default-no-db,all-optional | duckdb 组全量（CI 缺席补验） | 1300/0 |
| sqlite+all-optional+authentication | e2e_advanced 全模块激活 | 86/0 |
| 驱动互斥 | embedded 与 server-side 混合编译失败（无逃生门） | 组合验证不适用 --all-features |

## §6 静态门槛

| 门槛 | 命令口径 | 结果 |
| --- | --- | --- |
| fmt | `cargo fmt --all -- --check` | 净 |
| clippy | `cargo clippy --no-default-features --features <db>,default-no-db,all-optional --workspace --exclude dbnexus-examples --exclude dbnexus-macros -- -D warnings`（×4 驱动组）+ `-p dbnexus-macros -- -D warnings` | 全部零告警 |
| doc | `RUSTFLAGS="-D warnings" cargo doc --no-deps --features sqlite,default-no-db,all-optional` | 零告警 |
| deny | `cargo deny check` | 4 项 ok（licenses 经 clarify 绑定 LICENSE hash，见 deny.toml 注释） |
| audit | `cargo audit --stale` | rc=0（RUSTSEC-2025-0134 在 allowed，neo4rs 传递依赖） |
| examples | `examples/test_all_examples.sh`（项目自带脚本，51 个 [[bin]]，graph_ladybug 除外） | 51/51 通过，0 警告 0 失败 |
| macros | `cargo test -p dbnexus-macros`（CI exclude，本地补验） | db_entity_tests 2 聚合全过（pass×6 + compile_fail×3） |
