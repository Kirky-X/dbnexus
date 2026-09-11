// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! T415 运维 CLI 端到端测试（assert_cmd 驱动真实二进制）
//!
//! 覆盖 `migrate` / `health` / `user` 三个运维子命令的 JSON 输出与退出码契约
//! （0 成功 / 1 运行时失败 / 2 用法错误）。

use assert_cmd::Command;
use std::path::PathBuf;

/// 定位 dbnexus-cli 二进制
fn cli() -> Command {
    Command::cargo_bin("dbnexus-cli").expect("dbnexus-cli binary")
}

/// 唯一临时 sqlite 文件库 URL
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
    let line = stdout
        .lines()
        .last()
        .expect("至少输出一行 JSON");
    serde_json::from_str(line).expect("stdout 末行应为合法 JSON")
}

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

#[test]
fn test_user_add_list_remove_lifecycle() {
    let (path, url) = temp_db_url("user");

    // add
    let output = cli()
        .args([
            "user", "add", "--username", "ops_admin", "--password", "s3cret!", "--role", "admin",
            "--database-url", &url,
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
            "user", "add", "--username", "ops_admin", "--password", "x", "--database-url", &url,
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
        .args(["user", "remove", "--username", "ops_admin", "--database-url", &url])
        .output()
        .expect("run user remove");
    assert_eq!(rm.status.code(), Some(0));

    // 删除不存在的用户 → 运行时失败 1
    let rm2 = cli()
        .args(["user", "remove", "--username", "ops_admin", "--database-url", &url])
        .output()
        .expect("run user remove again");
    assert_eq!(rm2.status.code(), Some(1), "删除不存在用户应退出 1");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_user_invalid_args_exit_2() {
    let (path, url) = temp_db_url("user_bad");
    // 非法用户名 → 用法错误 2
    let output = cli()
        .args([
            "user", "add", "--username", "bad user!", "--password", "x", "--database-url", &url,
        ])
        .output()
        .expect("run invalid add");
    assert_eq!(output.status.code(), Some(2), "非法用户名应退出 2");
    let json = parse_json_line(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["error_code"], "invalid_username");

    // 空密码 → 用法错误 2
    let output2 = cli()
        .args([
            "user", "add", "--username", "ok_user", "--password", "", "--database-url", &url,
        ])
        .output()
        .expect("run empty password");
    assert_eq!(output2.status.code(), Some(2), "空密码应退出 2");

    let _ = std::fs::remove_file(&path);
}
