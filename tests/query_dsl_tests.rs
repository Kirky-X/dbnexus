// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 查询 DSL 宏 `q!` 测试
//!
//! 覆盖基础投影、条件组合（AND）、排序、分页、字符串转义、
//! `select *` 形态，以及宏产物经 `query_rows` 的端到端执行。

#![cfg(all(
    feature = "query-dsl",
    feature = "sqlite",
    feature = "sql-parser",
    feature = "runtime-tokio-rustls"
))]

use dbnexus::{q, QueryFragment};

/// 基础投影：ident 列名 + 表名
#[test]
fn test_basic_projection() {
    let fragment = q!(select [id, name] from users);
    assert_eq!(fragment.to_sql(), "SELECT id, name FROM users");
}

/// 条件组合：多条件 AND、各运算符、字符串转义、数值/布尔字面量
#[test]
fn test_condition_combinations() {
    let fragment = q! {
        select [id, name] from users
        where age > 18, status == "active", deleted != true, score >= 1.5, level <= 9
    };
    assert_eq!(
        fragment.to_sql(),
        "SELECT id, name FROM users WHERE age > 18 AND status = 'active' AND deleted != true AND score >= 1.5 AND level <= 9"
    );
}

/// 排序与分页
#[test]
fn test_order_and_limit() {
    let fragment = q! {
        select [id] from orders
        order by created_at desc
        limit 20
    };
    assert_eq!(fragment.to_sql(), "SELECT id FROM orders ORDER BY created_at DESC LIMIT 20");

    let fragment = q! {
        select [id] from orders
        order by id asc
        limit 5
    };
    assert_eq!(fragment.to_sql(), "SELECT id FROM orders ORDER BY id ASC LIMIT 5");
}

/// 字符串值内的单引号经标准转义（值逃逸不可能：literal token + 转义）
#[test]
fn test_string_value_escaping() {
    let fragment = q! {
        select [id] from users
        where name == "O'Brien"
    };
    assert_eq!(fragment.to_sql(), "SELECT id FROM users WHERE name = 'O''Brien'");
}

/// Rust 源码转义序列在生成 SQL 前被解码（换行不再以字面 `\n` 形态进入语句）
#[test]
fn test_escape_sequences_decoded() {
    use dbnexus::database::query_dsl::literal_from_token;

    // \n 解码为真实换行
    assert_eq!(literal_from_token("\"line1\\nline2\""), "'line1\nline2'");
    // \t / \r / \0
    assert_eq!(literal_from_token("\"a\\tb\""), "'a\tb'");
    assert_eq!(literal_from_token("\"a\\rb\""), "'a\rb'");
    assert_eq!(literal_from_token("\"a\\0b\""), "'a\0b'");
    // \\ 与 \" 解码
    assert_eq!(literal_from_token("\"a\\\\b\""), "'a\\b'");
    assert_eq!(literal_from_token("\"a\\\"b\""), "'a\"b'");
    // 解码后的单引号仍经 SQL 标准转义（防逃逸不变量）
    assert_eq!(literal_from_token("\"a\\'b\""), "'a''b'");
    // 双反斜杠 + n：解码为字面 `\n` 两字符（非换行），反斜杠原样保留
    assert_eq!(literal_from_token("\"a\\\\nb\""), "'a\\nb'");
    // 未知转义原样保留（\x.. 不支持，保持原文不失真）
    assert_eq!(literal_from_token("\"a\\x41b\""), "'a\\x41b'");
    // \u{...} 码点解码
    assert_eq!(literal_from_token("\"a\\u{4e2d}b\""), "'a中b'");
    // 非字符串字面量原样透传
    assert_eq!(literal_from_token("42"), "42");
    assert_eq!(literal_from_token("true"), "true");
}

/// select * 形态
#[test]
fn test_select_star() {
    let fragment = q! {
        select * from users
        where id == 1
    };
    assert_eq!(fragment.to_sql(), "SELECT * FROM users WHERE id = 1");
}

/// 条件组合 API：程序化追加（非宏路径）
#[test]
fn test_programmatic_condition_addition() {
    let mut fragment = QueryFragment::default();
    fragment.table = Some("t422".to_string());
    fragment.add_condition("age", dbnexus::DslOp::Gt, "10".to_string());
    assert_eq!(fragment.to_sql(), "SELECT * FROM t422 WHERE age > 10");
}

/// 端到端：宏产物经 query_rows 执行返回数据行
#[tokio::test]
async fn test_macro_fragment_executes_end_to_end() {
    let path = std::env::temp_dir().join(format!("dbnexus_t422_{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let url = format!("sqlite:{}?mode=rwc", path.display());
    let pool = dbnexus::DbPool::new(&url).await.expect("pool");
    let admin = pool.get_session("admin").await.expect("admin");
    admin
        .execute_raw_ddl("CREATE TABLE t422_users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)")
        .await
        .expect("create table");
    admin
        .execute_raw("INSERT INTO t422_users (id, name, age) VALUES (1, 'alice', 30), (2, 'bob', 15)")
        .await
        .expect("insert");

    let fragment = q! {
        select [id, name] from t422_users
        where age > 18
        order by id desc
    };
    let rows = pool.query_rows(&fragment.to_sql(), "admin").await.expect("query");
    assert_eq!(rows.len(), 1, "成人过滤应只剩 alice");
    assert_eq!(rows[0]["name"], "alice");

    let _ = std::fs::remove_file(&path);
}
