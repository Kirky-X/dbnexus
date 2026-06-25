// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 公共类型定义

use serde::{Deserialize, Serialize};

/// 数据库类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DatabaseType {
    /// SQLite
    #[default]
    Sqlite,
    /// PostgreSQL
    Postgres,
    /// MySQL
    MySql,
}

impl std::fmt::Display for DatabaseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatabaseType::Sqlite => write!(f, "sqlite"),
            DatabaseType::Postgres => write!(f, "postgres"),
            DatabaseType::MySql => write!(f, "mysql"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_type_display() {
        assert_eq!(DatabaseType::Sqlite.to_string(), "sqlite");
        assert_eq!(DatabaseType::Postgres.to_string(), "postgres");
        assert_eq!(DatabaseType::MySql.to_string(), "mysql");
    }

    #[test]
    fn test_database_type_default() {
        let db_type = DatabaseType::default();
        assert_eq!(db_type, DatabaseType::Sqlite);
    }

    #[test]
    fn test_database_type_equality() {
        assert_eq!(DatabaseType::Sqlite, DatabaseType::Sqlite);
        assert_ne!(DatabaseType::Sqlite, DatabaseType::Postgres);
        assert_ne!(DatabaseType::Postgres, DatabaseType::MySql);
    }

    #[test]
    fn test_database_type_clone_copy() {
        let db_type = DatabaseType::Postgres;
        let cloned = db_type;
        assert_eq!(db_type, cloned);
    }

    #[test]
    fn test_database_type_serialize_deserialize() {
        let db_type = DatabaseType::MySql;
        let json = serde_json::to_string(&db_type).expect("serialize failed");
        assert_eq!(json, "\"MySql\"");
        let deserialized: DatabaseType = serde_json::from_str(&json).expect("deserialize failed");
        assert_eq!(deserialized, DatabaseType::MySql);
    }

    #[test]
    fn test_database_type_debug() {
        let debug_format = format!("{:?}", DatabaseType::Sqlite);
        assert_eq!(debug_format, "Sqlite");
    }
}
