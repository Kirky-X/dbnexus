// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! confers 配置热重载集成（T405，`config-confers` feature）
//!
//! 经 confers 的 `ChangeStream` 订阅配置变更事件，回调映射为
//! `PermissionConfig` 后原子换装（`DbPool::set_permission_config`，
//! ArcSwap COW 无锁读）——权限策略热更新不再需要重启。
//!
//! 典型接线（上层组合）：
//!
//! ```rust,ignore
//! use confers::{InMemoryChangeStream, watcher::FsWatcher};
//!
//! let stream = Arc::new(InMemoryChangeStream::new());
//! // FsWatcher / etcd watch / K8s 源等将变更 publish 到 stream 后：
//! dbnexus::config_confers::attach_permission_hot_reload(
//!     pool.clone(),
//!     stream.clone(),
//!     |event| event.key == "permission" && event.new_value.is_some(),
//!     |event| parse_permission_config(&event.new_value.unwrap()),
//! ).await;
//! ```

use std::sync::Arc;

/// 经 confers `ChangeStream` 驱动权限配置原子换装的后台任务
///
/// - `keep`：事件过滤器（返回 true 才触发换装）
/// - `load`：事件 → `PermissionConfig` 映射（解析失败返回 Err 以跳过本次）
///
/// 返回的 JoinHandle 在 Drop 前持续运行；测试中可 abort。
#[cfg(feature = "permission")]
pub async fn attach_permission_hot_reload<F, L>(
    pool: Arc<crate::database::DbPool>,
    stream: Arc<dyn confers::ChangeStream>,
    keep: F,
    load: L,
) -> tokio::task::JoinHandle<Result<(), String>>
where
    F: Fn(&confers::ChangeEvent) -> bool + Send + Sync + 'static,
    L: Fn(&confers::ChangeEvent) -> Result<crate::access::permission::PermissionConfig, String>
        + Send
        + Sync
        + 'static,
{
    // 订阅在 attach 内同步完成（消除"先发布后订阅"竞态），消费循环后台运行
    let mut rx = match stream.subscribe().await {
        Ok(rx) => rx,
        Err(e) => {
            return tokio::spawn(async move {
                Err(format!("confers subscribe failed: {e}"))
            });
        }
    };

    tokio::spawn(async move {
        use futures::StreamExt;
        while let Some(event) = rx.next().await {
            if !keep(&event) {
                continue;
            }
            match load(&event) {
                Ok(config) => {
                    pool.set_permission_config(config)
                        .await
                        .map_err(|e| format!("permission hot reload failed: {e}"))?;
                }
                Err(e) => {
                    // T405：解析失败的变更事件跳过（fail-closed：不换装坏配置）
                    return Err(format!("skip invalid permission event: {e}"));
                }
            }
            let _ = confers::ChangeStream::ack(&*stream, event.version)
                .await
                .map_err(|e| format!("confers ack failed: {e}"))?;
        }
        Ok(())
    })
}

/// 从 ChangeEvent 的 new_value 文本解析 `PermissionConfig`（YAML/JSON 自适应）
#[cfg(feature = "permission")]
pub fn parse_permission_config(
    text: &str,
) -> Result<crate::access::permission::PermissionConfig, String> {
    let trimmed = text.trim_start();
    if trimmed.starts_with('{') {
        serde_json::from_str(text).map_err(|e| e.to_string())
    } else {
        crate::access::permission::PermissionConfig::from_yaml_str(text)
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_permission_config_yaml() {
        let yaml = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations: ["select", "insert"]
"#;
        let config = parse_permission_config(yaml).unwrap();
        assert!(config.roles.contains_key("admin"));
    }

    #[test]
    fn test_parse_permission_config_json() {
        let json = r#"{"roles": {"analyst": {"tables": [{"name": "orders", "operations": ["select"]}]}}}"#;
        let config = parse_permission_config(json).unwrap();
        assert!(config.roles.contains_key("analyst"));
    }

    #[tokio::test]
    async fn test_hot_reload_swaps_permission_config() {
        use confers::{ChangeEvent, ChangeSource, ChangeStream};

        let url = std::env::temp_dir().join(format!("dbnexus_hr_{}.db", std::process::id()));
        let db_url = format!("sqlite:{}?mode=rwc", url.display());
        let pool = Arc::new(crate::database::DbPool::new(&db_url).await.unwrap());

        let stream = Arc::new(confers::InMemoryChangeStream::new()) as Arc<dyn ChangeStream>;

        let yaml_new = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations: ["select"]
  analyst:
    tables:
      - name: "orders"
        operations: ["select"]
"#;
        let handle = attach_permission_hot_reload(
            pool.clone(),
            stream.clone(),
            |event| event.key == "permission",
            move |event| {
                let text = event
                    .new_value
                    .as_ref()
                    .map(|v| match v {
                        confers::ConfigValue::String(s) => s.clone(),
                        other => serde_json::to_string(other).unwrap_or_default(),
                    })
                    .unwrap_or_default();
                parse_permission_config(&text)
            },
        )
        .await;

        // 发布权限变更事件（new_value 为 YAML 文本）
        stream
            .publish(ChangeEvent {
                version: 0,
                key: "permission".to_string(),
                old_value: None,
                new_value: Some(confers::ConfigValue::String(yaml_new.to_string())),
                source: ChangeSource::File,
            })
            .await
            .unwrap();

        // 等待任务换装完成
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // analyst 角色已可 get_session（换装生效）
        let session = pool.get_session("analyst").await;
        assert!(
            session.is_ok(),
            "热重载后 analyst 角色应可用，实际: {:?}",
            session.err().map(|e| e.to_string())
        );

        handle.abort();
        let _ = std::fs::remove_file(&url);
    }
}
