// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 查询计划回归防护（`query-guard` feature）
//!
//! 工具面：批量取 EXPLAIN 信号，与基线 JSON 比对，报告计划退化。
//! 不进 Session 执行管道，不逐查询做运行时 EXPLAIN。

use std::collections::BTreeMap;

use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::foundation::{DatabaseType, DbError, DbResult};
use crate::i18n;

/// 单条查询的计划信号（方言差异吸收后的统一形状）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanSignal {
    /// 存在全表扫描节点
    pub full_scan: bool,
    /// 优化器预估行数（后端缺失时 None）
    pub est_rows: Option<u64>,
    /// 存在索引使用节点
    pub uses_index: bool,
}

fn default_growth() -> u32 {
    300
}

fn default_true() -> bool {
    true
}

/// 比对规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardRules {
    /// 预估行数增长百分比阈值（默认 300 = 4 倍）
    #[serde(default = "default_growth")]
    pub est_rows_growth_pct: u32,
    /// 基线无全表扫描而当前出现 → 回归
    #[serde(default = "default_true")]
    pub forbid_new_full_scan: bool,
}

impl Default for GuardRules {
    fn default() -> Self {
        Self {
            est_rows_growth_pct: default_growth(),
            forbid_new_full_scan: default_true(),
        }
    }
}

/// 回归类别
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegressionKind {
    /// 基线无全表扫描，当前出现
    NewFullScan,
    /// 预估行数增长超阈值
    EstRowsGrowth {
        /// 基线预估行数
        from: u64,
        /// 当前预估行数
        to: u64,
    },
}

/// 单条回归记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Regression {
    /// 查询指纹
    pub fingerprint: String,
    /// SQL 前 80 字符（不携带全文）
    pub sql_excerpt: String,
    /// 回归类别
    pub kind: RegressionKind,
}

/// 查询指纹：字面量归一化为 `?` 后 SHA-256 十六进制
pub fn fingerprint(sql: &str) -> String {
    let normalized = normalize_sql(sql);
    let digest = Sha256::digest(normalized.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// 字面量与空白归一化（保留结构，大小写折叠）
fn normalize_sql(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let chars: Vec<char> = sql.trim().chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            // 完全去掉空白：结构由关键字与标点表达，空格差异不影响指纹
            i += 1;
        } else if c == '\'' {
            // 跳过单引号字符串字面量（含 '' 转义）
            out.push('?');
            i += 1;
            while i < chars.len() {
                if chars[i] == '\'' {
                    if i + 1 < chars.len() && chars[i + 1] == '\'' {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
        } else if c.is_ascii_digit() {
            if !out.ends_with('?') {
                out.push('?');
            }
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
        } else {
            out.extend(c.to_lowercase());
            i += 1;
        }
    }
    out
}

/// 方言 EXPLAIN 输出 → PlanSignal（纯函数）
pub fn parse_plan_signal(backend: DatabaseType, plan: &serde_json::Value) -> DbResult<PlanSignal> {
    match backend {
        DatabaseType::Sqlite => parse_sqlite_plan(plan),
        DatabaseType::Postgres => parse_postgres_plan(plan),
        DatabaseType::MySql => parse_mysql_plan(plan),
        DatabaseType::DuckDb => parse_duckdb_plan(plan),
        DatabaseType::Ladybug | DatabaseType::Neo4j => Err(DbError::Unsupported(i18n::t_simple(
            "query-guard-unsupported-backend",
        ))),
    }
}

fn parse_sqlite_plan(plan: &serde_json::Value) -> DbResult<PlanSignal> {
    // 输入：{ "detail": "SCAN t" } 或 { "detail": "SEARCH t USING INDEX idx" }
    let detail = plan.get("detail").and_then(|v| v.as_str()).ok_or_else(|| {
        DbError::Config(crate::i18n::tr("query-guard-sqlite-plan-missing-detail"))
    })?;
    let uses_index = detail.contains("USING INDEX");
    let full_scan = detail.contains("SCAN") && !uses_index;
    Ok(PlanSignal {
        full_scan,
        est_rows: None,
        uses_index,
    })
}

fn parse_postgres_plan(plan: &serde_json::Value) -> DbResult<PlanSignal> {
    // PG FORMAT JSON 返回 [{ "Plan": { ... } }]
    let root = plan.get(0).and_then(|v| v.get("Plan")).unwrap_or(plan);
    let mut full_scan = false;
    let mut uses_index = false;
    let mut est_rows = None;
    fn walk(node: &serde_json::Value, fs: &mut bool, idx: &mut bool, rows: &mut Option<u64>) {
        if let Some(kind) = node.get("Node Type").and_then(|v| v.as_str()) {
            if kind == "Seq Scan" {
                *fs = true;
            } else if kind.contains("Index Scan") {
                *idx = true;
            }
        }
        if let None = rows
            && let Some(r) = node.get("Plan Rows").and_then(|v| v.as_u64())
        {
            *rows = Some(r);
        }
        if let Some(children) = node.get("Plans").and_then(|v| v.as_array()) {
            for c in children {
                walk(c, fs, idx, rows);
            }
        }
    }
    walk(root, &mut full_scan, &mut uses_index, &mut est_rows);
    // 若存在索引扫描节点，不把 Seq Scan 视为整条查询退化（子树可能是小表）
    if uses_index {
        full_scan = false;
    }
    Ok(PlanSignal {
        full_scan,
        est_rows,
        uses_index,
    })
}

fn parse_mysql_plan(plan: &serde_json::Value) -> DbResult<PlanSignal> {
    // 列式行：{ "type": "ALL", "key": null, "rows": 10 }
    let typ = plan.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let key_present = plan
        .get("key")
        .and_then(|v| v.as_str())
        .is_some_and(|s| !s.is_empty());
    let full_scan = typ == "ALL";
    let uses_index = key_present && matches!(typ, "ref" | "eq_ref" | "range" | "index" | "const");
    let est_rows = plan.get("rows").and_then(|v| v.as_u64()).or_else(|| {
        plan.get("rows")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
    });
    Ok(PlanSignal {
        full_scan,
        est_rows,
        uses_index,
    })
}

fn parse_duckdb_plan(plan: &serde_json::Value) -> DbResult<PlanSignal> {
    let text = plan.get("plan").and_then(|v| v.as_str()).unwrap_or("");
    let full_scan = text.contains("SEQ_SCAN");
    let uses_index = text.contains("INDEX_SCAN");
    Ok(PlanSignal {
        full_scan,
        est_rows: None,
        uses_index,
    })
}

/// 基线比对（新查询无对照物，不判回归）
pub fn compare(
    baseline: &BTreeMap<String, PlanSignal>,
    current: &BTreeMap<String, PlanSignal>,
    rules: &GuardRules,
) -> Vec<Regression> {
    let mut out = Vec::new();
    for (fp, now) in current {
        let Some(before) = baseline.get(fp) else {
            continue;
        };
        if rules.forbid_new_full_scan && !before.full_scan && now.full_scan {
            out.push(Regression {
                fingerprint: fp.clone(),
                sql_excerpt: String::new(),
                kind: RegressionKind::NewFullScan,
            });
        }
        if let (Some(from), Some(to)) = (before.est_rows, now.est_rows) {
            let growth_pct = if from == 0 {
                if to > 0 { u32::MAX } else { 0 }
            } else {
                ((to.saturating_sub(from)) as f64 / from as f64 * 100.0) as u32
            };
            if growth_pct > rules.est_rows_growth_pct {
                out.push(Regression {
                    fingerprint: fp.clone(),
                    sql_excerpt: String::new(),
                    kind: RegressionKind::EstRowsGrowth { from, to },
                });
            }
        }
    }
    out
}

/// 批量执行 EXPLAIN 并收集信号（工具面原始查询，不经权限管道）
pub async fn collect_signals(
    conn: &DatabaseConnection,
    backend: DatabaseType,
    queries: &[String],
) -> DbResult<BTreeMap<String, PlanSignal>> {
    if matches!(backend, DatabaseType::Ladybug | DatabaseType::Neo4j) {
        return Err(DbError::Unsupported(i18n::t_simple(
            "query-guard-unsupported-backend",
        )));
    }
    let mut out = BTreeMap::new();
    for sql in queries {
        let explain = match backend {
            DatabaseType::Sqlite => format!("EXPLAIN QUERY PLAN {sql}"),
            DatabaseType::Postgres => format!("EXPLAIN (FORMAT JSON) {sql}"),
            DatabaseType::MySql | DatabaseType::DuckDb => format!("EXPLAIN {sql}"),
            DatabaseType::Ladybug | DatabaseType::Neo4j => unreachable!(),
        };
        let stmt = Statement::from_string(conn.get_database_backend(), explain);
        let rows = conn
            .query_all_raw(stmt)
            .await
            .map_err(DbError::Connection)?;
        let plan = rows
            .first()
            .ok_or_else(|| DbError::Query("empty EXPLAIN result".into()))?;
        let value = plan_value(plan);
        let signal = parse_plan_signal(backend, &value)?;
        out.insert(fingerprint(sql), signal);
    }
    Ok(out)
}

fn plan_value(row: &sea_orm::QueryResult) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for name in row.column_names() {
        let v = if let Ok(Some(x)) = row.try_get::<Option<String>>("", name.as_str()) {
            serde_json::json!(x)
        } else if let Ok(Some(x)) = row.try_get::<Option<i64>>("", name.as_str()) {
            serde_json::json!(x)
        } else {
            serde_json::Value::Null
        };
        map.insert(name, v);
    }
    serde_json::Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fingerprint_normalization() {
        let a = fingerprint("SELECT * FROM users WHERE id = 1");
        let b = fingerprint("select  *  from users where id=2");
        assert_eq!(a, b);
        let c = fingerprint("SELECT * FROM orders WHERE id = 1");
        assert_ne!(a, c);
    }

    #[test]
    fn test_parse_sqlite_plan() {
        for (detail, fs, idx) in [
            ("SCAN users", true, false),
            ("SEARCH users USING INDEX idx_id (id=?)", false, true),
            ("SCAN users\nSEARCH orders USING INDEX idx", false, true),
        ] {
            let signal =
                parse_plan_signal(DatabaseType::Sqlite, &serde_json::json!({"detail": detail}))
                    .unwrap();
            assert_eq!(signal.full_scan, fs, "detail: {detail}");
            assert_eq!(signal.uses_index, idx, "detail: {detail}");
            assert_eq!(signal.est_rows, None);
        }
    }

    #[test]
    fn test_parse_postgres_json_plan() {
        let plan = serde_json::json!([{"Plan": {
            "Node Type": "Seq Scan", "Plan Rows": 1000,
            "Plans": [{"Node Type": "Index Scan", "Plan Rows": 10}]
        }}]);
        let signal = parse_plan_signal(DatabaseType::Postgres, &plan).unwrap();
        assert!(!signal.full_scan); // 有索引子树 → 不视为退化
        assert!(signal.uses_index);
        assert_eq!(signal.est_rows, Some(1000));
    }

    #[test]
    fn test_parse_mysql_plan() {
        let plan = serde_json::json!({"type": "ALL", "key": null, "rows": 10});
        let signal = parse_plan_signal(DatabaseType::MySql, &plan).unwrap();
        assert!(signal.full_scan);
        assert!(!signal.uses_index);
        assert_eq!(signal.est_rows, Some(10));
    }

    #[test]
    fn test_compare_rules() {
        let mut baseline = BTreeMap::new();
        baseline.insert(
            "fp1".into(),
            PlanSignal {
                full_scan: false,
                est_rows: Some(100),
                uses_index: true,
            },
        );
        let mut current = BTreeMap::new();
        current.insert(
            "fp1".into(),
            PlanSignal {
                full_scan: true,
                est_rows: Some(400),
                uses_index: false,
            },
        );
        current.insert(
            "fp2".into(),
            PlanSignal {
                full_scan: true,
                est_rows: None,
                uses_index: false,
            },
        );
        let regressions = compare(&baseline, &current, &GuardRules::default());
        assert_eq!(regressions.len(), 1); // EstRowsGrowth 恰 300% 不触发；NewFullScan 触发
        assert_eq!(regressions[0].kind, RegressionKind::NewFullScan);
    }
}
