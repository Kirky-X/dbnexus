// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! DuckDB COPY 批量写入契约测试
//!
//! - 编码层（纯函数）：`encode_duckdb_copy_rows` 的 CSV 转义规则
//!   （NULL=未引用空字段、空串=`""` 引用、逗号/引号/换行字段强制引用）。
//! - 语句层：`CopyStatement::build_from_file` 生成 `COPY ... FROM 'path'`
//!   及配套 CSV 选项，路径单引号转义，标识符白名单与 PG 路径同口径。
//! - 传输层：`DbPool::copy_in` 在 duckdb 驱动组下经临时文件 + `COPY FROM`
//!   真实落库（行数返回、NULL/空串/含分隔符字段 round-trip、注入载荷
//!   字面量化、空集拒绝、错误路径不残留临时文件）。

#![cfg(all(
    feature = "runtime-tokio-rustls",
    feature = "duckdb",
    feature = "copy",
    feature = "sql-parser"
))]

use dbnexus::database::copy::{CopyStatement, encode_duckdb_copy_rows};
use dbnexus::database::{DuckDbRow, DuckValue};

// 编码层：DuckDB CSV 转义规则

#[test]
fn test_encode_duckdb_rows_basic_types() {
    let rows = vec![vec![
        serde_json::json!(1),
        serde_json::json!(2.5),
        serde_json::json!(true),
        serde_json::json!("plain"),
    ]];
    assert_eq!(
        encode_duckdb_copy_rows(&rows),
        "1,2.5,true,plain\n",
        "ordinary values comma-separated, unquoted, trailing newline"
    );
}

#[test]
fn test_encode_duckdb_rows_null_vs_empty_string() {
    let rows = vec![vec![
        serde_json::Value::Null,
        serde_json::json!(""),
        serde_json::json!("x"),
    ]];
    let encoded = encode_duckdb_copy_rows(&rows);
    assert_eq!(
        encoded, ",\"\",x\n",
        "NULL should be an unquoted empty field (with NULLSTR '') and the empty string a quoted field to distinguish them"
    );
}

#[test]
fn test_encode_duckdb_rows_quotes_special_fields() {
    let rows = vec![
        vec![serde_json::json!("a,b")],
        vec![serde_json::json!("say \"hi\"")],
        vec![serde_json::json!("line1\nline2")],
        vec![serde_json::json!("\ttabbed")],
    ];
    // 整载荷逐字节断言（不能用 lines() 拆分：引用字段内嵌换行不是记录边界）
    let expected = concat!(
        "\"a,b\"\n",
        "\"say \"\"hi\"\"\"\n",
        "\"line1\nline2\"\n",
        "\ttabbed\n",
    );
    assert_eq!(encode_duckdb_copy_rows(&rows), expected);
}

#[test]
fn test_encode_duckdb_rows_empty_and_json_values() {
    assert_eq!(
        encode_duckdb_copy_rows(&[]),
        "",
        "an empty set encodes to an empty payload"
    );
    let rows = vec![vec![serde_json::json!({"k": 1}), serde_json::json!([1, 2])]];
    let encoded = encode_duckdb_copy_rows(&rows);
    // JSON 文本化（紧凑无空格）后含逗号/引号 → 整体引用并转义内嵌引号
    assert_eq!(encoded, "\"{\"\"k\"\":1}\",\"[1,2]\"\n");
}

// 语句层：build_from_file

#[test]
fn test_build_from_file_generates_csv_copy_with_paired_options() {
    let stmt = CopyStatement::new("t_copy", &["id".to_string(), "name".to_string()])
        .expect("a valid identifier should pass");
    let sql = stmt
        .build_from_file("/tmp/data.csv")
        .expect("a valid path should pass");
    assert_eq!(
        sql,
        r#"COPY "t_copy" ("id", "name") FROM '/tmp/data.csv' (FORMAT CSV, HEADER 0, DELIMITER ',', QUOTE '"', ESCAPE '"', NULLSTR '', ALLOW_QUOTED_NULLS false)"#,
        "the options string should pair with the encoding contract of encode_duckdb_copy_rows, actual: {sql}"
    );
}

#[test]
fn test_build_from_file_escapes_single_quotes_in_path() {
    let stmt = CopyStatement::new("t", &["a".to_string()]).expect("a valid identifier should pass");
    let sql = stmt
        .build_from_file("/tmp/it's.csv")
        .expect("a path containing a single quote should pass (escaped, not rejected)");
    assert!(
        sql.contains(r#"FROM '/tmp/it''s.csv'"#),
        "a single quote inside a path should be doubled to escape it and prevent statement escape, actual: {sql}"
    );
}

// 传输层：DbPool::copy_in（duckdb 驱动组）

async fn setup_duckdb_pool(table_ddl: &str) -> dbnexus::DbPool {
    let pool = dbnexus::DbPool::new("duckdb::memory:")
        .await
        .expect("duckdb pool");
    {
        let session = pool.get_session("admin").await.expect("admin session");
        session
            .execute_duckdb_raw(table_ddl)
            .await
            .expect("create table");
        // 建表后立即归还连接：`duckdb::memory:` 的池在 idle 队列空时会重新
        // open 出全新空库（连接间互不共享），copy_in 必须复用建过表的那条
        // 池连接——本组测试按"串行复用单连接"的口径组织
    }
    pool
}

async fn count_rows(pool: &dbnexus::DbPool, table: &str) -> i64 {
    let session = pool.get_session("admin").await.expect("admin session");
    let rows: Vec<DuckDbRow> = session
        .execute_duckdb(&format!("SELECT COUNT(*) AS cnt FROM {table}"))
        .await
        .expect("count query");
    match rows[0].get("cnt").expect("cnt column") {
        DuckValue::BigInt(n) => *n,
        DuckValue::Int(n) => *n as i64,
        other => panic!("unexpected count type: {other:?}"),
    }
}

#[tokio::test]
async fn test_copy_in_duckdb_loads_rows_and_reports_count() {
    let pool = setup_duckdb_pool("CREATE TABLE t_duck_copy (id INTEGER, name VARCHAR)").await;

    let stmt = CopyStatement::new("t_duck_copy", &["id".to_string(), "name".to_string()])
        .expect("a valid identifier should pass");
    let rows: Vec<Vec<serde_json::Value>> = (0..100)
        .map(|i| vec![serde_json::json!(i), serde_json::json!(format!("n{i}"))])
        .collect();

    let inserted = pool
        .copy_in(&stmt, &rows)
        .await
        .expect("copy_in should succeed");
    assert_eq!(
        inserted, 100,
        "copy_in should return the number of imported rows"
    );
    assert_eq!(
        count_rows(&pool, "t_duck_copy").await,
        100,
        "all rows should land in the database"
    );

    drop(pool);
}

#[tokio::test]
async fn test_copy_in_duckdb_roundtrip_null_empty_and_special_fields() {
    let pool =
        setup_duckdb_pool("CREATE TABLE t_round (id INTEGER, name VARCHAR, note VARCHAR)").await;

    let stmt = CopyStatement::new(
        "t_round",
        &["id".to_string(), "name".to_string(), "note".to_string()],
    )
    .expect("a valid identifier should pass");
    let rows = vec![
        vec![
            serde_json::json!(1),
            serde_json::Value::Null,
            serde_json::json!(""),
        ],
        vec![
            serde_json::json!(2),
            serde_json::json!("a,b \"q\""),
            serde_json::json!("multi\nline"),
        ],
        vec![
            serde_json::json!(3),
            serde_json::json!("x'); DROP TABLE t_round;--"),
            serde_json::Value::Null,
        ],
    ];
    let inserted = pool
        .copy_in(&stmt, &rows)
        .await
        .expect("copy_in should succeed");
    assert_eq!(inserted, 3);

    let session = pool.get_session("admin").await.expect("admin session");
    let result: Vec<DuckDbRow> = session
        .execute_duckdb("SELECT id, name, note FROM t_round ORDER BY id")
        .await
        .expect("round-trip query");
    assert_eq!(
        result.len(),
        3,
        "the injected payload should not take effect; the table should be intact"
    );

    let name_of = |r: &DuckDbRow| -> Option<String> {
        match r.get("name").expect("name column") {
            DuckValue::Null => None,
            DuckValue::Text(s) => Some(s.clone()),
            other => panic!("unexpected name type: {other:?}"),
        }
    };
    let note_of = |r: &DuckDbRow| -> Option<String> {
        match r.get("note").expect("note column") {
            DuckValue::Null => None,
            DuckValue::Text(s) => Some(s.clone()),
            other => panic!("unexpected note type: {other:?}"),
        }
    };

    // NULL 与空串语义可区分（round-trip 不塌缩）
    assert_eq!(name_of(&result[0]), None, "NULL should remain NULL");
    assert_eq!(
        name_of(&result[2]).as_deref(),
        Some("x'); DROP TABLE t_round;--")
    );
    assert_eq!(
        note_of(&result[0]).as_deref(),
        Some(""),
        "an empty string should remain an empty string"
    );
    assert_eq!(note_of(&result[1]).as_deref(), Some("multi\nline"));
    assert_eq!(name_of(&result[1]).as_deref(), Some("a,b \"q\""));

    drop(session);
    drop(pool);
}

#[tokio::test]
async fn test_copy_in_duckdb_rejects_empty_rows() {
    let pool = setup_duckdb_pool("CREATE TABLE t_empty (id INTEGER)").await;
    let stmt =
        CopyStatement::new("t_empty", &["id".to_string()]).expect("a valid identifier should pass");
    let err = pool.copy_in(&stmt, &[]).await.expect_err(
        "the empty set follows the same contract as the postgres path: explicit rejection",
    );
    assert!(
        format!("{err}").contains("row"),
        "the error should state the empty row set is invalid, actual: {err}"
    );
    drop(pool);
}

#[tokio::test]
async fn test_copy_in_duckdb_error_leaves_no_temp_file() {
    let pool = setup_duckdb_pool("CREATE TABLE t_other (id INTEGER)").await;

    // 断言口径为"无新增残留"而非"目录清空"：temp 目录可能存在历史孤儿
    // 或并行测试正在使用的载荷文件，均不在本测试的清理责任内
    let pre_existing: std::collections::HashSet<std::path::PathBuf> = std::env::temp_dir()
        .read_dir()
        .expect("read temp dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().starts_with("dbnexus_copy_"))
                .unwrap_or(false)
        })
        .collect();

    // 目标表不存在 → COPY 失败
    let stmt = CopyStatement::new("t_missing", &["id".to_string()])
        .expect("a valid identifier should pass");
    let result = pool.copy_in(&stmt, &[vec![serde_json::json!(1)]]).await;
    assert!(result.is_err(), "a missing target table should error");

    // 本测试新建的载荷文件成败皆清理；其它测试的活跃载荷不纳入断言
    let mut clean = false;
    for _ in 0..200 {
        let leftovers: Vec<_> = std::env::temp_dir()
            .read_dir()
            .expect("read temp dir")
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| !pre_existing.contains(p))
            .filter(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().starts_with("dbnexus_copy_"))
                    .unwrap_or(false)
            })
            .collect();
        if leftovers.is_empty() {
            clean = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(
        clean,
        "a failed path must not leave behind this run's COPY payload file (not converging within 2s)"
    );
    drop(pool);
}

// batch_insert × duckdb 参数化通道（json_to_duck_value 对接）

#[tokio::test]
async fn test_batch_insert_via_duckdb_param_channel_roundtrip() {
    use dbnexus::database::copy::{BatchInsertStatement, PlaceholderStyle};
    use dbnexus::database::json_to_duck_value;

    let pool = setup_duckdb_pool(
        "CREATE TABLE t_duck_batch (id INTEGER, score DOUBLE, label VARCHAR, extra VARCHAR)",
    )
    .await;

    let stmt = BatchInsertStatement::new(
        "t_duck_batch",
        &[
            "id".to_string(),
            "score".to_string(),
            "label".to_string(),
            "extra".to_string(),
        ],
    )
    .expect("a valid identifier should pass");
    let rows = vec![
        vec![
            serde_json::json!(1),
            serde_json::json!(2.5),
            serde_json::json!("hello"),
            serde_json::json!({"k": 1}),
        ],
        vec![
            serde_json::json!(2),
            serde_json::Value::Null,
            serde_json::Value::Null,
            serde_json::json!([1, 2]),
        ],
    ];
    let chunks = stmt
        .chunk_rows(&rows, PlaceholderStyle::QMark)
        .expect("the chunking should succeed");
    assert_eq!(chunks.len(), 1, "two rows in a single chunk");
    {
        let session = pool.get_session("admin").await.expect("admin session");
        for (sql, params) in &chunks {
            let binds: Vec<DuckValue> = params.iter().map(json_to_duck_value).collect();
            session
                .execute_duckdb_raw_with_params(sql, binds)
                .await
                .expect("the duckdb parameterized execution should succeed");
        }
    }

    let session = pool.get_session("admin").await.expect("admin session");
    let result: Vec<DuckDbRow> = session
        .execute_duckdb("SELECT id, score, label, extra FROM t_duck_batch ORDER BY id")
        .await
        .expect("round-trip query");
    assert_eq!(result.len(), 2, "both rows should land in the database");

    let as_text = |r: &DuckDbRow, col: &str| -> Option<String> {
        match r.get(col).expect(col) {
            DuckValue::Null => None,
            DuckValue::Text(s) => Some(s.clone()),
            other => panic!("unexpected {col} type: {other:?}"),
        }
    };
    // 类型 round-trip：整数列按 BIGINT 绑定读回整型、浮点保持 DOUBLE
    let (id, score) = match (
        result[0].get("id").expect("id"),
        result[0].get("score").expect("score"),
    ) {
        (DuckValue::Int(i), DuckValue::Double(s)) => (*i as i64, *s),
        (DuckValue::BigInt(i), DuckValue::Double(s)) => (*i, *s),
        other => panic!("unexpected id/score type: {other:?}"),
    };
    assert_eq!(id, 1);
    assert!(
        (score - 2.5).abs() < 1e-9,
        "floats should land in the database as-is"
    );
    assert_eq!(as_text(&result[0], "label").as_deref(), Some("hello"));
    // JSON 对象/数组经参数绑定文本化存储（与 sqlite 路径形态一致）
    assert_eq!(as_text(&result[0], "extra").as_deref(), Some(r#"{"k":1}"#));
    assert!(
        result[1].get("score").expect("score") == &DuckValue::Null,
        "NULL should remain NULL"
    );
    assert_eq!(as_text(&result[1], "label"), None);
    assert_eq!(as_text(&result[1], "extra").as_deref(), Some("[1,2]"));

    drop(session);
    drop(pool);
}
