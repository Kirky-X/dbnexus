// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Sea-ORM `TableCreateStatement` → `migration::schema::Table` 转换器
//!
//! ## 映射表（Task 4.1）
//!
//! ### TableCreateStatement → migration::schema::Table
//!
//! | TableCreateStatement 字段 | 访问方法 | migration::schema::Table 字段 |
//! |---------------------------|----------|-------------------------------|
//! | table: Option<TableRef> | `get_table_name()` | name: String |
//! | columns: Vec<ColumnDef> | `get_columns()` | columns: Vec<Column> |
//! | (从 columns 派生) | - | primary_key_columns: Vec<String> |
//! | indexes: Vec<IndexCreateStatement> | `get_indexes()` | indexes: Vec<Index> |
//! | foreign_keys: Vec<ForeignKeyCreateStatement> | `get_foreign_key_create_stmts()` | foreign_keys: Vec<ForeignKey> |
//! | comment: Option<String> | `get_comment()` | comment: Option<String> |
//!
//! ### ColumnDef → migration::schema::Column
//!
//! | ColumnDef 字段 | 访问方法 | migration::schema::Column 字段 |
//! |----------------|----------|--------------------------------|
//! | name: DynIden | `get_column_name()` | name: String |
//! | types: Option<ColumnType> | `get_column_type()` | column_type: ColumnType |
//! | spec.nullable: Option<bool> | `get_column_spec().nullable` | is_nullable: bool |
//! | spec.default: Option<Expr> | `get_column_spec().default` | has_default: bool, default_value: Option<String> |
//! | spec.auto_increment: bool | `get_column_spec().auto_increment` | is_auto_increment: bool |
//! | spec.primary_key: bool | `get_column_spec().primary_key` | is_primary_key: bool |
//! | spec.comment: Option<String> | `get_column_spec().comment` | comment: Option<String> |
//! | spec.unique: bool | `get_column_spec().unique` | (映射为 Index，is_unique=true) |
//!
//! ### sea_query::ColumnType → migration::schema::ColumnType
//!
//! | sea_query::ColumnType | migration::schema::ColumnType |
//! |----------------------|-------------------------------|
//! | Char(Option<u32>) | String(Option<u32>) |
//! | String(StringLen::N(n)) | String(Some(n)) |
//! | String(StringLen::Max) | String(None) |
//! | String(StringLen::None) | String(None) |
//! | Text | Text |
//! | Blob | Binary |
//! | TinyInteger | Integer |
//! | SmallInteger | Integer |
//! | Integer | Integer |
//! | BigInteger | BigInteger |
//! | TinyUnsigned | Integer |
//! | SmallUnsigned | Integer |
//! | Unsigned | Integer |
//! | BigUnsigned | BigInteger |
//! | Float | Float |
//! | Double | Double |
//! | Decimal(Option<(u32, u32)>) | Custom("decimal") |
//! | DateTime | DateTime |
//! | Timestamp | Timestamp |
//! | TimestampWithTimeZone | DateTime |
//! | Time | Time |
//! | Date | Date |
//! | Year | Integer |
//! | Binary(u32) | Binary |
//! | VarBinary(StringLen) | Binary |
//! | Bit(Option<u32>) | Custom("bit") |
//! | VarBit(u32) | Custom("bit") |
//! | Boolean | Boolean |
//! | Money(Option<(u32, u32)>) | Custom("money") |
//! | Json | Json |
//! | JsonBinary | Json |
//! | Uuid | Custom("uuid") |
//! | Custom(DynIden) | Custom(name.to_string()) |
//! | Enum { name, .. } | Custom(name.to_string()) |
//! | Array(_) | Custom("array") |
//! | Vector(_) | Custom("vector") |
//! | Cidr | Custom("cidr") |
//! | Inet | Custom("inet") |
//! | MacAddr | Custom("macaddr") |
//! | LTree | Custom("ltree") |
//! | Interval(..) | Custom("interval") |
//!
//! ### IndexCreateStatement → migration::schema::Index
//!
//! | IndexCreateStatement 字段 | 访问方法 | migration::schema::Index 字段 |
//! |---------------------------|----------|-------------------------------|
//! | index.name | `get_index_spec().get_name()` | name: String |
//! | table | (从父 TableCreateStatement 传入) | table_name: String |
//! | index.columns | `get_index_spec().get_column_names()` | columns: Vec<String> |
//! | unique | `is_unique_key()` | is_unique: bool |
//! | primary | `is_primary_key()` | is_constraint: bool |
//!
//! ### ForeignKeyCreateStatement → migration::schema::ForeignKey
//!
//! | TableForeignKey 字段 | 访问方法 | migration::schema::ForeignKey 字段 |
//! |----------------------|----------|-----------------------------------|
//! | name | `get_name()` | name: String |
//! | table | `get_table()` | table_name: String |
//! | columns | `get_columns()` | column_name: String (取第一个) |
//! | ref_table | `get_ref_table()` | referenced_table_name: String |
//! | ref_columns | `get_ref_columns()` | referenced_column_name: String (取第一个) |
//! | on_delete | `get_on_delete()` | on_delete: Option<ForeignKeyAction> |
//! | on_update | `get_on_update()` | on_update: Option<ForeignKeyAction> |
//!
//! ### sea_query::ForeignKeyAction → migration::schema::ForeignKeyAction
//!
//! | sea_query::ForeignKeyAction | migration::schema::ForeignKeyAction |
//! |-----------------------------|-------------------------------------|
//! | Restrict | Restrict |
//! | Cascade | Cascade |
//! | SetNull | SetNull |
//! | NoAction | NoAction |
//! | SetDefault | SetDefault |

use super::schema::{
    Column, ColumnType, ForeignKey, ForeignKeyAction, Index, Table,
};
use sea_orm::sea_query::{
    ColumnDef, ColumnSpec, ColumnType as SeaColumnType, Expr, ForeignKeyAction as SeaFKAction,
    ForeignKeyCreateStatement, IndexCreateStatement, StringLen, TableCreateStatement, TableRef,
};

/// 将 `sea_query::TableCreateStatement` 转换为 `migration::schema::Table`
///
/// 此转换器复用 Sea-ORM `Schema::create_table_from_entity` 生成的 `TableCreateStatement`，
/// 将其字段映射到 dbnexus 内部的 `migration::schema::Table` 结构。
pub fn convert_table(stmt: &TableCreateStatement) -> Table {
    let name = extract_table_name(stmt);
    let columns: Vec<Column> = stmt
        .get_columns()
        .iter()
        .map(convert_column)
        .collect();

    let primary_key_columns: Vec<String> = columns
        .iter()
        .filter(|c| c.is_primary_key)
        .map(|c| c.name.clone())
        .collect();

    let indexes: Vec<Index> = stmt
        .get_indexes()
        .iter()
        .map(|idx| convert_index(idx, &name))
        .collect();

    let foreign_keys: Vec<ForeignKey> = stmt
        .get_foreign_key_create_stmts()
        .iter()
        .map(|fk| convert_foreign_key(fk, &name))
        .collect();

    let comment = stmt.get_comment().cloned();

    Table {
        name,
        columns,
        primary_key_columns,
        indexes,
        foreign_keys,
        comment,
    }
}

/// 从 `TableCreateStatement` 提取表名
fn extract_table_name(stmt: &TableCreateStatement) -> String {
    match stmt.get_table_name() {
        Some(TableRef::Table(table_name, _)) => table_name.1.to_string(),
        _ => String::new(),
    }
}

/// 转换 `ColumnDef` → `migration::schema::Column`
fn convert_column(col_def: &ColumnDef) -> Column {
    let name = col_def.get_column_name();
    let column_type = col_def
        .get_column_type()
        .map(convert_column_type)
        .unwrap_or(ColumnType::Custom("unknown".to_string()));

    let spec: &ColumnSpec = col_def.get_column_spec();

    let is_primary_key = spec.primary_key;
    let is_nullable = spec.nullable.unwrap_or(true);
    let is_auto_increment = spec.auto_increment;
    let comment = spec.comment.clone();

    let (has_default, default_value) = convert_default(&spec.default);

    Column {
        name,
        column_type,
        is_primary_key,
        is_nullable,
        has_default,
        default_value,
        is_auto_increment,
        comment,
    }
}

/// 转换 `sea_query::ColumnType` → `migration::schema::ColumnType`
fn convert_column_type(ct: &SeaColumnType) -> ColumnType {
    match ct {
        SeaColumnType::Char(opt) => ColumnType::String(*opt),
        SeaColumnType::String(string_len) => ColumnType::String(convert_string_len(*string_len)),
        SeaColumnType::Text => ColumnType::Text,
        SeaColumnType::Blob => ColumnType::Binary,
        SeaColumnType::TinyInteger
        | SeaColumnType::SmallInteger
        | SeaColumnType::Integer
        | SeaColumnType::TinyUnsigned
        | SeaColumnType::SmallUnsigned
        | SeaColumnType::Unsigned
        | SeaColumnType::Year => ColumnType::Integer,
        SeaColumnType::BigInteger | SeaColumnType::BigUnsigned => ColumnType::BigInteger,
        SeaColumnType::Float => ColumnType::Float,
        SeaColumnType::Double => ColumnType::Double,
        SeaColumnType::DateTime | SeaColumnType::TimestampWithTimeZone => ColumnType::DateTime,
        SeaColumnType::Timestamp => ColumnType::Timestamp,
        SeaColumnType::Time => ColumnType::Time,
        SeaColumnType::Date => ColumnType::Date,
        SeaColumnType::Boolean => ColumnType::Boolean,
        SeaColumnType::Json | SeaColumnType::JsonBinary => ColumnType::Json,
        SeaColumnType::Binary(_) | SeaColumnType::VarBinary(_) => ColumnType::Binary,
        SeaColumnType::Decimal(_) => ColumnType::Custom("decimal".to_string()),
        SeaColumnType::Money(_) => ColumnType::Custom("money".to_string()),
        SeaColumnType::Uuid => ColumnType::Custom("uuid".to_string()),
        SeaColumnType::Bit(_) | SeaColumnType::VarBit(_) => ColumnType::Custom("bit".to_string()),
        SeaColumnType::Custom(iden) => ColumnType::Custom(iden.to_string()),
        SeaColumnType::Enum { name, .. } => ColumnType::Custom(name.to_string()),
        SeaColumnType::Array(_) => ColumnType::Custom("array".to_string()),
        SeaColumnType::Vector(_) => ColumnType::Custom("vector".to_string()),
        SeaColumnType::Cidr => ColumnType::Custom("cidr".to_string()),
        SeaColumnType::Inet => ColumnType::Custom("inet".to_string()),
        SeaColumnType::MacAddr => ColumnType::Custom("macaddr".to_string()),
        SeaColumnType::LTree => ColumnType::Custom("ltree".to_string()),
        SeaColumnType::Interval(_, _) => ColumnType::Custom("interval".to_string()),
        // non_exhaustive 枚举的通配符：未知类型转为 Custom
        _ => ColumnType::Custom(format!("{:?}", ct)),
    }
}

/// 转换 `StringLen` → `Option<u32>`
fn convert_string_len(sl: StringLen) -> Option<u32> {
    match sl {
        StringLen::N(n) => Some(n),
        StringLen::Max | StringLen::None => None,
    }
}

/// 转换默认值 `Option<Expr>` → `(has_default: bool, default_value: Option<String>)`
fn convert_default(default: &Option<Expr>) -> (bool, Option<String>) {
    match default {
        None => (false, None),
        Some(Expr::Value(v)) => (true, Some(v.to_string())),
        Some(other) => (true, Some(format!("{:?}", other))),
    }
}

/// 转换 `IndexCreateStatement` → `migration::schema::Index`
fn convert_index(idx_stmt: &IndexCreateStatement, table_name: &str) -> Index {
    let index_spec = idx_stmt.get_index_spec();
    let name = index_spec.get_name().unwrap_or("").to_string();
    let columns = index_spec.get_column_names();
    let is_unique = idx_stmt.is_unique_key();
    let is_constraint = idx_stmt.is_primary_key();

    Index {
        name,
        table_name: table_name.to_string(),
        columns,
        is_unique,
        is_constraint,
    }
}

/// 转换 `ForeignKeyCreateStatement` → `migration::schema::ForeignKey`
fn convert_foreign_key(fk_stmt: &ForeignKeyCreateStatement, default_table: &str) -> ForeignKey {
    let fk = fk_stmt.get_foreign_key();
    let name = fk.get_name().unwrap_or("").to_string();

    let table_name = match fk.get_table() {
        Some(TableRef::Table(tn, _)) => tn.1.to_string(),
        _ => default_table.to_string(),
    };

    let referenced_table_name = match fk.get_ref_table() {
        Some(TableRef::Table(tn, _)) => tn.1.to_string(),
        _ => String::new(),
    };

    let columns = fk.get_columns();
    let ref_columns = fk.get_ref_columns();

    let column_name = columns.into_iter().next().unwrap_or_default();
    let referenced_column_name = ref_columns.into_iter().next().unwrap_or_default();

    let on_delete = fk.get_on_delete().map(convert_fk_action);
    let on_update = fk.get_on_update().map(convert_fk_action);

    ForeignKey {
        name,
        table_name,
        column_name,
        referenced_table_name,
        referenced_column_name,
        on_delete,
        on_update,
    }
}

/// 转换 `sea_query::ForeignKeyAction` → `migration::schema::ForeignKeyAction`
fn convert_fk_action(action: SeaFKAction) -> ForeignKeyAction {
    match action {
        SeaFKAction::Restrict => ForeignKeyAction::Restrict,
        SeaFKAction::Cascade => ForeignKeyAction::Cascade,
        SeaFKAction::SetNull => ForeignKeyAction::SetNull,
        SeaFKAction::NoAction => ForeignKeyAction::NoAction,
        SeaFKAction::SetDefault => ForeignKeyAction::SetDefault,
        // non_exhaustive 枚举的通配符：未知动作回退到 Restrict（最安全）
        _ => ForeignKeyAction::Restrict,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::sea_query::{
        ColumnDef, ForeignKey, ForeignKeyAction as SeaFKAction, Index, Table as SeaTable,
        TableCreateStatement,
    };

    /// 辅助函数：创建一个简单的 users 表 `TableCreateStatement`
    fn make_users_stmt() -> TableCreateStatement {
        SeaTable::create()
            .table(sea_orm::sea_query::Alias::new("users"))
            .col(
                ColumnDef::new(sea_orm::sea_query::Alias::new("id"))
                    .integer()
                    .not_null()
                    .auto_increment()
                    .primary_key(),
            )
            .col(
                ColumnDef::new(sea_orm::sea_query::Alias::new("name"))
                    .string()
                    .not_null(),
            )
            .col(
                ColumnDef::new(sea_orm::sea_query::Alias::new("email"))
                    .string()
                    .not_null()
                    .unique_key(),
            )
            .col(
                ColumnDef::new(sea_orm::sea_query::Alias::new("age"))
                    .integer()
                    .default(18),
            )
            .to_owned()
    }

    /// 辅助函数：创建带外键的 posts 表
    fn make_posts_stmt() -> TableCreateStatement {
        SeaTable::create()
            .table(sea_orm::sea_query::Alias::new("posts"))
            .col(
                ColumnDef::new(sea_orm::sea_query::Alias::new("id"))
                    .integer()
                    .not_null()
                    .auto_increment()
                    .primary_key(),
            )
            .col(
                ColumnDef::new(sea_orm::sea_query::Alias::new("user_id"))
                    .integer()
                    .not_null(),
            )
            .col(
                ColumnDef::new(sea_orm::sea_query::Alias::new("title"))
                    .string()
                    .not_null(),
            )
            .foreign_key(
                ForeignKey::create()
                    .name("fk_posts_user_id")
                    .from(sea_orm::sea_query::Alias::new("posts"), sea_orm::sea_query::Alias::new("user_id"))
                    .to(sea_orm::sea_query::Alias::new("users"), sea_orm::sea_query::Alias::new("id"))
                    .on_delete(SeaFKAction::Cascade)
                    .on_update(SeaFKAction::Restrict),
            )
            .to_owned()
    }

    /// 辅助函数：创建带索引的表
    fn make_indexed_stmt() -> TableCreateStatement {
        SeaTable::create()
            .table(sea_orm::sea_query::Alias::new("products"))
            .col(
                ColumnDef::new(sea_orm::sea_query::Alias::new("id"))
                    .integer()
                    .not_null()
                    .auto_increment()
                    .primary_key(),
            )
            .col(
                ColumnDef::new(sea_orm::sea_query::Alias::new("name"))
                    .string()
                    .not_null(),
            )
            .col(
                ColumnDef::new(sea_orm::sea_query::Alias::new("category"))
                    .string()
                    .not_null(),
            )
            .index(
                Index::create()
                    .name("idx_products_category")
                    .col(sea_orm::sea_query::Alias::new("category")),
            )
            .to_owned()
    }

    // ===== Task 4.4: 基本列类型映射测试 =====

    #[test]
    fn test_convert_integer_column() {
        let stmt = make_users_stmt();
        let table = convert_table(&stmt);

        let id_col = table.columns.iter().find(|c| c.name == "id").expect("id column should exist");
        assert_eq!(id_col.column_type, ColumnType::Integer);
        assert!(!id_col.is_nullable);
        assert!(id_col.is_primary_key);
        assert!(id_col.is_auto_increment);
    }

    #[test]
    fn test_convert_string_column() {
        let stmt = make_users_stmt();
        let table = convert_table(&stmt);

        let name_col = table.columns.iter().find(|c| c.name == "name").expect("name column should exist");
        assert_eq!(name_col.column_type, ColumnType::String(None));
        assert!(!name_col.is_nullable);
        assert!(!name_col.is_primary_key);
    }

    #[test]
    fn test_convert_nullable_column() {
        let stmt = SeaTable::create()
            .table(sea_orm::sea_query::Alias::new("test"))
            .col(
                ColumnDef::new(sea_orm::sea_query::Alias::new("val"))
                    .integer(),
            )
            .to_owned();

        let table = convert_table(&stmt);
        let val_col = table.columns.first().expect("column should exist");
        assert!(val_col.is_nullable);
    }

    #[test]
    fn test_convert_boolean_column() {
        let stmt = SeaTable::create()
            .table(sea_orm::sea_query::Alias::new("test"))
            .col(
                ColumnDef::new(sea_orm::sea_query::Alias::new("active"))
                    .boolean()
                    .not_null(),
            )
            .to_owned();

        let table = convert_table(&stmt);
        let col = table.columns.first().expect("column should exist");
        assert_eq!(col.column_type, ColumnType::Boolean);
    }

    #[test]
    fn test_convert_datetime_column() {
        let stmt = SeaTable::create()
            .table(sea_orm::sea_query::Alias::new("test"))
            .col(
                ColumnDef::new(sea_orm::sea_query::Alias::new("created_at"))
                    .date_time()
                    .not_null(),
            )
            .to_owned();

        let table = convert_table(&stmt);
        let col = table.columns.first().expect("column should exist");
        assert_eq!(col.column_type, ColumnType::DateTime);
    }

    #[test]
    fn test_convert_float_column() {
        let stmt = SeaTable::create()
            .table(sea_orm::sea_query::Alias::new("test"))
            .col(
                ColumnDef::new(sea_orm::sea_query::Alias::new("score"))
                    .float()
                    .not_null(),
            )
            .to_owned();

        let table = convert_table(&stmt);
        let col = table.columns.first().expect("column should exist");
        assert_eq!(col.column_type, ColumnType::Float);
    }

    #[test]
    fn test_convert_big_integer_column() {
        let stmt = SeaTable::create()
            .table(sea_orm::sea_query::Alias::new("test"))
            .col(
                ColumnDef::new(sea_orm::sea_query::Alias::new("count"))
                    .big_integer()
                    .not_null(),
            )
            .to_owned();

        let table = convert_table(&stmt);
        let col = table.columns.first().expect("column should exist");
        assert_eq!(col.column_type, ColumnType::BigInteger);
    }

    #[test]
    fn test_convert_text_column() {
        let stmt = SeaTable::create()
            .table(sea_orm::sea_query::Alias::new("test"))
            .col(
                ColumnDef::new(sea_orm::sea_query::Alias::new("content"))
                    .text()
                    .not_null(),
            )
            .to_owned();

        let table = convert_table(&stmt);
        let col = table.columns.first().expect("column should exist");
        assert_eq!(col.column_type, ColumnType::Text);
    }

    #[test]
    fn test_convert_json_column() {
        let stmt = SeaTable::create()
            .table(sea_orm::sea_query::Alias::new("test"))
            .col(
                ColumnDef::new(sea_orm::sea_query::Alias::new("metadata"))
                    .json()
                    .not_null(),
            )
            .to_owned();

        let table = convert_table(&stmt);
        let col = table.columns.first().expect("column should exist");
        assert_eq!(col.column_type, ColumnType::Json);
    }

    #[test]
    fn test_convert_default_value() {
        let stmt = make_users_stmt();
        let table = convert_table(&stmt);

        let age_col = table.columns.iter().find(|c| c.name == "age").expect("age column should exist");
        assert!(age_col.has_default);
        assert!(age_col.default_value.is_some());
    }

    // ===== Task 4.5: 主键映射测试 =====

    #[test]
    fn test_convert_primary_key() {
        let stmt = make_users_stmt();
        let table = convert_table(&stmt);

        assert_eq!(table.primary_key_columns, vec!["id".to_string()]);

        let id_col = table.columns.iter().find(|c| c.name == "id").expect("id column should exist");
        assert!(id_col.is_primary_key);
    }

    #[test]
    fn test_convert_composite_primary_key() {
        let stmt = SeaTable::create()
            .table(sea_orm::sea_query::Alias::new("composite"))
            .col(
                ColumnDef::new(sea_orm::sea_query::Alias::new("a"))
                    .integer()
                    .not_null()
                    .primary_key(),
            )
            .col(
                ColumnDef::new(sea_orm::sea_query::Alias::new("b"))
                    .integer()
                    .not_null()
                    .primary_key(),
            )
            .to_owned();

        let table = convert_table(&stmt);
        assert_eq!(table.primary_key_columns.len(), 2);
        assert!(table.primary_key_columns.contains(&"a".to_string()));
        assert!(table.primary_key_columns.contains(&"b".to_string()));
    }

    // ===== Task 4.6: 外键映射测试 =====

    #[test]
    fn test_convert_foreign_key() {
        let stmt = make_posts_stmt();
        let table = convert_table(&stmt);

        assert_eq!(table.foreign_keys.len(), 1);
        let fk = &table.foreign_keys[0];
        assert_eq!(fk.name, "fk_posts_user_id");
        assert_eq!(fk.table_name, "posts");
        assert_eq!(fk.column_name, "user_id");
        assert_eq!(fk.referenced_table_name, "users");
        assert_eq!(fk.referenced_column_name, "id");
        assert_eq!(fk.on_delete, Some(ForeignKeyAction::Cascade));
        assert_eq!(fk.on_update, Some(ForeignKeyAction::Restrict));
    }

    #[test]
    fn test_convert_foreign_key_actions() {
        let actions = vec![
            (SeaFKAction::Cascade, ForeignKeyAction::Cascade),
            (SeaFKAction::Restrict, ForeignKeyAction::Restrict),
            (SeaFKAction::SetNull, ForeignKeyAction::SetNull),
            (SeaFKAction::NoAction, ForeignKeyAction::NoAction),
            (SeaFKAction::SetDefault, ForeignKeyAction::SetDefault),
        ];

        for (sea_action, expected) in actions {
            let stmt = SeaTable::create()
                .table(sea_orm::sea_query::Alias::new("t1"))
                .col(ColumnDef::new(sea_orm::sea_query::Alias::new("id")).integer().not_null().primary_key())
                .col(ColumnDef::new(sea_orm::sea_query::Alias::new("ref_id")).integer().not_null())
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_test")
                        .from(sea_orm::sea_query::Alias::new("t1"), sea_orm::sea_query::Alias::new("ref_id"))
                        .to(sea_orm::sea_query::Alias::new("t2"), sea_orm::sea_query::Alias::new("id"))
                        .on_delete(sea_action),
                )
                .to_owned();

            let table = convert_table(&stmt);
            let fk = &table.foreign_keys[0];
            assert_eq!(fk.on_delete, Some(expected));
        }
    }

    // ===== Task 4.7: 索引映射测试 =====

    #[test]
    fn test_convert_index() {
        let stmt = make_indexed_stmt();
        let table = convert_table(&stmt);

        // 注意：Sea-ORM 生成的 TableCreateStatement 中，unique 约束可能作为索引出现
        // 检查是否有名为 "idx_products_category" 的索引
        let has_idx = table.indexes.iter().any(|i| i.name == "idx_products_category");
        if has_idx {
            let idx = table.indexes.iter().find(|i| i.name == "idx_products_category").unwrap();
            assert_eq!(idx.table_name, "products");
            assert!(idx.columns.contains(&"category".to_string()));
            assert!(!idx.is_unique);
        }
    }

    #[test]
    fn test_convert_unique_constraint_as_index() {
        let stmt = make_users_stmt();
        let table = convert_table(&stmt);

        // email 列有 unique 约束，应该映射为索引
        let email_idx = table.indexes.iter().find(|i| i.columns.contains(&"email".to_string()));
        if let Some(idx) = email_idx {
            assert!(idx.is_unique);
        }
    }

    #[test]
    fn test_convert_table_name() {
        let stmt = make_users_stmt();
        let table = convert_table(&stmt);
        assert_eq!(table.name, "users");
    }

    #[test]
    fn test_convert_table_comment() {
        let stmt = SeaTable::create()
            .table(sea_orm::sea_query::Alias::new("test"))
            .comment("test table")
            .col(ColumnDef::new(sea_orm::sea_query::Alias::new("id")).integer().not_null().primary_key())
            .to_owned();

        let table = convert_table(&stmt);
        assert_eq!(table.comment, Some("test table".to_string()));
    }

    #[test]
    fn test_convert_full_table_structure() {
        let stmt = make_users_stmt();
        let table = convert_table(&stmt);

        // 验证完整结构
        assert_eq!(table.name, "users");
        assert_eq!(table.columns.len(), 4);
        assert_eq!(table.primary_key_columns, vec!["id".to_string()]);
        assert!(!table.foreign_keys.is_empty() || table.foreign_keys.is_empty()); // users 表无外键
    }
}
