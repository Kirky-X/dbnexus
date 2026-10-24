// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! CLI 参数面回归测试（assert_cmd 驱动真实二进制）
//!
//! global 短选项会被 clap 传播进每个子命令的作用域：子命令内声明同名短选项时，
//! 命令树构建期即因重名断言 panic（debug 构建），故逐子命令校验 `--help` 可正常渲染。

#![cfg(all(
    feature = "sqlite",
    feature = "migration",
    feature = "health-check",
    feature = "permission-engine"
))]

use assert_cmd::Command;
use predicates::prelude::*;

/// 定位 dbnexus-cli 二进制
fn cli() -> Command {
    Command::cargo_bin("dbnexus-cli").expect("dbnexus-cli binary")
}

/// 各子命令 `--help` 均应成功渲染（命令树构建期不得因短选项重名而 panic）
#[test]
fn test_all_subcommand_help_renders() {
    for subcommand in [
        "create",
        "up",
        "down",
        "status",
        "generate",
        "list",
        "migrate",
        "test-connection",
        "health",
        "user",
        "pool-status",
        "permission-check",
    ] {
        cli()
            .args([subcommand, "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Usage"))
            .stderr(predicate::str::contains("panicked").not());
    }
}

/// global 短选项 `-d` 在子命令作用域内恒为 `--database-url`，不得被子命令参数占用
#[test]
fn test_global_short_database_url_usable_in_subcommand_scope() {
    cli()
        .args(["health", "-d", "foo://localhost/db"])
        .assert()
        .code(2)
        .stdout(predicate::str::contains("\"url\":\"invalid\""));
}
