// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! db_repository 宏集成测试
//!
//! 验证 #[db_repository] 宏生成的 trait 和默认实现。

use dbnexus::{DbPool, db_repository};

// ============================================================================
// T027: 宏展开编译期验证
// ============================================================================

/// 使用 db_repository 宏定义仓储
#[db_repository(table = "test_users")]
struct TestUserRepository;

// 编译期验证：trait 存在且结构体实现了它
fn _assert_trait_impl(repo: &TestUserRepository) -> &dyn TestUserRepositoryTrait {
    repo
}

// ============================================================================
// T027: 运行时 CRUD 验证
// ============================================================================

/// 创建测试用 SQLite 内存池
async fn create_test_pool() -> DbPool {
    DbPool::new("sqlite::memory:")
        .await
        .expect("Failed to create test pool")
}

/// 初始化测试表
async fn setup_test_table(pool: &DbPool) {
    let session = pool.get_session("admin").await.expect("get_session");
    session
        .execute_raw_ddl("CREATE TABLE IF NOT EXISTS test_users (id INTEGER PRIMARY KEY, name TEXT, email TEXT)")
        .await
        .expect("create table");
}

/// T027: insert + find_by_id 端到端测试
#[tokio::test]
async fn test_repository_insert_and_find_by_id() {
    let pool = create_test_pool().await;
    setup_test_table(&pool).await;

    let repo = TestUserRepository;

    let data = serde_json::json!({"id": 1, "name": "Alice", "email": "alice@example.com"});
    repo.insert(&pool, data).await.expect("insert should succeed");

    let result = repo.find_by_id(&pool, 1).await.expect("find_by_id should succeed");
    assert!(result.is_some(), "should find the inserted record");
    let row = result.unwrap();
    assert_eq!(row["id"], 1);
}

/// T027: find_all 测试
#[tokio::test]
async fn test_repository_find_all() {
    let pool = create_test_pool().await;
    setup_test_table(&pool).await;

    let repo = TestUserRepository;

    repo.insert(
        &pool,
        serde_json::json!({"id": 1, "name": "Alice", "email": "a@test.com"}),
    )
    .await
    .unwrap();
    repo.insert(
        &pool,
        serde_json::json!({"id": 2, "name": "Bob", "email": "b@test.com"}),
    )
    .await
    .unwrap();

    let results = repo.find_all(&pool).await.expect("find_all should succeed");
    assert_eq!(results.len(), 2, "should find 2 records");
}

/// T027: update 测试
#[tokio::test]
async fn test_repository_update() {
    let pool = create_test_pool().await;
    setup_test_table(&pool).await;

    let repo = TestUserRepository;

    repo.insert(
        &pool,
        serde_json::json!({"id": 1, "name": "Alice", "email": "old@test.com"}),
    )
    .await
    .unwrap();
    repo.update(&pool, 1, serde_json::json!({"email": "new@test.com"}))
        .await
        .unwrap();

    // 验证更新成功（通过查询确认记录仍存在）
    let result = repo.find_by_id(&pool, 1).await.unwrap();
    assert!(result.is_some(), "record should still exist after update");
}

/// T027: delete_by_id 测试
#[tokio::test]
async fn test_repository_delete_by_id() {
    let pool = create_test_pool().await;
    setup_test_table(&pool).await;

    let repo = TestUserRepository;

    repo.insert(
        &pool,
        serde_json::json!({"id": 1, "name": "Alice", "email": "a@test.com"}),
    )
    .await
    .unwrap();
    repo.delete_by_id(&pool, 1).await.unwrap();

    let result = repo.find_by_id(&pool, 1).await.unwrap();
    assert!(result.is_none(), "record should be deleted");
}

/// T027: find_by_id 查询不存在的记录
#[tokio::test]
async fn test_repository_find_by_id_not_found() {
    let pool = create_test_pool().await;
    setup_test_table(&pool).await;

    let repo = TestUserRepository;
    let result = repo.find_by_id(&pool, 999).await.unwrap();
    assert!(result.is_none(), "non-existent record should be None");
}
