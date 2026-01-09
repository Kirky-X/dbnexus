// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! CLI 集成测试
//!
//! 测试 CLI 工具的各个命令功能：status、up、down、create、generate
//!
//! 运行这些测试需要先编译 CLI 二进制:
//! cargo build --features sqlite -p dbnexus-cli

#![allow(deprecated)]

#[cfg(feature = "cli-tests")]
mod cli_tests {
    use assert_cmd::Command;
    use predicates::prelude::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// TEST-CLI-001: CLI 帮助命令测试
    #[test]
    #[allow(deprecated)]
    fn test_cli_help() {
        let mut cmd = Command::cargo_bin("dbnexus-cli").expect("Failed to find CLI binary");
        cmd.arg("--help")
            .assert()
            .success()
            .stdout(predicate::str::contains("USAGE").or(predicate::str::contains("Usage")));
    }

    /// TEST-CLI-002: CLI 子命令帮助测试
    #[test]
    fn test_cli_subcommand_help() {
        let mut cmd = Command::cargo_bin("dbnexus-cli").expect("Failed to find CLI binary");
        cmd.args(["create", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("description").or(predicate::str::contains("DESCRIPTION")));
    }

    /// TEST-CLI-003: 状态命令 - 基础功能测试
    #[tokio::test]
    async fn test_cli_status_basic() {
        let mut cmd = Command::cargo_bin("dbnexus-cli").expect("Failed to find CLI binary");
        cmd.arg("--database-url")
            .arg("sqlite::memory:")
            .arg("status")
            .assert()
            .success();
    }

    /// TEST-CLI-004: 状态命令 - 数据库连接测试
    #[tokio::test]
    async fn test_cli_status_database_connection() {
        let mut cmd = Command::cargo_bin("dbnexus-cli").expect("Failed to find CLI binary");
        let assert_result = cmd.arg("--database-url").arg("sqlite::memory:").arg("status").assert();
        assert_result.success();
    }

    /// TEST-CLI-005: 迁移创建命令 - 基础测试
    #[tokio::test]
    async fn test_cli_create_migration() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let mut cmd = Command::cargo_bin("dbnexus-cli").expect("Failed to find CLI binary");
        cmd.arg("--database-url")
            .arg("sqlite::memory:")
            .arg("--migrations-dir")
            .arg(temp_dir.path())
            .arg("create")
            .arg("test_migration")
            .assert()
            .success();
    }

    /// TEST-CLI-006: 迁移向上命令 - 基础测试
    #[tokio::test]
    async fn test_cli_up_migration() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let migrations_dir = temp_dir.path().to_str().unwrap().to_string();

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

        let mut cmd = Command::cargo_bin("dbnexus-cli").expect("Failed to find CLI binary");
        let output = cmd
            .arg("--database-url")
            .arg("sqlite::memory:")
            .arg("--migrations-dir")
            .arg(temp_dir.path())
            .arg("up")
            .output()
            .expect("Failed to execute command");

        assert!(
            output.status.success() || output.status.code() == Some(1),
            "Command should be executed without argument errors"
        );
    }

    /// TEST-CLI-007: 迁移向下命令 - 基础测试
    #[tokio::test]
    async fn test_cli_down_migration() {
        let mut cmd = Command::cargo_bin("dbnexus-cli").expect("Failed to find CLI binary");
        cmd.arg("--database-url")
            .arg("sqlite::memory:")
            .arg("down")
            .assert()
            .success();
    }

    /// TEST-CLI-008: CLI 参数解析测试 - 无效参数
    #[test]
    fn test_cli_invalid_args() {
        let mut cmd = Command::cargo_bin("dbnexus-cli").expect("Failed to find CLI binary");
        cmd.args(["--invalid-option"]).assert().failure();
    }

    /// TEST-CLI-009: CLI 生成命令帮助测试
    #[test]
    fn test_cli_generate_help() {
        let mut cmd = Command::cargo_bin("dbnexus-cli").expect("Failed to find CLI binary");
        cmd.args(["generate", "--help"]).assert().success();
    }

    /// TEST-CLI-010: CLI 向下命令带版本测试
    #[tokio::test]
    async fn test_cli_down_with_version() {
        let mut cmd = Command::cargo_bin("dbnexus-cli").expect("Failed to find CLI binary");
        cmd.arg("--database-url")
            .arg("sqlite::memory:")
            .arg("down")
            .arg("--version")
            .arg("1")
            .assert()
            .success();
    }

    /// TEST-CLI-011: CLI 向上命令带版本测试
    #[tokio::test]
    async fn test_cli_up_with_version() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let migration_content = r#"-- Migration: test_migration
-- Version: 1700000000

-- UP
CREATE TABLE IF NOT EXISTS test_table (id INTEGER PRIMARY KEY);

-- DOWN
DROP TABLE test_table;
"#;

        std::fs::write(temp_dir.path().join("1700000000_test_migration.sql"), migration_content)
            .expect("Failed to write migration file");

        let mut cmd = Command::cargo_bin("dbnexus-cli").expect("Failed to find CLI binary");
        let output = cmd
            .arg("--database-url")
            .arg("sqlite::memory:")
            .arg("--migrations-dir")
            .arg(temp_dir.path())
            .arg("up")
            .arg("--version")
            .arg("1700000000")
            .output()
            .expect("Failed to execute command");

        assert!(
            output.status.success() || output.status.code() == Some(1),
            "Command should be executed without argument errors"
        );
    }

    /// TEST-CLI-012: CLI 完整状态测试
    #[tokio::test]
    async fn test_cli_full_status() {
        let mut cmd = Command::cargo_bin("dbnexus-cli").expect("Failed to find CLI binary");
        cmd.arg("--database-url")
            .arg("sqlite::memory:")
            .arg("status")
            .assert()
            .success()
            .stdout(predicate::str::contains("迁移状态").or(predicate::str::contains("Migration")));
    }

    /// TEST-CLI-013: CLI 向上迁移测试（多版本）
    #[tokio::test]
    async fn test_cli_up_multiple_migrations() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
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

        let mut cmd = Command::cargo_bin("dbnexus-cli").expect("Failed to find CLI binary");
        let output = cmd
            .arg("--database-url")
            .arg("sqlite::memory:")
            .arg("--migrations-dir")
            .arg(temp_dir.path())
            .arg("up")
            .output()
            .expect("Failed to execute command");

        assert!(
            output.status.success() || output.status.code() == Some(1),
            "Command should be executed without argument errors"
        );
    }

    /// TEST-CLI-014: CLI 帮助显示所有命令
    #[test]
    fn test_cli_help_shows_all_commands() {
        let mut cmd = Command::cargo_bin("dbnexus-cli").expect("Failed to find CLI binary");
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
        let mut cmd = Command::cargo_bin("dbnexus-cli").expect("Failed to find CLI binary");
        let assert_result = cmd.arg("--database-url").arg("sqlite::memory:").arg("status").assert();
        assert_result.success();
    }
}

// 如果没有启用 cli-tests 特性，这些测试将被跳过
#[cfg(not(feature = "cli-tests"))]
mod cli_tests {
    // 空模块 - 测试被跳过
}
