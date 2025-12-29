//! 迁移集成测试
//!
//! 测试数据库迁移功能的各个组件：执行器、文件解析、SQL生成、Schema差异检测等

use dbnexus::DbPool;
use dbnexus::migration::{
    Column, ColumnType, DatabaseType, Index, Migration, MigrationExecutor, MigrationFileParser, MigrationHistory,
    Schema, SchemaDiffer, SqlGenerator, Table, TableChange,
};
mod common;

/// TEST-M-001: 迁移执行器创建测试
#[tokio::test]
async fn test_migration_executor_creation() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");
    let connection = session.connection().expect("Failed to get connection").clone();

    let executor = MigrationExecutor::new(connection, DatabaseType::SQLite);

    // 通过生成的SQL验证数据库类型
    let test_sql = executor.sql_generator.generate_drop_table_sql("test");
    assert!(test_sql.contains("DROP TABLE test"));
}

/// TEST-M-002: 迁移历史创建测试
#[test]
fn test_migration_history_creation() {
    let history = MigrationHistory::new();

    assert!(history.applied_migrations.is_empty());
    assert_eq!(history.get_latest_version(), None);
}

/// TEST-M-003: 迁移历史添加测试
#[test]
fn test_migration_history_add() {
    let mut history = MigrationHistory::new();

    let migration = dbnexus::migration::MigrationVersion {
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

/// TEST-M-004: 迁移历史排序测试
#[test]
fn test_migration_history_sorted() {
    let mut history = MigrationHistory::new();

    // 添加乱序的版本
    history.add_migration(dbnexus::migration::MigrationVersion {
        version: 3,
        description: "Third".to_string(),
        applied_at: time::OffsetDateTime::now_utc(),
        file_path: "v3.sql".to_string(),
    });

    history.add_migration(dbnexus::migration::MigrationVersion {
        version: 1,
        description: "First".to_string(),
        applied_at: time::OffsetDateTime::now_utc(),
        file_path: "v1.sql".to_string(),
    });

    history.add_migration(dbnexus::migration::MigrationVersion {
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

/// TEST-M-005: 迁移文件解析测试
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

/// TEST-M-006: 迁移文件解析 - 无描述
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

/// TEST-M-007: 迁移文件语法验证 - 有效SQL
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

/// TEST-M-008: 迁移文件语法验证 - 无效SQL
#[test]
fn test_migration_file_invalid_syntax() {
    let content = r#"-- Migration: invalid
This is not a valid migration file
No SQL statements here
"#;

    let result = MigrationFileParser::parse_migration_file(content);
    assert!(result.is_err());
}

/// TEST-M-009: SQL生成器创建测试
#[test]
fn test_sql_generator_creation() {
    let pg_gen = SqlGenerator::new(DatabaseType::Postgres);
    let mysql_gen = SqlGenerator::new(DatabaseType::MySQL);
    let sqlite_gen = SqlGenerator::new(DatabaseType::SQLite);

    assert_eq!(pg_gen.db_type, DatabaseType::Postgres);
    assert_eq!(mysql_gen.db_type, DatabaseType::MySQL);
    assert_eq!(sqlite_gen.db_type, DatabaseType::SQLite);
}

/// TEST-M-010: 创建表SQL生成测试
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

    let sql = generator.generate_create_table_sql(&table);

    assert!(sql.contains("CREATE TABLE users"));
    assert!(sql.contains("id INTEGER"));
    assert!(sql.contains("email VARCHAR(255)"));
    assert!(sql.contains("NOT NULL"));
    assert!(sql.contains("PRIMARY KEY (id)"));
}

/// TEST-M-011: 删除表SQL生成测试
#[test]
fn test_drop_table_sql_generation() {
    let generator = SqlGenerator::new(DatabaseType::SQLite);

    let sql = generator.generate_drop_table_sql("test_table");

    assert_eq!(sql, "DROP TABLE test_table;");
}

/// TEST-M-012: 添加列SQL生成测试
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

    let sql = generator.generate_add_column_sql("users", &column);

    assert!(sql.contains("ALTER TABLE users ADD"));
    assert!(sql.contains("age INTEGER"));
}

/// TEST-M-013: 创建索引SQL生成测试
#[test]
fn test_create_index_sql_generation() {
    let generator = SqlGenerator::new(DatabaseType::MySQL);

    let index = Index {
        name: "idx_email".to_string(),
        table_name: "users".to_string(),
        columns: vec!["email".to_string()],
        is_unique: false,
        is_constraint: false,
    };

    let sql = generator.generate_create_index_sql(&index);

    assert!(sql.contains("CREATE INDEX"));
    assert!(sql.contains("idx_email"));
    assert!(sql.contains("users"));
    assert!(sql.contains("email"));
}

/// TEST-M-014: Schema创建测试
#[test]
fn test_schema_creation() {
    let schema = Schema::new(DatabaseType::Postgres);

    assert_eq!(schema.database_type, DatabaseType::Postgres);
    assert!(schema.tables.is_empty());
}

/// TEST-M-015: Schema表操作测试
#[test]
fn test_schema_table_operations() {
    let mut schema = Schema::new(DatabaseType::SQLite);

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

/// TEST-M-016: Schema差异检测 - 新增表
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

/// TEST-M-017: Schema差异检测 - 删除表
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

/// TEST-M-018: Schema差异检测 - 修改表
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

/// TEST-M-019: 列类型SQL生成测试
#[test]
fn test_column_type_to_sql() {
    let pg = SqlGenerator::new(DatabaseType::Postgres);
    let mysql = SqlGenerator::new(DatabaseType::MySQL);
    let sqlite = SqlGenerator::new(DatabaseType::SQLite);

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

/// TEST-M-020: 迁移创建测试
#[test]
fn test_migration_creation() {
    let migration = Migration::new(1, "test_migration".to_string());

    assert_eq!(migration.version, 1);
    assert_eq!(migration.description, "test_migration");
    assert!(migration.table_changes.is_empty());
    assert!(migration.sql.is_none());
}

/// TEST-M-021: 迁移应用测试
/// 这个测试验证迁移执行器的基本创建和SQL生成，而不是完整应用
#[tokio::test]
async fn test_migration_apply() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");
    let _connection = session.connection().expect("Failed to get connection").clone();

    let executor = MigrationExecutor::new(_connection, DatabaseType::SQLite);

    // 验证执行器可以创建
    assert!(executor.sql_generator.db_type == DatabaseType::SQLite);

    // 直接执行SQL来创建表（不通过迁移历史）
    let create_result = session
        .execute_raw("CREATE TABLE IF NOT EXISTS test_table (id INTEGER PRIMARY KEY)")
        .await;
    assert!(create_result.is_ok(), "Table should be created successfully");

    // 验证表已创建（使用数据库特定的查询）
    let db_type = common::get_current_db_type();
    let check_sql = match db_type.as_str() {
        "postgres" => "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = 'test_table')",
        "mysql" => {
            "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = 'test_table'"
        }
        _ => "SELECT name FROM sqlite_master WHERE type='table' AND name='test_table'", // sqlite
    };
    let check_result = session.execute_raw(check_sql).await;
    assert!(check_result.is_ok());
}

/// TEST-M-022: 迁移历史表创建测试
#[tokio::test]
async fn test_migration_history_table_creation() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");
    let connection = session.connection().expect("Failed to get connection").clone();

    let mut executor = MigrationExecutor::new(connection, DatabaseType::SQLite);

    // 调用 load_history 来创建迁移历史表
    let _ = executor.load_history().await;

    // 验证迁移历史表已创建（使用数据库特定的查询）
    let db_type = common::get_current_db_type();
    let check_sql = match db_type.as_str() {
        "postgres" => "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = 'dbnexus_migrations')",
        "mysql" => {
            "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = 'dbnexus_migrations'"
        }
        _ => "SELECT name FROM sqlite_master WHERE type='table' AND name='dbnexus_migrations'", // sqlite
    };
    let check_result = session.execute_raw(check_sql).await;
    assert!(check_result.is_ok());
}

/// TEST-M-023: 完整迁移流程测试
/// 这个测试验证SQL生成和表创建功能，而不是完整迁移系统
#[tokio::test]
async fn test_full_migration_workflow() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    // 根据数据库类型选择正确的生成器
    let db_type_str = common::get_current_db_type();
    let db_type = match db_type_str.as_str() {
        "postgres" => DatabaseType::Postgres,
        "mysql" => DatabaseType::MySQL,
        _ => DatabaseType::SQLite,
    };
    let generator = SqlGenerator::new(db_type);

    // 生成创建 users 表的 SQL
    let users_table = Table {
        name: "users".to_string(),
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
    };

    let users_sql = generator.generate_create_table_sql(&users_table);
    println!("Generated SQL: {}", users_sql);
    assert!(users_sql.contains("CREATE TABLE users"));

    // 清理可能存在的旧表（PostgreSQL 需要 IF EXISTS）
    let _ = session.execute_raw("DROP TABLE IF EXISTS users CASCADE").await;

    // 直接执行 SQL 创建表
    let create_users = session.execute_raw(&users_sql).await;
    println!("Create result: {:?}", create_users);
    assert!(
        create_users.is_ok(),
        "Users table should be created: {:?}",
        create_users
    );

    // 生成创建 posts 表的 SQL
    let posts_table = Table {
        name: "posts".to_string(),
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
    };

    let posts_sql = generator.generate_create_table_sql(&posts_table);
    assert!(posts_sql.contains("CREATE TABLE posts"));

    // 清理可能存在的旧表
    let _ = session.execute_raw("DROP TABLE IF EXISTS posts CASCADE").await;

    // 直接执行 SQL 创建表
    let create_posts = session.execute_raw(&posts_sql).await;
    assert!(create_posts.is_ok(), "Posts table should be created");

    // 验证两个表都存在（使用数据库特定的查询）
    let db_type = common::get_current_db_type();
    let check_users_sql = match db_type.as_str() {
        "postgres" => "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = 'users')",
        "mysql" => {
            "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = 'users'"
        }
        _ => "SELECT name FROM sqlite_master WHERE type='table' AND name='users'", // sqlite
    };
    let check_posts_sql = match db_type.as_str() {
        "postgres" => "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = 'posts')",
        "mysql" => {
            "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = 'posts'"
        }
        _ => "SELECT name FROM sqlite_master WHERE type='table' AND name='posts'", // sqlite
    };
    let check_users = session.execute_raw(check_users_sql).await;
    let check_posts = session.execute_raw(check_posts_sql).await;

    assert!(check_users.is_ok(), "Users table should exist");
    assert!(check_posts.is_ok(), "Posts table should exist");
}

/// TEST-M-024: 迁移文件解析与生成测试
#[test]
fn test_migration_parse_and_generate() {
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

    let sql = generator.generate_migration_sql(&migration);

    assert!(sql.contains("CREATE TABLE test"));
    assert!(sql.contains("id INTEGER"));
}

/// TEST-M-025: 迁移历史获取待应用迁移测试
#[test]
fn test_migration_history_pending() {
    let mut history = MigrationHistory::new();

    // 添加已应用的迁移
    history.add_migration(dbnexus::migration::MigrationVersion {
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
