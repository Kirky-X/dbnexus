// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! DbNexusKit 外部测试（T088-T090）
//!
//! 通过公共 API 测试 Kit 的 capability 注册、解析、替换、并发访问等场景。
//! 使用 Mock 实现来避免真实数据库依赖。

use dbnexus::DbNexusKit;
use std::sync::Arc;

// ============================================================================
// Mock 实现（用于测试，避免真实数据库依赖）
// ============================================================================

use async_trait::async_trait;
use dbnexus::Session;
use dbnexus::database::pool::{ConnectionPool, DatabaseSession, PoolStatus};
use dbnexus::foundation::config::DbConfig;
use dbnexus::foundation::error::DbError;

struct MockConnectionPool {
    status: PoolStatus,
}

impl MockConnectionPool {
    fn new() -> Self {
        Self {
            status: PoolStatus {
                total: 1,
                active: 0,
                idle: 1,
                wait_count: 0,
                max_waiters: 10,
                borrow_count: 0,
                max_active: 10,
            },
        }
    }
}

#[async_trait]
impl ConnectionPool for MockConnectionPool {
    async fn get_session(&self, _role: &str) -> Result<Session, DbError> {
        Err(DbError::Config("mock pool does not provide real sessions".to_string()))
    }

    fn status(&self) -> PoolStatus {
        self.status.clone()
    }

    fn config(&self) -> &DbConfig {
        // 返回一个静态默认配置的引用
        use std::sync::OnceLock;
        static CONFIG: OnceLock<DbConfig> = OnceLock::new();
        CONFIG.get_or_init(DbConfig::default)
    }
}

struct MockDatabaseSession {
    role: String,
}

#[async_trait]
impl DatabaseSession for MockDatabaseSession {
    async fn execute(&self, _sql: &str) -> Result<sea_orm::ExecResult, DbError> {
        Err(DbError::Config("mock session cannot execute".to_string()))
    }

    async fn execute_raw(&self, _sql: &str) -> Result<sea_orm::ExecResult, DbError> {
        Err(DbError::Config("mock session cannot execute".to_string()))
    }

    async fn execute_raw_ddl(&self, _sql: &str) -> Result<sea_orm::ExecResult, DbError> {
        Err(DbError::Config("mock session cannot execute".to_string()))
    }

    async fn begin_transaction(&self) -> Result<(), DbError> {
        Ok(())
    }

    async fn commit(&self) -> Result<(), DbError> {
        Ok(())
    }

    async fn rollback(&self) -> Result<(), DbError> {
        Ok(())
    }

    fn role(&self) -> &str {
        &self.role
    }

    async fn is_in_transaction(&self) -> bool {
        false
    }
}

fn make_mock_pool() -> Arc<dyn ConnectionPool> {
    Arc::new(MockConnectionPool::new())
}

fn make_mock_session(role: &str) -> Arc<dyn DatabaseSession> {
    Arc::new(MockDatabaseSession { role: role.to_string() })
}

// ============================================================================
// T089: Kit 测试
// ============================================================================

/// TEST-KIT-001: 注册并解析 capability
#[test]
fn test_kit_provide_and_resolve() {
    let kit = DbNexusKit::new();
    assert!(!kit.has_connection_pool());

    let pool = make_mock_pool();
    kit.provide_connection_pool(pool).expect("provide should succeed");
    assert!(kit.has_connection_pool());

    let resolved = kit.connection_pool();
    assert!(resolved.is_ok(), "resolve should succeed: {:?}", resolved.err());
}

/// TEST-KIT-002: 解析未注册的 capability 应失败
#[test]
fn test_kit_resolve_nonexistent() {
    let kit = DbNexusKit::new();
    assert!(!kit.has_connection_pool());

    let result = kit.connection_pool();
    assert!(result.is_err(), "resolve unregistered should fail");
}

/// TEST-KIT-003: 替换已注册的 capability
#[test]
fn test_kit_replace() {
    let kit = DbNexusKit::new();
    let pool1 = make_mock_pool();
    kit.provide_connection_pool(pool1).expect("provide should succeed");

    // 替换
    let pool2 = make_mock_pool();
    kit.replace_connection_pool(pool2);
    assert!(kit.has_connection_pool());

    // 仍然可解析
    let resolved = kit.connection_pool();
    assert!(resolved.is_ok(), "resolve after replace should succeed");
}

/// TEST-KIT-004: 替换未注册的 capability 不应 panic
#[test]
fn test_kit_replace_nonexistent() {
    let kit = DbNexusKit::new();
    assert!(!kit.has_database_session());

    // 替换未注册的 capability — 不应 panic
    let session = make_mock_session("admin");
    kit.replace_database_session(session);
    // replace 会直接插入
    assert!(kit.has_database_session());
}

/// TEST-KIT-005: 多个 capability 共存
#[test]
fn test_kit_multiple_capabilities() {
    let kit = DbNexusKit::new();

    let pool = make_mock_pool();
    let session = make_mock_session("admin");

    kit.provide_connection_pool(pool).expect("provide pool should succeed");
    kit.provide_database_session(session)
        .expect("provide session should succeed");

    assert!(kit.has_connection_pool());
    assert!(kit.has_database_session());

    // 两者都应可独立解析
    assert!(kit.connection_pool().is_ok());
    assert!(kit.database_session().is_ok());
}

/// TEST-KIT-006: 类型安全 — 不同 capability 不会混淆
#[test]
fn test_kit_type_safety() {
    let kit = DbNexusKit::new();

    let pool = make_mock_pool();
    let session = make_mock_session("reader");

    kit.provide_connection_pool(pool).expect("provide pool");
    kit.provide_database_session(session).expect("provide session");

    // connection_pool() 返回 ConnectionPool trait 对象
    let pool_resolved = kit.connection_pool().expect("resolve pool");
    assert_eq!(pool_resolved.status().total, 1);

    // database_session() 返回 DatabaseSession trait 对象
    let session_resolved = kit.database_session().expect("resolve session");
    assert_eq!(session_resolved.role(), "reader");
}

/// TEST-KIT-007: 并发访问 Kit
#[tokio::test]
async fn test_kit_concurrent_access() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let kit = DbNexusKit::new();
    let pool = make_mock_pool();
    kit.provide_connection_pool(pool).expect("provide pool");

    let kit_clone = kit.clone();
    let counter = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    for _ in 0..10 {
        let kit = kit_clone.clone();
        let counter = counter.clone();
        handles.push(tokio::spawn(async move {
            if kit.connection_pool().is_ok() {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        }));
    }

    for handle in handles {
        handle.await.expect("task should complete");
    }

    assert_eq!(
        counter.load(Ordering::SeqCst),
        10,
        "all concurrent accesses should succeed"
    );
}

/// TEST-KIT-008: Kit clone 共享底层状态
#[test]
fn test_kit_drop_cleanup() {
    let kit = DbNexusKit::new();
    let pool = make_mock_pool();
    kit.provide_connection_pool(pool).expect("provide pool");

    // clone 共享状态
    let kit_clone = kit.clone();
    assert!(kit_clone.has_connection_pool());

    // drop 原始 kit — clone 仍应可用
    drop(kit);
    assert!(
        kit_clone.has_connection_pool(),
        "clone should retain capabilities after original is dropped"
    );
    assert!(kit_clone.connection_pool().is_ok());
}
