// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 性能基准测试套件
//!
//! 使用 Criterion 进行全面的性能基准测试
//! 运行: cargo bench

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use dbnexus::{sql_parser::{SqlOperationType, SqlParser}, DbConfig};
use std::time::Duration;

/// 数据库配置基准
fn bench_db_config_building(c: &mut Criterion) {
    c.bench_function("db_config_build_basic", |b| {
        b.iter(|| {
            let _ = DbConfig {
                url: "sqlite::memory:".to_string(),
                max_connections: 10,
                min_connections: 1,
                ..Default::default()
            };
        })
    });

    c.bench_function("db_config_build_full", |b| {
        b.iter(|| {
            let _ = DbConfig {
                url: "postgresql://user:pass@localhost:5432/db".to_string(),
                max_connections: 100,
                min_connections: 5,
                idle_timeout: 300,
                acquire_timeout: 5000,
                permissions_path: Some("./permissions.yaml".to_string()),
                migrations_dir: Some("./migrations".to_string()),
                auto_migrate: true,
                ..Default::default()
            };
        })
    });
}

/// SQL 解析基准
fn bench_sql_parsing(c: &mut Criterion) {
    let parser = SqlParser::new();

    c.bench_function("sql_parse_select_simple", |b| {
        b.iter(|| {
            let _ = black_box(parser.parse_single("SELECT * FROM users WHERE id = 1"));
        })
    });

    c.bench_function("sql_parse_insert", |b| {
        b.iter(|| {
            let _ =
                black_box(parser.parse_single("INSERT INTO users (name, email) VALUES ('test', 'test@example.com')"));
        })
    });

    c.bench_function("sql_parse_update", |b| {
        b.iter(|| {
            let _ = black_box(parser.parse_single("UPDATE users SET name = 'new_name' WHERE id = 1"));
        })
    });

    c.bench_function("sql_parse_delete", |b| {
        b.iter(|| {
            let _ = black_box(parser.parse_single("DELETE FROM users WHERE id = 1"));
        })
    });

    c.bench_function("sql_parse_complex_select", |b| {
        b.iter(|| {
            let _ = black_box(parser.parse_single(
                "SELECT u.id, u.name, o.total
                 FROM users u
                 LEFT JOIN orders o ON u.id = o.user_id
                 WHERE u.status = 'active'
                 AND o.created_at > '2024-01-01'
                 ORDER BY o.total DESC
                 LIMIT 100",
            ));
        })
    });

    c.bench_function("sql_validate_operation_select", |b| {
        b.iter(|| {
            let result = parser.parse_single("SELECT * FROM users");
            let _ = black_box(matches!(result, Ok(r) if r.operation_type == SqlOperationType::Select));
        })
    });

    c.bench_function("sql_validate_operation_insert", |b| {
        b.iter(|| {
            let result = parser.parse_single("INSERT INTO users VALUES (1)");
            let _ = black_box(matches!(result, Ok(r) if r.operation_type == SqlOperationType::Insert));
        })
    });

    c.bench_function("sql_validate_operation_update", |b| {
        b.iter(|| {
            let result = parser.parse_single("UPDATE users SET id = 1");
            let _ = black_box(matches!(result, Ok(r) if r.operation_type == SqlOperationType::Update));
        })
    });

    c.bench_function("sql_validate_operation_delete", |b| {
        b.iter(|| {
            let result = parser.parse_single("DELETE FROM users");
            let _ = black_box(matches!(result, Ok(r) if r.operation_type == SqlOperationType::Delete));
        })
    });
}

/// 字符串操作基准
fn bench_string_operations(c: &mut Criterion) {
    c.bench_function("string_format_simple", |b| {
        b.iter(|| {
            let _ = format!("{}:{}:{}", "admin", "users", "SELECT");
        })
    });

    c.bench_function("string_format_long", |b| {
        b.iter(|| {
            let attrs: Vec<_> = (0..10).map(|i| format!("key_{}={}", i, i)).collect();
            let _ = attrs.join(":");
        })
    });

    c.bench_function("string_sanitize_basic", |b| {
        b.iter(|| {
            let input = "User logged in successfully. Query time: 123ms.";
            let sanitized: String = input
                .chars()
                .filter(|c| c.is_alphanumeric() || c.is_ascii_punctuation() || c.is_whitespace())
                .take(256)
                .collect();
            sanitized
        })
    });

    c.bench_function("string_mask_url", |b| {
        b.iter(|| {
            let url = "postgresql://user:password123@localhost:5432/db";
            let masked: String = url
                .chars()
                .map(|c| if c == '@' || c == ':' { c } else { '*' })
                .collect();
            masked
        })
    });
}

/// 配置解析基准
fn bench_config_parsing(c: &mut Criterion) {
    c.bench_function("config_parse_yaml", |b| {
        b.iter(|| {
            let yaml = r#"
roles:
  admin:
    - subject: admin
      resource: "*"
      actions: ["SELECT", "INSERT", "UPDATE", "DELETE"]
  user:
    - subject: user
      resource: users
      actions: ["SELECT"]
"#;
            let _ = serde_yaml::from_str::<serde_yaml::Value>(yaml);
        })
    });

    c.bench_function("config_url_parse_simple", |b| {
        b.iter(|| {
            let url = "sqlite::memory:";
            let _ = url::Url::parse(url);
        })
    });

    c.bench_function("config_url_parse_postgres", |b| {
        b.iter(|| {
            let url = "postgresql://user:pass@localhost:5432/db";
            let _ = url::Url::parse(url);
        })
    });
}

/// 集合操作基准
fn bench_collection_operations(c: &mut Criterion) {
    use std::collections::HashSet;

    c.bench_function("hashset_insert_100", |b| {
        b.iter(|| {
            let mut set = HashSet::new();
            for i in 0..100 {
                set.insert(i);
            }
            set
        })
    });

    c.bench_function("hashset_contains_100", |b| {
        b.iter(|| {
            let set: HashSet<_> = (0..100).collect();
            let mut found = 0;
            for i in 0..100 {
                if set.contains(&i) {
                    found += 1;
                }
            }
            found
        })
    });

    c.bench_function("vec_preallocated_100", |b| {
        b.iter(|| {
            let mut vec = Vec::with_capacity(100);
            for i in 0..100 {
                vec.push(i);
            }
            vec
        })
    });

    c.bench_function("vec_default_100", |b| {
        b.iter(|| {
            let mut vec = Vec::new();
            for i in 0..100 {
                vec.push(i);
            }
            vec
        })
    });
}

/// 正则表达式基准
fn bench_regex_operations(c: &mut Criterion) {
    use regex::Regex;

    static SQL_INJECTION_REGEX: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r#"(['";]|(--)|(/*)|\bOR\b|\bAND\b|\bUNION\b|\bSELECT\b)"#).unwrap());

    static PATH_TRAVERSAL_REGEX: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"\.\.|%2e%2e|/").unwrap());

    c.bench_function("regex_sql_injection_match", |b| {
        b.iter(|| {
            let input = "'; DROP TABLE users; --";
            let _ = SQL_INJECTION_REGEX.is_match(input);
        })
    });

    c.bench_function("regex_sql_injection_no_match", |b| {
        b.iter(|| {
            let input = "SELECT * FROM users WHERE id = 1";
            let _ = SQL_INJECTION_REGEX.is_match(input);
        })
    });

    c.bench_function("regex_path_traversal_match", |b| {
        b.iter(|| {
            let input = "../../../etc/passwd";
            let _ = PATH_TRAVERSAL_REGEX.is_match(input);
        })
    });

    c.bench_function("regex_path_traversal_no_match", |b| {
        b.iter(|| {
            let input = "/var/lib/data/file.txt";
            let _ = PATH_TRAVERSAL_REGEX.is_match(input);
        })
    });
}

/// 序列化基准
fn bench_serialization(c: &mut Criterion) {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct TestEvent {
        user_id: String,
        operation: String,
        entity_type: String,
        entity_id: String,
        timestamp: String,
        severity: String,
    }

    let test_event = TestEvent {
        user_id: "admin".to_string(),
        operation: "SELECT".to_string(),
        entity_type: "users".to_string(),
        entity_id: "123".to_string(),
        timestamp: "2024-01-01T12:00:00Z".to_string(),
        severity: "INFO".to_string(),
    };

    c.bench_function("json_serialize_event", |b| {
        b.iter(|| {
            let _ = serde_json::to_string(&test_event);
        })
    });

    c.bench_function("json_deserialize_event", |b| {
        b.iter(|| {
            let json = r#"{"user_id":"admin","operation":"SELECT","entity_type":"users","entity_id":"123","timestamp":"2024-01-01T12:00:00Z","severity":"INFO"}"#;
            let _: TestEvent = serde_json::from_str(json).unwrap();
        })
    });
}

/// 错误处理基准
fn bench_error_handling(c: &mut Criterion) {
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum TestError {
        #[error("Configuration error: {0}")]
        Config(String),
        #[error("Connection error: {0}")]
        Connection(String),
        #[error("Permission denied: {0}")]
        Permission(String),
    }

    c.bench_function("error_creation_config", |b| {
        b.iter(|| TestError::Config("Invalid configuration value".to_string()))
    });

    c.bench_function("error_creation_connection", |b| {
        b.iter(|| TestError::Connection("Connection refused".to_string()))
    });

    c.bench_function("error_display_config", |b| {
        b.iter(|| {
            let err = TestError::Config("Invalid configuration value".to_string());
            let _ = format!("{}", err);
        })
    });

    c.bench_function("error_chain_display", |b| {
        b.iter(|| {
            let err = TestError::Connection("Database connection failed".to_string());
            let _ = format!("Error: {}", err);
        })
    });
}

/// 临时文件操作基准
fn bench_temp_file_operations(c: &mut Criterion) {
    use std::io::Write;

    c.bench_function("temp_file_create", |b| {
        b.iter(|| {
            let mut file = tempfile::NamedTempFile::new().unwrap();
            file.write_all(b"test data").unwrap();
            file
        })
    });

    c.bench_function("temp_dir_create", |b| {
        b.iter(|| {
            let _ = tempfile::TempDir::new().unwrap();
        })
    });
}

/// 基准测试配置
criterion_group!(
    name = benches;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(Duration::from_secs(5))
        .warm_up_time(Duration::from_secs(1))
        .noise_threshold(0.02)
        .significance_level(0.01);
    targets =
        bench_db_config_building,
        bench_sql_parsing,
        bench_string_operations,
        bench_config_parsing,
        bench_collection_operations,
        bench_regex_operations,
        bench_serialization,
        bench_error_handling,
        bench_temp_file_operations,
);

criterion_main!(benches);
