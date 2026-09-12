// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 统一 DDL 守卫端口测试
//!
//! 验证 `DdlGuardPolicy` 端口（白名单/干跑/审计）经 Session 统一漏斗生效：
//! 默认内置白名单守卫行为不变；注入自定义/审计/干跑策略后全部 DDL 路径生效。

#![cfg(all(
    feature = "sqlite",
    feature = "sql-parser",
    feature = "runtime-tokio-rustls"
))]

use dbnexus::{DdlAuditRecord, DdlGuardPolicy, DdlValidationResult, DbPool, DbPoolBuilder};
use std::sync::{Arc, Mutex};

/// 拒绝一切的测试策略（验证端口可注入自定义实现）
struct DenyAllGuard;

impl DdlGuardPolicy for DenyAllGuard {
    fn validate(&self, _sql: &str) -> Result<DdlValidationResult, String> {
        Ok(DdlValidationResult::Forbidden("deny all (test)".to_string()))
    }

    fn audit(&self, sql: &str, _result: &DdlValidationResult) {
        AUDIT_LOG
            .lock()
            .unwrap()
            .push(format!("deny_all:{sql}"));
    }
}

static AUDIT_LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn temp_url(tag: &str) -> String {
    let path = std::env::temp_dir().join(format!(
        "dbnexus_t416_{}_{}.db",
        tag,
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    format!("sqlite:{}?mode=rwc", path.display())
}

/// 默认路径：内置白名单守卫拦截 DROP DATABASE（既有契约，经统一漏斗后不变）
#[tokio::test]
async fn test_default_whitelist_guard_still_blocks_drop_database() {
    let url = temp_url("default");
    let pool = DbPool::new(&url).await.unwrap();
    let session = pool.get_session("admin").await.unwrap();

    let err = session
        .execute_raw_ddl("DROP DATABASE production")
        .await
        .unwrap_err();
    assert!(
        format!("{err}").contains("forbidden") || format!("{err}").contains("not allowed"),
        "默认白名单应拦截，实际: {err}"
    );

    // 白名单内语句正常放行
    session
        .execute_raw_ddl("CREATE TABLE t_default (id INTEGER PRIMARY KEY)")
        .await
        .unwrap();
    let _ = std::fs::remove_file(&url);
}

/// 注入自定义策略后经漏斗全局生效（admin DDL 也被拦截）
#[tokio::test]
async fn test_injected_deny_guard_overrides_admin_ddl() {
    let url = temp_url("deny");
    let pool = DbPoolBuilder::new()
        .url(&url)
        .ddl_guard(Arc::new(DenyAllGuard))
        .build()
        .await
        .unwrap();
    let session = pool.get_session("admin").await.unwrap();

    let err = session
        .execute_raw_ddl("CREATE TABLE t_denied (id INTEGER)")
        .await
        .unwrap_err();
    assert!(
        format!("{err}").contains("deny all (test)"),
        "注入的拒绝策略应生效，实际: {err}"
    );

    // audit 钩子被漏斗调用
    assert!(
        AUDIT_LOG
            .lock()
            .unwrap()
            .iter()
            .any(|entry| entry.contains("t_denied")),
        "audit 钩子应收到决策事件"
    );
    let _ = std::fs::remove_file(&url);
}

/// 运行时换装：set_ddl_guard 即时影响后续 DDL
#[tokio::test]
async fn test_set_ddl_guard_runtime_swap() {
    let url = temp_url("swap");
    let pool = DbPool::new(&url).await.unwrap();
    let session = pool.get_session("admin").await.unwrap();

    // 注入前正常
    session
        .execute_raw_ddl("CREATE TABLE t_swap (id INTEGER)")
        .await
        .unwrap();

    // 注入拒绝策略后同一 admin 路径被拦截
    pool.set_ddl_guard(Arc::new(DenyAllGuard));
    let err = session
        .execute_raw_ddl("ALTER TABLE t_swap ADD COLUMN c INTEGER")
        .await
        .unwrap_err();
    assert!(
        format!("{err}").contains("deny all (test)"),
        "运行时换装应即时生效，实际: {err}"
    );
    let _ = std::fs::remove_file(&url);
}

/// 审计装饰器：漏斗路径上的决策实时转发到外部 sink
#[tokio::test]
async fn test_auditing_guard_receives_funnel_decisions() {
    let url = temp_url("audit");
    let events: Arc<Mutex<Vec<DdlAuditRecord>>> = Arc::new(Mutex::new(Vec::new()));
    let sink_events = events.clone();

    let pool = DbPoolBuilder::new()
        .url(&url)
        .ddl_guard(Arc::new(dbnexus::AuditingDdlGuard::new(
            Arc::new(dbnexus::DdlGuard::new()),
            Arc::new(move |record: &DdlAuditRecord| {
                sink_events.lock().unwrap().push(DdlAuditRecord {
                    sql: record.sql.clone(),
                    allowed: record.allowed,
                    reason: record.reason.clone(),
                });
            }),
        )))
        .build()
        .await
        .unwrap();
    let session = pool.get_session("admin").await.unwrap();

    let _ = session
        .execute_raw_ddl("CREATE TABLE t_audit (id INTEGER)")
        .await;
    let _ = session.execute_raw_ddl("DROP DATABASE prod").await;

    let events = events.lock().unwrap();
    assert_eq!(events.len(), 2, "两条 DDL 的决策都应被审计");
    assert!(events[0].allowed);
    assert!(!events[1].allowed);
    let _ = std::fs::remove_file(&url);
}
