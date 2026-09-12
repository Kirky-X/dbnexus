// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! COPY 批量写入契约测试
//!
//! - COPY 协议是 PostgreSQL 专属路径（`postgres` + `copy` feature 下启用协议传输）；
//!   sqlite 环境无 COPY 协议，本契约测试断言：语句构建/行编码纯函数可用，
//!   `copy_in` 走显式错误路径（契约：非 postgres 后端拒绝，绝不退化为逐行 INSERT）。
//! - pg 协议传输路径在 postgres 驱动组下经 `#[cfg(feature = "postgres")]` 编译门控，
//!   无 pg 服务器环境不执行传输断言（MVP 口径，真实联调留给使用方）。

#![cfg(all(
    feature = "runtime-tokio-rustls",
    feature = "sqlite",
    feature = "copy"
))]

use dbnexus::database::copy::{CopyFormat, CopyStatement};
use dbnexus::DbError;

fn temp_db_url(tag: &str) -> (String, std::path::PathBuf) {
    let path =
        std::env::temp_dir().join(format!("dbnexus_t407_{}_{}.db", tag, std::process::id()));
    (format!("sqlite:{}?mode=rwc", path.display()), path)
}

// ============================================================================
// COPY 语句构建（纯函数，任意驱动组可测）
// ============================================================================

#[test]
fn test_copy_statement_build_text() {
    let stmt = CopyStatement::new("t_copy", &["id".to_string(), "name".to_string()])
        .expect("合法标识符应通过校验");
    let sql = stmt.build();
    assert_eq!(
        sql,
        r#"COPY "t_copy" ("id", "name") FROM STDIN"#,
        "语句应为 postgres 文本格式 COPY FROM STDIN，实际: {sql}"
    );
    // format() 幂等（可重复取）
    assert_eq!(stmt.build(), sql);
}

#[test]
fn test_copy_statement_rejects_injection_identifiers() {
    // 表名/列名注入必须被构建期拒绝（标识符白名单：字母/数字/下划线/点）
    for bad in [
        "t; DROP TABLE users",
        "t\" --",
        "t OR 1=1",
        "t\\N",
        "",
        "t ",
    ] {
        let err = CopyStatement::new(bad, &["id".to_string()])
            .err()
            .expect("非法标识符应返回错误");
        assert!(
            format!("{err}").contains("identifier"),
            "错误应说明 identifier 非法，实际: {err}"
        );
    }
    // 列名注入同样拒绝
    assert!(CopyStatement::new("t", &["id; DROP TABLE x".to_string()]).is_err());
}

// ============================================================================
// 行编码（PG text COPY 格式转义）
// ============================================================================

#[test]
fn test_copy_row_encoding_escapes_and_null() {
    let rows = vec![
        vec![
            serde_json::json!(1),
            serde_json::json!("a\tb"),
            serde_json::json!("x\ny"),
            serde_json::json!("b\\s"),
            serde_json::Value::Null,
            serde_json::json!(""),
        ],
        vec![
            serde_json::json!(2.5),
            serde_json::json!(true),
            serde_json::json!({"k": 1}),
            serde_json::json!(null),
            serde_json::json!("c"),
            serde_json::json!(3),
        ],
    ];
    let encoded = dbnexus::database::copy::encode_copy_rows(&rows);
    let lines: Vec<&str> = encoded.lines().collect();
    assert_eq!(lines.len(), 2, "两行数据应以 \\n 分隔");
    assert_eq!(lines[0], "1\ta\\tb\tx\\ny\tb\\\\s\t\\N\t");
    assert_eq!(lines[1], "2.5\ttrue\t{\"k\":1}\t\\N\tc\t3");
}

// ============================================================================
// sqlite 契约：非 postgres 后端 copy_in 显式拒绝（跳过说明）
// ============================================================================

#[tokio::test]
async fn test_copy_in_contract_rejects_non_postgres_backend() {
    let (url, path) = temp_db_url("contract");
    let pool = dbnexus::DbPool::new(&url).await.unwrap();

    let stmt = CopyStatement::new("t_copy", &["id".to_string()]).unwrap();
    let rows = vec![vec![serde_json::json!(1)]];
    let err = pool.copy_in(&stmt, &rows).await.unwrap_err();

    match &err {
        DbError::Query(msg) | DbError::Config(msg) => {
            assert!(
                msg.contains("postgres"),
                "非 postgres 后端应报 COPY 仅 postgres 支持，实际: {msg}"
            );
        }
        other => panic!("应返回明确错误而非 panic，实际: {other:?}"),
    }

    let _ = std::fs::remove_file(&path);
}
