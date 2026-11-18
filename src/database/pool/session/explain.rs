// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 权限检查路径预演（不执行 SQL）
//!
//! 复用执行管道的解析/逐表 RBAC/RLS 判定件，返回结构化结论；
//! 不触驱动、不开事务。供 MCP `dbnexus_explain` 与未来调试面共用。

use serde::{Deserialize, Serialize};

use super::Session;
use crate::foundation::DbResult;

/// 权限检查路径预演结论
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessExplanation {
    /// 是否允许执行
    pub allowed: bool,
    /// 已检查的表名清单（JOIN/子查询逐表）
    pub tables_checked: Vec<String>,
    /// 结构化拒绝原因码（来自 DenialReason::reason_code）
    pub denial_reason: Option<String>,
    /// RLS 谓词是否可注入（策略命中且列白名单通过）
    pub rls_applied: bool,
    /// RLS 是否走降级路径（主表注入，其余表由 RBAC 兜底）
    pub rls_degraded: bool,
}

impl Session {
    /// 预演 SQL 的权限检查路径（admin 角色恒放行，与执行管道口径一致）
    #[cfg(all(feature = "permission", feature = "sql-parser"))]
    pub async fn explain_access(&self, sql: &str) -> DbResult<AccessExplanation> {
        use crate::access::SqlParser;
        #[cfg(feature = "data-protection")]
        use crate::access::data_protection::RlsDecision;
        use crate::access::sql_parser::SqlOperationType;
        use crate::foundation::DenialReason;

        let mut explanation = AccessExplanation {
            allowed: true,
            tables_checked: Vec::new(),
            denial_reason: None,
            rls_applied: false,
            rls_degraded: false,
        };

        if self.is_admin {
            return Ok(explanation);
        }

        let parser = SqlParser::shared().await;
        let parsed = match parser.parse_single(sql).await {
            Ok(parsed) => parsed,
            Err(_) => {
                explanation.allowed = false;
                explanation.denial_reason = Some(DenialReason::SqlParseFailed.reason_code().into());
                return Ok(explanation);
            }
        };
        explanation.tables_checked = parsed.all_table_names.clone();

        // 逐表 RBAC 检查（只判定，不产生审计副作用）
        let action = match parsed.operation_type {
            SqlOperationType::Select => crate::access::PermissionAction::Select,
            SqlOperationType::Insert => crate::access::PermissionAction::Insert,
            SqlOperationType::Update => crate::access::PermissionAction::Update,
            SqlOperationType::Delete => crate::access::PermissionAction::Delete,
            _ => {
                explanation.allowed = false;
                explanation.denial_reason =
                    Some(DenialReason::UnsupportedStatement.reason_code().into());
                return Ok(explanation);
            }
        };
        for table in &parsed.all_table_names {
            let decision = self
                .permission_ctx
                .check_table_access_decision(table, &action)
                .await;
            match decision {
                crate::access::TableAccessDecision::Allowed => {}
                crate::access::TableAccessDecision::Denied => {
                    explanation.allowed = false;
                    let reason = if self.permission_ctx.has_role_policy(self.role()).await {
                        DenialReason::TableNotInPolicy {
                            table: table.clone(),
                            action: action.to_string(),
                        }
                    } else {
                        DenialReason::RoleNotDefined {
                            role: self.role().to_string(),
                        }
                    };
                    explanation.denial_reason = Some(reason.reason_code().into());
                    return Ok(explanation);
                }
                crate::access::TableAccessDecision::RateLimited { .. } => {
                    explanation.allowed = false;
                    explanation.denial_reason = Some("RATE_LIMITED".into());
                    return Ok(explanation);
                }
            }
        }

        // RLS 可注入性判定（纯判定；data-protection 未启用时跳过）
        #[cfg(feature = "data-protection")]
        {
            let dp = { self.pool_inner.data_protection.read().await.clone() };
            if let Some(rls) = dp.rls.as_ref()
                && !rls.is_empty()
            {
                let in_scope = matches!(
                    parsed.operation_type,
                    SqlOperationType::Select | SqlOperationType::Update | SqlOperationType::Delete
                );
                let decision = rls.decide(
                    &parsed.all_table_names,
                    in_scope,
                    sql,
                    super::execute::sql_dialect(self.connection()?.get_database_backend()),
                );
                match decision {
                    RlsDecision::Inject { .. } => explanation.rls_applied = true,
                    RlsDecision::InjectDegraded { .. } => {
                        explanation.rls_applied = true;
                        explanation.rls_degraded = true;
                    }
                    RlsDecision::Reject { reason } => {
                        explanation.allowed = false;
                        explanation.denial_reason = Some(match reason {
                            crate::access::data_protection::RlsRejection::MultiTableUnsupported { .. }
                            | crate::access::data_protection::RlsRejection::SetOperationUnsupported => {
                                DenialReason::RlsMultiTableUnsupported { tables: vec![] }
                                    .reason_code()
                                    .into()
                            }
                            crate::access::data_protection::RlsRejection::UnsafePolicyColumn { .. } => {
                                DenialReason::RlsPredicateRejected { table: String::new() }
                                    .reason_code()
                                    .into()
                            }
                            crate::access::data_protection::RlsRejection::ParseFailed => {
                                DenialReason::RlsParseFailed.reason_code().into()
                            }
                        });
                        return Ok(explanation);
                    }
                    RlsDecision::Passthrough | RlsDecision::OutOfScope => {}
                }
            }
        }

        Ok(explanation)
    }
}

#[cfg(all(test, feature = "permission", feature = "sql-parser"))]
mod tests {
    use super::*;

    #[test]
    fn access_explanation_serializes_fields() {
        let e = AccessExplanation {
            allowed: true,
            tables_checked: vec!["orders".into()],
            denial_reason: None,
            rls_applied: false,
            rls_degraded: false,
        };
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["allowed"], true);
        assert_eq!(json["tables_checked"][0], "orders");
        assert_eq!(json["denial_reason"], serde_json::Value::Null);
    }
}
