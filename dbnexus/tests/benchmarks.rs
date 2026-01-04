// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 性能基准测试
//!
//! 测试 dbnexus 的关键性能指标：
//! - 连接池获取延迟
//! - 权限检查开销
//! - 查询操作吞吐量
//! - 并发处理能力
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
use dbnexus::DbPool;
use dbnexus::permission::{Operation, PermissionConfig, RolePolicy, TablePermission};
use rand::Rng;
use std::sync::Arc;
use std::time::{Duration, Instant};

mod common;

/// 生成随机字符串用于测试数据
fn random_string(length: usize) -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();
    let result: String = (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();
    result
}

/// 基准测试：连接池获取延迟
fn pool_acquisition_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("pool_acquisition", |b| {
        b.to_async(&rt).iter_batched_async(
            || {
                let config = common::get_test_config();
                DbPool::with_config(config)
            },
            |pool| async move { black_box(pool.get_session("admin").await) },
            criterion::BatchSize::SmallInput,
        )
    });
}

/// 基准测试：权限检查开销
fn permission_check_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    // 创建权限配置
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
"#;
    let perm_config = PermissionConfig::from_yaml(perm_content).unwrap();

    c.bench_function("permission_check", |b| {
        b.to_async(&rt).iter_batched_async(
            || {
                let config = common::get_test_config();
                let pool = rt.block_on(DbPool::with_config(config)).unwrap();
                let session = rt.block_on(pool.get_session("admin")).unwrap();
                let mut ctx = session.permission_ctx().clone();
                rt.block_on(ctx.load_policy(&perm_config)).unwrap();
                ctx
            },
            |ctx| async move { black_box(ctx.check_table_access("users", &Operation::Select)) },
            criterion::BatchSize::SmallInput,
        )
    });
}

/// 基准测试：权限配置加载
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

/// 基准测试：池状态查询
fn pool_status_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("pool_status", |b| {
        b.to_async(&rt).iter_batched_async(
            || {
                let config = common::get_test_config();
                DbPool::with_config(config)
            },
            |pool| async move { black_box(pool.status()) },
            criterion::BatchSize::SmallInput,
        )
    });
}

/// 基准测试：配置自动修正
fn config_auto_correct_benchmark(c: &mut Criterion) {
    use dbnexus::config::ConfigCorrector;

    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("config_auto_correct", |b| {
        b.to_async(&rt).iter_batched_async(
            || {
                let config = common::get_test_config();
                (config, DbPool::with_config(config))
            },
            |(config, _pool)| async move {
                let corrector = ConfigCorrector::new();
                black_box(corrector.correct(&config))
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

/// 基准测试：并发会话获取吞吐量
fn concurrent_session_throughput(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("concurrent_session_throughput", |b| {
        b.to_async(&rt).iter_batched_async(
            || {
                let config = common::get_test_config();
                let pool = Arc::new(rt.block_on(DbPool::with_config(config)).unwrap());
                pool
            },
            |pool| async move {
                let num_sessions = 10;
                let mut handles = Vec::new();

                for i in 0..num_sessions {
                    let pool = pool.clone();
                    handles.push(tokio::spawn(
                        async move { pool.get_session(&format!("user{}", i)).await },
                    ));
                }

                let results = futures::future::join_all(handles).await;
                black_box(results.into_iter().filter(|r| r.is_ok()).count())
            },
            criterion::BatchSize::SmallInput,
        )
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

/// 基准测试：连接池满负载压力测试
fn pool_stress_test(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("pool_stress_test", |b| {
        b.to_async(&rt).iter_batched_async(
            || {
                let config = common::get_test_config();
                let pool = Arc::new(rt.block_on(DbPool::with_config(config)).unwrap());
                pool
            },
            |pool| async move {
                let num_operations = 50;
                let mut handles = Vec::new();

                for i in 0..num_operations {
                    let pool = pool.clone();
                    handles.push(tokio::spawn(async move {
                        let session = pool.get_session("admin").await?;
                        let status = pool.status();
                        Ok::<_, dbnexus::DbError>((session, status))
                    }));
                }

                let results = futures::future::join_all(handles).await;
                black_box(results.into_iter().filter(|r| r.is_ok()).count())
            },
            criterion::BatchSize::SmallInput,
        )
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
                black_box(dbnexus::config::DatabaseType::from_url(url));
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
                idle_timeout: Duration::from_secs(300),
                acquire_timeout: Duration::from_millis(5000),
                permissions_path: None,
            };
            black_box(config.validate())
        })
    });
}

/// 基准测试：会话角色获取
fn session_role_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("session_role", |b| {
        b.to_async(&rt).iter_batched_async(
            || {
                let config = common::get_test_config();
                rt.block_on(DbPool::with_config(config)).unwrap()
            },
            |pool| async move {
                let session = pool.get_session("admin").await.unwrap();
                black_box(session.role())
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

/// 基准测试：权限上下文克隆
fn permission_context_clone_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("permission_context_clone", |b| {
        b.to_async(&rt).iter_batched_async(
            || {
                let config = common::get_test_config();
                let pool = rt.block_on(DbPool::with_config(config)).unwrap();
                let session = rt.block_on(pool.get_session("admin")).unwrap();
                session.permission_ctx().clone()
            },
            |ctx| async move {
                for _ in 0..100 {
                    black_box(ctx.clone());
                }
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

criterion_group!(
    benches,
    pool_acquisition_benchmark,
    permission_check_benchmark,
    permission_config_load_benchmark,
    pool_status_benchmark,
    config_auto_correct_benchmark,
    concurrent_session_throughput,
    role_policy_check_benchmark,
    operation_display_benchmark,
    pool_stress_test,
    database_url_parse_benchmark,
    config_validation_benchmark,
    session_role_benchmark,
    permission_context_clone_benchmark,
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

/// 测量异步操作执行时间
#[allow(dead_code)]
async fn measure_duration_async<F, T, Fut>(name: &str, f: F) -> (T, Duration)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = T>,
{
    let start = Instant::now();
    let result = f().await;
    let duration = start.elapsed();
    (result, duration)
}

/// 打印性能测试结果
#[allow(dead_code)]
fn print_benchmark_result(name: &str, duration: Duration, iterations: u64) {
    let avg_ns = duration.as_nanos() / iterations;
    let avg_us = avg_ns as f64 / 1000.0;
    let avg_ms = avg_us / 1000.0;

    println!(
        "{}: {} iterations, avg: {:.2}ns ({:.4}μs, {:.4}ms)",
        name, iterations, avg_ns, avg_us, avg_ms
    );
}
