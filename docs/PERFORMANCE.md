# 性能基线（PERFORMANCE Baseline）

> T414：端到端性能基线。数字为本机一次性采样（非 SLA、非回归门禁），用于后续优化的对照参考。
> CI 阈值门禁待负载稳定后再启用（与工作区其他项目口径一致）。

## 环境

| 项 | 值 |
| --- | --- |
| 日期 | 2026-09-11 |
| 平台 | WSL2 linux 6.6.87.2-microsoft-standard-WSL2 x86_64（12 逻辑核） |
| 工具链 | rustc/cargo 1.97.1 |
| Profile | `bench`（optimized），criterion 默认采样 |
| 数据库 | sqlite 临时文件库（`sqlite:<path>?mode=rwc`），embedded 驱动组 |

## 端到端基准（`benches/e2e_bench.rs`）

复现：

```bash
cargo bench --bench e2e_bench --features "sqlite,runtime-tokio-rustls,sql-parser"
```

| 基准 | 路径 | 基线（中位数） |
| --- | --- | --- |
| `e2e_pool/get_session_admin` | `DbPool::get_session("admin")` 句柄获取（权限校验 + 池记账） | **≈ 0.24 µs** |
| `e2e_query/query_rows_select_single` | `DbPool::query_rows("SELECT id, val FROM t_e2e WHERE id = 1", "admin")` 完整行查询管道（解析→权限→执行→行→JSON 出口） | **≈ 480 µs**（波动 350–590 µs） |
| `e2e_write/execute_raw_insert_x64` | `Session::execute_raw` 循环 INSERT，64 行/迭代 | **≈ 708 ms/迭代**（≈ 11 ms/行，≈ 90 行/s） |

### 解读

- **池获取**是纯内存路径（句柄 + 权限检查），亚微秒级；真实连接建立发生在首次执行时（sea-orm/sqlx 惰性连接）。
- **简单查询**约 0.5 ms，主要成本在 sql-parser 校验 + sea-orm `query_all_raw` + `serde_json` 行构造，属可接受 e2e 常量。
- **逐条 INSERT** ≈ 11 ms/行：每条语句独立走解析/权限/执行全管道。批量写入请优先使用事务批提交或
  `copy` feature（T407，pg COPY 协议路径）；逐条路径的数字即为未批处理时的下界参考。

## 其他既有基准（同一 docs 口径汇总）

| 基准文件 | 覆盖面 | 运行 |
| --- | --- | --- |
| `benches/permission_bench.rs` | 池构造（sqlite::memory:）、DbConfig 路径 | `cargo bench --bench permission_bench --features permission` |
| `benches/permission_engine_bench.rs` | PermissionCache insert/get/miss | `cargo bench --bench permission_engine_bench --features permission-engine` |
| `benches/sharding_bench.rs` | 分片路由哈希、跨分片绑定冲突检测 | `cargo bench --bench sharding_bench --features sharding` |
| `benches/metrics_bench.rs` | 百分位计算、Prometheus 导出、直方图记录 | `cargo bench --bench metrics_bench --features metrics` |

## CI 门禁说明

```bash
# CI 中仅验证基准可编译可运行（不设阈值断言，防虚拟机抖动误报）：
cargo bench --bench e2e_bench --features "sqlite,runtime-tokio-rustls,sql-parser" -- --test
```

阈值断言待在稳定硬件上采集多轮数据后另行启用。
