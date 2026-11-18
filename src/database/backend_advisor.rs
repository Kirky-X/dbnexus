// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 后端选型建议（`backend-advisor` feature）
//!
//! 开发期启发式：按查询结构特征给后端排序建议。纯函数零 IO，不做自动路由。
//! 特征提取经 `SqlParser` 确认语句可解析（fail-closed），图语句走形态文本特判。

use serde::{Deserialize, Serialize};

use crate::access::sql_parser::SqlParser;
use crate::foundation::{DatabaseType, DbError, DbResult};
use crate::i18n;

/// SQL 语句的结构化特征
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QueryFeatures {
    /// 聚合函数（COUNT/SUM/AVG/MIN/MAX）
    pub aggregation: bool,
    /// GROUP BY
    pub group_by: bool,
    /// 窗口函数 OVER(...)
    pub window_fn: bool,
    /// JOIN 出现次数
    pub join_count: usize,
    /// 子查询嵌套深度（按 SELECT 关键字层数近似）
    pub subquery_depth: usize,
    /// WITH ... AS CTE
    pub cte: bool,
    /// INSERT/UPDATE/DELETE
    pub writes: bool,
    /// 多值 INSERT（> 100 个值组，启发式）
    pub bulk_insert: bool,
    /// 单表 + 无聚合 + WHERE 等值/IN
    pub point_lookup: bool,
}

/// 单个后端建议
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackendSuggestion {
    /// 后端
    pub backend: DatabaseType,
    /// 评分 0-100
    pub score: u8,
    /// 本地化理由文本
    pub reasons: Vec<String>,
}

/// 后端建议报告
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackendAdvice {
    /// 首推
    pub primary: BackendSuggestion,
    /// 次选（按分排序，最多 2）
    pub alternatives: Vec<BackendSuggestion>,
}

/// 图查询形态检测（MATCH (…)-[…]-(…) 或 CALL {），非子串匹配
pub fn looks_like_graph_query(sql: &str) -> bool {
    let s = sql.trim();
    let upper = s.to_ascii_uppercase();
    if upper.starts_with("MATCH ") && upper.contains('(') && upper.contains(')') {
        return true;
    }
    if upper.starts_with("CALL {") {
        return true;
    }
    false
}

/// 提取查询特征（关系型语句经 SqlParser 确认可解析，失败 → Err）
pub async fn extract_features(sql: &str) -> DbResult<QueryFeatures> {
    if looks_like_graph_query(sql) {
        // 图语句不送关系型解析器；返回空特征（suggest_backend 特判处理）
        return Ok(QueryFeatures::default());
    }
    let parser = SqlParser::shared().await;
    parser
        .parse_single(sql)
        .await
        .map_err(|_| DbError::Query("unparsable SQL: no backend advice".into()))?;

    let upper = sql.to_ascii_uppercase();
    let stripped = strip_literals(&upper);
    let count_word = |w: &str| -> usize {
        stripped
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .filter(|t| *t == w)
            .count()
    };
    let aggregation = ["COUNT", "SUM", "AVG", "MIN", "MAX"]
        .iter()
        .any(|f| count_word(f) > 0);
    let group_by = stripped.contains("GROUP BY");
    let window_fn = stripped.contains("OVER (");
    let join_count = count_word("JOIN");
    let subquery_depth = count_word("SELECT").saturating_sub(1);
    let cte = stripped.contains("WITH ") && stripped.contains(" AS (");
    let writes = ["INSERT", "UPDATE", "DELETE"]
        .iter()
        .any(|w| count_word(w) > 0);
    // 多值 INSERT：VALUES 后逗号数量 > 100（每个值组至少一个逗号）
    let bulk_insert = stripped.contains("VALUES")
        && stripped
            .split("VALUES")
            .nth(1)
            .unwrap_or("")
            .matches(',')
            .count()
            > 100;
    let point_lookup = !aggregation
        && !group_by
        && join_count == 0
        && subquery_depth == 0
        && !writes
        && (stripped.contains("WHERE") && (stripped.contains('=') || stripped.contains(" IN (")));

    Ok(QueryFeatures {
        aggregation,
        group_by,
        window_fn,
        join_count,
        subquery_depth,
        cte,
        writes,
        bulk_insert,
        point_lookup,
    })
}

/// 去掉字符串字面量内容（保留引号占位），避免字面量中的关键字误判
fn strip_literals(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_quote = false;
    for c in s.chars() {
        if c == '\'' {
            in_quote = !in_quote;
            out.push('\'');
        } else if in_quote {
            out.push('x');
        } else {
            out.push(c);
        }
    }
    out
}

fn suggestion(backend: DatabaseType, score: u8, reason_key: &str) -> BackendSuggestion {
    BackendSuggestion {
        backend,
        score,
        reasons: vec![
            i18n::t_simple(reason_key),
            i18n::t_simple("advice-verify-scale"),
        ],
    }
}

/// 后端建议（确定性规则表，零 IO）
pub async fn suggest_backend(sql: &str) -> DbResult<BackendAdvice> {
    if looks_like_graph_query(sql) {
        return Ok(BackendAdvice {
            primary: suggestion(DatabaseType::Ladybug, 100, "advice-reason-graph-traversal"),
            alternatives: vec![suggestion(
                DatabaseType::Neo4j,
                80,
                "advice-reason-graph-traversal",
            )],
        });
    }
    let f = extract_features(sql).await?;

    let advice = if f.aggregation || f.group_by || f.window_fn {
        BackendAdvice {
            primary: suggestion(DatabaseType::DuckDb, 100, "advice-reason-olap-aggregation"),
            alternatives: vec![
                suggestion(
                    DatabaseType::Postgres,
                    70,
                    "advice-reason-default-relational",
                ),
                suggestion(DatabaseType::MySql, 60, "advice-reason-mysql-peer"),
            ],
        }
    } else if f.writes && (f.join_count > 0 || f.cte || f.subquery_depth > 0 || f.bulk_insert) {
        BackendAdvice {
            primary: suggestion(
                DatabaseType::Postgres,
                100,
                "advice-reason-transactional-server",
            ),
            alternatives: vec![suggestion(
                DatabaseType::MySql,
                70,
                "advice-reason-mysql-peer",
            )],
        }
    } else if f.bulk_insert {
        BackendAdvice {
            primary: suggestion(DatabaseType::DuckDb, 100, "advice-reason-bulk-load"),
            alternatives: vec![suggestion(
                DatabaseType::Postgres,
                60,
                "advice-reason-mysql-peer",
            )],
        }
    } else if f.point_lookup {
        BackendAdvice {
            primary: suggestion(
                DatabaseType::Sqlite,
                100,
                "advice-reason-embedded-point-lookup",
            ),
            alternatives: vec![suggestion(
                DatabaseType::Postgres,
                65,
                "advice-reason-mysql-peer",
            )],
        }
    } else if f.writes {
        BackendAdvice {
            primary: suggestion(
                DatabaseType::Postgres,
                100,
                "advice-reason-transactional-server",
            ),
            alternatives: vec![suggestion(
                DatabaseType::MySql,
                70,
                "advice-reason-mysql-peer",
            )],
        }
    } else {
        BackendAdvice {
            primary: suggestion(
                DatabaseType::Postgres,
                100,
                "advice-reason-default-relational",
            ),
            alternatives: vec![suggestion(
                DatabaseType::Sqlite,
                70,
                "advice-reason-embedded-point-lookup",
            )],
        }
    };
    Ok(advice)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_extract_features() {
        let cases = [
            ("SELECT COUNT(*) FROM users", true, false, false, 0, 0),
            (
                "SELECT * FROM users u JOIN orders o ON u.id=o.uid",
                false,
                false,
                false,
                1,
                0,
            ),
            (
                "SELECT * FROM (SELECT * FROM users) t",
                false,
                false,
                false,
                0,
                1,
            ),
            ("INSERT INTO users VALUES (1)", false, false, true, 0, 0),
        ];
        for (sql, agg, group, writes, joins, depth) in cases {
            let f = extract_features(sql).await.unwrap();
            assert_eq!(f.aggregation, agg, "sql: {sql}");
            assert_eq!(f.group_by, group, "sql: {sql}");
            assert_eq!(f.writes, writes, "sql: {sql}");
            assert_eq!(f.join_count, joins, "sql: {sql}");
            assert_eq!(f.subquery_depth, depth, "sql: {sql}");
        }
        let window = extract_features("SELECT SUM(x) OVER (PARTITION BY y) FROM t")
            .await
            .unwrap();
        assert!(window.window_fn && window.aggregation);
        let cte = extract_features("WITH t AS (SELECT 1) SELECT * FROM t")
            .await
            .unwrap();
        assert!(cte.cte);
        assert!(extract_features("NOT SQL !!!").await.is_err());
    }

    #[tokio::test]
    async fn test_suggest_backend_rules() {
        let olap =
            suggest_backend("SELECT department, COUNT(*) FROM employees GROUP BY department")
                .await
                .unwrap();
        assert_eq!(olap.primary.backend, DatabaseType::DuckDb);

        let point = suggest_backend("SELECT * FROM users WHERE id = 42")
            .await
            .unwrap();
        assert_eq!(point.primary.backend, DatabaseType::Sqlite);

        let txn = suggest_backend("UPDATE users SET name='x' WHERE id=1")
            .await
            .unwrap();
        assert_eq!(txn.primary.backend, DatabaseType::Postgres);

        let graph = suggest_backend("MATCH (a:User)-[:FRIEND]->(b:User) RETURN a")
            .await
            .unwrap();
        assert_eq!(graph.primary.backend, DatabaseType::Ladybug);
        assert_eq!(graph.alternatives[0].backend, DatabaseType::Neo4j);

        // 含 MATCH 子串的关系型 SQL 不误判
        let not_graph = suggest_backend("SELECT * FROM matches WHERE id=1")
            .await
            .unwrap();
        assert_ne!(not_graph.primary.backend, DatabaseType::Ladybug);
    }

    #[test]
    fn test_i18n_keys_resolve() {
        for key in [
            "advice-reason-graph-traversal",
            "advice-reason-olap-aggregation",
            "advice-reason-transactional-server",
            "advice-reason-bulk-load",
            "advice-reason-embedded-point-lookup",
            "advice-reason-default-relational",
            "advice-reason-mysql-peer",
            "advice-verify-scale",
        ] {
            assert_ne!(i18n::t_simple(key), key, "key: {key}");
        }
    }
}
