// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! T423：实体事件总线 + Outbox 测试（sqlite）
//!
//! 覆盖：事件登记 → 单轮投递 → 总线订阅者接收 → 状态标记已投递；
//! 未投递事件可重放；后台投递器端到端冒烟。

#![cfg(all(
    feature = "entity-events",
    feature = "sqlite",
    feature = "sql-parser",
    feature = "runtime-tokio-rustls"
))]

use dbnexus::{
    DbOutboxStore, EntityAction, EntityEvent, InMemoryEntityEventBus, OutboxDispatcher,
    OutboxStore,
};
use std::sync::Arc;
use std::time::Duration;

async fn temp_store(tag: &str) -> (Arc<DbOutboxStore>, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("dbnexus_t423_{}_{}.db", tag, std::process::id()));
    let _ = std::fs::remove_file(&path);
    let url = format!("sqlite:{}?mode=rwc", path.display());
    let pool = Arc::new(dbnexus::DbPool::new(&url).await.expect("pool"));
    let store = Arc::new(DbOutboxStore::new(pool).expect("store"));
    store.ensure_table().await.expect("ensure outbox table");
    (store, path)
}

/// 登记两事件 → 单轮投递 → 订阅者全部收到 → 状态标记 dispatched；再投递为空
#[tokio::test]
async fn test_record_dispatch_once_and_mark() {
    let (store, path) = temp_store("once").await;
    let bus = Arc::new(InMemoryEntityEventBus::default());
    let mut rx = bus.subscribe().await;

    store
        .record(&EntityEvent::insert("users", "1").with_payload(serde_json::json!({"name": "alice"})))
        .await
        .expect("record 1");
    store
        .record(&EntityEvent::new("users", EntityAction::Update, "1"))
        .await
        .expect("record 2");

    let dispatched = OutboxDispatcher::dispatch_once(store.as_ref(), bus.as_ref(), 10)
        .await
        .expect("dispatch once");
    assert_eq!(dispatched, 2, "两事件应全部投递");

    let first = rx.recv().await.expect("first event");
    assert_eq!(first.entity, "users");
    assert_eq!(first.action, EntityAction::Insert);
    assert_eq!(first.entity_id, "1");
    assert_eq!(first.payload.as_ref().unwrap()["name"], "alice");

    let second = rx.recv().await.expect("second event");
    assert_eq!(second.action, EntityAction::Update);

    // 已投递事件不重放
    let replayed = OutboxDispatcher::dispatch_once(store.as_ref(), bus.as_ref(), 10)
        .await
        .expect("re-dispatch");
    assert_eq!(replayed, 0, "dispatched 状态不应再投递");

    let _ = std::fs::remove_file(&path);
}

/// 投递中断后重放：未标记的事件仍在 pending（模拟投递失败恢复）
#[tokio::test]
async fn test_pending_events_survive_for_replay() {
    let (store, path) = temp_store("replay").await;
    // 只登记，不投递
    store.record(&EntityEvent::insert("orders", "7")).await.expect("record");
    store.record(&EntityEvent::insert("orders", "8")).await.expect("record");

    // 只取第一条（模拟批处理中断）
    let pending = store.fetch_pending(1).await.expect("fetch pending");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].1.entity_id, "7", "按登记顺序取最旧");

    // 剩余 pending 保持可重放
    let again = store.fetch_pending(10).await.expect("fetch again");
    assert_eq!(again.len(), 2, "未标记的事件保持 pending");

    let _ = std::fs::remove_file(&path);
}

/// 后台投递器：登记事件后由后台任务自动投递到订阅者
#[tokio::test]
async fn test_background_dispatcher_smoke() {
    let (store, path) = temp_store("bg").await;
    let bus = Arc::new(InMemoryEntityEventBus::default());
    let mut rx = bus.subscribe().await;

    let shutdown = Arc::new(tokio::sync::Notify::new());
    let handle = OutboxDispatcher::spawn(
        store.clone(),
        bus.clone(),
        10, // 10ms 轮询
        100,
        shutdown.clone(),
    );

    store.record(&EntityEvent::delete("users", "9")).await.expect("record");

    let event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("应在超时前收到事件")
        .expect("channel open");
    assert_eq!(event.action, EntityAction::Delete);
    assert_eq!(event.entity_id, "9");

    shutdown.notify_one();
    handle.abort(); // 冒烟后即收

    let _ = std::fs::remove_file(&path);
}
