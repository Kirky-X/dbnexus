// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 安全姿态自审计
//!
//! 以编译期 feature 状态聚合当前构建的安全能力，输出机器可读的姿态报告。
//! 纯函数，无 IO/DB 依赖；`cfg!` 在编译期求值，恒编译无 feature 门控。

use serde::{Deserialize, Serialize};

/// 安全姿态档案：development 只报告；production 对 Required 项强制判定
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PostureProfile {
    /// 只报告，不判定达标
    #[default]
    Development,
    /// 对 Required 项强制判定
    Production,
}

/// 单项检查的判定级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PostureSeverity {
    /// production 档必须启用
    Required,
    /// 建议启用；缺失时 production 降级为 Degraded
    Recommended,
    /// 仅信息项
    Info,
}

/// 单项检查结论
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostureCheck {
    /// 稳定检查 ID
    pub id: &'static str,
    /// 承载 feature 名
    pub feature: &'static str,
    /// 该 feature 是否编译进当前构建
    pub enabled: bool,
    /// production 档下的判定级别
    pub severity: PostureSeverity,
    /// 缺失影响的 i18n 键
    pub effect_key: &'static str,
}

/// 安全姿态报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPosture {
    /// 判定档案
    pub profile: PostureProfile,
    /// Compliant | Degraded | NonCompliant
    pub status: &'static str,
    /// 检查项清单
    pub checks: Vec<PostureCheck>,
}

/// 聚合当前构建的安全姿态（纯函数，无 IO/DB）
pub fn security_posture(profile: PostureProfile) -> SecurityPosture {
    let checks = vec![
        PostureCheck {
            id: "rbac",
            feature: "permission",
            enabled: cfg!(feature = "permission"),
            severity: PostureSeverity::Required,
            effect_key: "posture-effect-rbac",
        },
        PostureCheck {
            id: "injection-detection",
            feature: "sql-parser",
            enabled: cfg!(feature = "sql-parser"),
            severity: PostureSeverity::Required,
            effect_key: "posture-effect-injection-detection",
        },
        PostureCheck {
            id: "policy-cache",
            feature: "cache",
            enabled: cfg!(feature = "cache"),
            severity: PostureSeverity::Recommended,
            effect_key: "posture-effect-policy-cache",
        },
        PostureCheck {
            id: "rls-and-masking",
            feature: "data-protection",
            enabled: cfg!(feature = "data-protection"),
            severity: PostureSeverity::Recommended,
            effect_key: "posture-effect-rls-and-masking",
        },
        PostureCheck {
            id: "pdp",
            feature: "permission-engine",
            enabled: cfg!(feature = "permission-engine"),
            severity: PostureSeverity::Info,
            effect_key: "posture-effect-pdp",
        },
        PostureCheck {
            id: "unified-facade",
            feature: "permission-facade",
            enabled: cfg!(feature = "permission-facade"),
            severity: PostureSeverity::Info,
            effect_key: "posture-effect-unified-facade",
        },
        PostureCheck {
            id: "audit",
            feature: "audit",
            enabled: cfg!(feature = "audit"),
            severity: PostureSeverity::Recommended,
            effect_key: "posture-effect-audit",
        },
        PostureCheck {
            id: "jwt-auth",
            feature: "authentication",
            enabled: cfg!(feature = "authentication"),
            severity: PostureSeverity::Recommended,
            effect_key: "posture-effect-jwt-auth",
        },
        PostureCheck {
            id: "drivers",
            feature: "sqlite|duckdb|postgres|mysql",
            enabled: cfg!(any(
                feature = "sqlite",
                feature = "duckdb",
                feature = "postgres",
                feature = "mysql"
            )),
            severity: PostureSeverity::Info,
            effect_key: "posture-effect-drivers",
        },
    ];

    let status = match profile {
        PostureProfile::Development => "Compliant",
        PostureProfile::Production => {
            let missing_required = checks
                .iter()
                .any(|c| c.severity == PostureSeverity::Required && !c.enabled);
            let missing_recommended = checks
                .iter()
                .any(|c| c.severity == PostureSeverity::Recommended && !c.enabled);
            if missing_required {
                "NonCompliant"
            } else if missing_recommended {
                "Degraded"
            } else {
                "Compliant"
            }
        }
    };

    SecurityPosture {
        profile,
        status,
        checks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn development_is_always_compliant() {
        for p in [PostureProfile::Development, PostureProfile::Production] {
            let posture = security_posture(p);
            assert_eq!(posture.checks.len(), 9);
            assert!(!posture.status.is_empty());
        }
        assert_eq!(
            security_posture(PostureProfile::Development).status,
            "Compliant"
        );
    }

    #[test]
    fn check_enabled_matches_cfg() {
        let posture = security_posture(PostureProfile::Development);
        let by_id = |id: &str| posture.checks.iter().find(|c| c.id == id).unwrap();
        assert_eq!(by_id("rbac").enabled, cfg!(feature = "permission"));
        assert_eq!(
            by_id("injection-detection").enabled,
            cfg!(feature = "sql-parser")
        );
        assert_eq!(by_id("pdp").enabled, cfg!(feature = "permission-engine"));
        assert_eq!(by_id("audit").enabled, cfg!(feature = "audit"));
        assert_eq!(
            by_id("drivers").enabled,
            cfg!(any(
                feature = "sqlite",
                feature = "duckdb",
                feature = "postgres",
                feature = "mysql"
            ))
        );
    }

    #[test]
    fn production_status_reflects_feature_combination() {
        let posture = security_posture(PostureProfile::Production);
        let missing_required = posture
            .checks
            .iter()
            .any(|c| c.severity == PostureSeverity::Required && !c.enabled);
        let missing_recommended = posture
            .checks
            .iter()
            .any(|c| c.severity == PostureSeverity::Recommended && !c.enabled);
        let expected = if missing_required {
            "NonCompliant"
        } else if missing_recommended {
            "Degraded"
        } else {
            "Compliant"
        };
        assert_eq!(posture.status, expected);
    }

    #[test]
    fn required_checks_are_rbac_and_injection() {
        let posture = security_posture(PostureProfile::Development);
        let required: Vec<_> = posture
            .checks
            .iter()
            .filter(|c| c.severity == PostureSeverity::Required)
            .map(|c| c.id)
            .collect();
        assert_eq!(required, vec!["rbac", "injection-detection"]);
    }

    #[test]
    fn every_effect_key_resolves_to_a_message() {
        let posture = security_posture(PostureProfile::Development);
        for check in posture.checks {
            let message = crate::i18n::t_simple(check.effect_key);
            assert_ne!(
                message, check.effect_key,
                "effect key must resolve: {}",
                check.effect_key
            );
        }
    }
}
