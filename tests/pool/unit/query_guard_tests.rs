// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! query-guard SQLite 真库计划信号测试

#![cfg(all(
    feature = "query-guard",
    feature = "sqlite",
    feature = "runtime-tokio-rustls"
))]

use dbnexus::database::query_guard::{collect_signals, fingerprint};
use dbnexus::foundation::DatabaseType;
use sea_orm::{ConnectionTrait, Database};

#[tokio::test]
async fn test_sqlite_explain_signals() {
    let conn = Database::connect("sqlite::memory:").await.unwrap();
    conn.execute_unprepared(
        "CREATE TABLE guard_users (id INTEGER PRIMARY KEY, email TEXT, name TEXT)",
    )
    .await
    .unwrap();
    for i in 0..20 {
        conn.execute_unprepared(&format!(
            "INSERT INTO guard_users (id, email, name) VALUES ({i}, 'u{i}@x', 'name{i}')"
        ))
        .await
        .unwrap();
    }
    conn.execute_unprepared("CREATE INDEX idx_guard_email ON guard_users(email)")
        .await
        .unwrap();

    let queries = vec![
        "SELECT * FROM guard_users WHERE email = 'u1@x'".to_string(),
        "SELECT * FROM guard_users WHERE name = 'name1'".to_string(),
    ];
    let signals = collect_signals(&conn, DatabaseType::Sqlite, &queries)
        .await
        .unwrap();
    let by_fp = |sql: &str| signals.get(&fingerprint(sql)).unwrap();
    assert!(
        by_fp(&queries[0]).uses_index,
        "indexed column should use index"
    );
    assert!(
        by_fp(&queries[1]).full_scan,
        "non-indexed column should full scan"
    );
    assert_eq!(by_fp(&queries[0]).est_rows, None, "sqlite has no est_rows");
}
