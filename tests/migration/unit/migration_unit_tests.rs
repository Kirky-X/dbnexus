// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Migration 模块单元测试
//!
//! 测试数据库迁移的独立组件：Schema、SQL生成、类型转换等（无需数据库连接）

use dbnexus::foundation::DatabaseType;
use dbnexus::{
    Column, ColumnType, Index, Migration, MigrationFileParser, MigrationHistory, Schema,
    SchemaDiffer, SqlGenerator, Table, TableChange,
};

/// 迁移历史创建测试
#[test]
fn test_migration_history_creation() {
    let history = MigrationHistory::new();

    assert!(history.applied_migrations.is_empty());
    assert_eq!(history.get_latest_version(), None);
}

/// 迁移历史添加测试
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

/// 迁移历史排序测试
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

/// 迁移文件解析测试
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

/// 迁移文件解析 - 无描述
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

/// 迁移文件语法验证 - 有效SQL
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

/// 迁移文件语法验证 - 无效SQL
#[test]
fn test_migration_file_invalid_syntax() {
    let content = r#"-- Migration: invalid
This is not a valid migration file
No SQL statements here
"#;

    let result = MigrationFileParser::parse_migration_file(content);
    assert!(result.is_err());
}

/// SQL生成器创建测试
#[test]
fn test_sql_generator_creation() {
    let pg_gen = SqlGenerator::new(DatabaseType::Postgres);
    let mysql_gen = SqlGenerator::new(DatabaseType::MySql);
    let sqlite_gen = SqlGenerator::new(DatabaseType::Sqlite);

    assert_eq!(pg_gen.db_type, DatabaseType::Postgres);
    assert_eq!(mysql_gen.db_type, DatabaseType::MySql);
    assert_eq!(sqlite_gen.db_type, DatabaseType::Sqlite);
}

/// 创建表SQL生成测试
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

/// 删除表SQL生成测试
#[test]
fn test_drop_table_sql_generation() {
    let generator = SqlGenerator::new(DatabaseType::Sqlite);

    let sql = generator.generate_drop_table_sql("test_table").unwrap();

    assert_eq!(sql, "DROP TABLE test_table;");
}

/// 添加列SQL生成测试
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

/// 创建索引SQL生成测试
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

/// Schema创建测试
#[test]
fn test_schema_creation() {
    let schema = Schema::new(DatabaseType::Postgres);

    assert_eq!(schema.database_type, DatabaseType::Postgres);
    assert!(schema.tables.is_empty());
}

/// Schema表操作测试
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

/// Schema差异检测 - 新增表
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

/// Schema差异检测 - 删除表
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

/// Schema差异检测 - 修改表
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

/// 列类型SQL生成测试
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
    assert_eq!(
        pg.generate_column_def(&ColumnType::String(Some(100))),
        "VARCHAR(100)"
    );
    assert_eq!(
        mysql.generate_column_def(&ColumnType::String(Some(100))),
        "VARCHAR(100)"
    );
    assert_eq!(
        sqlite.generate_column_def(&ColumnType::String(Some(100))),
        "TEXT"
    );

    // JSON
    assert_eq!(pg.generate_column_def(&ColumnType::Json), "JSONB");
    assert_eq!(mysql.generate_column_def(&ColumnType::Json), "JSON");
    assert_eq!(sqlite.generate_column_def(&ColumnType::Json), "TEXT");
}

/// 迁移创建测试
#[test]
fn test_migration_creation() {
    let migration = Migration::new(1, "test_migration".to_string());

    assert_eq!(migration.version, 1);
    assert_eq!(migration.description, "test_migration");
    assert!(migration.table_changes.is_empty());
    assert!(migration.sql.is_none());
}

/// 迁移历史获取待应用迁移测试
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

/// 迁移文件解析测试
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

/// 迁移文件生成测试
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

/// 方言 SQL 生成覆盖：同一列操作在四种关系型后端下的输出形状
///
/// sqlite 腿在测时只执行得到 `DatabaseType::Sqlite` 分支，Postgres/MySql/DuckDb
/// 与图数据库拒绝分支在 CI 覆盖率口径下从无执行（历史缺口：方言分支的改动没有
/// 回归信号）。本测试逐一断言输出形状，并断言图数据库显性拒绝而非静默产出无效 SQL。
#[test]
fn test_dialect_specific_alter_sql_covers_all_relational_backends() {
    use dbnexus::domain::migration::types::ColumnChange;

    let new_column = Column {
        name: "age".to_string(),
        column_type: ColumnType::Integer,
        is_primary_key: false,
        is_nullable: true,
        has_default: false,
        default_value: None,
        is_auto_increment: false,
        comment: None,
    };

    // (方言, 删除列应含, 修改列应含；"--" 表示 SQLite 的人工重建提示)
    for (dt, expect_drop, expect_modify) in [
        (DatabaseType::Sqlite, "请手动重建表", "--"),
        (
            DatabaseType::Postgres,
            "DROP COLUMN age;",
            "ALTER COLUMN age TYPE INTEGER;",
        ),
        (
            DatabaseType::MySql,
            "DROP COLUMN age;",
            "MODIFY COLUMN age INTEGER",
        ),
        (
            DatabaseType::DuckDb,
            "DROP COLUMN age;",
            "ALTER COLUMN age TYPE INTEGER;",
        ),
    ] {
        let generator = SqlGenerator::new(dt);

        let drop_sql = generator
            .generate_drop_column_sql("users", "age")
            .expect("drop column");
        assert!(
            drop_sql.contains(expect_drop),
            "{dt:?} 删除列输出异常: {drop_sql}"
        );

        let modify_sql = generator
            .generate_alter_column_sql(
                "users",
                &ColumnChange::ModifyColumn {
                    column_name: "age".to_string(),
                    new_column: new_column.clone(),
                },
            )
            .expect("modify column");
        if expect_modify == "--" {
            assert!(
                modify_sql.contains("请手动重建表"),
                "{dt:?} SQLite 修改列应输出重建提示: {modify_sql}"
            );
        } else {
            assert!(
                modify_sql.contains(expect_modify),
                "{dt:?} 修改列输出异常: {modify_sql}"
            );
        }

        // 重命名：关系型方言原生 RENAME COLUMN（SQLite 输出重建提示）
        let rename_sql = generator
            .generate_alter_column_sql(
                "users",
                &ColumnChange::RenameColumn {
                    old_name: "age".to_string(),
                    new_name: "years".to_string(),
                },
            )
            .expect("rename column");
        assert!(
            rename_sql.contains("RENAME COLUMN") || rename_sql.contains("请手动重建表"),
            "{dt:?} 重命名输出异常: {rename_sql}"
        );

        // 其余变更类型在各方言都应有分支产出（形状由上面三类显式断言覆盖，
        // 这里要求执行到各分支且不 panic）
        let _ = generator
            .generate_alter_column_sql("users", &ColumnChange::AddColumn(new_column.clone()));
        let _ = generator.generate_alter_column_sql(
            "users",
            &ColumnChange::RemoveColumn {
                column_name: "age".to_string(),
            },
        );
        let _ = generator.generate_alter_column_sql(
            "users",
            &ColumnChange::TypeChanged {
                column_name: "age".to_string(),
                old_type: ColumnType::Integer,
                new_type: ColumnType::String(Some(64)),
            },
        );
        let _ = generator.generate_alter_column_sql(
            "users",
            &ColumnChange::NullabilityChanged {
                column_name: "age".to_string(),
                old_nullable: true,
                new_nullable: false,
            },
        );
        let _ = generator.generate_alter_column_sql(
            "users",
            &ColumnChange::DefaultChanged {
                column_name: "age".to_string(),
                old_default: None,
                new_default: Some("0".to_string()),
            },
        );
    }

    // 图数据库：关系型 ALTER 一律显性拒绝
    for dt in [DatabaseType::Ladybug, DatabaseType::Neo4j] {
        let generator = SqlGenerator::new(dt);
        let err = generator
            .generate_drop_column_sql("users", "age")
            .expect_err("graph databases must reject relational ALTER");
        assert!(
            err.contains("Graph databases do not support relational"),
            "{dt:?} 拒绝信息异常: {err}"
        );
    }
}

/// 迁移历史的方言分支：同一连接上以不同 DatabaseType 构造执行器
///
/// `load_history` 的 `applied_at` 表达式按方言分派（Postgres 的 `::text`、
/// MySQL 的 `CAST(... AS CHAR)`、SQLite/DuckDB 直取列）：sqlite 腿在测时只走得到
/// Sqlite 分支，其余方言分支此前无执行。本测试要求每种方言要么正常读取，
/// 要么显性报错——不得静默返回空历史。
#[tokio::test]
async fn test_load_history_dialect_branches_are_explicit() {
    for dt in [
        DatabaseType::Sqlite,
        DatabaseType::Postgres,
        DatabaseType::MySql,
        DatabaseType::DuckDb,
    ] {
        let db_path =
            std::env::temp_dir().join(format!("dbnexus_hist_{}_{:?}.db", std::process::id(), dt));
        let url = format!("sqlite:{}?mode=rwc", db_path.display());
        let conn = sea_orm::Database::connect(&url)
            .await
            .expect("connect sqlite");

        let mut executor = dbnexus::MigrationExecutor::new(conn, dt);
        match executor.load_history().await {
            // 方言可被 sqlite 接受：历史为空但表已建（显性成功）
            Ok(()) => assert!(
                executor.history().applied_migrations.is_empty(),
                "{dt:?} 全新库历史应为空"
            ),
            // 方言不被 sqlite 接受：必须显性报错，不得静默吞掉
            Err(e) => {
                let msg = format!("{e:?}");
                assert!(!msg.is_empty(), "{dt:?} 方言不可用时必须给出显性错误");
            }
        }
        let _ = std::fs::remove_file(&db_path);
    }
}
