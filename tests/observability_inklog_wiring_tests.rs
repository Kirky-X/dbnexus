// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 结构化日志接线测试（可观测性 Logs 支柱）
//!
//! 验证四类运维关键事件经 `log` 门面发出记录：
//! - 池获取超时（`DbPool::acquire_connection` 超时分支）
//! - 权限拒绝（`Session::check_permission` 拒绝路径）
//! - 熔断器状态转换（Closed→Open / HalfOpen→Open / HalfOpen→Closed）
//! - 慢查询（`MetricsCollector::record_query` 超阈值）
//!
//! 权限拒绝的日志覆盖范围（显式声明）：`permission_denied()` 集中构造点
//! （覆盖 check_permission / execute 逐表 / ORM / check_table_permission
//! 等全部拒绝调用点）+ 三处「SQL 解析失败 fail-closed 拒绝」分支
//! （execute_raw_impl / query_rows_impl / duckdb_security_gate）。其余
//! fail-closed 拒绝（无表名/表名非法/DDL 角色白名单等固定文案路径）不发
//! 日志，仅返回错误——如需观测请走 permission_denied 集中点。
//!
//! 日志门面为 `log` crate：接线点无条件编译，未安装 logger 时为 no-op
//! （默认构建行为不变）；具体后端（如 inklog `LoggerManager`）由消费方自行
//! 安装为全局 log 后端，记录即路由到其结构化管道——dbnexus 本身不依赖任何
//! 日志后端 crate。测试经进程内唯一安装的 TestLogger 收集记录并断言级别与内容。

use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use dbnexus::foundation::PoolConfig;
use dbnexus::observability::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerState, MetricsCollector,
};
use dbnexus::{DbConfig, DbPool};

#[path = "common/mod.rs"]
mod common;

#[derive(Debug, Clone)]
struct LogEntry {
    level: log::Level,
    message: String,
}

struct TestLogger(Arc<Mutex<Vec<LogEntry>>>);

impl log::Log for TestLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Trace
    }

    fn log(&self, record: &log::Record) {
        self.0
            .lock()
            .expect("test logger lock poisoned")
            .push(LogEntry {
                level: record.level(),
                message: format!("{}", record.args()),
            });
    }

    fn flush(&self) {}
}

/// 进程内唯一安装的测试 logger；返回共享记录缓冲。
///
/// `log` 全局 logger 每进程只能安装一次，所有用例共享同一缓冲，
/// 断言按「级别 + 消息包含」匹配，互不干扰。
fn log_entries() -> &'static Arc<Mutex<Vec<LogEntry>>> {
    static ENTRIES: OnceLock<Arc<Mutex<Vec<LogEntry>>>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        let entries = Arc::new(Mutex::new(Vec::new()));
        if let Err(err) = log::set_boxed_logger(Box::new(TestLogger(entries.clone()))) {
            panic!("test logger must install exactly once per test process: {err}");
        }
        log::set_max_level(log::LevelFilter::Trace);
        entries
    })
}

fn has_entry(level: log::Level, needle: &str) -> bool {
    log_entries()
        .lock()
        .expect("log entries lock poisoned")
        .iter()
        .any(|e| e.level == level && e.message.contains(needle))
}

fn dump_entries() -> Vec<(log::Level, String)> {
    log_entries()
        .lock()
        .expect("log entries lock poisoned")
        .iter()
        .map(|e| (e.level, e.message.clone()))
        .collect()
}

// ============================================================================
// 权限拒绝
// ============================================================================

/// user 角色访问未授权表必须被拒绝，并发出 WARN 记录
#[cfg(feature = "permission")]
#[tokio::test]
#[allow(clippy::unwrap_used)]
async fn permission_denied_emits_warn_record() {
    use dbnexus::access::PermissionAction;

    log_entries();
    let (config, _temp_dir) = common::get_test_config_with_permissions(true);
    let pool = DbPool::with_config(config).await.expect("create pool");
    let admin = pool.get_session("admin").await.expect("admin session");

    let perm_json = r#"
{
  "roles": {
    "admin": {
      "tables": [
        { "name": "*", "operations": ["select", "insert", "update", "delete"] }
      ]
    },
    "user": {
      "tables": [
        { "name": "users", "operations": ["select", "insert"] }
      ]
    }
  }
}
"#;
    let perm_config: dbnexus::access::PermissionConfig =
        serde_json::from_str(perm_json).expect("parse permission JSON");
    admin
        .permission_ctx()
        .load_policy(&perm_config)
        .await
        .expect("load policy");

    let user = pool.get_session("user").await.expect("user session");
    let result = user
        .check_permission("restricted_table", &PermissionAction::Select)
        .await;
    assert!(result.is_err(), "unauthorized table access must be denied");

    assert!(
        has_entry(log::Level::Warn, "permission denied"),
        "permission denial must emit a WARN record; entries: {:?}",
        dump_entries()
    );
    assert!(
        has_entry(log::Level::Warn, "restricted_table"),
        "WARN record must carry the denied table name; entries: {:?}",
        dump_entries()
    );
}

/// 畸形标识符（换行/回车/ESC 控制字符）写入日志前必须消毒：
/// 权限拒绝记录是安全审计记录，注入的换行可伪造日志行、ESC 可操纵终端渲染
#[cfg(feature = "permission")]
#[tokio::test]
#[allow(clippy::unwrap_used)]
async fn permission_denied_sanitizes_control_characters() {
    use dbnexus::access::PermissionAction;

    log_entries();
    let (config, _temp_dir) = common::get_test_config_with_permissions(true);
    let pool = DbPool::with_config(config).await.expect("create pool");
    let admin = pool.get_session("admin").await.expect("admin session");

    let perm_json = r#"
{
  "roles": {
    "admin": {
      "tables": [
        { "name": "*", "operations": ["select", "insert", "update", "delete"] }
      ]
    },
    "user": {
      "tables": [
        { "name": "users", "operations": ["select", "insert"] }
      ]
    }
  }
}
"#;
    let perm_config: dbnexus::access::PermissionConfig =
        serde_json::from_str(perm_json).expect("parse permission JSON");
    admin
        .permission_ctx()
        .load_policy(&perm_config)
        .await
        .expect("load policy");

    let user = pool.get_session("user").await.expect("user session");
    let forged = "legit\nFAKE LOG LINE injected\x1b[31mred";
    let result = user
        .check_permission(forged, &PermissionAction::Select)
        .await;
    assert!(result.is_err(), "unauthorized table access must be denied");

    // Cf 格式字符（零宽/双向控制/BOM）同样必须剔除：不可见字符可在终端
    // 渲染中视觉重排或隐藏文本（Trojan-Source 式欺骗）
    let vis_override = "col\u{202E}ltr\u{2066}iso\u{200B}zw\u{FEFF}bom";
    let _ = user
        .check_permission(vis_override, &PermissionAction::Select)
        .await;

    let entries = log_entries().lock().expect("log entries lock poisoned");
    let denial: Vec<&LogEntry> = entries
        .iter()
        .filter(|e| e.level == log::Level::Warn && e.message.contains("permission denied"))
        .collect();
    assert!(
        !denial.is_empty(),
        "permission denial must emit a WARN record; entries: {:?}",
        dump_entries()
    );
    for entry in &denial {
        assert!(
            !entry.message.contains('\n') && !entry.message.contains('\r'),
            "denial record must stay on one line, got: {:?}",
            entry.message
        );
        assert!(
            !entry.message.contains('\x1b'),
            "denial record must not contain raw ESC bytes, got: {:?}",
            entry.message
        );
        for cf in ['\u{200B}', '\u{200E}', '\u{202E}', '\u{2066}', '\u{FEFF}'] {
            assert!(
                !entry.message.contains(cf),
                "denial record must not contain format char U+{:04X}, got: {:?}",
                cf as u32,
                entry.message
            );
        }
    }
    assert!(
        denial.iter().any(|e| e.message.contains("legitFAKE")),
        "control characters must be stripped while visible content is preserved, got: {:?}",
        denial.iter().map(|e| e.message.clone()).collect::<Vec<_>>()
    );
    assert!(
        denial.iter().any(|e| e.message.contains("colltrisozwbom")),
        "Cf format chars must be stripped while visible content is preserved, got: {:?}",
        denial.iter().map(|e| e.message.clone()).collect::<Vec<_>>()
    );
}

// ============================================================================
// 池获取超时
// ============================================================================

/// 池中唯一连接被占用时，acquire_timeout 内拿不到连接必须发出 WARN 记录
#[tokio::test]
#[allow(clippy::unwrap_used)]
async fn pool_acquire_timeout_emits_warn_record() {
    log_entries();

    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        pool_config: PoolConfig {
            max_connections: 1,
            min_connections: 1,
            idle_timeout: 300,
            acquire_timeout: 100,
        },
        admin_role: "admin".to_string(),
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.expect("create pool");

    let _holder = pool.get_session("admin").await.expect("holder session");

    let result = pool.get_session("admin").await;
    assert!(
        result.is_err(),
        "acquire must time out while the only connection is held, got: {:?}",
        result.err()
    );

    assert!(
        has_entry(log::Level::Warn, "pool acquire timeout"),
        "pool acquire timeout must emit a WARN record; entries: {:?}",
        dump_entries()
    );
}

// ============================================================================
// 熔断器状态转换
// ============================================================================

/// 熔断器三个转换方向各自发出对应级别记录：
/// Closed→Open 与 HalfOpen→Open 为 WARN，HalfOpen→Closed 恢复为 INFO
#[cfg(feature = "health-check")]
#[tokio::test]
#[allow(clippy::unwrap_used)]
async fn circuit_breaker_transitions_emit_records() {
    // 测试体为纯内存操作，必须先装 logger 再触发状态转换，
    // 否则记录发在安装前（no-op）导致缓冲为空。
    log_entries();

    let breaker = CircuitBreaker::new(CircuitBreakerConfig {
        failure_threshold: 1,
        success_threshold: 1,
        timeout_ms: 0,
        window_size: 10,
    });

    breaker.record_failure().await;
    assert_eq!(breaker.state().await, CircuitBreakerState::Open);
    assert!(
        has_entry(log::Level::Warn, "circuit breaker opened"),
        "Closed→Open must emit a WARN record; entries: {:?}",
        dump_entries()
    );

    breaker
        .can_execute()
        .await
        .expect("timeout_ms=0 flips to half-open");
    assert_eq!(breaker.state().await, CircuitBreakerState::HalfOpen);

    breaker.record_failure().await;
    assert_eq!(breaker.state().await, CircuitBreakerState::Open);
    assert!(
        has_entry(log::Level::Warn, "circuit breaker reopened"),
        "HalfOpen→Open must emit a WARN record; entries: {:?}",
        dump_entries()
    );

    breaker
        .can_execute()
        .await
        .expect("timeout_ms=0 flips to half-open");
    breaker.record_success().await;
    assert_eq!(breaker.state().await, CircuitBreakerState::Closed);
    assert!(
        has_entry(log::Level::Info, "circuit breaker closed"),
        "HalfOpen→Closed must emit an INFO record; entries: {:?}",
        dump_entries()
    );
}

// ============================================================================
// 慢查询
// ============================================================================

/// 耗时超过阈值的查询必须发出 WARN 记录，且携带查询类型与耗时
#[cfg(feature = "metrics")]
#[tokio::test]
#[allow(clippy::unwrap_used)]
async fn slow_query_emits_warn_record() {
    log_entries();

    let collector = MetricsCollector::new();
    collector.set_slow_query_threshold(0);

    collector.record_query(
        "inklog_wiring_slow_query",
        Duration::from_millis(5),
        true,
        None,
    );

    assert!(
        has_entry(log::Level::Warn, "slow query"),
        "slow query must emit a WARN record; entries: {:?}",
        dump_entries()
    );
    assert!(
        has_entry(log::Level::Warn, "inklog_wiring_slow_query"),
        "WARN record must carry the query type; entries: {:?}",
        dump_entries()
    );
}
