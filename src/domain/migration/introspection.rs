// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 活库 schema 内省（`migration` feature）
//!
//! 从真实数据库系统目录读取表/列/主键结构，输出对齐 [`Schema`] 的快照，
//! 供 `SchemaDiffer` / `schema-diff` CLI 做漂移检测。只读，不换装策略。

use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};

use crate::domain::migration::{Column, Schema, Table};
use crate::foundation::{DatabaseType, DbError, DbResult};
use crate::i18n;

/// 从活连接读取结构化 schema
pub async fn introspect_schema(
    conn: &DatabaseConnection,
    backend: DatabaseType,
) -> DbResult<Schema> {
    if matches!(backend, DatabaseType::Ladybug | DatabaseType::Neo4j) {
        return Err(DbError::Unsupported(i18n::t_simple(
            "introspect-unsupported-backend",
        )));
    }
    let mut schema = Schema::new(backend);
    match backend {
        DatabaseType::Sqlite => introspect_sqlite(conn, &mut schema).await?,
        DatabaseType::Postgres => {
            introspect_information_schema(conn, &mut schema, "public").await?
        }
        DatabaseType::MySql => {
            introspect_information_schema(conn, &mut schema, "DATABASE()").await?
        }
        DatabaseType::DuckDb => introspect_duckdb(conn, &mut schema).await?,
        DatabaseType::Ladybug | DatabaseType::Neo4j => unreachable!(),
    }
    schema.rebuild_index();
    Ok(schema)
}

/// 后端 RAW 类型名 → ColumnType（未知类型映射 Text 并告警）
pub(crate) fn map_column_type(backend: &DatabaseType, raw: &str) -> Column {
    let upper = raw.trim().to_uppercase();
    let base = upper.split('(').next().unwrap_or("").trim();
    let length = upper
        .split_once('(')
        .and_then(|(_, rest)| rest.split(')').next())
        .and_then(|n| n.trim().parse::<u32>().ok());
    let column_type = match base {
        "INT" | "INTEGER" | "SMALLINT" | "TINYINT" | "MEDIUMINT" | "SERIAL" | "INTSERIAL" => {
            ColumnType::Integer
        }
        "BIGINT" | "BIGSERIAL" => ColumnType::BigInteger,
        "VARCHAR" | "CHAR" | "CHARACTER" | "NVARCHAR" | "STRING" => ColumnType::String(length),
        "TEXT" | "CLOB" | "LONGTEXT" | "MEDIUMTEXT" => ColumnType::Text,
        "BOOL" | "BOOLEAN" => ColumnType::Boolean,
        "FLOAT" | "REAL" | "FLOAT4" => ColumnType::Float,
        "DOUBLE" | "DOUBLE PRECISION" | "FLOAT8" => ColumnType::Double,
        "DATE" => ColumnType::Date,
        "TIME" => ColumnType::Time,
        "TIMESTAMP" | "DATETIME" | "TIMESTAMPTZ" => ColumnType::Timestamp,
        "JSON" | "JSONB" => ColumnType::Json,
        "BLOB" | "BYTEA" | "BINARY" | "VARBINARY" => ColumnType::Binary,
        "DECIMAL" | "NUMERIC" => ColumnType::Custom(raw.to_string()),
        other => {
            log::warn!(
                "{}",
                i18n::t(
                    "introspect-unknown-type",
                    &[
                        ("backend", format!("{backend:?}")),
                        ("type", other.to_string())
                    ]
                )
            );
            ColumnType::Text
        }
    };
    Column {
        name: String::new(),
        column_type,
        is_primary_key: false,
        is_nullable: true,
        has_default: false,
        default_value: None,
        is_auto_increment: false,
        comment: None,
    }
}

use crate::domain::migration::ColumnType;

async fn query_json(conn: &DatabaseConnection, sql: &str) -> DbResult<Vec<serde_json::Value>> {
    let stmt = Statement::from_string(conn.get_database_backend(), sql.to_string());
    let rows = conn
        .query_all_raw(stmt)
        .await
        .map_err(DbError::Connection)?;
    Ok(rows.iter().map(json_from_row).collect())
}

/// 从 QueryResult 提取所有列为 JSON 对象
fn json_from_row(row: &sea_orm::QueryResult) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for name in row.column_names() {
        let v = if let Ok(Some(x)) = row.try_get::<Option<i64>>("", name.as_str()) {
            serde_json::json!(x)
        } else if let Ok(Some(x)) = row.try_get::<Option<f64>>("", name.as_str()) {
            serde_json::json!(x)
        } else if let Ok(Some(x)) = row.try_get::<Option<String>>("", name.as_str()) {
            serde_json::json!(x)
        } else if let Ok(Some(x)) = row.try_get::<Option<bool>>("", name.as_str()) {
            serde_json::json!(x)
        } else {
            serde_json::Value::Null
        };
        map.insert(name, v);
    }
    serde_json::Value::Object(map)
}

async fn introspect_sqlite(conn: &DatabaseConnection, schema: &mut Schema) -> DbResult<()> {
    let tables = query_json(
        conn,
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
    )
    .await?;
    for row in tables {
        let name = row.get("name").and_then(|v| v.as_str()).unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let escaped = name.replace('\'', "''");
        let cols = query_json(
            conn,
            &format!(
                "SELECT name, type, \"notnull\", dflt_value, pk FROM pragma_table_info('{escaped}')"
            ),
        )
        .await?;
        let mut table = Table {
            name: name.to_string(),
            columns: vec![],
            primary_key_columns: vec![],
            indexes: vec![],
            foreign_keys: vec![],
            comment: None,
        };
        for c in cols {
            let col_name = c.get("name").and_then(|v| v.as_str()).unwrap_or_default();
            let raw_type = c.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let mut column = map_column_type(&DatabaseType::Sqlite, raw_type);
            column.name = col_name.to_string();
            column.is_nullable = c.get("notnull").and_then(|v| v.as_u64()) == Some(0);
            let default = c
                .get("dflt_value")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            column.has_default = default.is_some();
            column.default_value = default;
            let pk = c.get("pk").and_then(|v| v.as_u64()).unwrap_or(0);
            column.is_primary_key = pk > 0;
            if pk > 0 {
                table.primary_key_columns.push(col_name.to_string());
            }
            table.columns.push(column);
        }
        table.primary_key_columns.sort_unstable();
        schema.add_table(table);
    }
    Ok(())
}

async fn introspect_information_schema(
    conn: &DatabaseConnection,
    schema: &mut Schema,
    scope: &str,
) -> DbResult<()> {
    let tables = query_json(
        conn,
        &format!("SELECT table_name FROM information_schema.tables WHERE table_schema = {scope}"),
    )
    .await?;
    for row in tables {
        let name = row
            .get("table_name")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let escaped = name.replace('\'', "''");
        let cols = query_json(
            conn,
            &format!("SELECT column_name, data_type, is_nullable, column_default FROM information_schema.columns WHERE table_schema = {scope} AND table_name = '{escaped}'"),
        )
        .await?;
        let mut table = Table {
            name: name.to_string(),
            columns: vec![],
            primary_key_columns: vec![],
            indexes: vec![],
            foreign_keys: vec![],
            comment: None,
        };
        for c in cols {
            let col_name = c
                .get("column_name")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let raw_type = c.get("data_type").and_then(|v| v.as_str()).unwrap_or("");
            let mut column = map_column_type(&schema.database_type, raw_type);
            column.name = col_name.to_string();
            column.is_nullable = c.get("is_nullable").and_then(|v| v.as_str()) == Some("YES");
            let default = c
                .get("column_default")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            column.has_default = default.is_some();
            column.default_value = default;
            table.columns.push(column);
        }
        schema.add_table(table);
    }
    Ok(())
}

async fn introspect_duckdb(conn: &DatabaseConnection, schema: &mut Schema) -> DbResult<()> {
    let tables = query_json(conn, "SELECT table_name FROM duckdb_tables()").await?;
    for row in tables {
        let name = row
            .get("table_name")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let escaped = name.replace('\'', "''");
        let cols = query_json(
            conn,
            &format!("SELECT column_name, data_type, is_nullable FROM duckdb_columns() WHERE table_name = '{escaped}'"),
        )
        .await?;
        let mut table = Table {
            name: name.to_string(),
            columns: vec![],
            primary_key_columns: vec![],
            indexes: vec![],
            foreign_keys: vec![],
            comment: None,
        };
        for c in cols {
            let col_name = c
                .get("column_name")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let raw_type = c.get("data_type").and_then(|v| v.as_str()).unwrap_or("");
            let mut column = map_column_type(&DatabaseType::DuckDb, raw_type);
            column.name = col_name.to_string();
            column.is_nullable = c
                .get("is_nullable")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            table.columns.push(column);
        }
        schema.add_table(table);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_column_type_samples() {
        let cases = [
            ("INTEGER", ColumnType::Integer),
            ("BIGINT", ColumnType::BigInteger),
            ("VARCHAR(64)", ColumnType::String(Some(64))),
            ("TEXT", ColumnType::Text),
            ("BOOLEAN", ColumnType::Boolean),
            ("DOUBLE", ColumnType::Double),
            ("TIMESTAMP", ColumnType::Timestamp),
            ("BLOB", ColumnType::Binary),
            ("WEIRD_TYPE", ColumnType::Text),
        ];
        for (raw, expected) in cases {
            let col = map_column_type(&DatabaseType::Sqlite, raw);
            assert_eq!(col.column_type, expected, "raw: {raw}");
        }
    }

    #[tokio::test]
    async fn introspect_sqlite_roundtrip() {
        let conn = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        conn.execute_unprepared(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER DEFAULT 0)",
        )
        .await
        .unwrap();
        let schema = introspect_schema(&conn, DatabaseType::Sqlite)
            .await
            .unwrap();
        assert_eq!(schema.tables.len(), 1);
        let table = &schema.tables[0];
        assert_eq!(table.name, "users");
        assert_eq!(table.columns.len(), 3);
        assert_eq!(table.primary_key_columns, vec!["id".to_string()]);
        assert!(!table.columns[1].is_nullable);
    }

    #[tokio::test]
    async fn graph_backends_rejected() {
        let conn = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        for backend in [DatabaseType::Ladybug, DatabaseType::Neo4j] {
            let err = introspect_schema(&conn, backend).await.unwrap_err();
            assert!(matches!(err, DbError::Unsupported(_)));
        }
    }
}
