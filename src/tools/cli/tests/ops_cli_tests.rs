// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 运维 CLI 端到端测试（assert_cmd 驱动真实二进制）
//!
//! 覆盖 `migrate` / `health` / `user` / `permission-check` 等运维子命令的
//! JSON 输出与退出码契约（0 成功 / 1 运行时失败 / 2 用法错误）。
//!
//! 子命令经 cli 的 `migration`/`health-check`/`permission-engine` 等 features
//! 门控（均属 cli default 集合）；workspace `--no-default-features` 口径会
//! 关闭它们使子命令不存在，整套用例仅在 feature 集合齐备的构建下编译运行
//! （`cargo test -p dbnexus-cli` 默认口径覆盖）。

#![cfg(all(
    feature = "sqlite",
    feature = "migration",
    feature = "health-check",
    feature = "permission-engine"
))]

use assert_cmd::Command;
use std::path::PathBuf;

/// 定位 dbnexus-cli 二进制
fn cli() -> Command {
    Command::cargo_bin("dbnexus-cli").expect("dbnexus-cli binary")
}

/// 唯一临时 sqlite 文件库 URL（仅 sqlite 驱动构建可执行）
#[cfg(feature = "sqlite")]
fn temp_db_url(tag: &str) -> (PathBuf, String) {
    let path = std::env::temp_dir().join(format!(
        "dbnexus_t415_cli_{}_{}.db",
        tag,
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let url = format!("sqlite:{}?mode=rwc", path.display());
    (path, url)
}

fn parse_json_line(stdout: &str) -> serde_json::Value {
    let line = stdout.lines().last().expect("至少输出一行 JSON");
    serde_json::from_str(line).expect("stdout 末行应为合法 JSON")
}

#[cfg(feature = "sqlite")]
#[test]
fn test_health_healthy_exit_0() {
    let (path, url) = temp_db_url("health_ok");
    let output = cli()
        .args(["health", "--database-url", &url])
        .output()
        .expect("run health");
    assert_eq!(output.status.code(), Some(0), "健康库应退出 0");
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["status"], "healthy");
    assert_eq!(json["checks"]["connect"], "ok");
    assert_eq!(json["checks"]["query"], "ok");
    let _ = std::fs::remove_file(&path);
}

#[cfg(feature = "sqlite")]
#[test]
fn test_health_unreachable_exit_1() {
    // 指向不存在目录的 sqlite 文件 → 连接失败 → 不健康（退出码 1）
    let output = cli()
        .args([
            "health",
            "--database-url",
            "sqlite:/nonexistent_dir_t415/x.db?mode=rwc",
        ])
        .output()
        .expect("run health");
    assert_eq!(output.status.code(), Some(1), "不可达库应退出 1");
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["status"], "unhealthy");
}

#[test]
fn test_health_invalid_url_exit_2() {
    let output = cli()
        .args(["health", "--database-url", "foo://localhost/db"])
        .output()
        .expect("run health");
    assert_eq!(output.status.code(), Some(2), "非法协议应退出 2");
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["status"], "unhealthy");
    assert_eq!(json["checks"]["url"], "invalid");
}

#[cfg(feature = "sqlite")]
#[test]
fn test_migrate_applies_directory_exit_0() {
    let (path, url) = temp_db_url("migrate");
    let dir = std::env::temp_dir().join(format!("dbnexus_t415_migrations_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("001_create_items.sql"),
        "-- UP:\nCREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT);\n-- DOWN:\nDROP TABLE items;\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("002_seed.sql"),
        "-- UP:\nINSERT INTO items (id, name) VALUES (1, 'a');\n-- DOWN:\nDELETE FROM items;\n",
    )
    .unwrap();

    let output = cli()
        .args([
            "migrate",
            "--database-url",
            &url,
            "--migrations-dir",
            dir.to_str().unwrap(),
        ])
        .output()
        .expect("run migrate");
    assert_eq!(output.status.code(), Some(0), "迁移成功应退出 0");
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["status"], "ok");
    assert_eq!(json["pending_total"], 2);
    assert_eq!(json["applied"].as_array().unwrap().len(), 2);

    // 幂等：再次执行 → 无待应用
    let output2 = cli()
        .args([
            "migrate",
            "--database-url",
            &url,
            "--migrations-dir",
            dir.to_str().unwrap(),
        ])
        .output()
        .expect("run migrate again");
    assert_eq!(output2.status.code(), Some(0));
    let json2 = parse_json_line(&String::from_utf8_lossy(&output2.stdout));
    assert_eq!(json2["pending_total"], 0);

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(feature = "sqlite")]
#[test]
fn test_user_add_list_remove_lifecycle() {
    let (path, url) = temp_db_url("user");

    // add
    let output = cli()
        .args([
            "user",
            "add",
            "--username",
            "ops_admin",
            "--password",
            "s3cret!",
            "--role",
            "admin",
            "--database-url",
            &url,
        ])
        .output()
        .expect("run user add");
    assert_eq!(output.status.code(), Some(0), "add 应退出 0");
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["status"], "ok");
    assert_eq!(json["action"], "add");
    assert_eq!(json["username"], "ops_admin");

    // 重复 add → 用户已存在（运行时失败 1）
    let dup = cli()
        .args([
            "user",
            "add",
            "--username",
            "ops_admin",
            "--password",
            "x",
            "--database-url",
            &url,
        ])
        .output()
        .expect("run duplicate add");
    assert_eq!(dup.status.code(), Some(1), "重复 add 应退出 1");

    // list
    let list = cli()
        .args(["user", "list", "--database-url", &url])
        .output()
        .expect("run user list");
    assert_eq!(list.status.code(), Some(0));
    let list_json = parse_json_line(&String::from_utf8_lossy(&list.stdout));
    assert_eq!(list_json["count"], 1);
    assert_eq!(list_json["users"][0]["username"], "ops_admin");

    // remove
    let rm = cli()
        .args([
            "user",
            "remove",
            "--username",
            "ops_admin",
            "--database-url",
            &url,
        ])
        .output()
        .expect("run user remove");
    assert_eq!(rm.status.code(), Some(0));

    // 删除不存在的用户 → 运行时失败 1
    let rm2 = cli()
        .args([
            "user",
            "remove",
            "--username",
            "ops_admin",
            "--database-url",
            &url,
        ])
        .output()
        .expect("run user remove again");
    assert_eq!(rm2.status.code(), Some(1), "删除不存在用户应退出 1");

    let _ = std::fs::remove_file(&path);
}

#[cfg(feature = "sqlite")]
#[test]
fn test_user_invalid_args_exit_2() {
    let (path, url) = temp_db_url("user_bad");
    // 非法用户名 → 用法错误 2
    let output = cli()
        .args([
            "user",
            "add",
            "--username",
            "bad user!",
            "--password",
            "x",
            "--database-url",
            &url,
        ])
        .output()
        .expect("run invalid add");
    assert_eq!(output.status.code(), Some(2), "非法用户名应退出 2");
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["error_code"], "invalid_username");

    // 空密码 → 用法错误 2
    let output2 = cli()
        .args([
            "user",
            "add",
            "--username",
            "ok_user",
            "--password",
            "",
            "--database-url",
            &url,
        ])
        .output()
        .expect("run empty password");
    assert_eq!(output2.status.code(), Some(2), "空密码应退出 2");

    let _ = std::fs::remove_file(&path);
}

// ============================================================================
// pool-status（池状态：health_snapshot JSON 输出）
// ============================================================================

#[cfg(all(feature = "sqlite", feature = "health-check"))]
#[test]
fn test_pool_status_healthy_exit_0() {
    let (path, url) = temp_db_url("pool_status_ok");
    let output = cli()
        .args(["pool-status", "--database-url", &url])
        .output()
        .expect("run pool-status");
    assert_eq!(output.status.code(), Some(0), "可连库应退出 0");
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert!(
        json["status"] == "healthy" || json["status"] == "degraded",
        "可连库状态应为 healthy/degraded: {}",
        json["status"]
    );
    assert!(json["pool"]["total"].is_u64(), "应输出池连接总数");
    assert!(json["pool"]["saturation"].is_number(), "应输出池饱和度");
    let _ = std::fs::remove_file(&path);
}

#[cfg(all(feature = "sqlite", feature = "health-check"))]
#[test]
fn test_pool_status_unreachable_exit_1() {
    let output = cli()
        .args([
            "pool-status",
            "--database-url",
            "sqlite:/nonexistent_dir_r4/x.db?mode=rwc",
        ])
        .output()
        .expect("run pool-status");
    assert_eq!(output.status.code(), Some(1), "不可达库应退出 1");
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["status"], "unhealthy");
}

#[cfg(feature = "health-check")]
#[test]
fn test_pool_status_invalid_url_exit_2() {
    let output = cli()
        .args(["pool-status", "--database-url", "foo://localhost/db"])
        .output()
        .expect("run pool-status");
    assert_eq!(output.status.code(), Some(2), "非法协议应退出 2");
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["checks"]["url"], "invalid");
}

// ============================================================================
// shard-info（分片信息：策略/分片清单/路由演示）
// ============================================================================

#[test]
fn test_shard_info_lists_all_shards_exit_0() {
    let output = cli()
        .args([
            "shard-info",
            "--strategy",
            "yearly",
            "--total-shards",
            "4",
            "--prefix",
            "db",
        ])
        .output()
        .expect("run shard-info");
    assert_eq!(output.status.code(), Some(0));
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["status"], "ok");
    assert_eq!(json["strategy"], "yearly");
    assert_eq!(json["total_shards"], 4);
    let shards = json["shards"].as_array().expect("shards 数组");
    assert_eq!(shards.len(), 4);
    assert_eq!(shards[0]["shard_id"], 0);
    assert_eq!(shards[0]["name"], "db_0");
    assert!(shards[0]["connection_string"].is_string());
}

#[test]
fn test_shard_info_route_key_exit_0() {
    let output = cli()
        .args([
            "shard-info",
            "--strategy",
            "hash",
            "--total-shards",
            "8",
            "--route-key",
            "user-42",
        ])
        .output()
        .expect("run shard-info with route key");
    assert_eq!(output.status.code(), Some(0));
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    let route = &json["route"];
    assert_eq!(route["key"], "user-42");
    let shard_id = route["shard_id"].as_u64().expect("shard_id 数值");
    assert!(shard_id < 8, "路由结果必须落在分片范围内: {}", shard_id);
}

#[test]
fn test_shard_info_zero_shards_exit_2() {
    let output = cli()
        .args(["shard-info", "--strategy", "yearly", "--total-shards", "0"])
        .output()
        .expect("run shard-info");
    assert_eq!(output.status.code(), Some(2), "total_shards=0 应退出 2");
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["status"], "error");
    assert_eq!(json["error_code"], "invalid_total_shards");
}

#[test]
fn test_shard_info_unknown_strategy_exit_2() {
    let output = cli()
        .args([
            "shard-info",
            "--strategy",
            "quadratic",
            "--total-shards",
            "4",
        ])
        .output()
        .expect("run shard-info");
    assert_eq!(
        output.status.code(),
        Some(2),
        "未知策略应退出 2（禁止静默回落）"
    );
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["error_code"], "unknown_strategy");
}

// ============================================================================
// audit-query（审计查询：DbAuditStorage + AuditQueryFilters）
// ============================================================================

#[cfg(all(feature = "audit", feature = "sqlite"))]
#[test]
fn test_audit_query_empty_exit_0() {
    let (path, url) = temp_db_url("audit_empty");
    let output = cli()
        .args(["audit-query", "--database-url", &url])
        .output()
        .expect("run audit-query");
    assert_eq!(output.status.code(), Some(0), "空审计集应退出 0");
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["status"], "ok");
    assert_eq!(json["count"], 0);
    assert_eq!(json["events"].as_array().unwrap().len(), 0);
    let _ = std::fs::remove_file(&path);
}

/// 借 dbnexus 库 API 直写两条审计事件（CLI 只读；写入属库侧行为）
#[cfg(all(feature = "audit", feature = "sqlite"))]
fn seed_audit_events(url: &str, user_a: &str, user_b: &str) {
    use dbnexus::{AuditEvent, AuditOperation, AuditSeverity, AuditStatus, AuditStorage};
    use std::sync::Arc;

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let pool = Arc::new(dbnexus::DbPool::new(url).await.expect("seed pool"));
        let storage = dbnexus::DbAuditStorage::new(Arc::clone(&pool));
        storage.init().await.expect("init audit table");

        for (uid, op, sev, status) in [
            (
                user_a,
                AuditOperation::Create,
                AuditSeverity::Info,
                AuditStatus::Success,
            ),
            (
                user_b,
                AuditOperation::Delete,
                AuditSeverity::High,
                AuditStatus::Failure,
            ),
        ] {
            let event = AuditEvent {
                id: format!("evt-{uid}"),
                timestamp: chrono::Utc::now(),
                operation: op,
                entity_type: "users".to_string(),
                entity_id: "42".to_string(),
                user_id: uid.to_string(),
                user_role: "admin".to_string(),
                client_ip: "127.0.0.1".to_string(),
                severity: sev,
                result: status,
                error_message: None,
                before_value: None,
                after_value: None,
                extra: None,
                request_id: "req-r4".to_string(),
                session_id: String::new(),
                trace_context: None,
            };
            storage.store(&event).await.expect("store audit event");
        }
    });
}

#[cfg(all(feature = "audit", feature = "sqlite"))]
#[test]
fn test_audit_query_lists_seeded_events() {
    let (path, url) = temp_db_url("audit_seeded");
    seed_audit_events(&url, "alice", "bob");

    let output = cli()
        .args(["audit-query", "--database-url", &url])
        .output()
        .expect("run audit-query");
    assert_eq!(output.status.code(), Some(0));
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["status"], "ok");
    assert_eq!(json["count"], 2);

    // user 过滤：只返回 alice 的事件
    let filtered = cli()
        .args(["audit-query", "--database-url", &url, "--user", "alice"])
        .output()
        .expect("run audit-query --user");
    assert_eq!(filtered.status.code(), Some(0));
    let fjson = parse_json_line(&String::from_utf8_lossy(&filtered.stdout));
    assert_eq!(fjson["count"], 1);
    assert_eq!(fjson["events"][0]["user_id"], "alice");

    let _ = std::fs::remove_file(&path);
}

#[cfg(feature = "audit")]
/// seed 多条事件后 --limit 截断并如实报告 truncated
#[cfg(all(feature = "audit", feature = "sqlite"))]
#[test]
fn test_audit_query_limit_truncates() {
    let (path, url) = temp_db_url("audit_limit");
    seed_audit_events(&url, "alice", "bob");

    let output = cli()
        .args(["audit-query", "--database-url", &url, "--limit", "1"])
        .output()
        .expect("run audit-query --limit");
    assert_eq!(output.status.code(), Some(0));
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["count"], 1, "limit=1 应只返回 1 条");
    assert_eq!(json["truncated"], true, "超出上限应报告截断");
    let _ = std::fs::remove_file(&path);
}

/// other:<text> 约定映射自由文本操作变体 Other(String)
#[cfg(all(feature = "audit", feature = "sqlite"))]
#[test]
fn test_audit_query_other_operation_filter() {
    let (path, url) = temp_db_url("audit_other");
    seed_audit_events(&url, "alice", "bob");

    let output = cli()
        .args([
            "audit-query",
            "--database-url",
            &url,
            "--operation",
            "other:rebalance",
        ])
        .output()
        .expect("run audit-query other:");
    assert_eq!(output.status.code(), Some(0), "other: 前缀应可解析");
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["status"], "ok");
    assert_eq!(json["count"], 0, "未写入 Other 事件时应为空集");
    let _ = std::fs::remove_file(&path);
}

/// --limit 超上界（>1_000_000）应按用法错误拒绝
#[cfg(feature = "audit")]
#[test]
fn test_audit_query_limit_over_max_exit_2() {
    let output = cli()
        .args([
            "audit-query",
            "--database-url",
            "sqlite::memory:",
            "--limit",
            "1000001",
        ])
        .output()
        .expect("run audit-query --limit");
    assert_eq!(output.status.code(), Some(2));
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["error_code"], "invalid_limit");
}

/// --limit=0 表示不限制（显式选择全量物化），空集仍为成功
#[cfg(all(feature = "audit", feature = "sqlite"))]
#[test]
fn test_audit_query_limit_zero_means_unlimited() {
    let (path, url) = temp_db_url("audit_unlimited");
    let output = cli()
        .args(["audit-query", "--database-url", &url, "--limit", "0"])
        .output()
        .expect("run audit-query --limit 0");
    assert_eq!(output.status.code(), Some(0));
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["count"], 0);
    assert_eq!(json["truncated"], false);
    let _ = std::fs::remove_file(&path);
}

#[cfg(feature = "audit")]
#[test]
fn test_audit_query_invalid_operation_exit_2() {
    let output = cli()
        .args([
            "audit-query",
            "--database-url",
            "sqlite::memory:",
            "--operation",
            "frobnicate",
        ])
        .output()
        .expect("run audit-query");
    assert_eq!(output.status.code(), Some(2), "非法操作枚举应退出 2");
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["error_code"], "invalid_operation");
}

#[cfg(feature = "audit")]
#[cfg(feature = "audit")]
#[test]
fn test_audit_query_invalid_since_exit_2() {
    let output = cli()
        .args([
            "audit-query",
            "--database-url",
            "sqlite::memory:",
            "--since",
            "not-a-timestamp",
        ])
        .output()
        .expect("run audit-query");
    assert_eq!(output.status.code(), Some(2), "非法时间格式应退出 2");
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["error_code"], "invalid_since");
}

// ============================================================================
// permission-check（权限校验：YamlPermissionProvider + PDP，allow/deny 分流）
// ============================================================================

/// 权限配置文件（YAML 为 JSON 超集，两种写法均可被 provider 解析）
fn write_permissions_file(tag: &str, body: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "dbnexus_r4_permissions_{}_{}.json",
        tag,
        std::process::id()
    ));
    std::fs::write(&path, body).unwrap();
    path
}

#[test]
fn test_permission_check_allow_exit_0() {
    let perms = write_permissions_file(
        "allow",
        r#"{"roles": {"ops": [{"name": "ops-select", "subject": "ops", "resource": "users", "allow": ["select", "insert"], "deny": []}]}}"#,
    );
    let output = cli()
        .args([
            "permission-check",
            "--role",
            "ops",
            "--table",
            "users",
            "--action",
            "select",
            "--permissions",
            perms.to_str().unwrap(),
        ])
        .output()
        .expect("run permission-check");
    assert_eq!(output.status.code(), Some(0), "allow 决策应退出 0");
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["status"], "ok");
    assert_eq!(json["decision"], "allow");
    assert_eq!(json["raw_decision"], "allow");
    std::fs::remove_file(&perms).unwrap();
}

#[test]
fn test_permission_check_deny_exit_1() {
    // guest 无任何规则 → default_decision(Deny) fail-closed → 退出 1
    let perms = write_permissions_file(
        "deny",
        r#"{"roles": {"ops": [{"name": "ops-select", "subject": "ops", "resource": "users", "allow": ["select"], "deny": []}]}}"#,
    );
    let output = cli()
        .args([
            "permission-check",
            "--role",
            "guest",
            "--table",
            "users",
            "--action",
            "delete",
            "--permissions",
            perms.to_str().unwrap(),
        ])
        .output()
        .expect("run permission-check");
    assert_eq!(output.status.code(), Some(1), "deny 决策应退出 1");
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["status"], "ok");
    assert_eq!(json["decision"], "deny", "fail-closed 退出语义不变");
    assert_eq!(
        json["raw_decision"], "not_applicable",
        "无匹配策略应可辨（多为拼写/subject 配错）"
    );
    std::fs::remove_file(&perms).unwrap();
}

#[test]
fn test_permission_check_missing_file_exit_2() {
    let output = cli()
        .args([
            "permission-check",
            "--role",
            "ops",
            "--table",
            "users",
            "--action",
            "select",
            "--permissions",
            "/nonexistent_dir_r4/permissions.json",
        ])
        .output()
        .expect("run permission-check");
    assert_eq!(output.status.code(), Some(2), "权限文件缺失应退出 2");
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["error_code"], "permissions_file_unavailable");
}

#[test]
fn test_permission_check_invalid_action_exit_2() {
    let perms = write_permissions_file("act", r#"{"roles": {}}"#);
    let output = cli()
        .args([
            "permission-check",
            "--role",
            "ops",
            "--table",
            "users",
            "--action",
            "drop table users",
            "--permissions",
            perms.to_str().unwrap(),
        ])
        .output()
        .expect("run permission-check");
    assert_eq!(output.status.code(), Some(2), "非法 action 应退出 2");
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["error_code"], "invalid_action");
    std::fs::remove_file(&perms).unwrap();
}
