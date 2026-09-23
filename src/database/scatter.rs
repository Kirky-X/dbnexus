// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 跨分片查询引擎 — Scatter-Gather 执行器
//!
//! 向所有已注册分片并行发送查询，收集并聚合结果。

use std::sync::Arc;
use std::time::Duration;

use futures::stream::{FuturesUnordered, StreamExt};

use crate::database::sharding::ShardRouter;

// ============================================================================
// 类型定义
// ============================================================================

/// 部分失败策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartialFailurePolicy {
    /// 任何分片失败则整体失败
    Fail,
    /// 返回已成功分片的结果 + 失败分片信息
    BestEffort,
}

/// 聚合函数类型
#[derive(Debug, Clone)]
pub enum AggregateFunction {
    /// COUNT 聚合
    Count,
    /// SUM 聚合
    Sum(String),
    /// AVG 聚合
    Avg(String),
    /// MIN 聚合
    Min(String),
    /// MAX 聚合
    Max(String),
}

/// 聚合值
#[derive(Debug, Clone)]
pub enum AggregateValue {
    /// COUNT 结果
    Count(i64),
    /// SUM 结果
    Sum(f64),
    /// AVG 结果
    Avg(f64),
    /// MIN 结果
    Min(f64),
    /// MAX 结果
    Max(f64),
}

impl AggregateFunction {
    /// 对跨分片行数据计算聚合值（修复此前 aggregated 恒为 None 的缺陷）
    ///
    /// 数值列从 JSON 行中提取（`row[col]` 为 Number 才计入）；
    /// COUNT 基于行数，AVG 在无数值样本时返回 None，其余空数据返回中性值。
    pub fn compute_from_rows(
        &self,
        shard_rows: &[(u32, Vec<serde_json::Value>)],
    ) -> Option<AggregateValue> {
        let all_rows = || shard_rows.iter().flat_map(|(_, rows)| rows.iter());
        match self {
            AggregateFunction::Count => {
                let total: usize = shard_rows.iter().map(|(_, rows)| rows.len()).sum();
                Some(AggregateValue::Count(total as i64))
            }
            AggregateFunction::Sum(col) => {
                let sum: f64 = all_rows().filter_map(|r| extract_numeric(r, col)).sum();
                Some(AggregateValue::Sum(sum))
            }
            AggregateFunction::Avg(col) => {
                let mut sum = 0.0f64;
                let mut n = 0usize;
                for v in all_rows().filter_map(|r| extract_numeric(r, col)) {
                    sum += v;
                    n += 1;
                }
                if n == 0 {
                    None
                } else {
                    Some(AggregateValue::Avg(sum / n as f64))
                }
            }
            AggregateFunction::Min(col) => {
                let mut min: Option<f64> = None;
                for v in all_rows().filter_map(|r| extract_numeric(r, col)) {
                    min = Some(min.map_or(v, |m: f64| m.min(v)));
                }
                min.map(AggregateValue::Min)
            }
            AggregateFunction::Max(col) => {
                let mut max: Option<f64> = None;
                for v in all_rows().filter_map(|r| extract_numeric(r, col)) {
                    max = Some(max.map_or(v, |m: f64| m.max(v)));
                }
                max.map(AggregateValue::Max)
            }
        }
    }
}

/// 从 JSON 行提取数值列
fn extract_numeric(row: &serde_json::Value, col: &str) -> Option<f64> {
    match row.get(col) {
        Some(serde_json::Value::Number(n)) => n.as_f64(),
        _ => None,
    }
}

/// 单分片错误
#[derive(Debug, Clone)]
pub struct ShardError {
    /// 分片 ID
    pub shard_id: u32,
    /// 错误信息
    pub error: String,
}

/// 跨分片查询结果
#[derive(Debug)]
pub struct ScatterResult {
    /// 各分片返回的行数 (shard_id, row_count)
    pub shard_row_counts: Vec<(u32, u64)>,
    /// 失败分片列表
    pub failed_shards: Vec<ShardError>,
    /// 聚合结果（可选）
    pub aggregated: Option<AggregateValue>,
    /// 各分片返回的真实数据行 (shard_id, rows)——取回的数据行而非仅行数
    pub shard_rows: Vec<(u32, Vec<serde_json::Value>)>,
    /// 全局排序归并 + 分页后的行（仅 `scatter_query_rows_merged` 填充，其余路径为空）
    pub merged_rows: Vec<serde_json::Value>,
}

/// 全局排序键（跨分片归并用）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderKey {
    /// 列名（JSON 行对象的键）
    pub column: String,
    /// 是否降序
    pub desc: bool,
}

impl OrderKey {
    /// 升序排序键
    pub fn asc(column: impl Into<String>) -> Self {
        Self {
            column: column.into(),
            desc: false,
        }
    }

    /// 降序排序键
    pub fn desc(column: impl Into<String>) -> Self {
        Self {
            column: column.into(),
            desc: true,
        }
    }
}

/// 按排序键列表比较两行；同键值返回 Equal（保持稳定序）
fn compare_rows_by_keys(
    a: &serde_json::Value,
    b: &serde_json::Value,
    keys: &[OrderKey],
) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    for key in keys {
        let ord = match (a.get(&key.column), b.get(&key.column)) {
            (None, None) => Ordering::Equal,
            // 缺失恒排末尾（asc 与 desc 一致）
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(x), Some(y)) => compare_json_values(x, y, key.desc),
        };
        if ord != Ordering::Equal {
            return ord;
        }
    }
    Ordering::Equal
}

/// 单键值比较：同类值（Number 按数值 / String 按字典序）受 `desc` 反转；
/// 混型类型类序固定（Number < String < 其他），不随 desc 反转
fn compare_json_values(
    a: &serde_json::Value,
    b: &serde_json::Value,
    desc: bool,
) -> std::cmp::Ordering {
    use serde_json::Value;
    use std::cmp::Ordering;
    let ordered = |ord: Ordering| if desc { ord.reverse() } else { ord };
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let (fx, fy) = (x.as_f64(), y.as_f64());
            match (fx, fy) {
                (Some(x), Some(y)) => ordered(x.partial_cmp(&y).unwrap_or(Ordering::Equal)),
                _ => Ordering::Equal,
            }
        }
        (Value::String(x), Value::String(y)) => ordered(x.cmp(y)),
        // 类型类序固定（不受 desc 影响）
        (Value::Number(_), _) => Ordering::Less,
        (_, Value::Number(_)) => Ordering::Greater,
        (Value::String(_), _) => Ordering::Less,
        (_, Value::String(_)) => Ordering::Greater,
        _ => Ordering::Equal,
    }
}

/// 跨分片行全局归并排序
///
/// 输入各分片行集（分片内无需有序），输出全局有序的全部行。排序稳定：
/// 同键值行保持「分片 ID 升序、分片内原序」。
pub fn merge_shard_rows(
    shard_rows: &[(u32, Vec<serde_json::Value>)],
    order_by: &[OrderKey],
) -> Vec<serde_json::Value> {
    if order_by.is_empty() {
        return shard_rows
            .iter()
            .flat_map(|(_, rows)| rows.iter().cloned())
            .collect();
    }
    let mut indexed: Vec<(u32, usize, &serde_json::Value)> = shard_rows
        .iter()
        .flat_map(|(shard_id, rows)| {
            rows.iter()
                .enumerate()
                .map(move |(i, row)| (*shard_id, i, row))
        })
        .collect();
    indexed.sort_by(|(sa, ia, ra), (sb, ib, rb)| {
        compare_rows_by_keys(ra, rb, order_by)
            .then_with(|| sa.cmp(sb))
            .then_with(|| ia.cmp(ib))
    });
    indexed.into_iter().map(|(_, _, row)| row.clone()).collect()
}

/// 全局分页：对归并后的行集应用 offset/limit（作用于全局行序而非单分片）
pub fn apply_global_pagination(
    rows: Vec<serde_json::Value>,
    limit: u64,
    offset: u64,
) -> Vec<serde_json::Value> {
    if limit == 0 {
        return Vec::new();
    }
    let total = rows.len() as u64;
    let start = offset.min(total) as usize;
    let end = (start as u64).saturating_add(limit).min(total) as usize;
    rows.into_iter().skip(start).take(end - start).collect()
}

// ============================================================================
// ScatterGatherExecutor
// ============================================================================

/// Scatter-Gather 查询执行器
pub struct ScatterGatherExecutor {
    router: Arc<ShardRouter>,
    timeout: Duration,
    partial_failure: PartialFailurePolicy,
}

impl ScatterGatherExecutor {
    /// 创建执行器
    pub fn new(
        router: Arc<ShardRouter>,
        timeout: Duration,
        partial_failure: PartialFailurePolicy,
    ) -> Self {
        Self {
            router,
            timeout,
            partial_failure,
        }
    }

    /// 执行 scatter-gather 查询（兼容入口，行为=scatter_query_rows(sql, role, None)）
    pub async fn scatter_query(&self, sql: &str, role: &str) -> Result<ScatterResult, String> {
        self.scatter_query_rows(sql, role, None).await
    }

    /// scatter-gather 行查询——取回各分片真实数据行并可选聚合
    ///
    /// 与旧 `scatter_query` 的差异：
    /// 1. 各分片结果可取回数据行（`shard_rows`），不再只保留行数；
    /// 2. `aggregated` 按传入聚合函数跨分片计算（修复此前恒为 None 的缺陷）。
    pub async fn scatter_query_rows(
        &self,
        sql: &str,
        role: &str,
        agg: Option<&AggregateFunction>,
    ) -> Result<ScatterResult, String> {
        let shards = self.router.all_shards();
        let mut futures = FuturesUnordered::new();

        for shard_info in shards {
            let shard_id = shard_info.shard_id;
            if let Some(pool) = self.router.get_pool(shard_id) {
                let sql = sql.to_string();
                let role = role.to_string();
                futures.push(async move {
                    match pool.get_session(&role).await {
                        Ok(session) => match session.query_rows(&sql).await {
                            Ok(rows) => {
                                let count = rows.len() as u64;
                                Ok((shard_id, count, rows))
                            }
                            Err(e) => Err(ShardError {
                                shard_id,
                                error: e.to_string(),
                            }),
                        },
                        Err(e) => Err(ShardError {
                            shard_id,
                            error: e.to_string(),
                        }),
                    }
                });
            }
        }

        let mut shard_row_counts = Vec::new();
        let mut shard_rows = Vec::new();
        let mut failed_shards = Vec::new();

        let collect_future = async {
            while let Some(result) = futures.next().await {
                match result {
                    Ok((shard_id, count, rows)) => {
                        shard_row_counts.push((shard_id, count));
                        shard_rows.push((shard_id, rows));
                    }
                    Err(err) => failed_shards.push(err),
                }
            }
        };

        // 超时控制
        match tokio::time::timeout(self.timeout, collect_future).await {
            Ok(()) => {}
            Err(_) => return Err("Scatter-gather query timed out".to_string()),
        }

        // 部分失败策略
        if !failed_shards.is_empty() && self.partial_failure == PartialFailurePolicy::Fail {
            return Err(format!(
                "Scatter-gather failed: {} shard(s) failed",
                failed_shards.len()
            ));
        }

        // 跨分片聚合（修复此前 aggregated 恒为 None 的缺陷）
        let aggregated = agg.and_then(|f| f.compute_from_rows(&shard_rows));

        Ok(ScatterResult {
            shard_row_counts,
            failed_shards,
            aggregated,
            shard_rows,
            merged_rows: Vec::new(),
        })
    }

    /// scatter-gather 全局有序查询：跨分片归并排序 + 全局分页
    ///
    /// 与 `scatter_query_rows` 的差异：结果经 [`merge_shard_rows`] 全局排序后
    /// 应用 [`apply_global_pagination`]，limit/offset 作用于**全局行序**，
    /// 修正旧路径「各分片各自 LIMIT 后按完成顺序拼接」的错误语义。
    ///
    /// # 性能建议
    ///
    /// 各分片 SQL 应自带 `ORDER BY` 与 `LIMIT(limit + offset)` 下推，
    /// 减少网络传输；归并层不依赖分片行集有序（无序时退化为全量排序）。
    pub async fn scatter_query_rows_merged(
        &self,
        sql: &str,
        role: &str,
        agg: Option<&AggregateFunction>,
        order_by: &[OrderKey],
        limit: u64,
        offset: u64,
    ) -> Result<ScatterResult, String> {
        let mut result = self.scatter_query_rows(sql, role, agg).await?;
        if !order_by.is_empty() {
            let merged = merge_shard_rows(&result.shard_rows, order_by);
            result.merged_rows = apply_global_pagination(merged, limit, offset);
        }
        Ok(result)
    }

    /// 对 scatter 结果执行 COUNT 聚合
    pub fn aggregate_count(result: &ScatterResult) -> AggregateValue {
        let total: u64 = result.shard_row_counts.iter().map(|(_, count)| count).sum();
        AggregateValue::Count(total as i64)
    }

    /// 对 scatter 结果执行 SUM 聚合
    pub fn aggregate_sum(values: &[f64]) -> AggregateValue {
        AggregateValue::Sum(values.iter().sum())
    }

    /// 对 scatter 结果执行 AVG 聚合
    pub fn aggregate_avg(values: &[f64]) -> AggregateValue {
        if values.is_empty() {
            AggregateValue::Avg(0.0)
        } else {
            AggregateValue::Avg(values.iter().sum::<f64>() / values.len() as f64)
        }
    }

    /// 对 scatter 结果执行 MIN 聚合
    pub fn aggregate_min(values: &[f64]) -> AggregateValue {
        AggregateValue::Min(values.iter().copied().fold(f64::INFINITY, f64::min))
    }

    /// 对 scatter 结果执行 MAX 聚合
    pub fn aggregate_max(values: &[f64]) -> AggregateValue {
        AggregateValue::Max(values.iter().copied().fold(f64::NEG_INFINITY, f64::max))
    }
}

#[cfg(test)]
mod merge_tests {
    use super::*;
    use serde_json::json;

    /// 三分片交错数据归并后与全量排序结果一致（R-scatter-001）
    #[test]
    fn merge_orders_rows_globally_across_shards() {
        let shard_rows = vec![
            (
                0u32,
                vec![json!({"id": 5, "score": 90}), json!({"id": 1, "score": 70})],
            ),
            (
                1u32,
                vec![json!({"id": 3, "score": 90}), json!({"id": 2, "score": 85})],
            ),
            (
                2u32,
                vec![json!({"id": 4, "score": 60}), json!({"id": 6, "score": 90})],
            ),
        ];
        let merged = merge_shard_rows(&shard_rows, &[OrderKey::asc("score")]);
        let scores: Vec<i64> = merged
            .iter()
            .map(|r| r["score"].as_i64().unwrap())
            .collect();
        assert_eq!(scores, vec![60, 70, 85, 90, 90, 90]);
    }

    /// 稳定性：同键值行按 分片 ID 升序、分片内原序
    #[test]
    fn merge_is_stable_for_equal_keys() {
        let shard_rows = vec![
            (
                1u32,
                vec![json!({"k": 1, "tag": "b1"}), json!({"k": 1, "tag": "b2"})],
            ),
            (0u32, vec![json!({"k": 1, "tag": "a1"})]),
        ];
        let merged = merge_shard_rows(&shard_rows, &[OrderKey::asc("k")]);
        let tags: Vec<&str> = merged.iter().map(|r| r["tag"].as_str().unwrap()).collect();
        assert_eq!(tags, vec!["a1", "b1", "b2"]);
    }

    /// 降序 + 多键排序
    #[test]
    fn merge_supports_desc_and_multi_key() {
        let shard_rows = vec![
            (0u32, vec![json!({"a": 1, "b": 2}), json!({"a": 2, "b": 1})]),
            (1u32, vec![json!({"a": 1, "b": 9})]),
        ];
        let merged = merge_shard_rows(&shard_rows, &[OrderKey::asc("a"), OrderKey::desc("b")]);
        let bs: Vec<i64> = merged.iter().map(|r| r["b"].as_i64().unwrap()).collect();
        assert_eq!(bs, vec![9, 2, 1]);
    }

    /// 缺失键恒排末尾（asc 与 desc 一致）；混型数值优先
    #[test]
    fn missing_keys_sort_last_regardless_of_direction() {
        let shard_rows = vec![(
            0u32,
            vec![
                json!({"v": 10}),
                json!({"other": 1}),
                json!({"v": "text"}),
                json!({"v": 2}),
            ],
        )];
        for key in [OrderKey::asc("v"), OrderKey::desc("v")] {
            let merged = merge_shard_rows(&shard_rows, &[key]);
            let last = merged.last().unwrap();
            assert!(last.get("v").is_none(), "missing-key row must be last");
            // 数值在字符串之前（类型类序不受 desc 影响）
            let positions: Vec<usize> = merged
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    if r.get("v").is_some_and(|v| v.is_string()) {
                        i
                    } else {
                        usize::MAX
                    }
                })
                .filter(|i| *i != usize::MAX)
                .collect();
            assert_eq!(
                positions,
                vec![2],
                "string value row sits between numbers and missing"
            );
        }
    }

    /// 空排序键 = 不过排序，仅拼接（兼容入口语义）
    #[test]
    fn empty_order_by_concatenates_without_sort() {
        let shard_rows = vec![
            (0u32, vec![json!({"id": 2})]),
            (1u32, vec![json!({"id": 1})]),
        ];
        let merged = merge_shard_rows(&shard_rows, &[]);
        assert_eq!(merged.len(), 2);
    }

    /// 全局分页（R-scatter-002）：offset/limit 作用于全局行序
    #[test]
    fn global_pagination_applies_after_merge() {
        let shard_rows = vec![
            (0u32, vec![json!({"id": 1}), json!({"id": 4})]),
            (
                1u32,
                vec![json!({"id": 2}), json!({"id": 3}), json!({"id": 5})],
            ),
        ];
        let merged = merge_shard_rows(&shard_rows, &[OrderKey::asc("id")]);
        let page1 = apply_global_pagination(merged.clone(), 2, 0);
        let page2 = apply_global_pagination(merged.clone(), 2, 2);
        let page3 = apply_global_pagination(merged.clone(), 2, 4);
        let as_ids = |rows: Vec<serde_json::Value>| {
            rows.iter()
                .map(|r| r["id"].as_i64().unwrap())
                .collect::<Vec<i64>>()
        };
        assert_eq!(as_ids(page1), vec![1, 2]);
        assert_eq!(as_ids(page2), vec![3, 4]);
        assert_eq!(as_ids(page3), vec![5]);
        // 边界：offset 超界、limit=0
        assert!(apply_global_pagination(merged.clone(), 2, 99).is_empty());
        assert!(apply_global_pagination(merged, 0, 0).is_empty());
    }
}
