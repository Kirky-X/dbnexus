// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 泛型仓储
//!
//! 提供类型安全的 `Repository<T>` CRUD 端口与基于 `DbPool` 行查询管道的
//! JSON 行实现（`JsonRepository`），并配套 `impl_json_repository!` 宏为
//! 具体实体一键生成 `Repository<T>` 实现。
//!
//! # 设计口径
//!
//! - 实体约束为 `Serialize + DeserializeOwned`：行进出统一走
//!   `serde_json::Value`（与 `query_rows` 出口一致）；
//! - 底层执行复用 `DbPool::query_rows` / `Session::execute_raw`，自动继承
//!   解析校验/权限检查/注入检测/慢查询统计整条防御链；
//! - 表名与列名做标识符白名单校验（防注入），字符串值按 SQL 标准转义
//!   （单引号加倍）；复杂值（数组/对象）序列化为 JSON 字符串存储。
//!
//! # 示例
//!
//! ```ignore
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Serialize, Deserialize, Debug, PartialEq)]
//! struct User { id: i64, name: String }
//!
//! #[derive(Default)]
//! struct UserRepo;
//! dbnexus::impl_json_repository!(UserRepo, User, table = "users");
//!
//! // let rows = repo.find_all(&pool, 10, 0).await?;
//! ```

use async_trait::async_trait;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::database::DbPool;
use crate::foundation::{DbError, DbResult};

/// 泛型仓储 CRUD 端口
///
/// `T` 为实体类型（`Serialize + DeserializeOwned`）。实现方决定存储后端
/// （`JsonRepository` 为 DbPool JSON 行实现的参考实现）。
#[async_trait]
pub trait Repository<T>: Send + Sync
where
    T: Serialize + DeserializeOwned,
{
    /// 仓储对应的表名
    fn table(&self) -> &str;

    /// 插入实体，返回新生成的主键 ID
    async fn insert(&self, pool: &DbPool, entity: &T) -> DbResult<i64>;

    /// 按主键查询实体
    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<T>>;

    /// 分页查询（`limit`/`offset`，按主键升序）
    async fn find_all(&self, pool: &DbPool, limit: u64, offset: u64) -> DbResult<Vec<T>>;

    /// 按主键更新实体，返回受影响行数
    async fn update(&self, pool: &DbPool, id: i64, entity: &T) -> DbResult<u64>;

    /// 按主键删除实体，返回受影响行数
    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<u64>;

    /// 统计表内总行数
    async fn count(&self, pool: &DbPool) -> DbResult<u64>;
}

/// 校验 SQL 标识符（表名/列名）白名单：字母或下划线开头，仅含
/// 字母/数字/下划线，长度 1-64
pub(crate) fn is_safe_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && name.len() <= 64
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// 将 JSON 值转为 SQL 字面量
///
/// - 字符串：单引号加倍转义（SQL 标准转义，值经字面量传递，
///   注入扫描器对字符串字面量内容不敏感）；
/// - 数组/对象：序列化为 JSON 字符串字面量；
/// - 布尔：TRUE/FALSE；空值：NULL。
pub(crate) fn sql_literal(value: &Value) -> DbResult<String> {
    match value {
        Value::Null => Ok("NULL".to_string()),
        Value::Bool(b) => Ok(if *b { "TRUE" } else { "FALSE" }.to_string()),
        Value::Number(n) => Ok(n.to_string()),
        Value::String(s) => Ok(format!("'{}'", s.replace('\'', "''"))),
        Value::Array(_) | Value::Object(_) => {
            let json = serde_json::to_string(value).map_err(|e| {
                DbError::Config(format!("entity nested value serialize failed: {e}"))
            })?;
            Ok(format!("'{}'", json.replace('\'', "''")))
        }
    }
}

/// 基于 `DbPool` 行查询管道的 `Repository<T>` 参考实现
///
/// 行数据以 `serde_json::Value` 对象承载（与 `query_rows` 出口一致），
/// 实体经 serde 与行对象互转。
pub struct JsonRepository {
    table: String,
    id_column: String,
    role: String,
}

impl JsonRepository {
    /// 创建仓储（默认主键列 `id`，默认执行角色 `admin`）
    ///
    /// # Errors
    ///
    /// 表名不含合法标识符时返回 `DbError::Config`
    pub fn new(table: &str) -> DbResult<Self> {
        if !is_safe_identifier(table) {
            return Err(DbError::Config(format!(
                "repository table name must be a safe identifier: '{table}'"
            )));
        }
        Ok(Self {
            table: table.to_string(),
            id_column: "id".to_string(),
            role: "admin".to_string(),
        })
    }

    /// 自定义主键列
    ///
    /// # Errors
    ///
    /// 列名不含合法标识符时返回 `DbError::Config`
    pub fn with_id_column(mut self, id_column: &str) -> DbResult<Self> {
        if !is_safe_identifier(id_column) {
            return Err(DbError::Config(format!(
                "repository id column must be a safe identifier: '{id_column}'"
            )));
        }
        self.id_column = id_column.to_string();
        Ok(self)
    }

    /// 自定义执行角色（透传给 `query_rows` 的角色参数）
    pub fn with_role(mut self, role: &str) -> Self {
        self.role = role.to_string();
        self
    }

    fn entity_object<T: Serialize>(&self, entity: &T) -> DbResult<serde_json::Map<String, Value>> {
        match serde_json::to_value(entity)
            .map_err(|e| DbError::Config(format!("entity serialize failed: {e}")))?
        {
            Value::Object(map) => Ok(map),
            _ => Err(DbError::Config(
                "repository entity must serialize to a JSON object".to_string(),
            )),
        }
    }
}

#[async_trait]
impl<T> Repository<T> for JsonRepository
where
    T: Serialize + DeserializeOwned + Send + Sync,
{
    fn table(&self) -> &str {
        &self.table
    }

    async fn insert(&self, pool: &DbPool, entity: &T) -> DbResult<i64> {
        let map = self.entity_object(entity)?;
        if map.is_empty() {
            return Err(DbError::Config(
                "repository insert requires at least one column".to_string(),
            ));
        }
        let mut columns = Vec::with_capacity(map.len());
        let mut values = Vec::with_capacity(map.len());
        for (col, value) in &map {
            if !is_safe_identifier(col) {
                return Err(DbError::Config(format!(
                    "repository column must be a safe identifier: '{col}'"
                )));
            }
            columns.push(col.clone());
            values.push(sql_literal(value)?);
        }
        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            self.table,
            columns.join(", "),
            values.join(", ")
        );
        let session = pool.get_session(&self.role).await?;
        let exec = session.execute_raw(&sql).await?;
        Ok(exec.last_insert_id() as i64)
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<T>> {
        let sql = format!(
            "SELECT * FROM {} WHERE {} = {}",
            self.table, self.id_column, id
        );
        let rows = pool.query_rows(&sql, &self.role).await?;
        match rows.into_iter().next() {
            Some(row) => Ok(Some(serde_json::from_value(row).map_err(|e| {
                DbError::Config(format!("entity deserialize failed: {e}"))
            })?)),
            None => Ok(None),
        }
    }

    async fn find_all(&self, pool: &DbPool, limit: u64, offset: u64) -> DbResult<Vec<T>> {
        let sql = format!(
            "SELECT * FROM {} ORDER BY {} LIMIT {} OFFSET {}",
            self.table, self.id_column, limit, offset
        );
        let rows = pool.query_rows(&sql, &self.role).await?;
        rows.into_iter()
            .map(|row| {
                serde_json::from_value(row)
                    .map_err(|e| DbError::Config(format!("entity deserialize failed: {e}")))
            })
            .collect()
    }

    async fn update(&self, pool: &DbPool, id: i64, entity: &T) -> DbResult<u64> {
        let map = self.entity_object(entity)?;
        if map.is_empty() {
            return Err(DbError::Config(
                "repository update requires at least one column".to_string(),
            ));
        }
        let mut assignments = Vec::with_capacity(map.len());
        for (col, value) in &map {
            if !is_safe_identifier(col) {
                return Err(DbError::Config(format!(
                    "repository column must be a safe identifier: '{col}'"
                )));
            }
            if col == &self.id_column {
                continue; // 主键不参与 SET
            }
            assignments.push(format!("{} = {}", col, sql_literal(value)?));
        }
        if assignments.is_empty() {
            return Err(DbError::Config(
                "repository update requires at least one non-id column".to_string(),
            ));
        }
        let sql = format!(
            "UPDATE {} SET {} WHERE {} = {}",
            self.table,
            assignments.join(", "),
            self.id_column,
            id
        );
        let session = pool.get_session(&self.role).await?;
        let exec = session.execute_raw(&sql).await?;
        Ok(exec.rows_affected())
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<u64> {
        let sql = format!(
            "DELETE FROM {} WHERE {} = {}",
            self.table, self.id_column, id
        );
        let session = pool.get_session(&self.role).await?;
        let exec = session.execute_raw(&sql).await?;
        Ok(exec.rows_affected())
    }

    async fn count(&self, pool: &DbPool) -> DbResult<u64> {
        // 注：sqlite 方言的 query_rows 按主表列内省构造行对象，
        // `SELECT COUNT(*) AS cnt` 的聚合列不在表列清单内会被丢弃，
        // 故以主键列全量取回后计行数（MVP 口径，大表请配合分页/上游聚合）。
        let sql = format!("SELECT {} FROM {}", self.id_column, self.table);
        let rows = pool.query_rows(&sql, &self.role).await?;
        Ok(rows.len() as u64)
    }
}

/// 为具体实体一键生成 `Repository<T>` 实现
///
/// 生成的实现内部委托 [`JsonRepository`]（每次调用按声明的表名构造，
/// 无共享状态）。实体需 `Serialize + DeserializeOwned`，承载结构体需
/// `Default`（或自行提供构造）。
///
/// ```ignore
/// #[derive(Default)]
/// struct UserRepo;
/// dbnexus::impl_json_repository!(UserRepo, User, table = "users");
/// // 或自定义主键列：
/// dbnexus::impl_json_repository!(UserRepo, User, table = "users", id = "user_id");
/// ```
#[macro_export]
macro_rules! impl_json_repository {
    ($repo:ident, $entity:ty, table = $table:expr $(, id = $id:expr)?) => {
        #[async_trait::async_trait]
        impl $crate::database::repository::Repository<$entity> for $repo {
            fn table(&self) -> &str {
                // 表名为编译期常量；JsonRepository::new 的运行时校验仍会执行
                $table
            }

            async fn insert(
                &self,
                pool: &$crate::database::DbPool,
                entity: &$entity,
            ) -> $crate::foundation::DbResult<i64> {
                $crate::database::repository::Repository::<$entity>::insert(
                    &($crate::database::repository::JsonRepository::new($table)?
                        $(.with_id_column($id)?)?),
                    pool,
                    entity,
                )
                .await
            }

            async fn find_by_id(
                &self,
                pool: &$crate::database::DbPool,
                id: i64,
            ) -> $crate::foundation::DbResult<Option<$entity>> {
                $crate::database::repository::Repository::<$entity>::find_by_id(
                    &($crate::database::repository::JsonRepository::new($table)?
                        $(.with_id_column($id)?)?),
                    pool,
                    id,
                )
                .await
            }

            async fn find_all(
                &self,
                pool: &$crate::database::DbPool,
                limit: u64,
                offset: u64,
            ) -> $crate::foundation::DbResult<Vec<$entity>> {
                $crate::database::repository::Repository::<$entity>::find_all(
                    &($crate::database::repository::JsonRepository::new($table)?
                        $(.with_id_column($id)?)?),
                    pool,
                    limit,
                    offset,
                )
                .await
            }

            async fn update(
                &self,
                pool: &$crate::database::DbPool,
                id: i64,
                entity: &$entity,
            ) -> $crate::foundation::DbResult<u64> {
                $crate::database::repository::Repository::<$entity>::update(
                    &($crate::database::repository::JsonRepository::new($table)?
                        $(.with_id_column($id)?)?),
                    pool,
                    id,
                    entity,
                )
                .await
            }

            async fn delete(
                &self,
                pool: &$crate::database::DbPool,
                id: i64,
            ) -> $crate::foundation::DbResult<u64> {
                $crate::database::repository::Repository::<$entity>::delete(
                    &($crate::database::repository::JsonRepository::new($table)?
                        $(.with_id_column($id)?)?),
                    pool,
                    id,
                )
                .await
            }

            async fn count(
                &self,
                pool: &$crate::database::DbPool,
            ) -> $crate::foundation::DbResult<u64> {
                $crate::database::repository::Repository::<$entity>::count(
                    &($crate::database::repository::JsonRepository::new($table)?
                        $(.with_id_column($id)?)?),
                    pool,
                )
                .await
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_safe_identifier() {
        assert!(is_safe_identifier("users"));
        assert!(is_safe_identifier("_private_tbl2"));
        assert!(!is_safe_identifier("1abc"));
        assert!(!is_safe_identifier("has space"));
        assert!(!is_safe_identifier("a; DROP TABLE x"));
        assert!(!is_safe_identifier(""));
        assert!(!is_safe_identifier(&"a".repeat(65)));
    }

    #[test]
    fn test_sql_literal_escaping() {
        assert_eq!(sql_literal(&Value::Null).unwrap(), "NULL");
        assert_eq!(sql_literal(&Value::Bool(true)).unwrap(), "TRUE");
        assert_eq!(sql_literal(&serde_json::json!(42)).unwrap(), "42");
        // 单引号加倍转义
        assert_eq!(
            sql_literal(&serde_json::json!("O'Brien")).unwrap(),
            "'O''Brien'"
        );
        // 数组/对象 → JSON 字符串
        let lit = sql_literal(&serde_json::json!([1, 2])).unwrap();
        assert_eq!(lit, "'[1,2]'");
    }

    // 宏展开 in-crate 最小验证（类型推断检查）
    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Clone)]
    struct _MacroUser {
        id: i64,
        name: String,
    }

    #[derive(Default)]
    struct _MacroUserRepo;

    crate::impl_json_repository!(_MacroUserRepo, _MacroUser, table = "t418_users");

    #[test]
    fn test_macro_impl_type_inference_in_crate() {
        let repo = _MacroUserRepo;
        assert_eq!(repo.table(), "t418_users");
    }

    #[test]
    fn test_json_repository_validates_identifiers() {
        assert!(JsonRepository::new("users").is_ok());
        assert!(JsonRepository::new("users; DROP TABLE x").is_err());
        assert!(
            JsonRepository::new("users")
                .unwrap()
                .with_id_column("key")
                .is_ok()
        );
        assert!(
            JsonRepository::new("users")
                .unwrap()
                .with_id_column("1bad")
                .is_err()
        );
    }
}
