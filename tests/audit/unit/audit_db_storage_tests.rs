// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! AuditStorage 的 DB 实现（DbAuditStorage）sqlite 单测
//!
//! saga_logs 同款模式：DDL IF NOT EXISTS + upsert + query_rows 读取/清理。
//! 不开 permission feature（对齐 saga_unit_tests db 测试模式）。

#![cfg(all(
    feature = "runtime-tokio-rustls",
    feature = "sqlite",
    feature = "sql-parser",
    feature = "audit"
))]

use std::sync::Arc;

use dbnexus::{
    AuditEvent, AuditOperation, AuditQueryFilters, AuditSeverity, AuditStatus, AuditStorage,
    DbAuditStorage,
};

fn temp_db_url(tag: &str) -> (String, std::path::PathBuf) {
    let path =
        std::env::temp_dir().join(format!("dbnexus_t408_{}_{}.db", tag, std::process::id()));
    (format!("sqlite:{}?mode=rwc", path.display()), path)
}

#[tokio::test]
async fn test_db_audit_store_and_query_roundtrip() {
    let (url, path) = temp_db_url("roundtrip");
    let pool = Arc::new(dbnexus::DbPool::new(&url).await.unwrap());
    let storage = DbAuditStorage::new(pool);
    storage.init().await.unwrap();

    let e1 = AuditEvent::create("users", "1", "admin")
        .with_user("root", "10.0.0.1")
        .with_severity(AuditSeverity::High);
    let e2 = AuditEvent::delete("orders", "9", "analyst")
        .with_user("operator", "10.0.0.2")
        .with_severity(AuditSeverity::Critical);

    storage.store(&e1).await.unwrap();
    storage.store(&e2).await.unwrap();

    // 全量查询
    let all = storage.query(&AuditQueryFilters::default()).await.unwrap();
    assert_eq!(all.len(), 2, "应读回 2 条审计事件");

    // 按 entity_type 过滤
    let filtered = storage
        .query(&AuditQueryFilters {
            entity_type: Some("orders".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].entity_id, "9");
    assert_eq!(filtered[0].operation, AuditOperation::Delete);
    assert_eq!(filtered[0].severity, AuditSeverity::Critical);
    assert_eq!(filtered[0].user_id, "analyst");

    // 按 user_id 过滤（构造器第三参为 user_id；with_user 设置 role/ip）
    let by_user = storage
        .query(&AuditQueryFilters {
            user_id: Some("admin".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(by_user.len(), 1);
    assert_eq!(by_user[0].entity_type, "users");
    assert_eq!(by_user[0].user_role, "root");

    // 事件字段完整往返（JSON 载荷）
    assert_eq!(by_user[0].id, e1.id);
    assert_eq!(by_user[0].request_id, e1.request_id);
    assert_eq!(by_user[0].timestamp, e1.timestamp);

    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn test_db_audit_filters_time_and_cleanup() {
    let (url, path) = temp_db_url("cleanup");
    let pool = Arc::new(dbnexus::DbPool::new(&url).await.unwrap());
    let storage = DbAuditStorage::new(pool);
    storage.init().await.unwrap();

    let now = chrono::Utc::now();
    let mut old = AuditEvent::create("users", "old", "u1");
    old.timestamp = now - chrono::Duration::days(2);
    let mut fresh = AuditEvent::create("users", "new", "u2");
    fresh.timestamp = now;

    storage.store(&old).await.unwrap();
    storage.store(&fresh).await.unwrap();

    // 时间范围过滤
    let by_range = storage
        .query(&AuditQueryFilters {
            start_time: Some(now - chrono::Duration::hours(1)),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(by_range.len(), 1, "只应命中 fresh 事件");
    assert_eq!(by_range[0].entity_id, "new");

    // 清理旧事件
    let removed = storage
        .cleanup(&(now - chrono::Duration::hours(1)))
        .await
        .unwrap();
    assert_eq!(removed, 1, "应清理 1 条旧事件");

    let remaining = storage.query(&AuditQueryFilters::default()).await.unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].entity_id, "new");

    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn test_db_audit_upsert_same_id_overwrites() {
    let (url, path) = temp_db_url("upsert");
    let pool = Arc::new(dbnexus::DbPool::new(&url).await.unwrap());
    let storage = DbAuditStorage::new(pool);
    storage.init().await.unwrap();

    let mut event = AuditEvent::create("users", "42", "u1");
    storage.store(&event).await.unwrap();
    event.after_value = Some(r#"{"done":true}"#.to_string());
    storage.store(&event).await.unwrap();

    let all = storage.query(&AuditQueryFilters::default()).await.unwrap();
    assert_eq!(all.len(), 1, "同 id 二次写入应覆盖（幂等 upsert）");
    assert_eq!(all[0].after_value.as_deref(), Some(r#"{"done":true}"#));

    // operation 过滤（serde 序列化形态匹配）
    let ops = storage
        .query(&AuditQueryFilters {
            operation: Some(AuditOperation::Create),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(ops.len(), 1);

    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn test_db_audit_init_is_idempotent_and_auditlogger_compat() {
    let (url, path) = temp_db_url("logger");
    let pool = Arc::new(dbnexus::DbPool::new(&url).await.unwrap());
    let storage = DbAuditStorage::new(pool);
    // 重复 init 幂等
    storage.init().await.unwrap();
    storage.init().await.unwrap();

    // 可作为 AuditLogger 的存储后端（与内存实现并列）
    let logger = dbnexus::AuditLogger::with_config(
        dbnexus::AuditConfig::default(),
        Arc::new(storage),
    );
    logger
        .log_update("users", "7", "admin", Some("b".to_string()), Some("a".to_string()))
        .await
        .unwrap();

    let results = logger
        .query(&AuditQueryFilters {
            entity_type: Some("users".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].result, AuditStatus::Success);

    let _ = std::fs::remove_file(&path);
}
