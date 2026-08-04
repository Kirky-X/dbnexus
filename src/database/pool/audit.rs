// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 安全审计模块（HD-5 + MD-4 修复）
//!
//! 将原 `session.rs` 中的 admin 权限绕过审计逻辑独立到本模块，
//! 实现 Session 主体逻辑与安全审计关注点分离（SRP）。
//!
//! # 功能
//!
//! - [`audit_admin_bypass`]: admin 角色绕过权限检查时记录审计事件（no-op，不影响流程）
//! - [`warn_if_default_admin_role_used`]: 检查是否使用了默认 admin 角色

#[cfg(feature = "permission")]
use crate::access::PermissionAction;

/// admin 权限绕过审计事件（no-op）
///
/// admin 角色绕过权限检查时调用此函数，保留审计链接入口。
///
/// # 参数
///
/// * `role` - 当前角色名称
/// * `table` - 被访问的表名
/// * `operation` - 权限操作类型
#[cfg(feature = "permission")]
pub(super) fn audit_admin_bypass(_role: &str, _table: &str, _operation: &PermissionAction) {
    // 审计事件入口，当前为 no-op（已移除日志框架依赖）
}

/// 检查是否使用了默认 admin 角色（vuln-0001 修复）
///
/// 当 `admin_role` 为 "admin"（默认值）时，返回 `true` 表示不安全。
///
/// # 参数
///
/// * `admin_role` - 当前配置的 admin 角色名称
///
/// # 返回
///
/// `true` 表示使用了默认 "admin" 角色（不安全），`false` 表示已自定义
pub fn warn_if_default_admin_role_used(admin_role: &str) -> bool {
    admin_role == "admin"
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // vuln-0001 安全审计单元测试（从 session.rs 移入，HD-5 + MD-4）
    // ============================================================================

    /// vuln-0001 回归测试：warn_if_default_admin_role_used 对默认 "admin" 返回 true
    #[test]
    fn test_vuln_0001_warn_default_admin_role() {
        assert!(
            warn_if_default_admin_role_used("admin"),
            "default admin_role 'admin' should trigger warning"
        );
    }

    /// vuln-0001 回归测试：warn_if_default_admin_role_used 对自定义角色返回 false
    #[test]
    fn test_vuln_0001_custom_admin_role_no_warning() {
        assert!(
            !warn_if_default_admin_role_used("super-admin-2026"),
            "custom admin_role should not trigger warning"
        );
    }

    /// vuln-0001 回归测试：admin bypass 审计函数不 panic
    #[cfg(feature = "permission")]
    #[test]
    fn test_vuln_0001_audit_admin_bypass_no_panic() {
        audit_admin_bypass("admin", "users", &PermissionAction::Select);
        audit_admin_bypass("admin", "users", &PermissionAction::Insert);
        audit_admin_bypass("admin", "orders", &PermissionAction::Delete);
    }
}
