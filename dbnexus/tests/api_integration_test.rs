// Temporary integration test for DBNexus API
// Run with: cargo test --features "sqlite" --test api_integration_test

use dbnexus::PermissionAction;
use dbnexus::{
    DbError, DbPool,
    config::{DbConfig, DbConfigBuilder},
    pool::PoolStatus,
};

#[tokio::test]
async fn test_config_builder_basic() {
    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(10)
        .min_connections(2)
        .build()
        .unwrap();

    assert_eq!(config.url, "sqlite::memory:");
    assert_eq!(config.max_connections, 10);
}

#[tokio::test]
async fn test_config_direct() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 15,
        min_connections: 3,
        idle_timeout: 600,
        acquire_timeout: 10000,
        permissions_path: None,
        migrations_dir: None,
        auto_migrate: false,
        migration_timeout: 120,
        admin_role: "admin".to_string(),
    };

    assert_eq!(config.max_connections, 15);
}

#[tokio::test]
async fn test_dbpool_new() {
    let pool = DbPool::new("sqlite::memory:").await.unwrap();
    let status = pool.status();
    assert!(status.total >= 1);
}

#[tokio::test]
async fn test_dbpool_try_from() {
    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(10)
        .min_connections(3)
        .build()
        .unwrap();

    let pool = DbPool::try_from(&config).unwrap();
    assert_eq!(pool.config().max_connections, 10);
}

#[tokio::test]
async fn test_session_get() {
    let pool = DbPool::new("sqlite::memory:").await.unwrap();
    let session = pool.get_session("admin").await.unwrap();
    assert_eq!(session.role(), "admin");
}

#[tokio::test]
async fn test_permission_action() {
    assert_eq!(PermissionAction::Select.to_string(), "SELECT");
    assert_eq!(PermissionAction::Insert.to_string(), "INSERT");
    assert_eq!(PermissionAction::Update.to_string(), "UPDATE");
    assert_eq!(PermissionAction::Delete.to_string(), "DELETE");
}

#[tokio::test]
async fn test_pool_status() {
    let status = PoolStatus {
        total: 10,
        active: 3,
        idle: 7,
    };
    assert_eq!(status.total, 10);
    assert_eq!(status.active, 3);
    assert_eq!(status.idle, 7);
}

#[tokio::test]
async fn test_db_error() {
    let err: DbError = DbError::Config("test".to_string());
    assert!(err.to_string().contains("Configuration error"));
}

#[tokio::test]
async fn test_yaml_loading() {
    let yaml = r#"url: "sqlite::memory:""#;
    let config = DbConfig::from_yaml_str(yaml).unwrap();
    assert_eq!(config.url, "sqlite::memory:");
}

#[tokio::test]
async fn test_database_type() {
    use dbnexus::config::DatabaseType;
    assert_eq!(
        DatabaseType::parse_database_type("sqlite::memory:"),
        DatabaseType::Sqlite
    );
    assert_eq!(DatabaseType::Sqlite.as_str(), "sqlite");
    assert!(!DatabaseType::Sqlite.is_real_database());
}

#[tokio::test]
async fn test_execute_raw() {
    let pool = DbPool::new("sqlite::memory:").await.unwrap();
    let session = pool.get_session("admin").await.unwrap();

    session
        .execute_raw("CREATE TABLE test (id INTEGER PRIMARY KEY, value TEXT)")
        .await
        .unwrap();
    session
        .execute_raw("INSERT INTO test (value) VALUES ('hello')")
        .await
        .unwrap();

    let result = session.execute_raw("SELECT COUNT(*) FROM test").await;
    assert!(result.is_ok());
}
