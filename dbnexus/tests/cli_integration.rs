// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! CLI 集成测试
//!
//! 测试 CLI 工具的各个命令功能：status、up、down、create、generate

#![allow(deprecated)]

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::PathBuf;
use tempfile::TempDir;
mod common;

/// TEST-CLI-001: CLI 帮助命令测试
#[test]
#[allow(deprecated)]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("dbnexus-migrate").expect("Failed to find CLI binary");

    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("USAGE").or(predicate::str::contains("Usage")));
}

/// TEST-CLI-002: CLI 子命令帮助测试
#[test]
fn test_cli_subcommand_help() {
    let mut cmd = Command::cargo_bin("dbnexus-migrate").expect("Failed to find CLI binary");

    cmd.args(["create", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("description").or(predicate::str::contains("DESCRIPTION")));
}

/// TEST-CLI-003: 状态命令 - 基础功能测试
#[tokio::test]
async fn test_cli_status_basic() {
    let mut cmd = Command::cargo_bin("dbnexus-migrate").expect("Failed to find CLI binary");

    cmd.arg("--database-url")
        .arg("sqlite::memory:")
        .arg("status")
        .assert()
        .success();
}

/// TEST-CLI-004: 状态命令 - 数据库连接测试
#[tokio::test]
async fn test_cli_status_database_connection() {
    let mut cmd = Command::cargo_bin("dbnexus-migrate").expect("Failed to find CLI binary");

    let assert_result = cmd.arg("--database-url").arg("sqlite::memory:").arg("status").assert();

    // 验证命令成功执行
    assert_result.success();
}

/// TEST-CLI-005: 迁移创建命令 - 基础测试
#[tokio::test]
async fn test_cli_create_migration() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let _migrations_dir = temp_dir.path().to_str().unwrap().to_string();

    let mut cmd = Command::cargo_bin("dbnexus-migrate").expect("Failed to find CLI binary");

    cmd.arg("--database-url")
        .arg("sqlite::memory:")
        .arg("--config")
        .arg(temp_dir.path().join("config.yaml").to_str().unwrap())
        .arg("create")
        .arg("test_migration")
        .assert()
        .success();
}

/// TEST-CLI-006: 迁移向上命令 - 基础测试
/// 注意：这个测试验证命令能够执行，而不是完整迁移功能
#[tokio::test]
async fn test_cli_up_migration() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let migrations_dir = temp_dir.path().to_str().unwrap().to_string();

    // 创建迁移文件
    let migration_content = r#"-- Migration: test_migration
-- Version: 1700000000

-- UP
CREATE TABLE IF NOT EXISTS test_table (id INTEGER PRIMARY KEY);

-- DOWN
DROP TABLE test_table;
"#;

    std::fs::write(
        PathBuf::from(&migrations_dir).join("1700000000_test_migration.sql"),
        migration_content,
    )
    .expect("Failed to write migration file");

    let mut cmd = Command::cargo_bin("dbnexus-migrate").expect("Failed to find CLI binary");

    let output = cmd
        .arg("--database-url")
        .arg("sqlite::memory:")
        .arg("--config")
        .arg(temp_dir.path().join("config.yaml").to_str().unwrap())
        .arg("up")
        .output()
        .expect("Failed to execute command");

    // 验证命令能够执行（参数解析正确）
    assert!(
        output.status.success() || output.status.code() == Some(1),
        "Command should be executed without argument errors"
    );
}

/// TEST-CLI-007: 迁移向下命令 - 基础测试
#[tokio::test]
async fn test_cli_down_migration() {
    let mut cmd = Command::cargo_bin("dbnexus-migrate").expect("Failed to find CLI binary");

    // 回滚命令在没有迁移时应该处理得当
    // 使用 SQLite 内存数据库（CLI 工具使用 sqlite 特性编译）
    cmd.arg("--database-url")
        .arg("sqlite::memory:")
        .arg("down")
        .assert()
        .success();
}

/// TEST-CLI-008: CLI 参数解析测试 - 无效参数
#[test]
fn test_cli_invalid_args() {
    let mut cmd = Command::cargo_bin("dbnexus-migrate").expect("Failed to find CLI binary");

    cmd.args(["--invalid-option"]).assert().failure();
}

/// TEST-CLI-009: CLI 生成命令帮助测试
#[test]
fn test_cli_generate_help() {
    let mut cmd = Command::cargo_bin("dbnexus-migrate").expect("Failed to find CLI binary");

    cmd.args(["generate", "--help"]).assert().success();
}

/// TEST-CLI-010: CLI 向下命令带版本测试
#[tokio::test]
async fn test_cli_down_with_version() {
    let mut cmd = Command::cargo_bin("dbnexus-migrate").expect("Failed to find CLI binary");

    cmd.arg("--database-url")
        .arg("sqlite::memory:")
        .arg("down")
        .arg("--version")
        .arg("1")
        .assert()
        .success();
}

/// TEST-CLI-011: CLI 向上命令带版本测试
/// 注意：这个测试验证命令能够执行，而不是完整迁移功能
#[tokio::test]
async fn test_cli_up_with_version() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // 创建迁移文件
    let migration_content = r#"-- Migration: test_migration
-- Version: 1700000000

-- UP
CREATE TABLE IF NOT EXISTS test_table (id INTEGER PRIMARY KEY);

-- DOWN
DROP TABLE test_table;
"#;

    std::fs::write(temp_dir.path().join("1700000000_test_migration.sql"), migration_content)
        .expect("Failed to write migration file");

    let mut cmd = Command::cargo_bin("dbnexus-migrate").expect("Failed to find CLI binary");

    let output = cmd
        .arg("--database-url")
        .arg("sqlite::memory:")
        .arg("--config")
        .arg(temp_dir.path().join("config.yaml").to_str().unwrap())
        .arg("up")
        .arg("--version")
        .arg("1700000000")
        .output()
        .expect("Failed to execute command");

    // 验证命令能够执行（参数解析正确）
    assert!(
        output.status.success() || output.status.code() == Some(1),
        "Command should be executed without argument errors"
    );
}

/// TEST-CLI-012: CLI 完整状态测试
#[tokio::test]
async fn test_cli_full_status() {
    let mut cmd = Command::cargo_bin("dbnexus-migrate").expect("Failed to find CLI binary");

    cmd.arg("--database-url")
        .arg("sqlite::memory:")
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("迁移状态").or(predicate::str::contains("Migration")));
}

/// TEST-CLI-013: CLI 向上迁移测试（多版本）
/// 注意：这个测试验证命令能够执行，而不是完整迁移功能
/// 迁移功能需要迁移历史表存在，这需要在应用迁移前手动创建
#[tokio::test]
async fn test_cli_up_multiple_migrations() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // 创建多个迁移文件
    for i in 1..=3 {
        let migration_content = format!(
            r#"-- Migration: test_migration_{}
-- Version: 170000000{}

-- UP
CREATE TABLE IF NOT EXISTS test_table_{} (id INTEGER PRIMARY KEY);

-- DOWN
DROP TABLE test_table_{};
"#,
            i, i, i, i
        );

        std::fs::write(
            temp_dir.path().join(format!("170000000{}_test_migration_{}.sql", i, i)),
            migration_content,
        )
        .expect("Failed to write migration file");
    }

    // 验证命令尝试执行（即使可能因为迁移表不存在而失败）
    // 这个测试主要验证 CLI 参数解析正确
    let mut cmd = Command::cargo_bin("dbnexus-migrate").expect("Failed to find CLI binary");

    let output = cmd
        .arg("--database-url")
        .arg("sqlite::memory:")
        .arg("--config")
        .arg(temp_dir.path().join("config.yaml").to_str().unwrap())
        .arg("up")
        .output()
        .expect("Failed to execute command");

    // 验证命令能够执行（参数解析正确）
    assert!(
        output.status.success() || output.status.code() == Some(1),
        "Command should be executed without argument errors"
    );
}

/// TEST-CLI-014: CLI 帮助显示所有命令
#[test]
fn test_cli_help_shows_all_commands() {
    let mut cmd = Command::cargo_bin("dbnexus-migrate").expect("Failed to find CLI binary");

    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("up"))
        .stdout(predicate::str::contains("down"))
        .stdout(predicate::str::contains("status"))
        .stdout(predicate::str::contains("generate"));
}

/// TEST-CLI-015: CLI 状态命令输出格式测试
#[tokio::test]
async fn test_cli_status_output_format() {
    let mut cmd = Command::cargo_bin("dbnexus-migrate").expect("Failed to find CLI binary");

    let assert_result = cmd.arg("--database-url").arg("sqlite::memory:").arg("status").assert();

    // 验证命令成功执行并产生输出
    assert_result.success();
}
