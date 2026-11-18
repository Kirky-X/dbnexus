// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 权限策略干跑评估（纯函数，不建池/不执行 SQL/不换装）
//!
//! 给定基线策略、候选策略、角色与一组 SQL，逐条判定表级 RBAC 结论变化。
//! 不触碰 `PermissionCache`/`PolicyDecisionPoint` 运行时。

use serde::{Deserialize, Serialize};

use super::permission::{PermissionAction, PermissionConfig, RolePolicy};
use crate::access::sql_parser::{SqlOperationType, SqlParser};
use crate::i18n;

/// 单条 SQL 的干跑结论
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeKind {
    /// 基线允许 → 候选允许
    AllowToAllow,
    /// 基线允许 → 候选拒绝（回归）
    AllowToDeny,
    /// 基线拒绝 → 候选允许（新增授权）
    DenyToAllow,
    /// 两者都拒绝
    DenyToDeny,
    /// 解析失败/不支持语句/无表名
    Unparsable,
}

/// 单条 SQL 的干跑结论详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryOutcome {
    /// 输入文件行号（从 1 起）
    pub line: usize,
    /// 结论分类
    pub kind: OutcomeKind,
    /// 涉及的表名（解析成功时）
    pub tables: Vec<String>,
    /// AllowToDeny 时的缺失授权建议
    pub suggestion: Option<String>,
}

/// 汇总计数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DryRunSummary {
    /// 查询总数
    pub total: usize,
    /// 允许变拒绝的回归数
    pub allow_to_deny: usize,
    /// 拒绝变允许的新增授权数
    pub deny_to_allow: usize,
    /// 无法解析的查询数
    pub unparsable: usize,
}

/// 干跑报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DryRunReport {
    /// 评估角色
    pub role: String,
    /// 逐条查询结论
    pub per_query: Vec<QueryOutcome>,
    /// 汇总计数
    pub summary: DryRunSummary,
}

/// 单策略表级判定（角色不存在 → 拒绝；通配符语义透传 RolePolicy::allows）
fn policy_allows(
    config: &PermissionConfig,
    role: &str,
    table: &str,
    action: &PermissionAction,
) -> bool {
    config
        .roles
        .get(role)
        .is_some_and(|policy: &RolePolicy| policy.allows(table, action))
}

/// 干跑评估（async 仅因 SqlParser::shared() 单例获取）
pub async fn dry_run_policies(
    baseline: &PermissionConfig,
    candidate: &PermissionConfig,
    role: &str,
    queries: &[String],
) -> DryRunReport {
    let parser = SqlParser::shared().await;
    let mut per_query = Vec::with_capacity(queries.len());

    for (idx, sql) in queries.iter().enumerate() {
        let line = idx + 1;
        let parsed = parser.parse_single(sql).await.ok();
        let Some(parsed) = parsed else {
            per_query.push(QueryOutcome {
                line,
                kind: OutcomeKind::Unparsable,
                tables: vec![],
                suggestion: None,
            });
            continue;
        };
        let action = match parsed.operation_type {
            SqlOperationType::Select => PermissionAction::Select,
            SqlOperationType::Insert => PermissionAction::Insert,
            SqlOperationType::Update => PermissionAction::Update,
            SqlOperationType::Delete => PermissionAction::Delete,
            _ => {
                per_query.push(QueryOutcome {
                    line,
                    kind: OutcomeKind::Unparsable,
                    tables: vec![],
                    suggestion: None,
                });
                continue;
            }
        };
        if parsed.all_table_names.is_empty() {
            per_query.push(QueryOutcome {
                line,
                kind: OutcomeKind::Unparsable,
                tables: vec![],
                suggestion: None,
            });
            continue;
        }

        let baseline_ok = parsed
            .all_table_names
            .iter()
            .all(|t| policy_allows(baseline, role, t, &action));
        let candidate_ok = parsed
            .all_table_names
            .iter()
            .all(|t| policy_allows(candidate, role, t, &action));

        let (kind, suggestion) = match (baseline_ok, candidate_ok) {
            (true, true) => (OutcomeKind::AllowToAllow, None),
            (true, false) => {
                let role_undefined = !candidate.roles.contains_key(role);
                let missing: Vec<String> = parsed
                    .all_table_names
                    .iter()
                    .filter(|t| !policy_allows(candidate, role, t, &action))
                    .map(|t| format!("{t}:{action}"))
                    .collect();
                let suggestion = if role_undefined {
                    Some(i18n::t(
                        "dry-run-role-not-defined",
                        &[("role", role.to_string())],
                    ))
                } else {
                    Some(i18n::t(
                        "dry-run-missing-grant",
                        &[
                            ("table", missing.join(", ")),
                            ("action", action.to_string()),
                            ("role", role.to_string()),
                        ],
                    ))
                };
                (OutcomeKind::AllowToDeny, suggestion)
            }
            (false, true) => (OutcomeKind::DenyToAllow, None),
            (false, false) => (OutcomeKind::DenyToDeny, None),
        };

        per_query.push(QueryOutcome {
            line,
            kind,
            tables: parsed.all_table_names,
            suggestion,
        });
    }

    let allow_to_deny = per_query
        .iter()
        .filter(|o| o.kind == OutcomeKind::AllowToDeny)
        .count();
    let deny_to_allow = per_query
        .iter()
        .filter(|o| o.kind == OutcomeKind::DenyToAllow)
        .count();
    let unparsable = per_query
        .iter()
        .filter(|o| o.kind == OutcomeKind::Unparsable)
        .count();
    let total = per_query.len();

    DryRunReport {
        role: role.to_string(),
        per_query,
        summary: DryRunSummary {
            total,
            allow_to_deny,
            deny_to_allow,
            unparsable,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn config(role: &str, tables: &[(&str, PermissionAction)]) -> PermissionConfig {
        let mut roles = HashMap::new();
        roles.insert(
            role.to_string(),
            RolePolicy {
                tables: tables
                    .iter()
                    .map(|(t, a)| super::super::permission::TablePermission {
                        name: (*t).to_string(),
                        operations: vec![a.clone()],
                    })
                    .collect(),
                origin_constraints: vec![],
            },
        );
        PermissionConfig { roles }
    }

    #[tokio::test]
    async fn test_four_classifications_and_unparsable() {
        let baseline = config(
            "r",
            &[
                ("users", PermissionAction::Select),
                ("orders", PermissionAction::Select),
            ],
        );
        let candidate = config("r", &[("users", PermissionAction::Select)]);
        let queries = vec![
            "SELECT * FROM users".to_string(),  // allow->allow
            "SELECT * FROM orders".to_string(), // allow->deny
            "DROP TABLE users".to_string(),     // unparsable
            "SELECT 1".to_string(),             // unparsable (no table)
        ];
        let report = dry_run_policies(&baseline, &candidate, "r", &queries).await;
        assert_eq!(report.summary.total, 4);
        assert_eq!(report.summary.allow_to_deny, 1);
        assert_eq!(report.summary.unparsable, 2);
        assert_eq!(report.per_query[0].kind, OutcomeKind::AllowToAllow);
        assert_eq!(report.per_query[1].kind, OutcomeKind::AllowToDeny);
        assert!(
            report.per_query[1]
                .suggestion
                .as_ref()
                .unwrap()
                .contains("orders")
        );
    }

    #[tokio::test]
    async fn test_wildcard_and_join_partial_denial() {
        let baseline = config("r", &[("*", PermissionAction::Select)]);
        let candidate = config("r", &[("users", PermissionAction::Select)]);
        let report = dry_run_policies(
            &baseline,
            &candidate,
            "r",
            &["SELECT * FROM users JOIN orders ON users.id = orders.id".to_string()],
        )
        .await;
        assert_eq!(report.per_query[0].kind, OutcomeKind::AllowToDeny);
        assert_eq!(report.per_query[0].tables.len(), 2);
        assert!(
            report.per_query[0]
                .suggestion
                .as_ref()
                .unwrap()
                .contains("orders")
        );
    }

    #[tokio::test]
    async fn test_role_not_defined() {
        let baseline = config("r", &[("users", PermissionAction::Select)]);
        let candidate = PermissionConfig::default();
        let report = dry_run_policies(
            &baseline,
            &candidate,
            "r",
            &["SELECT * FROM users".to_string()],
        )
        .await;
        assert_eq!(report.per_query[0].kind, OutcomeKind::AllowToDeny);
        assert!(
            report.per_query[0]
                .suggestion
                .as_ref()
                .unwrap()
                .contains("r")
        );
    }

    #[test]
    fn test_outcome_kind_serde() {
        let json = serde_json::to_string(&OutcomeKind::AllowToDeny).unwrap();
        assert_eq!(json, "\"allow_to_deny\"");
    }

    #[test]
    fn test_i18n_keys_resolve() {
        assert_ne!(
            i18n::t(
                "dry-run-missing-grant",
                &[
                    ("table", "t".into()),
                    ("action", "Select".into()),
                    ("role", "r".into())
                ]
            ),
            "dry-run-missing-grant"
        );
        assert_ne!(
            i18n::t("dry-run-role-not-defined", &[("role", "r".into())]),
            "dry-run-role-not-defined"
        );
    }
}
