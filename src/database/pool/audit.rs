// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 安全审计模块（HD-5 + MD-4 修复）
//!
//! 将原 `session.rs` 中的 admin 权限绕过审计逻辑独立到本模块，
//! 实现 Session 主体逻辑与安全审计关注点分离（SRP）。
//!
//! # 功能
//!
//! - [`audit_admin_bypass`]: admin 角色绕过权限检查时记录审计事件（进程级审计环）
//! - [`warn_if_default_admin_role_used`]: 检查是否使用了默认 admin 角色
//! - [`warn_and_record_default_admin_role`]: 检查并记录默认 admin 角色告警事件
//! - [`admin_bypass_count`] / [`take_admin_bypass_events`]: 审计事件观测接口

use std::collections::VecDeque;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

#[cfg(feature = "permission")]
use crate::access::PermissionAction;

/// 进程级审计环容量上限（超出后丢弃最旧事件）
const BYPASS_RING_CAPACITY: usize = 1000;

/// 审计事件类别：admin 角色绕过权限检查
#[cfg(feature = "permission")]
pub(crate) const BYPASS_KIND_ADMIN: &str = "admin_bypass";

/// 审计事件类别：使用了默认 admin 角色（vuln-0001）
pub(crate) const BYPASS_KIND_DEFAULT_ADMIN_ROLE: &str = "default_admin_role";

/// 进程级轻量安全审计事件
///
/// 供外部观测 admin bypass 审计环（见 [`take_admin_bypass_events`]）。
/// `kind` 取值为 `"admin_bypass"`（admin 角色绕过权限检查）或
/// `"default_admin_role"`（使用了默认 admin 角色，vuln-0001）。
#[derive(Debug, Clone)]
pub struct BypassEvent {
    /// 事件类别：`"admin_bypass"` 或 `"default_admin_role"`
    pub kind: &'static str,
    /// 当前角色名称
    pub role: String,
    /// 被访问的表名（池初始化等无表场景为 "-"）
    pub table: String,
    /// 权限操作类型（Debug 格式字符串）
    pub operation: String,
    /// 事件记录时刻
    pub at: Instant,
}

/// 进程级审计环缓冲（超出 [`BYPASS_RING_CAPACITY`] 丢弃最旧事件）
static BYPASS_RING: LazyLock<Mutex<VecDeque<BypassEvent>>> =
    LazyLock::new(|| Mutex::new(VecDeque::new()));

/// 累计记录的事件总数（含因环满被丢弃的事件，单调递增）
static BYPASS_TOTAL: AtomicU64 = AtomicU64::new(0);

/// admin 权限绕过审计事件
///
/// admin 角色绕过权限检查时调用此函数，将事件记入进程级审计环
/// （[`record_admin_bypass`]），保留审计链接且不影响主流程。
///
/// # 参数
///
/// * `role` - 当前角色名称
/// * `table` - 被访问的表名
/// * `operation` - 权限操作类型
#[cfg(feature = "permission")]
pub(super) fn audit_admin_bypass(role: &str, table: &str, operation: &PermissionAction) {
    record_admin_bypass(BYPASS_KIND_ADMIN, role, table, &format!("{operation:?}"));
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

/// 检查默认 admin 角色并记录审计事件（vuln-0001 修复）
///
/// 当 [`warn_if_default_admin_role_used`] 判定使用了默认 "admin" 角色时，
/// 记录一条 `default_admin_role` 类别的审计事件；自定义角色不记录。
pub(super) fn warn_and_record_default_admin_role(admin_role: &str) {
    if warn_if_default_admin_role_used(admin_role) {
        record_admin_bypass(BYPASS_KIND_DEFAULT_ADMIN_ROLE, admin_role, "-", "PoolInit");
    }
}

/// 记录一条安全审计事件到进程级审计环
///
/// 环满时丢弃最旧事件；总计数（含被丢弃事件）单调递增，供
/// [`admin_bypass_count`] / [`take_admin_bypass_events`] 观测。
pub(crate) fn record_admin_bypass(kind: &'static str, role: &str, table: &str, operation: &str) {
    let event = BypassEvent {
        kind,
        role: role.to_string(),
        table: table.to_string(),
        operation: operation.to_string(),
        at: Instant::now(),
    };
    let mut ring = BYPASS_RING.lock().unwrap_or_else(|e| e.into_inner());
    push_bypass_event(&mut ring, &BYPASS_TOTAL, event);
}

/// 按"超限丢弃最旧"策略入队（独立成函数便于对容量策略做确定性单元测试）
fn push_bypass_event(ring: &mut VecDeque<BypassEvent>, total: &AtomicU64, event: BypassEvent) {
    if ring.len() >= BYPASS_RING_CAPACITY {
        ring.pop_front();
    }
    ring.push_back(event);
    total.fetch_add(1, Ordering::Relaxed);
}

/// 获取累计记录的审计事件数量（累计语义：含因环满被丢弃的事件，只增不减）
///
/// 公开观测接口：外部 crate 可经 `dbnexus::database::pool::audit::admin_bypass_count`
/// 读取进程级审计环的总写入计数（与 [`take_admin_bypass_events`] 的"取出即清空"
/// 独立，take 不会重置本计数）。
pub fn admin_bypass_count() -> u64 {
    BYPASS_TOTAL.load(Ordering::Relaxed)
}

/// 取出全部已记录的审计事件（取出即清空：环缓冲被排空，事件不重复投递）
///
/// 公开观测接口：外部 crate 可经 `dbnexus::database::pool::audit::take_admin_bypass_events`
/// 排空进程级审计环并拿到事件副本（按记录顺序）；适合测试或上层聚合并上报。
pub fn take_admin_bypass_events() -> Vec<BypassEvent> {
    let mut ring = BYPASS_RING.lock().unwrap_or_else(|e| e.into_inner());
    ring.drain(..).collect()
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

    /// 审计环容量策略：超过上限丢弃最旧事件（本地环，确定性断言）
    #[test]
    fn test_bypass_ring_drops_oldest_when_full() {
        let total = AtomicU64::new(0);
        let mut ring: VecDeque<BypassEvent> = VecDeque::new();

        for i in 0..(BYPASS_RING_CAPACITY + 10) {
            push_bypass_event(
                &mut ring,
                &total,
                BypassEvent {
                    kind: BYPASS_KIND_DEFAULT_ADMIN_ROLE,
                    role: "admin".to_string(),
                    table: format!("t{i}"),
                    operation: "Select".to_string(),
                    at: Instant::now(),
                },
            );
        }

        // 环长度不超过容量，总计数仍完整累加
        assert_eq!(ring.len(), BYPASS_RING_CAPACITY);
        assert_eq!(
            total.load(Ordering::Relaxed),
            (BYPASS_RING_CAPACITY + 10) as u64
        );
        // 最旧的 10 条（t0..t9）被丢弃
        assert_eq!(ring.front().unwrap().table, format!("t{}", 10));
        assert_eq!(
            ring.back().unwrap().table,
            format!("t{}", BYPASS_RING_CAPACITY + 9)
        );
    }

    /// 记录 N 条 → 计数递增、内容可观测、take 后清空
    ///
    /// 进程级环被同进程内其他测试共享（池构造等也会记录），因此计数用
    /// 单调递增断言、内容按本测试专属标记过滤，保证不受并发写入影响。
    #[test]
    fn test_record_admin_bypass_count_take_and_clear() {
        let marker = "record-take-marker";
        let before = admin_bypass_count();

        record_admin_bypass(BYPASS_KIND_DEFAULT_ADMIN_ROLE, marker, "tbl-a", "Select");
        record_admin_bypass(BYPASS_KIND_DEFAULT_ADMIN_ROLE, marker, "tbl-b", "Insert");
        record_admin_bypass(BYPASS_KIND_DEFAULT_ADMIN_ROLE, marker, "tbl-c", "Delete");

        // 总计数（含被丢弃事件）本次至少 +3
        assert!(admin_bypass_count() >= before + 3);

        let events = take_admin_bypass_events();
        let mine: Vec<_> = events.iter().filter(|e| e.role == marker).collect();
        assert_eq!(mine.len(), 3);
        assert_eq!(mine[0].table, "tbl-a");
        assert_eq!(mine[0].operation, "Select");
        assert_eq!(mine[1].operation, "Insert");
        assert_eq!(mine[2].table, "tbl-c");
        assert!(mine[0].at <= mine[2].at, "事件应按记录顺序保存");

        // take 后清空：标记事件不再残留
        assert!(take_admin_bypass_events().iter().all(|e| e.role != marker));
    }

    /// vuln-0001 回归测试：默认 admin 角色触发审计记录，事件可观测
    #[test]
    fn test_warn_and_record_default_admin_role() {
        let before = admin_bypass_count();
        warn_and_record_default_admin_role("admin");
        // 本次调用至少记录 1 条（其他测试可能并发记录，故用 >=）
        assert!(
            admin_bypass_count() > before,
            "默认 admin 角色应触发审计记录"
        );

        // 自定义角色由 test_vuln_0001_custom_admin_role_no_warning 覆盖（不触发）
        let events = take_admin_bypass_events();
        assert!(
            events
                .iter()
                .any(|e| e.kind == BYPASS_KIND_DEFAULT_ADMIN_ROLE
                    && e.table == "-"
                    && e.operation == "PoolInit"),
            "应存在 default_admin_role 类别的池初始化事件"
        );
    }

    /// vuln-0001 回归测试：admin bypass 审计事件被真实记录（内容可观测）
    #[cfg(feature = "permission")]
    #[test]
    fn test_vuln_0001_audit_admin_bypass_records_event() {
        let marker = "bypass-audit-marker";
        let before = admin_bypass_count();

        audit_admin_bypass(marker, "users", &PermissionAction::Select);
        audit_admin_bypass(marker, "users", &PermissionAction::Insert);
        audit_admin_bypass(marker, "orders", &PermissionAction::Delete);

        assert!(admin_bypass_count() >= before + 3);

        let events = take_admin_bypass_events();
        let mine: Vec<_> = events.iter().filter(|e| e.role == marker).collect();
        assert_eq!(mine.len(), 3);
        assert_eq!(mine[0].kind, BYPASS_KIND_ADMIN);
        assert_eq!(mine[0].table, "users");
        assert_eq!(mine[0].operation, "Select");
        assert_eq!(mine[2].table, "orders");
        assert_eq!(mine[2].operation, "Delete");

        // take 后清空：标记事件不再残留
        assert!(take_admin_bypass_events().iter().all(|e| e.role != marker));
    }

    /// 可达性回归：观测 API 与 BypassEvent 必须能经完整公开路径访问
    ///
    /// crate 内的完整路径 `crate::database::pool::audit::...` 与外部 crate 的
    /// `dbnexus::database::pool::audit::...` 一一对应（lib.rs `pub mod database`
    /// → database `pub mod pool` → pool `pub mod audit`）；
    /// 外部可达性另由 tests/audit/unit/audit_unit_tests.rs 以外部路径证明。
    #[test]
    fn test_observation_api_reachable_via_full_public_path() {
        use crate::database::pool::audit as public_audit;

        let before = public_audit::admin_bypass_count();
        // 记录仍由内部触发（record_admin_bypass 保持 crate 内可见）
        record_admin_bypass(
            BYPASS_KIND_DEFAULT_ADMIN_ROLE,
            "public-path-marker",
            "-",
            "PoolInit",
        );
        assert!(public_audit::admin_bypass_count() > before);

        let events = public_audit::take_admin_bypass_events();
        // BypassEvent 及其字段公开可读，Debug/Clone 派生可用
        let marker_events: Vec<_> = events
            .iter()
            .filter(|e| e.role == "public-path-marker")
            .collect();
        assert_eq!(marker_events.len(), 1);
        assert_eq!(marker_events[0].kind, "default_admin_role");
        let _ = format!("{:?}", marker_events[0]);
        let _cloned = marker_events[0].clone();

        // take 取出即清空：标记事件不再残留
        assert!(public_audit::take_admin_bypass_events()
            .iter()
            .all(|e| e.role != "public-path-marker"));
    }
}
