// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! DBNexus 示例项目
//!
//! 提供交互式菜单来浏览和运行所有 DBNexus 功能示例。
//!
//! # 运行方式
//!
//! ## 交互式菜单
//! ```bash
//! cargo run
//! ```
//!
//! ## 直接运行特定示例
//! ```bash
//! cargo run --bin quickstart
//! cargo run --bin permissions
//! cargo run --bin metrics
//! ```
//!
//! ## 通过命令行参数运行
//! ```bash
//! cargo run -- quickstart
//! cargo run -- permissions
//! ```

use std::env;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();

    // 如果提供了参数，直接运行指定的示例
    if args.len() > 1 {
        let example_name = &args[1];
        return run_example(example_name);
    }

    // 否则显示交互式菜单
    show_interactive_menu()
}

fn run_example(name: &str) -> ExitCode {
    let binary_name = match name {
        "quickstart" => "quickstart",
        "config" => "config",
        "permissions" => "permissions",
        "transactions" => "transactions",
        "sql_parser" => "sql_parser",
        "permission_engine" => "permission_engine",
        "metrics" => "metrics",
        "tracing" => "tracing",
        "audit" => "audit",
        "cache" => "cache",
        "sharding" => "sharding",
        "migration" => "migration",
        "global_index" => "global_index",
        _ => {
            eprintln!("❌ 未知的示例名称: {}", name);
            eprintln!("\n可用的示例:");
            list_examples();
            return ExitCode::FAILURE;
        }
    };

    println!("🚀 运行 {} 示例...\n", name);

    // 使用 cargo run --bin 运行指定的示例
    let status = Command::new("cargo")
        .args(["run", "--bin", binary_name])
        .status();

    match status {
        Ok(exit_status) => {
            if exit_status.success() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(exit_status.code().unwrap_or(1) as u8)
            }
        }
        Err(e) => {
            eprintln!("❌ 运行示例失败: {}", e);
            ExitCode::FAILURE
        }
    }
}

fn show_interactive_menu() -> ExitCode {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║           DBNexus 示例项目 - 交互式菜单                    ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();

    println!("📚 核心功能示例:");
    println!("  1. quickstart       - 快速开始示例");
    println!("  2. config           - 配置管理示例");
    println!("  3. permissions      - 权限控制示例");
    println!("  4. transactions     - 事务管理示例");
    println!("  5. sql_parser       - SQL 解析器示例");
    println!("  6. permission_engine - 权限引擎示例");
    println!();

    println!("🏢 企业功能示例:");
    println!("  7. metrics          - Prometheus 指标监控");
    println!("  8. tracing          - OpenTelemetry 分布式追踪");
    println!("  9. audit            - 审计日志");
    println!();

    println!("🚀 高级功能示例:");
    println!("  10. cache           - 缓存使用");
    println!("  11. sharding        - 分片管理");
    println!("  12. migration       - 数据库迁移");
    println!("  13. global_index    - 全局索引");
    println!();

    println!("🔧 其他:");
    println!("  0. 退出");
    println!();

    print!("请选择要运行的示例 (0-13): ");
    use std::io::Write;
    std::io::stdout().flush().unwrap();

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();

    match input.trim() {
        "0" => {
            println!("👋 再见！");
            ExitCode::SUCCESS
        }
        "1" => run_example("quickstart"),
        "2" => run_example("config"),
        "3" => run_example("permissions"),
        "4" => run_example("transactions"),
        "5" => run_example("sql_parser"),
        "6" => run_example("permission_engine"),
        "7" => run_example("metrics"),
        "8" => run_example("tracing"),
        "9" => run_example("audit"),
        "10" => run_example("cache"),
        "11" => run_example("sharding"),
        "12" => run_example("migration"),
        "13" => run_example("global_index"),
        _ => {
            println!("❌ 无效的选择，请重新运行程序");
            ExitCode::FAILURE
        }
    }
}

fn list_examples() {
    println!("  核心功能:");
    println!("    - quickstart");
    println!("    - config");
    println!("    - permissions");
    println!("    - transactions");
    println!("    - sql_parser");
    println!("    - permission_engine");
    println!("  企业功能:");
    println!("    - metrics");
    println!("    - tracing");
    println!("    - audit");
    println!("  高级功能:");
    println!("    - cache");
    println!("    - sharding");
    println!("    - migration");
    println!("    - global_index");
}