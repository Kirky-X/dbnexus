// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
#[path = "../../common/mod.rs"]
mod common;

#[tokio::test]
async fn test_concurrent_connection_acquire() {
    let (pool, _temp_dir) = common::create_test_pool().await.expect("Failed");
    let pool = std::sync::Arc::new(pool);
    let mut handles = Vec::new();
    for _ in 0..10 {
        let pool = pool.clone();
        handles.push(tokio::spawn(async move { pool.get_session("admin").await.ok() }));
    }
    let results: Vec<Result<Option<_>, _>> = futures::future::join_all(handles).await;
    let count = results.iter().filter(|r| r.as_ref().unwrap_or(&None).is_some()).count();
    eprintln!("Concurrent connection: {}/10", count);
    assert!(count >= 5, "At least half should succeed");
}
