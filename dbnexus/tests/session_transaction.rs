//! Session 和事务集成测试

use dbnexus::DbPool;
mod common;

#[tokio::test]
async fn test_session_role() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");
    assert_eq!(session.role(), "admin");
}

#[tokio::test]
async fn test_session_permission_ctx() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");
    let ctx = session.permission_ctx();
    assert_eq!(ctx.role(), "admin");
}

#[tokio::test]
async fn test_session_mark_write() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");
    session.mark_write();
    assert!(session.should_use_master());
}

#[tokio::test]
async fn test_transaction_begin() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");
    assert!(!session.is_in_transaction());
    session.begin_transaction().await.expect("Failed to begin transaction");
    assert!(session.is_in_transaction());
}

#[tokio::test]
async fn test_transaction_commit() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");
    session.begin_transaction().await.expect("Failed to begin transaction");
    assert!(session.is_in_transaction());
    session.commit().await.expect("Failed to commit transaction");
    assert!(!session.is_in_transaction());
}

#[tokio::test]
async fn test_transaction_rollback() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");
    session.begin_transaction().await.expect("Failed to begin transaction");
    assert!(session.is_in_transaction());
    session.rollback().await.expect("Failed to rollback transaction");
    assert!(!session.is_in_transaction());
}

#[tokio::test]
async fn test_transaction_double_begin_error() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");
    session.begin_transaction().await.expect("Failed to begin transaction");
    let result = session.begin_transaction().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_transaction_commit_without_begin_error() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");
    let result = session.commit().await;
    assert!(result.is_err());
}
