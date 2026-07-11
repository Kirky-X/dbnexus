// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
#[path = "../../common/mod.rs"]
mod common;

#[tokio::test]
async fn test_multi_database_pool() {
    let (pool, _temp_dir) = common::create_test_pool().await.expect("Failed");
    let _session = pool.get_session("admin").await.expect("Failed");
    let status = pool.status();
    assert!(status.total >= 1);
}
