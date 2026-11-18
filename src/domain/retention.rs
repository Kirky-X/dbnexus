// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 数据保留策略（`retention` feature）
//!
//! 声明式保留时长解析 + 确定性批删执行件（归档先行，失败不删）；
//! 调度外置（应用 cron / K8s CronJob 触发），框架不做定时器。

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::database::identifier::is_safe_identifier;
use crate::foundation::{DbError, DbResult};
use crate::{DbPool, Session};

/// 从实体宏声明读取保留元数据
pub trait HasRetentionMeta {
    /// 返回 (声明, 时间列, 归档标签)
    fn retention_meta() -> Option<(&'static str, &'static str, Option<&'static str>)>;
}

/// 保留时长（内部以秒数存储）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionPolicy {
    /// 保留窗口秒数
    pub(crate) seconds: u64,
}

impl RetentionPolicy {
    /// 解析声明（d=天 / w=周 / mo=30 天固定近似）
    pub fn parse(decl: &str) -> Result<Self, RetentionError> {
        let decl = decl.trim();
        if decl.is_empty() {
            return Err(RetentionError::InvalidFormat(decl.to_string()));
        }
        let split = decl.find(|c: char| !c.is_ascii_digit());
        let Some(idx) = split else {
            return Err(RetentionError::InvalidFormat(decl.to_string()));
        };
        let (num, unit) = decl.split_at(idx);
        let value: u64 = num
            .parse()
            .map_err(|_| RetentionError::InvalidFormat(decl.to_string()))?;
        if value == 0 {
            return Err(RetentionError::ZeroDuration);
        }
        let days = match unit {
            "d" => value,
            "w" => value
                .checked_mul(7)
                .ok_or(RetentionError::InvalidFormat(decl.to_string()))?,
            "mo" => value
                .checked_mul(30)
                .ok_or(RetentionError::InvalidFormat(decl.to_string()))?,
            "" => return Err(RetentionError::InvalidFormat(decl.to_string())),
            other => return Err(RetentionError::InvalidUnit(other.to_string())),
        };
        let seconds = days
            .checked_mul(86_400)
            .ok_or(RetentionError::InvalidFormat(decl.to_string()))?;
        Ok(Self { seconds })
    }

    /// 保留窗口秒数
    pub fn as_secs(&self) -> u64 {
        self.seconds
    }
}

/// 保留策略解析错误
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RetentionError {
    /// 格式非法（无数字/无单位/溢出）
    #[error("invalid retention declaration: {0}")]
    InvalidFormat(String),
    /// 未知单位
    #[error("invalid retention unit: {0}")]
    InvalidUnit(String),
    /// 零时长
    #[error("retention duration must be positive")]
    ZeroDuration,
}

/// 清理报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurgeReport {
    /// 表名
    pub table: String,
    /// 已删除行数
    pub deleted_rows: u64,
    /// 批次数
    pub batches: u32,
    /// 截止时间戳（RFC3339）
    pub cutoff: String,
    /// 已归档行数
    pub archived_rows: u64,
}

/// 到期清理执行器
pub struct RetentionExecutor {
    pool: Arc<DbPool>,
    table: String,
    role: String,
    policy: RetentionPolicy,
    time_column: String,
    batch_size: u64,
}

/// 归档回调类型（异步，写入成功后才删除）
pub type ArchiveFn<'a> = &'a mut (
            dyn FnMut(&[serde_json::Value]) -> Pin<Box<dyn Future<Output = DbResult<()>> + Send>>
                + Send
        );

impl RetentionExecutor {
    /// 构造：表名/列名白名单校验，角色必填
    pub fn new(
        pool: Arc<DbPool>,
        table: &str,
        role: &str,
        policy: RetentionPolicy,
        time_column: &str,
    ) -> DbResult<Self> {
        if !is_safe_identifier(table) {
            return Err(DbError::Config(crate::i18n::t(
                "retention-unsafe-table",
                &[("table", table.to_string())],
            )));
        }
        if !is_safe_identifier(time_column) {
            return Err(DbError::Config(format!(
                "unsafe retention column: {time_column}"
            )));
        }
        if role.trim().is_empty() {
            return Err(DbError::Config(crate::i18n::tr("retention-role-required")));
        }
        Ok(Self {
            pool,
            table: table.to_string(),
            role: role.to_string(),
            policy,
            time_column: time_column.to_string(),
            batch_size: 1000,
        })
    }

    /// 设置批大小
    pub fn with_batch_size(mut self, size: u64) -> Self {
        self.batch_size = size.max(1);
        self
    }

    /// 计算截止时间戳（RFC3339）与 unix 秒数
    fn cutoff(&self) -> (String, i64) {
        // 用系统时间换算为 unix 秒（不引入 chrono 依赖，RFC3339 由 civil 格式手写）
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let cutoff = now - self.policy.seconds as i64;
        (format_rfc3339(cutoff), cutoff)
    }

    fn session(&self) -> impl Future<Output = DbResult<Session>> + '_ {
        self.pool.get_session(&self.role)
    }

    /// 逐批删除到期行（单批失败即停，返回已删数量 + 错误）
    pub async fn purge_expired(&self, limit: u64) -> DbResult<PurgeReport> {
        let (cutoff_text, cutoff_secs) = self.cutoff();
        let mut report = PurgeReport {
            table: self.table.clone(),
            deleted_rows: 0,
            batches: 0,
            cutoff: cutoff_text,
            archived_rows: 0,
        };
        let session = self.session().await?;
        loop {
            if limit > 0 && report.deleted_rows >= limit {
                break;
            }
            let batch = if limit == 0 {
                self.batch_size
            } else {
                self.batch_size.min(limit - report.deleted_rows)
            };
            let sql = purge_sql(
                session.connection()?.get_database_backend(),
                &self.table,
                &self.time_column,
            );
            let result = session
                .execute_with_params(
                    &sql,
                    &[serde_json::json!(cutoff_secs), serde_json::json!(batch)],
                )
                .await?;
            let deleted = result.rows_affected();
            report.deleted_rows += deleted;
            report.batches += 1;
            if deleted < batch {
                break;
            }
        }
        Ok(report)
    }

    /// 先读到期行交归档回调，成功后删除同批（归档失败零删除）
    pub async fn purge_expired_with_archive(
        &self,
        limit: u64,
        archiver: ArchiveFn<'_>,
    ) -> DbResult<PurgeReport> {
        let (cutoff_text, cutoff_secs) = self.cutoff();
        let mut report = PurgeReport {
            table: self.table.clone(),
            deleted_rows: 0,
            batches: 0,
            cutoff: cutoff_text,
            archived_rows: 0,
        };
        let session = self.session().await?;
        let (select_sql, ph1, ph2) =
            if session.connection()?.get_database_backend() == sea_orm::DbBackend::Postgres {
                (
                    format!(
                        "SELECT id FROM {} WHERE {} < $1 LIMIT $2",
                        self.table, self.time_column
                    ),
                    "$1",
                    "$2",
                )
            } else {
                (
                    format!(
                        "SELECT id FROM {} WHERE {} < ? LIMIT ?",
                        self.table, self.time_column
                    ),
                    "?",
                    "?",
                )
            };
        let _ = (ph1, ph2);
        loop {
            if limit > 0 && report.deleted_rows >= limit {
                break;
            }
            let batch = if limit == 0 {
                self.batch_size
            } else {
                self.batch_size.min(limit - report.deleted_rows)
            };
            let rows = session
                .query_rows_with_params(
                    &select_sql,
                    &[serde_json::json!(cutoff_secs), serde_json::json!(batch)],
                )
                .await?;
            if rows.is_empty() {
                break;
            }
            archiver(&rows).await?; // 归档失败 → 直接返回（不删除）
            let sql = purge_sql(
                session.connection()?.get_database_backend(),
                &self.table,
                &self.time_column,
            );
            let result = session
                .execute_with_params(
                    &sql,
                    &[
                        serde_json::json!(cutoff_secs),
                        serde_json::json!(rows.len() as u64),
                    ],
                )
                .await?;
            report.archived_rows += rows.len() as u64;
            report.deleted_rows += result.rows_affected();
            report.batches += 1;
        }
        Ok(report)
    }
}

/// 方言 DELETE 语句（PG 用 ctid 子查询，其余 DELETE ... LIMIT）
fn purge_sql(backend: sea_orm::DbBackend, table: &str, column: &str) -> String {
    let limit_placeholder = if backend == sea_orm::DbBackend::Postgres {
        "$2"
    } else {
        "?"
    };
    if backend == sea_orm::DbBackend::Postgres {
        format!(
            "DELETE FROM {table} WHERE ctid IN (SELECT ctid FROM {table} WHERE {column} < $1 LIMIT {limit_placeholder})"
        )
    } else {
        // SQLite/MySQL: 用主键/rowid 子查询规避 DELETE LIMIT 方言差异（SQLite 编译默认不带 UPDATE_DELETE_LIMIT）
        format!(
            "DELETE FROM {table} WHERE rowid IN (SELECT rowid FROM {table} WHERE {column} < ? LIMIT {limit_placeholder})"
        )
    }
}

/// unix 秒 → RFC3339 UTC（无 chrono 依赖； civil-from-days 算法）
fn format_rfc3339(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let sod = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        sod / 3600,
        (sod % 3600) / 60,
        sod % 60
    )
}

fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    (y + if m <= 2 { 1 } else { 0 }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retention_parse() {
        assert_eq!(
            RetentionPolicy::parse("90d").unwrap().as_secs(),
            90 * 86_400
        );
        assert_eq!(RetentionPolicy::parse("2w").unwrap().as_secs(), 14 * 86_400);
        assert_eq!(
            RetentionPolicy::parse("3mo").unwrap().as_secs(),
            90 * 86_400
        );
        assert_eq!(
            RetentionPolicy::parse("0d"),
            Err(RetentionError::ZeroDuration)
        );
        assert!(matches!(
            RetentionPolicy::parse("90"),
            Err(RetentionError::InvalidFormat(_))
        ));
        assert!(matches!(
            RetentionPolicy::parse("90x"),
            Err(RetentionError::InvalidUnit(_))
        ));
    }

    #[test]
    fn test_purge_sql_dialects() {
        let pg = purge_sql(sea_orm::DbBackend::Postgres, "t", "created_at");
        assert!(pg.contains("ctid"));
        let sqlite = purge_sql(sea_orm::DbBackend::Sqlite, "t", "created_at");
        assert!(sqlite.contains("rowid IN"));
    }

    #[test]
    fn test_rfc3339() {
        assert_eq!(format_rfc3339(0), "1970-01-01T00:00:00Z");
    }
}
