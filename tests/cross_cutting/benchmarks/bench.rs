// Copyright (c) 2026 Kirky.X
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
//! 配置解析通过 confers 库

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
use dbnexus::access::permission::{PermissionAction, PermissionConfig, RolePolicy, TablePermission};
use dbnexus::foundation::config::ConfigError;
use std::time::{Duration, Instant};

#[path = "../../common/mod.rs"]
mod common;

/// 使用 serde_json 直接解析 JSON 配置（测试用）
#[cfg(feature = "confers")]
fn parse_json_config(json: &str) -> Result<PermissionConfig, ConfigError> {
    serde_json::from_str(json).map_err(|e| ConfigError::InvalidFormat(format!("JSON deserialize error: {}", e)))
}

/// 基准测试：权限配置加载（同步）
#[cfg(feature = "confers")]
fn permission_config_load_benchmark(c: &mut Criterion) {
    c.bench_function("permission_config_load", |b| {
        b.iter(|| {
            let perm_content = r#"{
  "roles": {
    "admin": {
      "tables": [
        {
          "name": "*",
          "operations": ["select", "insert", "update", "delete"]
        }
      ]
    },
    "user": {
      "tables": [
        {
          "name": "users",
          "operations": ["select", "insert"]
        }
      ]
    }
  }
}"#;
            black_box(parse_json_config(perm_content).unwrap())
        })
    });
}

/// 基准测试：RolePolicy allows 检查
fn role_policy_check_benchmark(c: &mut Criterion) {
    let policy = RolePolicy {
        tables: vec![
            TablePermission {
                name: "users".to_string(),
                operations: vec![
                    PermissionAction::Select,
                    PermissionAction::Insert,
                    PermissionAction::Update,
                ],
            },
            TablePermission {
                name: "orders".to_string(),
                operations: vec![PermissionAction::Select, PermissionAction::Insert],
            },
        ],
    };

    c.bench_function("role_policy_check", |b| {
        b.iter(|| {
            black_box(policy.allows("users", &PermissionAction::Select));
            black_box(policy.allows("users", &PermissionAction::Delete));
            black_box(policy.allows("orders", &PermissionAction::Insert));
        })
    });
}

/// 基准测试：PermissionAction 显示转换
fn operation_display_benchmark(c: &mut Criterion) {
    let operations = vec![
        PermissionAction::Select,
        PermissionAction::Insert,
        PermissionAction::Update,
        PermissionAction::Delete,
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
                black_box(dbnexus::DatabaseType::from_url(url));
            }
        })
    });
}

/// 基准测试：配置验证
fn config_validation_benchmark(c: &mut Criterion) {
    c.bench_function("config_validation", |b| {
        b.iter(|| {
            let config = dbnexus::DbConfig {
                url: "sqlite::memory:".to_string(),
                max_connections: 10,
                min_connections: 1,
                idle_timeout: 300,
                acquire_timeout: 5000,
                admin_role: "admin".to_string(),
                ..Default::default()
            };
            black_box(config)
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
fn measure_duration<F, T>(_name: &str, f: F) -> (T, Duration)
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
