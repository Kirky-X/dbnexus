# ⚡ Dbnexus Performance Baseline

> This document records DBNexus's benchmark infrastructure and end-to-end baseline data. The numbers are one-off samples from a local machine (not an SLA, not a regression gate), used as a reference for future optimization comparisons. CI threshold gates will be enabled only after the workload stabilizes (consistent with the other projects in the workspace).

## 📋 Table of Contents

<details open>
<summary>📑 Table of Contents</summary>

- [Benchmark Infrastructure](#benchmark-infrastructure)
- [End-to-End Baseline](#end-to-end-baseline)
  - [Measurement Environment](#measurement-environment)
  - [Baseline Data](#baseline-data)
  - [Interpretation](#interpretation)
- [Specialized Benchmarks](#specialized-benchmarks)
- [Historical Optimization Comparison](#historical-optimization-comparison)
- [CI Gate Notes](#ci-gate-notes)

</details>

---

## Benchmark Infrastructure

The repository ships 9 criterion benchmarks (under [benches/](../../benches/)); criterion is provided via dev-dependencies and is not enabled through a feature:

| Benchmark file | Coverage | Command |
|----------|--------|----------|
| `benches/e2e_bench.rs` | End-to-end: pool acquisition / row query / batch write | `cargo bench --bench e2e_bench --features "sqlite,runtime-tokio-rustls,sql-parser"` |
| `benches/batch_insert_bench.rs` | Multi-row INSERT: row-by-row vs 500-row chunking (sqlite temp-file database) | `cargo bench --bench batch_insert_bench --features "sqlite,copy,sql-parser,runtime-tokio-rustls"` |
| `benches/duckdb_copy_bench.rs` | DuckDB COPY: row-by-row vs `copy_in` (duckdb::memory:) | `cargo bench --bench duckdb_copy_bench --features "duckdb,copy,sql-parser,runtime-tokio-rustls"` |
| `benches/permission_bench.rs` | Pool construction (sqlite::memory:), DbConfig paths | `cargo bench --bench permission_bench --features "sqlite,runtime-tokio-rustls"` (the `[[bench]]` required-features are `sqlite` + `runtime-tokio-rustls`; passing only `permission` gets silently skipped by cargo) |
| `benches/permission_engine_bench.rs` | PermissionCache insert / get / miss | `cargo bench --bench permission_engine_bench --features permission-engine` |
| `benches/sharding_bench.rs` | Shard-routing hashing, cross-shard binding-conflict detection | `cargo bench --bench sharding_bench --features sharding` |
| `benches/metrics_bench.rs` | Percentile computation, Prometheus export (including 1/50/200 label-scale curves + slow-query ring full state), histogram recording, health_snapshot (`metrics,sqlite,health-check,sql-parser,runtime-tokio-rustls`) | `cargo bench --bench metrics_bench --features metrics` |
| `benches/oxcache_query_cache_bench.rs` | oxcache query-cache decorator: miss vs hit paths of N+1-shaped queries (sqlite temp-file database + Moka backend) | `cargo bench --bench oxcache_query_cache_bench --features "sqlite,oxcache-integration,sql-parser,runtime-tokio-rustls"` |
| `benches/prepare_cache_bench.rs` | Statement-level prepared statement LRU cache: steady-state insert/eviction cost at full capacity does not degrade linearly with capacity | `cargo bench --bench prepare_cache_bench --features prepare-cache` |

For the process records and per-round comparisons of the two platform optimization rounds, see [benches/baseline-after.md](../../benches/baseline-after.md).

---

## End-to-End Baseline

### Measurement Environment

| Item | Value |
|----|----|
| Date | 2026-09-11 |
| Platform | WSL2 linux 6.6.87.2-microsoft-standard-WSL2 x86_64 (12 logical cores) |
| Toolchain | rustc/cargo 1.97.1 |
| Profile | `bench` (optimized), criterion default sampling |
| Database | sqlite temp-file database (`sqlite:<path>?mode=rwc`), embedded driver group |

### Baseline Data

Reproduction command:

```bash
cargo bench --bench e2e_bench --features "sqlite,runtime-tokio-rustls,sql-parser"
```

| Benchmark | Path | Baseline (median) |
|------|------|----------------|
| `e2e_pool/get_session_admin` | `DbPool::get_session("admin")` handle acquisition (permission validation + pool accounting) | **≈ 0.24 µs** |
| `e2e_query/query_rows_select_single` | `DbPool::query_rows("SELECT id, val FROM t_e2e WHERE id = 1", "admin")` full row-query pipeline (parse → permission → execute → rows → JSON exit) | **≈ 480 µs** (fluctuates 350–590 µs) |
| `e2e_write/execute_raw_insert_x64` | `Session::execute_raw` INSERT loop, 64 rows/iteration | **≈ 708 ms/iteration** (≈ 11 ms/row, ≈ 90 rows/s) |

### Interpretation

- **Pool acquisition** is a pure in-memory path (handle + permission check), sub-microsecond; the real connection is established on first execution (sea-orm/sqlx lazy connections).
- **A simple query** takes about 0.5 ms; the main cost is sql-parser validation + sea-orm `query_all_raw` + `serde_json` row construction — an acceptable end-to-end constant.
- **Row-by-row INSERT** ≈ 11 ms/row: each statement independently goes through the full parse/permission/execution pipeline. For batch writes, prefer transactional batch commits or the `copy` feature (the pg COPY protocol path); the row-by-row numbers are the lower-bound reference for the unbatched path.

---

## Batch Write Baseline (the `copy` feature)

### Measurement Environment

| Item | Value |
|----|----|
| Date | 2026-09-29 |
| Platform | WSL2 linux 6.6.87.2-microsoft-standard-WSL2 x86_64 |
| Toolchain | rustc/cargo 1.97.1 |
| Profile | `bench` (optimized), criterion multi-round statistical sampling (explicit `sample_size(30)` inside the bench, `--noplot`) |

### Baseline Data

The table below is a clean serial sampling taken in a single session on the same day (after clearing the criterion archive, the two benchmark groups ran separately one after another; no parallel tasks within the session):

| Benchmark | Path | Baseline (median) |
|------|------|----------------|
| `batch_insert_sqlite/row_by_row_500` | row-by-row `execute_with_params` (each row independently goes through the full parse/permission/execution pipeline) | **≈ 5.54 s/500 rows** (≈ 90 rows/s) |
| `batch_insert_sqlite/multi_value_chunked_500` | `BatchInsertStatement::chunk_rows` building + a single 500-row chunk executed with bound parameters | **≈ 22.8 ms/500 rows** (≈ 21.9K rows/s) |
| `copy_duckdb/row_by_row_500` | row-by-row `execute_duckdb_raw_with_params` (duckdb::memory:) | **≈ 294 ms/500 rows** (≈ 1.70K rows/s) |
| `copy_duckdb/copy_in_500` | `DbPool::copy_in` (CSV payload + temp-file `COPY FROM`, payload I/O offloaded via `spawn_blocking`) | **≈ 1.58 ms/500 rows** (≈ 316K rows/s) |

On the sqlite side, the multi-row path is about a **243× throughput improvement** over the row-by-row path (the round-trips/parsing/permission overhead of 500 statements collapses into 1 statement); on the duckdb side, `copy_in` is about a **187× throughput improvement** over row-by-row (bulk import bypasses per-row round-trips). The throughput figures are derived directly from the baselines in the table. Absolute values are strongly affected by host load: on the sqlite side, multiple rounds of the ratio fall in the 235×–285× range (the accounting differs slightly depending on whether the chunk_rows build is inside the timing); on the duckdb side, historical rounds span a wide range (41×–187×; the 41× round was sampled during a high-load period on the host and predates the `spawn_blocking` offload of payload I/O — the 1.58 ms vs 9.3 ms difference between rounds includes the benefit of that change, not attributed separately) — trust the multi-round median from a clean serial environment; for re-measurement, use a clean serial environment.

### Interpretation

- **Multi-row INSERT is the first choice for sqlite/mysql batch writes**: parameterized bindings keep injection safety, and 500-row chunks leave headroom under the SQLite variable limit.
- **COPY is the first choice for postgres/duckdb bulk imports**: the PG protocol streaming / DuckDB file import bypasses per-row round-trips; for small row sets, prefer multi-row INSERT.

---

## Specialized Benchmarks

Beyond the table above, benchmarks focus on subsystem hot spots:

- **Shard routing** (`sharding_bench`): the `shard_id_for_key` hashing hot path and `enforce_shard_binding_conflict` binding-conflict detection.
- **Metrics export** (`metrics_bench`): `prometheus_export` full export, `prometheus_export_scale` 1/50/200 label-scale curves (including slow-query ring full state), `histogram_record` histogram recording, `health_snapshot`.
- **Permission cache** (`permission_engine_bench`): `permission_cache_hit` / `permission_cache_miss`.

### Metrics Export and Health Snapshot Baselines (2026-09-30, WSL2 multi-round sampling ranges; 2-3 rounds of independent processes per point, idle machine; the ranges are not stable envelopes — cross-process drift can exceed 2×)

Scale baselines for the `/metrics` scrape path (pass-through of the http-health endpoint) and the `/readyz` data source:

| Benchmark | Shape | Median range (multi-round) |
|------|------|----------------|
| `prometheus_export_scale/labels_1` | 1 label + slow-query ring full | ≈ 2.1-2.2 µs |
| `prometheus_export_scale/labels_50` | 50 labels + slow-query ring full | ≈ 22-29 µs |
| `prometheus_export_scale/labels_200` | 200 labels + slow-query ring full | ≈ 85-240 µs |
| `health_snapshot` | pool state snapshot (sqlite::memory:) | ≈ 0.4-0.8 µs |

Export cost grows linearly with the label count (≈0.4 µs/label at small scale; the 200-label point has significant host noise — judge regressions by the multi-round in-process trend); the contents of the slow-query ring are currently not exported, and the full ring is only reserved as a precondition for future integration. `health_snapshot` and the scale points are all sub-microsecond to hundred-microsecond baselines; on WSL2, cross-process drift can reach 2×, and absolute differences within the ranges above are noise.

### oxcache Query Cache Baselines (2026-09-30, WSL2 multi-round sampling; `oxcache_query_cache_bench`)

N+1-shaped 200 point queries (sqlite temp-file database + Moka backend), the core validation of the hit benefit against decorator overhead:

| Benchmark | Path | Median |
|------|------|----------------|
| `n_plus_one_200_always_miss` | 200 point queries all miss after each round's invalidation (key derivation + miss + execution + backfill) | **cold first round ≈ 106.5 ms; warm state ≈ 9-14 ms/200 queries** (the miss cost is dominated by database I/O and is sensitive to the page-cache state) |
| `n_plus_one_200_all_hit` | 200 point queries all hit after warm-up (key derivation + get hit, zero database round-trips) | **≈ 102-112 µs/200 queries** (≈ 0.51-0.56 µs/query) |
| `hit_path_1_table` | single-table hit full path (key derivation + version read + get + deserialization) | ≈ 0.6 µs |
| `hit_path_2_tables` | two-table hit full path (batch version read of 2 keys) | ≈ 0.77 µs |

Hit benefit multiplier in two accountings: **warm state ≈ 90-120×** (steady-state re-measurement accounting; use this for capacity/benefit decisions); the cold first round can reach ≈ 1045× (first-I/O amplification of the file database, not steady state). The hit path is stable (within ±11%), while the miss path fluctuates strongly with host I/O — evaluate the benefit using hit-path cost + warm-state miss; the decorator's own hot path (key derivation + backend get) is sub-microsecond per query.

For the per-item data and the per-round comparison tables of the two optimization rounds, see [benches/baseline-after.md](../../benches/baseline-after.md).

---

## Historical Optimization Comparison

Cumulative results of the two platform optimization rounds (excerpted from [benches/baseline-after.md](../../benches/baseline-after.md), 2026-08-14, Linux x86_64, release profile, lto=thin):

| Benchmark | Original baseline (µs) | Final (µs) | Cumulative change |
|--------|---------------|-----------|----------|
| shard_id_for_key | 2.9725 – 3.0496 | 2.8871 – 2.9188 | -3.9% |
| enforce_shard_binding_conflict | 5.8179 – 5.9490 | 5.4019 – 5.5934 | -5.5% |
| prometheus_export | 4.2057 – 4.3566 | 4.0485 – 4.1789 | -3.1% |
| histogram_record | 882.21 – 888.70 ns | 796.62 – 804.15 ns | -9.4% |
| permission_cache_hit | 8.8730 – 8.9906 | 8.1415 – 8.1825 | -8.3% |
| permission_cache_miss | 3.6778 – 3.7570 | 3.5827 – 3.6179 | -2.8% |

All 6 benchmarks improved with no regressions, an average cumulative improvement of about 5.5%. Main techniques: atomic memory-order downgrades (`SeqCst` → `Relaxed` / `Acquire` / `AcqRel`), removing string allocations from hashing hot paths, making CircuitBreaker states lock-free, and CacheLine alignment of hot structures.

---

## CI Gate Notes

CI does not run benchmarks: no workflow under `.github/workflows/` has a bench step (`grep 'bench' .github/workflows/*.yml` matches nothing); benchmarks run locally only, with the commands in the tables above.

Threshold assertions will be enabled separately after multiple rounds of data are collected on stable hardware.
