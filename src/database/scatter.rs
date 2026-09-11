// Copyright (c) 2026 Kirky.X
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
    /// T401：对跨分片行数据计算聚合值（修复此前 aggregated 恒为 None 的缺陷）
    ///
    /// 数值列从 JSON 行中提取（`row[col]` 为 Number 才计入）；
    /// COUNT 基于行数，AVG 在无数值样本时返回 None，其余空数据返回中性值。
    pub fn compute_from_rows(
        &self,
        shard_rows: &[(u32, Vec<serde_json::Value>)],
    ) -> Option<AggregateValue> {
        let all_rows = || {
            shard_rows
                .iter()
                .flat_map(|(_, rows)| rows.iter())
        };
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

/// T401：从 JSON 行提取数值列
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
    /// T401：各分片返回的真实数据行 (shard_id, rows)——取回的数据行而非仅行数
    pub shard_rows: Vec<(u32, Vec<serde_json::Value>)>,
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

    /// T401：scatter-gather 行查询——取回各分片真实数据行并可选聚合
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
        })
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
