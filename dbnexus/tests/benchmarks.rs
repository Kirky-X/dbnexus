// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 性能基准测试
//!
//! 测试 dbnexus 的关键性能指标：
//! - 权限策略检查开销
//! - 配置解析性能
//! - Operation 显示转换
//!
//! # 运行基准测试
//!
//! ```bash
//! cargo bench --features sqlite
//! ```
//!
//! # 注意
//!
//! 这些测试设计用于性能验证，实际延迟会因硬件和环境而异。

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use dbnexus::permission::{Operation, PermissionConfig, RolePolicy, TablePermission};
use std::time::{Duration, Instant};

mod common;

/// 基准测试：权限配置加载（同步）
fn permission_config_load_benchmark(c: &mut Criterion) {
    c.bench_function("permission_config_load", |b| {
        b.iter(|| {
            let perm_content = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations:
          - SELECT
          - INSERT
          - UPDATE
          - DELETE
  user:
    tables:
      - name: "users"
        operations:
          - SELECT
          - INSERT
"#;
            black_box(PermissionConfig::from_yaml(perm_content).unwrap())
        })
    });
}

/// 基准测试：RolePolicy allows 检查
fn role_policy_check_benchmark(c: &mut Criterion) {
    let policy = RolePolicy {
        tables: vec![
            TablePermission {
                name: "users".to_string(),
                operations: vec![Operation::Select, Operation::Insert, Operation::Update],
            },
            TablePermission {
                name: "orders".to_string(),
                operations: vec![Operation::Select, Operation::Insert],
            },
        ],
    };

    c.bench_function("role_policy_check", |b| {
        b.iter(|| {
            black_box(policy.allows("users", &Operation::Select));
            black_box(policy.allows("users", &Operation::Delete));
            black_box(policy.allows("orders", &Operation::Insert));
        })
    });
}

/// 基准测试：Operation 显示转换
fn operation_display_benchmark(c: &mut Criterion) {
    let operations = vec![
        Operation::Select,
        Operation::Insert,
        Operation::Update,
        Operation::Delete,
    ];

    c.bench_function("operation_display", |b| {
        b.iter(|| {
            for op in &operations {
                black_box(op.to_string());
            }
        })
    });
}

/// 基准测试：数据库URL解析
fn database_url_parse_benchmark(c: &mut Criterion) {
    let urls = vec![
        "sqlite::memory:",
        "sqlite:./test.db",
        "postgres://user:pass@localhost:5432/dbname",
        "mysql://user:pass@localhost:3306/dbname",
    ];

    c.bench_function("database_url_parse", |b| {
        b.iter(|| {
            for url in &urls {
                black_box(dbnexus::config::DatabaseType::parse_database_type(url));
            }
        })
    });
}

/// 基准测试：配置验证
fn config_validation_benchmark(c: &mut Criterion) {
    use dbnexus::config::DbConfig;

    c.bench_function("config_validation", |b| {
        b.iter(|| {
            let config = DbConfig {
                url: "sqlite::memory:".to_string(),
                max_connections: 10,
                min_connections: 1,
                idle_timeout: 300,
                acquire_timeout: 5000,
                permissions_path: None,
                migrations_dir: None,
                auto_migrate: false,
                migration_timeout: 60,
            };
            black_box(config.validate())
        })
    });
}

criterion_group!(
    benches,
    permission_config_load_benchmark,
    role_policy_check_benchmark,
    operation_display_benchmark,
    database_url_parse_benchmark,
    config_validation_benchmark,
);

criterion_main!(benches);

/// 性能测试辅助函数

/// 测量操作执行时间
#[allow(dead_code)]
fn measure_duration<F, T>(name: &str, f: F) -> (T, Duration)
where
    F: FnOnce() -> T,
{
    let start = Instant::now();
    let result = f();
    let duration = start.elapsed();
    (result, duration)
}

/// 打印性能测试结果
#[allow(dead_code)]
fn print_benchmark_result(name: &str, duration: Duration, iterations: u64) {
    let avg_ns = duration.as_nanos() / iterations as u128;
    let avg_us = avg_ns as f64 / 1000.0;
    let avg_ms = avg_us / 1000.0;

    println!(
        "{}: {} iterations, avg: {}ns ({:.4}μs, {:.4}ms)",
        name, iterations, avg_ns, avg_us, avg_ms
    );
}