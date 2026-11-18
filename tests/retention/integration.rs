// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 数据保留策略端到端测试（SQLite 文件库）

#![cfg(all(
    feature = "retention",
    feature = "sqlite",
    feature = "runtime-tokio-rustls"
))]

use dbnexus::DbPool;
use dbnexus::domain::retention::{RetentionExecutor, RetentionPolicy};
use std::sync::Arc;

async fn setup() -> (DbPool, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!(
        "dbnexus_retention_{}_{}.db",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_file(&path);
    let url = format!("sqlite:{}?mode=rwc", path.display());
    let pool = DbPool::new(&url).await.unwrap();
    let session = pool.get_session("admin").await.unwrap();
    session
        .execute_raw_ddl(
            "CREATE TABLE retention_logs (id INTEGER PRIMARY KEY, created_at INTEGER, data TEXT)",
        )
        .await
        .unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    // 2 行过期（100 天前），1 行新鲜
    for (id, ts) in [(1, now - 200 * 86_400), (2, now - 150 * 86_400), (3, now)] {
        session
            .execute_raw(&format!(
                "INSERT INTO retention_logs (id, created_at, data) VALUES ({id}, {ts}, 'd{id}')"
            ))
            .await
            .unwrap();
    }
    (pool, path)
}

#[tokio::test]
async fn test_purge_expired_only_deletes_old_rows() {
    let (pool, path) = setup().await;
    let executor = RetentionExecutor::new(
        Arc::new(pool),
        "retention_logs",
        "admin",
        RetentionPolicy::parse("90d").unwrap(),
        "created_at",
    )
    .unwrap();
    let report = executor.purge_expired(0).await.unwrap();
    assert_eq!(report.deleted_rows, 2, "report: {report:?}");
    assert!(report.batches >= 1);
    assert!(report.cutoff.ends_with('Z'));

    let pool2 = DbPool::new(&format!("sqlite:{}?mode=rwc", path.display()))
        .await
        .unwrap();
    let session = pool2.get_session("admin").await.unwrap();
    let rows = session
        .query_rows("SELECT id FROM retention_logs")
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"], 3);
    std::fs::remove_file(&path).unwrap();
}

#[tokio::test]
async fn test_archive_failure_keeps_rows() {
    let (pool, path) = setup().await;
    let executor = RetentionExecutor::new(
        Arc::new(pool),
        "retention_logs",
        "admin",
        RetentionPolicy::parse("90d").unwrap(),
        "created_at",
    )
    .unwrap();
    let mut archiver = |_rows: &[serde_json::Value]| {
        Box::pin(async { Err(dbnexus::DbError::Config("archive failed".into())) })
            as std::pin::Pin<Box<dyn std::future::Future<Output = dbnexus::DbResult<()>> + Send>>
    };
    let result = executor.purge_expired_with_archive(0, &mut archiver).await;
    assert!(result.is_err(), "archive failure must abort purge");
    let pool2 = DbPool::new(&format!("sqlite:{}?mode=rwc", path.display()))
        .await
        .unwrap();
    let rows = pool2
        .query_rows("SELECT id FROM retention_logs", "admin")
        .await
        .unwrap();
    assert_eq!(rows.len(), 3, "no rows may be deleted on archive failure");
    std::fs::remove_file(&path).unwrap();
}

#[tokio::test]
async fn test_archive_success_deletes_rows() {
    let (pool, path) = setup().await;
    let executor = RetentionExecutor::new(
        Arc::new(pool),
        "retention_logs",
        "admin",
        RetentionPolicy::parse("90d").unwrap(),
        "created_at",
    )
    .unwrap();
    let archived = Arc::new(std::sync::Mutex::new(0usize));
    let counter = Arc::clone(&archived);
    let mut archiver = move |rows: &[serde_json::Value]| {
        let counter = Arc::clone(&counter);
        let n = rows.len();
        Box::pin(async move {
            *counter.lock().unwrap() += n;
            Ok(())
        })
            as std::pin::Pin<Box<dyn std::future::Future<Output = dbnexus::DbResult<()>> + Send>>
    };
    let report = executor
        .purge_expired_with_archive(0, &mut archiver)
        .await
        .unwrap();
    assert_eq!(report.archived_rows, 2);
    assert_eq!(report.deleted_rows, 2);
    assert_eq!(*archived.lock().unwrap(), 2);
    std::fs::remove_file(&path).unwrap();
}
