// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 审计事件的 DB 持久化存储（T408，`audit` + `sql-parser` feature）
//!
//! [`DbAuditStorage`](crate::domain::audit::audit_db::DbAuditStorage) 实现既有
//! [`AuditStorage`](super::AuditStorage) 端口，与内存实现（`MemoryAuditStorage`）
//! 并列可选：`AuditLogger` 按同一端口注入任一后端。
//!
//! 模式与 `DbSagaLog`（T402）同款：
//! - DDL `CREATE TABLE IF NOT EXISTS`（幂等 init）
//! - 写入 upsert（同 id 覆盖）
//! - 行读取/清理经统一行查询 `query_rows`（admin 角色，不依赖 permission feature）
//!
//! 表结构：键列（id/timestamp/user_id/entity_type/operation/severity/result）
//! 供过滤，完整事件以 JSON 文本列 `event` 保存（`AuditEvent::to_json` 往返）。

use std::sync::Arc;

use chrono::{DateTime, Utc};

use super::{AuditEvent, AuditStorage};

/// DB 审计存储（T408）
///
/// 复用 dbnexus 自身的连接池执行 DDL/UPSERT；查询与清理经 `DbPool::query_rows`
/// 与 `Session::execute_raw`。默认以 admin 角色执行（审计为内部管控面）。
pub struct DbAuditStorage {
    pool: Arc<crate::database::DbPool>,
}

impl DbAuditStorage {
    /// 创建 DB 审计存储（表在首次使用前经 `init` 幂等创建）
    pub fn new(pool: Arc<crate::database::DbPool>) -> Self {
        Self { pool }
    }

    /// 建表（幂等）
    pub async fn init(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let session = self.pool.get_session("admin").await?;
        session
            .execute_raw_ddl(
                "CREATE TABLE IF NOT EXISTS audit_events (\
                 id TEXT PRIMARY KEY, timestamp TEXT NOT NULL, \
                 user_id TEXT NOT NULL, entity_type TEXT NOT NULL, \
                 operation TEXT NOT NULL, severity TEXT NOT NULL, \
                 result TEXT NOT NULL, event TEXT NOT NULL)",
            )
            .await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl AuditStorage for DbAuditStorage {
    /// upsert 审计事件（同 id 覆盖；表在未 init 时按需幂等建表）
    async fn store(
        &self,
        event: &AuditEvent,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.init().await?;
        let event_text = event.to_json()?;
        let sql = format!(
            "INSERT INTO audit_events (id, timestamp, user_id, entity_type, \
             operation, severity, result, event) \
             VALUES ('{}', '{}', '{}', '{}', '{}', '{}', '{}', '{}') \
             ON CONFLICT(id) DO UPDATE SET timestamp = excluded.timestamp, \
             user_id = excluded.user_id, entity_type = excluded.entity_type, \
             operation = excluded.operation, severity = excluded.severity, \
             result = excluded.result, event = excluded.event",
            sql_escape(&event.id),
            sql_escape(&event.timestamp.to_rfc3339()),
            sql_escape(&event.user_id),
            sql_escape(&event.entity_type),
            sql_escape(&serde_json::to_string(&event.operation)?),
            sql_escape(&serde_json::to_string(&event.severity)?),
            sql_escape(&serde_json::to_string(&event.result)?),
            sql_escape(&event_text),
        );
        let session = self.pool.get_session("admin").await?;
        session.execute_raw(&sql).await?;
        Ok(())
    }

    /// 按过滤器查询审计事件（时间升序）
    async fn query(
        &self,
        filters: &super::AuditQueryFilters,
    ) -> Result<Vec<AuditEvent>, Box<dyn std::error::Error + Send + Sync>> {
        let mut conditions: Vec<String> = Vec::new();
        if let Some(user_id) = &filters.user_id {
            conditions.push(format!("user_id = '{}'", sql_escape(user_id)));
        }
        if let Some(entity_type) = &filters.entity_type {
            conditions.push(format!("entity_type = '{}'", sql_escape(entity_type)));
        }
        if let Some(operation) = &filters.operation {
            conditions.push(format!(
                "operation = '{}'",
                sql_escape(&serde_json::to_string(operation)?)
            ));
        }
        if let Some(severity) = &filters.severity {
            conditions.push(format!(
                "severity = '{}'",
                sql_escape(&serde_json::to_string(severity)?)
            ));
        }
        if let Some(result) = &filters.result {
            conditions.push(format!(
                "result = '{}'",
                sql_escape(&serde_json::to_string(result)?)
            ));
        }
        if let Some(start) = &filters.start_time {
            conditions.push(format!(
                "timestamp >= '{}'",
                sql_escape(&start.to_rfc3339())
            ));
        }
        if let Some(end) = &filters.end_time {
            conditions.push(format!("timestamp <= '{}'", sql_escape(&end.to_rfc3339())));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };

        let rows = self
            .pool
            .query_rows(
                &format!(
                    "SELECT event FROM audit_events{} ORDER BY timestamp, id",
                    where_clause
                ),
                "admin",
            )
            .await?;
        let mut events = Vec::with_capacity(rows.len());
        for row in &rows {
            let text = match row.get("event").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => continue,
            };
            events.push(AuditEvent::from_json(text)?);
        }
        Ok(events)
    }

    /// 清理指定时间之前的审计事件，返回删除行数
    async fn cleanup(
        &self,
        before: &DateTime<Utc>,
    ) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        let session = self.pool.get_session("admin").await?;
        let sql = format!(
            "DELETE FROM audit_events WHERE timestamp < '{}'",
            sql_escape(&before.to_rfc3339())
        );
        let result = session.execute_raw(&sql).await?;
        Ok(result.rows_affected())
    }
}

/// SQL 文本字面量转义（单引号翻倍；与 DbSagaLog 同款口径）
fn sql_escape(s: &str) -> String {
    s.replace('\'', "''")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_escape_quotes() {
        assert_eq!(sql_escape("it's"), "it''s");
        assert_eq!(sql_escape("plain"), "plain");
    }
}
