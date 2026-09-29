# ⚡ Dbnexus 性能基线

> 本文档记录 DBNexus 的性能基准设施与端到端基线数据。数字为本机一次性采样（非 SLA、非回归门禁），用作后续优化的对照参考。CI 阈值门禁待负载稳定后再启用（与工作区其他项目口径一致）。

## 📋 目录

<details open>
<summary>📑 目录</summary>

- [基准设施](#基准设施)
- [端到端基线](#端到端基线)
  - [测量环境](#测量环境)
  - [基线数据](#基线数据)
  - [解读](#解读)
- [专项基准](#专项基准)
- [历史优化对照](#历史优化对照)
- [CI 门禁说明](#ci-门禁说明)

</details>

---

## 基准设施

仓库内置 5 个 criterion 基准（位于 [benches/](../benches/)），随 `bench` 特性提供：

| 基准文件 | 覆盖面 | 运行命令 |
|----------|--------|----------|
| `benches/e2e_bench.rs` | 端到端：池获取 / 行查询 / 批量写 | `cargo bench --bench e2e_bench --features "sqlite,runtime-tokio-rustls,sql-parser"` |
| `benches/batch_insert_bench.rs` | 多值 INSERT：逐行 vs 500 行分块（sqlite 临时文件库） | `cargo bench --bench batch_insert_bench --features "sqlite,copy,sql-parser,runtime-tokio-rustls"` |
| `benches/duckdb_copy_bench.rs` | DuckDB COPY：逐行 vs `copy_in`（duckdb::memory:） | `cargo bench --bench duckdb_copy_bench --features "duckdb,copy,sql-parser,runtime-tokio-rustls"` |
| `benches/permission_bench.rs` | 池构造（sqlite::memory:）、DbConfig 路径 | `cargo bench --bench permission_bench --features permission` |
| `benches/permission_engine_bench.rs` | PermissionCache insert / get / miss | `cargo bench --bench permission_engine_bench --features permission-engine` |
| `benches/sharding_bench.rs` | 分片路由哈希、跨分片绑定冲突检测 | `cargo bench --bench sharding_bench --features sharding` |
| `benches/metrics_bench.rs` | 百分位计算、Prometheus 导出、直方图记录 | `cargo bench --bench metrics_bench --features metrics` |

两轮平台优化的过程记录与逐轮对照见 [benches/baseline-after.md](../benches/baseline-after.md)。

---

## 端到端基线

### 测量环境

| 项 | 值 |
|----|----|
| 日期 | 2026-09-11 |
| 平台 | WSL2 linux 6.6.87.2-microsoft-standard-WSL2 x86_64（12 逻辑核） |
| 工具链 | rustc/cargo 1.97.1 |
| Profile | `bench`（optimized），criterion 默认采样 |
| 数据库 | sqlite 临时文件库（`sqlite:<path>?mode=rwc`），embedded 驱动组 |

### 基线数据

复现命令：

```bash
cargo bench --bench e2e_bench --features "sqlite,runtime-tokio-rustls,sql-parser"
```

| 基准 | 路径 | 基线（中位数） |
|------|------|----------------|
| `e2e_pool/get_session_admin` | `DbPool::get_session("admin")` 句柄获取（权限校验 + 池记账） | **≈ 0.24 µs** |
| `e2e_query/query_rows_select_single` | `DbPool::query_rows("SELECT id, val FROM t_e2e WHERE id = 1", "admin")` 完整行查询管道（解析 → 权限 → 执行 → 行 → JSON 出口） | **≈ 480 µs**（波动 350–590 µs） |
| `e2e_write/execute_raw_insert_x64` | `Session::execute_raw` 循环 INSERT，64 行/迭代 | **≈ 708 ms/迭代**（≈ 11 ms/行，≈ 90 行/s） |

### 解读

- **池获取**是纯内存路径（句柄 + 权限检查），亚微秒级；真实连接建立发生在首次执行时（sea-orm/sqlx 惰性连接）。
- **简单查询**约 0.5 ms，主要成本在 sql-parser 校验 + sea-orm `query_all_raw` + `serde_json` 行构造，属可接受的端到端常量。
- **逐条 INSERT** ≈ 11 ms/行：每条语句独立走解析/权限/执行全管道。批量写入请优先使用事务批提交或 `copy` 特性（pg COPY 协议路径）；逐条路径的数字即为未批处理时的下界参考。

---

## 批量写入基线（`copy` 特性）

### 测量环境

| 项 | 值 |
|----|----|
| 日期 | 2026-09-29 |
| 平台 | WSL2 linux 6.6.87.2-microsoft-standard-WSL2 x86_64 |
| 工具链 | rustc/cargo 1.97.1 |
| Profile | `bench`（optimized），criterion 多轮统计采样（bench 内显式 `sample_size(30)`，`--noplot`） |

### 基线数据

| 基准 | 路径 | 基线（中位数） |
|------|------|----------------|
| `batch_insert_sqlite/row_by_row_500` | 逐行 `execute_with_params`（每行独立走解析/权限/执行全管道） | **≈ 18.1 s/500 行**（≈ 27.6 行/s） |
| `batch_insert_sqlite/multi_value_chunked_500` | `BatchInsertStatement::chunk_rows` 构建 + 500 行单块参数化执行 | **≈ 77.0 ms/500 行**（≈ 6.49K 行/s） |
| `copy_duckdb/row_by_row_500` | 逐行 `execute_duckdb_raw_with_params`（duckdb::memory:） | **≈ 381 ms/500 行**（≈ 1.31K 行/s） |
| `copy_duckdb/copy_in_500` | `DbPool::copy_in`（CSV 载荷 + 临时文件 `COPY FROM`） | **≈ 9.3 ms/500 行**（≈ 53.8K 行/s） |

sqlite 侧多值路径对逐行路径约 **235× 吞吐提升**（500 条语句的往返/解析/权限开销收敛为 1 条语句）；duckdb 侧 `copy_in` 对逐行约 **41× 吞吐提升**（批量导入绕过逐行往返）。吞吐列由表内基线值直接换算。绝对值受宿主机负载影响明显：sqlite 侧多轮比值落在 235×–285× 区间（含 chunk_rows 构建入计时前后的口径差异）；duckdb 侧跨轮跨度大（41×–186×，前轮 1.8 ms 为废弃的 `--quick` 单轮口径，与本轮多轮中位数不可直接对比），采信以当前多轮中位数为准；复测请用干净串行环境。

### 解读

- **多值 INSERT 是 sqlite/mysql 批量写入的首选**：参数化绑定保持注入安全，500 行分块对齐 SQLite 变量上限余量。
- **COPY 是 postgres/duckdb 大批量导入的首选**：PG 协议流式传输 / DuckDB 文件导入绕过逐行往返；小行集优先多值 INSERT。

---

## 专项基准

上表之外的基准聚焦子系统热点：

- **分片路由**（`sharding_bench`）：`shard_id_for_key` 哈希热路径与 `enforce_shard_binding_conflict` 绑定冲突检测。
- **指标导出**（`metrics_bench`）：`prometheus_export` 全量导出、`histogram_record` 直方图记录。
- **权限缓存**（`permission_engine_bench`）：`permission_cache_hit` / `permission_cache_miss`。

单项数据与两轮优化的逐轮对照表见 [benches/baseline-after.md](../benches/baseline-after.md)。

---

## 历史优化对照

两轮平台优化的累计结果（摘自 [benches/baseline-after.md](../benches/baseline-after.md)，2026-08-14，Linux x86_64，release profile，lto=thin）：

| 基准项 | 原始基线 (µs) | 最终 (µs) | 累计变化 |
|--------|---------------|-----------|----------|
| shard_id_for_key | 2.9725 – 3.0496 | 2.8871 – 2.9188 | -3.9% |
| enforce_shard_binding_conflict | 5.8179 – 5.9490 | 5.4019 – 5.5934 | -5.5% |
| prometheus_export | 4.2057 – 4.3566 | 4.0485 – 4.1789 | -3.1% |
| histogram_record | 882.21 – 888.70 ns | 796.62 – 804.15 ns | -9.4% |
| permission_cache_hit | 8.8730 – 8.9906 | 8.1415 – 8.1825 | -8.3% |
| permission_cache_miss | 3.6778 – 3.7570 | 3.5827 – 3.6179 | -2.8% |

6 项基准全部正向提升、无回退，平均累计提升约 5.5%。主要手段：原子内存序降级（`SeqCst` → `Relaxed` / `Acquire` / `AcqRel`）、哈希热路径去字符串分配、CircuitBreaker 状态无锁化、热点结构 CacheLine 对齐。

---

## CI 门禁说明

```bash
# CI 中仅验证基准可编译可运行（不设阈值断言，防虚拟机抖动误报）：
cargo bench --bench e2e_bench --features "sqlite,runtime-tokio-rustls,sql-parser" -- --test
```

阈值断言待在稳定硬件上采集多轮数据后另行启用。
