// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 安全审计模块（HD-5 + MD-4 修复）
//!
//! 将原 `session.rs` 中的 admin 权限绕过审计逻辑独立到本模块，
//! 实现 Session 主体逻辑与安全审计关注点分离（SRP）。
//!
//! # 功能
//!
//! - [`audit_admin_bypass`]: admin 角色绕过权限检查时记录审计日志
//! - [`warn_if_default_admin_role_used`]: 检查是否使用了默认 admin 角色并发出警告

#[cfg(feature = "permission")]
use crate::access::PermissionAction;

/// 记录 admin 权限绕过审计日志（vuln-0001 修复）
///
/// admin 角色绕过权限检查时调用此函数，记录审计日志以保留审计链。
///
/// **性能优化**（perf-HIGH）：此函数位于 SQL 热路径（`Session::execute()` → `check_permission()`），
/// 生产环境 admin 重负载场景下每秒可达千次调用。原实现先 `format!` 分配 String（~200ns），
/// 再 `eprintln!` 同步阻塞 I/O（10-100μs，stderr 全局锁），并造成 tracing 启用时的双重输出。
///
/// 现遵循项目惯例（参见 `access/permission/cache.rs` 中的 `warn_log!` 宏）：
/// - `tracing` feature 启用时：仅走 `tracing::warn!`（异步、结构化、可采样、可过滤）
/// - 未启用 `tracing` 时：降级为 `eprintln!`，但直接使用其 lazy 格式化（不预先 `format!`）
///
/// # 参数
///
/// * `role` - 当前角色名称
/// * `table` - 被访问的表名
/// * `operation` - 权限操作类型
#[cfg(feature = "permission")]
pub(super) fn audit_admin_bypass(role: &str, table: &str, operation: &PermissionAction) {
    #[cfg(feature = "tracing")]
    {
        tracing::warn!(
            target: "dbnexus.security.audit",
            role = role,
            table = table,
            operation = ?operation,
            "Admin role bypassed permission check (vuln-0001 audit)"
        );
    }
    #[cfg(not(feature = "tracing"))]
    {
        // 非 tracing 时降级为 eprintln!（遵循 access/permission/cache.rs 中的 warn_log! 惯例）
        // 直接使用 eprintln! 的 lazy 格式化，避免 format! 预分配 String
        eprintln!(
            "[SECURITY AUDIT] Admin role '{}' bypassed permission check: operation={:?} table={}",
            role, operation, table
        );
    }
}

/// 检查是否使用了默认 admin 角色并发出警告（vuln-0001 修复）
///
/// 当 `admin_role` 为 "admin"（默认值）时，记录安全警告。
/// 返回 `true` 表示使用了默认值（不安全），`false` 表示已自定义。
///
/// 日志输出遵循与 [`audit_admin_bypass`] 一致的模式：
/// - `tracing` feature 启用时：仅走 `tracing::warn!`
/// - 未启用 `tracing` 时：降级为 `eprintln!`
///
/// # 参数
///
/// * `admin_role` - 当前配置的 admin 角色名称
///
/// # 返回
///
/// `true` 表示使用了默认 "admin" 角色（不安全），`false` 表示已自定义
pub fn warn_if_default_admin_role_used(admin_role: &str) -> bool {
    if admin_role == "admin" {
        #[cfg(feature = "tracing")]
        {
            tracing::warn!(
                target: "dbnexus.security.audit",
                admin_role = admin_role,
                "Using default admin_role 'admin' is insecure (vuln-0001). \
                 Set a custom admin_role via DbConfig.admin_role or DbPoolBuilder::admin_role()."
            );
        }
        #[cfg(not(feature = "tracing"))]
        {
            eprintln!(
                "[SECURITY WARNING] Using default admin_role 'admin' is insecure. \
                 Set a custom admin_role via DbConfig.admin_role or DbPoolBuilder::admin_role()."
            );
        }
        true
    } else {
        false
    }
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

    /// vuln-0001 回归测试：admin bypass 审计日志不 panic
    #[cfg(feature = "permission")]
    #[test]
    fn test_vuln_0001_audit_admin_bypass_no_panic() {
        // audit_admin_bypass 应该正常执行而不 panic
        // 它输出到 stderr，我们只验证不 panic
        audit_admin_bypass("admin", "users", &PermissionAction::Select);
        audit_admin_bypass("admin", "users", &PermissionAction::Insert);
        audit_admin_bypass("admin", "orders", &PermissionAction::Delete);
    }
}
