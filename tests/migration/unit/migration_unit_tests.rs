// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Migration 模块单元测试
//!
//! 测试数据库迁移的独立组件：Schema、SQL生成、类型转换等（无需数据库连接）

use dbnexus::foundation::DatabaseType;
use dbnexus::{
    Column, ColumnType, Index, Migration, MigrationFileParser, MigrationHistory, Schema, SchemaDiffer, SqlGenerator,
    Table, TableChange,
};

/// TEST-M-U-001: 迁移历史创建测试
#[test]
fn test_migration_history_creation() {
    let history = MigrationHistory::new();

    assert!(history.applied_migrations.is_empty());
    assert_eq!(history.get_latest_version(), None);
}

/// TEST-M-U-002: 迁移历史添加测试
#[test]
fn test_migration_history_add() {
    let mut history = MigrationHistory::new();

    let migration = dbnexus::MigrationVersion {
        version: 1,
        description: "Initial migration".to_string(),
        applied_at: time::OffsetDateTime::now_utc(),
        file_path: "migration_v1.sql".to_string(),
    };

    history.add_migration(migration.clone());

    assert_eq!(history.applied_migrations.len(), 1);
    assert_eq!(history.get_latest_version(), Some(1));
    assert!(history.is_version_applied(1));
    assert!(!history.is_version_applied(2));
}

/// TEST-M-U-003: 迁移历史排序测试
#[test]
fn test_migration_history_sorted() {
    let mut history = MigrationHistory::new();

    // 添加乱序的版本
    history.add_migration(dbnexus::MigrationVersion {
        version: 3,
        description: "Third".to_string(),
        applied_at: time::OffsetDateTime::now_utc(),
        file_path: "v3.sql".to_string(),
    });

    history.add_migration(dbnexus::MigrationVersion {
        version: 1,
        description: "First".to_string(),
        applied_at: time::OffsetDateTime::now_utc(),
        file_path: "v1.sql".to_string(),
    });

    history.add_migration(dbnexus::MigrationVersion {
        version: 2,
        description: "Second".to_string(),
        applied_at: time::OffsetDateTime::now_utc(),
        file_path: "v2.sql".to_string(),
    });

    // 验证已排序
    assert_eq!(history.applied_migrations[0].version, 1);
    assert_eq!(history.applied_migrations[1].version, 2);
    assert_eq!(history.applied_migrations[2].version, 3);
}

/// TEST-M-U-004: 迁移文件解析测试
#[test]
fn test_migration_file_parser_basic() {
    let content = r#"-- Migration: create_users_table
-- Version: 1700000000

-- UP
CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL
);

-- DOWN
DROP TABLE users;
"#;

    let result = MigrationFileParser::parse_migration_file(content);

    assert!(result.is_ok());
    let (description, full_content) = result.unwrap();
    assert!(
        description.contains("create_users_table"),
        "Description should contain 'create_users_table', got: {}",
        description
    );
    assert!(full_content.contains("CREATE TABLE"));
}

/// TEST-M-U-005: 迁移文件解析 - 无描述
#[test]
fn test_migration_file_parser_no_description() {
    let content = r#"-- UP
CREATE TABLE users (
    id INTEGER PRIMARY KEY
);

-- DOWN
DROP TABLE users;
"#;

    let result = MigrationFileParser::parse_migration_file(content);

    assert!(result.is_ok());
    let (description, _) = result.unwrap();
    assert_eq!(description, "Migration");
}

/// TEST-M-U-006: 迁移文件语法验证 - 有效SQL
#[test]
fn test_migration_file_valid_syntax() {
    let content = r#"-- Migration: create_table
-- UP
CREATE TABLE test (id INTEGER PRIMARY KEY);
-- DOWN
DROP TABLE test;
"#;

    let result = MigrationFileParser::parse_migration_file(content);
    assert!(result.is_ok());
}

/// TEST-M-U-007: 迁移文件语法验证 - 无效SQL
#[test]
fn test_migration_file_invalid_syntax() {
    let content = r#"-- Migration: invalid
This is not a valid migration file
No SQL statements here
"#;

    let result = MigrationFileParser::parse_migration_file(content);
    assert!(result.is_err());
}

/// TEST-M-U-008: SQL生成器创建测试
#[test]
fn test_sql_generator_creation() {
    let pg_gen = SqlGenerator::new(DatabaseType::Postgres);
    let mysql_gen = SqlGenerator::new(DatabaseType::MySql);
    let sqlite_gen = SqlGenerator::new(DatabaseType::Sqlite);

    assert_eq!(pg_gen.db_type, DatabaseType::Postgres);
    assert_eq!(mysql_gen.db_type, DatabaseType::MySql);
    assert_eq!(sqlite_gen.db_type, DatabaseType::Sqlite);
}

/// TEST-M-U-009: 创建表SQL生成测试
#[test]
fn test_create_table_sql_generation() {
    let generator = SqlGenerator::new(DatabaseType::Postgres);

    let table = Table {
        name: "users".to_string(),
        columns: vec![
            Column {
                name: "id".to_string(),
                column_type: ColumnType::Integer,
                is_primary_key: true,
                is_nullable: false,
                has_default: false,
                default_value: None,
                is_auto_increment: true,
                comment: None,
            },
            Column {
                name: "email".to_string(),
                column_type: ColumnType::String(Some(255)),
                is_primary_key: false,
                is_nullable: false,
                has_default: false,
                default_value: None,
                is_auto_increment: false,
                comment: None,
            },
        ],
        primary_key_columns: vec!["id".to_string()],
        indexes: vec![],
        foreign_keys: vec![],
        comment: None,
    };

    let sql = generator.generate_create_table_sql(&table).unwrap();

    assert!(sql.contains("CREATE TABLE users"));
    assert!(sql.contains("id INTEGER"));
    assert!(sql.contains("email VARCHAR(255)"));
    assert!(sql.contains("NOT NULL"));
    assert!(sql.contains("PRIMARY KEY (id)"));
}

/// TEST-M-U-010: 删除表SQL生成测试
#[test]
fn test_drop_table_sql_generation() {
    let generator = SqlGenerator::new(DatabaseType::Sqlite);

    let sql = generator.generate_drop_table_sql("test_table").unwrap();

    assert_eq!(sql, "DROP TABLE test_table;");
}

/// TEST-M-U-011: 添加列SQL生成测试
#[test]
fn test_add_column_sql_generation() {
    let generator = SqlGenerator::new(DatabaseType::Postgres);

    let column = Column {
        name: "age".to_string(),
        column_type: ColumnType::Integer,
        is_primary_key: false,
        is_nullable: true,
        has_default: true,
        default_value: Some("0".to_string()),
        is_auto_increment: false,
        comment: None,
    };

    let sql = generator.generate_add_column_sql("users", &column).unwrap();

    assert!(sql.contains("ALTER TABLE users ADD"));
    assert!(sql.contains("age INTEGER"));
}

/// TEST-M-U-012: 创建索引SQL生成测试
#[test]
fn test_create_index_sql_generation() {
    let generator = SqlGenerator::new(DatabaseType::MySql);

    let index = Index {
        name: "idx_email".to_string(),
        table_name: "users".to_string(),
        columns: vec!["email".to_string()],
        is_unique: false,
        is_constraint: false,
    };

    let sql = generator.generate_create_index_sql(&index).unwrap();

    assert!(sql.contains("CREATE INDEX"));
    assert!(sql.contains("idx_email"));
    assert!(sql.contains("users"));
    assert!(sql.contains("email"));
}

/// TEST-M-U-013: Schema创建测试
#[test]
fn test_schema_creation() {
    let schema = Schema::new(DatabaseType::Postgres);

    assert_eq!(schema.database_type, DatabaseType::Postgres);
    assert!(schema.tables.is_empty());
}

/// TEST-M-U-014: Schema表操作测试
#[test]
fn test_schema_table_operations() {
    let mut schema = Schema::new(DatabaseType::Sqlite);

    let table = Table {
        name: "users".to_string(),
        columns: vec![],
        primary_key_columns: vec![],
        indexes: vec![],
        foreign_keys: vec![],
        comment: None,
    };

    schema.add_table(table.clone());

    assert!(schema.has_table("users"));
    assert!(!schema.has_table("orders"));

    let retrieved = schema.get_table("users");
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().name, "users");
}

/// TEST-M-U-015: Schema差异检测 - 新增表
#[test]
fn test_schema_diff_new_table() {
    let old_schema = Schema::new(DatabaseType::Postgres);
    let mut new_schema = Schema::new(DatabaseType::Postgres);

    let users_table = Table {
        name: "users".to_string(),
        columns: vec![Column {
            name: "id".to_string(),
            column_type: ColumnType::Integer,
            is_primary_key: true,
            is_nullable: false,
            has_default: false,
            default_value: None,
            is_auto_increment: true,
            comment: None,
        }],
        primary_key_columns: vec!["id".to_string()],
        indexes: vec![],
        foreign_keys: vec![],
        comment: None,
    };

    new_schema.add_table(users_table);

    let differ = SchemaDiffer::new(old_schema, new_schema);
    let migrations = differ.diff();

    assert_eq!(migrations.len(), 1);
    assert_eq!(migrations[0].table_changes.len(), 1);

    if let TableChange::CreateTable(table) = &migrations[0].table_changes[0] {
        assert_eq!(table.name, "users");
    } else {
        panic!("Expected CreateTable change");
    }
}

/// TEST-M-U-016: Schema差异检测 - 删除表
#[test]
fn test_schema_diff_drop_table() {
    let mut old_schema = Schema::new(DatabaseType::Postgres);
    let new_schema = Schema::new(DatabaseType::Postgres);

    let users_table = Table {
        name: "users".to_string(),
        columns: vec![],
        primary_key_columns: vec![],
        indexes: vec![],
        foreign_keys: vec![],
        comment: None,
    };

    old_schema.add_table(users_table);

    let differ = SchemaDiffer::new(old_schema, new_schema);
    let migrations = differ.diff();

    assert_eq!(migrations.len(), 1);

    if let TableChange::DropTable { table_name } = &migrations[0].table_changes[0] {
        assert_eq!(table_name, "users");
    } else {
        panic!("Expected DropTable change");
    }
}

/// TEST-M-U-017: Schema差异检测 - 修改表
#[test]
fn test_schema_diff_alter_table() {
    let mut old_schema = Schema::new(DatabaseType::Postgres);
    let mut new_schema = Schema::new(DatabaseType::Postgres);

    let old_table = Table {
        name: "users".to_string(),
        columns: vec![Column {
            name: "id".to_string(),
            column_type: ColumnType::Integer,
            is_primary_key: true,
            is_nullable: false,
            has_default: false,
            default_value: None,
            is_auto_increment: true,
            comment: None,
        }],
        primary_key_columns: vec!["id".to_string()],
        indexes: vec![],
        foreign_keys: vec![],
        comment: None,
    };

    let new_table = Table {
        name: "users".to_string(),
        columns: vec![
            Column {
                name: "id".to_string(),
                column_type: ColumnType::Integer,
                is_primary_key: true,
                is_nullable: false,
                has_default: false,
                default_value: None,
                is_auto_increment: true,
                comment: None,
            },
            Column {
                name: "email".to_string(),
                column_type: ColumnType::String(Some(255)),
                is_primary_key: false,
                is_nullable: false,
                has_default: false,
                default_value: None,
                is_auto_increment: false,
                comment: None,
            },
        ],
        primary_key_columns: vec!["id".to_string()],
        indexes: vec![],
        foreign_keys: vec![],
        comment: None,
    };

    old_schema.add_table(old_table);
    new_schema.add_table(new_table);

    let differ = SchemaDiffer::new(old_schema, new_schema);
    let migrations = differ.diff();

    assert_eq!(migrations.len(), 1);
    assert_eq!(migrations[0].table_changes.len(), 1);

    if let TableChange::AlterTable { added_columns, .. } = &migrations[0].table_changes[0] {
        assert_eq!(added_columns.len(), 1);
        assert_eq!(added_columns[0].name, "email");
    } else {
        panic!("Expected AlterTable change");
    }
}

/// TEST-M-U-018: 列类型SQL生成测试
#[test]
fn test_column_type_to_sql() {
    let pg = SqlGenerator::new(DatabaseType::Postgres);
    let mysql = SqlGenerator::new(DatabaseType::MySql);
    let sqlite = SqlGenerator::new(DatabaseType::Sqlite);

    // Integer
    assert_eq!(pg.generate_column_def(&ColumnType::Integer), "INTEGER");
    assert_eq!(mysql.generate_column_def(&ColumnType::Integer), "INTEGER");
    assert_eq!(sqlite.generate_column_def(&ColumnType::Integer), "INTEGER");

    // Boolean
    assert_eq!(pg.generate_column_def(&ColumnType::Boolean), "BOOLEAN");
    assert_eq!(mysql.generate_column_def(&ColumnType::Boolean), "BOOLEAN");
    assert_eq!(sqlite.generate_column_def(&ColumnType::Boolean), "INTEGER");

    // String
    assert_eq!(pg.generate_column_def(&ColumnType::String(Some(100))), "VARCHAR(100)");
    assert_eq!(
        mysql.generate_column_def(&ColumnType::String(Some(100))),
        "VARCHAR(100)"
    );
    assert_eq!(sqlite.generate_column_def(&ColumnType::String(Some(100))), "TEXT");

    // JSON
    assert_eq!(pg.generate_column_def(&ColumnType::Json), "JSONB");
    assert_eq!(mysql.generate_column_def(&ColumnType::Json), "JSON");
    assert_eq!(sqlite.generate_column_def(&ColumnType::Json), "TEXT");
}

/// TEST-M-U-019: 迁移创建测试
#[test]
fn test_migration_creation() {
    let migration = Migration::new(1, "test_migration".to_string());

    assert_eq!(migration.version, 1);
    assert_eq!(migration.description, "test_migration");
    assert!(migration.table_changes.is_empty());
    assert!(migration.sql.is_none());
}

/// TEST-M-U-020: 迁移历史获取待应用迁移测试
#[test]
fn test_migration_history_pending() {
    let mut history = MigrationHistory::new();

    // 添加已应用的迁移
    history.add_migration(dbnexus::MigrationVersion {
        version: 1,
        description: "v1".to_string(),
        applied_at: time::OffsetDateTime::now_utc(),
        file_path: "v1.sql".to_string(),
    });

    let all_migrations = vec![
        Migration::new(1, "v1".to_string()),
        Migration::new(2, "v2".to_string()),
        Migration::new(3, "v3".to_string()),
    ];

    let pending = history.get_pending_migrations(&all_migrations);

    assert_eq!(pending.len(), 2);
    assert_eq!(pending[0].version, 2);
    assert_eq!(pending[1].version, 3);
}

/// TEST-M-U-021: 迁移文件解析测试
#[test]
fn test_migration_parse_succeeds() {
    let generator = SqlGenerator::new(DatabaseType::Postgres);

    let mut migration = Migration::new(1, "test".to_string());
    migration.add_table_change(TableChange::CreateTable(Table {
        name: "test".to_string(),
        columns: vec![Column {
            name: "id".to_string(),
            column_type: ColumnType::Integer,
            is_primary_key: true,
            is_nullable: false,
            has_default: false,
            default_value: None,
            is_auto_increment: false,
            comment: None,
        }],
        primary_key_columns: vec!["id".to_string()],
        indexes: vec![],
        foreign_keys: vec![],
        comment: None,
    }));

    let sql = generator.generate_migration_sql(&migration).unwrap();

    assert!(sql.contains("CREATE TABLE test"));
}

/// TEST-M-U-021: 迁移文件生成测试
#[test]
fn test_migration_generate_succeeds() {
    let generator = SqlGenerator::new(DatabaseType::Postgres);

    let mut migration = Migration::new(1, "test".to_string());
    migration.add_table_change(TableChange::CreateTable(Table {
        name: "test".to_string(),
        columns: vec![Column {
            name: "id".to_string(),
            column_type: ColumnType::Integer,
            is_primary_key: true,
            is_nullable: false,
            has_default: false,
            default_value: None,
            is_auto_increment: false,
            comment: None,
        }],
        primary_key_columns: vec!["id".to_string()],
        indexes: vec![],
        foreign_keys: vec![],
        comment: None,
    }));

    let sql = generator.generate_migration_sql(&migration).unwrap();

    assert!(sql.contains("id INTEGER"));
}
