// Copyright (c) 2025-2026 Kirky.X🌠
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
//! - 表名与列名做标识符白名单校验（防注入）；**值一律经绑定参数传递**
//!   （PostgreSQL `$N`、其余方言 `?`），SQL 文本不含值字面量；
//!   复杂值（数组/对象）序列化为 JSON 字符串后绑定。
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
    ///
    /// # 性能警示
    ///
    /// OFFSET 分页在深页码需扫描并丢弃前 N 行，代价与页深成正比；
    /// 优先使用 [`JsonRepository::find_all_cursor`] 键集分页。
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
pub(crate) fn sql_literal(value: &Value, backend: SqlBackend) -> DbResult<String> {
    match value {
        Value::Null => Ok("NULL".to_string()),
        Value::Bool(b) => Ok(if *b { "TRUE" } else { "FALSE" }.to_string()),
        Value::Number(n) => Ok(n.to_string()),
        Value::String(s) => Ok(format!("'{}'", escape_sql_string(s, backend))),
        Value::Array(_) | Value::Object(_) => {
            let json = serde_json::to_string(value).map_err(|e| {
                DbError::Config(format!("entity nested value serialize failed: {e}"))
            })?;
            Ok(format!("'{}'", escape_sql_string(&json, backend)))
        }
    }
}

/// 字符串字面量转义的数据库后端口径
///
/// 反斜杠语义按后端分化：MySQL 默认模式（`NO_BACKSLASH_ESCAPES` 关闭）下
/// `\` 是转义符，仅加倍单引号可被 `\'` 序列击穿（注入向量）；PostgreSQL
/// （standard_conforming_strings=on）、SQLite、DuckDB 遵循 SQL 标准，
/// `\` 是字面量，加倍会引入多余字符。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlBackend {
    /// MySQL：先加倍反斜杠，再加倍单引号
    MySql,
    /// SQL 标准口径（PostgreSQL / SQLite / DuckDB）：仅加倍单引号
    Standard,
}

/// 按后端转义字符串字面量内容（不含外层引号）
fn escape_sql_string(s: &str, backend: SqlBackend) -> String {
    match backend {
        SqlBackend::MySql => s.replace('\\', "\\\\").replace('\'', "''"),
        SqlBackend::Standard => s.replace('\'', "''"),
    }
}

/// 解析池的 sea-orm 后端（占位符方言判定用）
fn sea_backend(pool: &DbPool) -> sea_orm::DatabaseBackend {
    crate::database::DbPool::get_database_backend(&pool.inner.config.url)
}

/// 绑定占位符：PostgreSQL `$N`，其余方言 `?`
fn placeholder(backend: sea_orm::DatabaseBackend, i: usize) -> String {
    match backend {
        sea_orm::DatabaseBackend::Postgres => format!("${i}"),
        _ => "?".to_string(),
    }
}

/// 从连接池解析字面量转义后端口径
pub(crate) fn resolve_sql_backend(pool: &DbPool) -> SqlBackend {
    if crate::database::DbPool::get_database_backend(&pool.inner.config.url)
        == sea_orm::DatabaseBackend::MySql
    {
        SqlBackend::MySql
    } else {
        SqlBackend::Standard
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
    /// 显式列清单（`with_columns` 设置后查询生成列投影，替代 `SELECT *`）
    columns: Option<Vec<String>>,
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
            columns: None,
        })
    }

    /// 设置显式列清单（列投影，避免 `SELECT *`）
    ///
    /// 设置后 `find_by_id` / `find_all` / `find_all_cursor` 生成
    /// `SELECT {columns}` 而非 `SELECT *`；列名逐一校验
    /// [`is_safe_identifier`]。
    ///
    /// # Errors
    ///
    /// 任一列名不含合法标识符时返回 `DbError::Config`
    pub fn with_columns(mut self, columns: &[&str]) -> DbResult<Self> {
        for col in columns {
            if !is_safe_identifier(col) {
                return Err(DbError::Config(format!(
                    "repository column must be a safe identifier: '{col}'"
                )));
            }
        }
        self.columns = Some(columns.iter().map(|c| c.to_string()).collect());
        Ok(self)
    }

    /// 查询列子句：显式投影或 `*`
    fn select_clause(&self) -> String {
        match &self.columns {
            Some(cols) => cols.join(", "),
            None => "*".to_string(),
        }
    }

    /// 游标分页（键集分页，深分页友好）
    ///
    /// 返回 `{id_column} > {after_id}` 的升序 `limit` 行；翻页时传上一页
    /// 末行 id 即可全量遍历，无 OFFSET 深分页的扫描放大。
    ///
    /// # Errors
    ///
    /// 透传 [`DbPool::query_rows`] 的错误
    pub async fn find_all_cursor<T: Serialize + DeserializeOwned>(
        &self,
        pool: &DbPool,
        after_id: i64,
        limit: u64,
    ) -> DbResult<Vec<T>> {
        let backend = sea_backend(pool);
        let sql = format!(
            "SELECT {} FROM {} WHERE {} > {} ORDER BY {} LIMIT {}",
            self.select_clause(),
            self.table,
            self.id_column,
            placeholder(backend, 1),
            self.id_column,
            placeholder(backend, 2)
        );
        let session = pool.get_session(&self.role).await?;
        let rows = session
            .query_rows_with_params(
                &sql,
                &[
                    serde_json::Value::from(after_id),
                    serde_json::Value::from(limit),
                ],
            )
            .await?;
        rows.into_iter()
            .map(|row| {
                serde_json::from_value(row)
                    .map_err(|e| DbError::Config(format!("entity deserialize failed: {e}")))
            })
            .collect()
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

    /// 乐观锁更新：带版本条件的比较并置换（CAS）
    ///
    /// 从 `entity` 的 `version_column` 字段取期望版本，生成
    /// `UPDATE {table} SET {cols}, {version_column} = {version_column} + 1
    /// WHERE {id_column} = {id} AND {version_column} = {expected}`。
    /// 影响行数为 0（版本过期或行不存在）时返回 `DbError::VersionConflict`，
    /// 调用方应重读实体后重试；成功时版本列自增 1。
    ///
    /// # Errors
    ///
    /// - `version_column` 非法标识符或实体缺失该字段 / 版本非整数：`DbError::Config`
    /// - 条件未命中：`DbError::VersionConflict`
    pub async fn update_if_version<T: Serialize + DeserializeOwned>(
        &self,
        pool: &DbPool,
        id: i64,
        entity: &T,
        version_column: &str,
    ) -> DbResult<u64> {
        if !is_safe_identifier(version_column) {
            return Err(DbError::Config(format!(
                "repository version column must be a safe identifier: '{version_column}'"
            )));
        }
        let map = self.entity_object(entity)?;
        let expected = map
            .get(version_column)
            .ok_or_else(|| {
                DbError::Config(format!(
                    "update_if_version requires field '{version_column}' on the entity"
                ))
            })?
            .as_i64()
            .ok_or_else(|| {
                DbError::Config(format!(
                    "version column '{version_column}' must be an integer"
                ))
            })?;

        let backend = sea_backend(pool);
        let mut assignments = Vec::new();
        let mut params: Vec<Value> = Vec::new();
        let mut n = 0usize;
        for (col, value) in &map {
            if !is_safe_identifier(col) {
                return Err(DbError::Config(format!(
                    "repository column must be a safe identifier: '{col}'"
                )));
            }
            // 主键与版本列不参与 SET（版本列由 +1 表达式承载）
            if col == &self.id_column || col == version_column {
                continue;
            }
            n += 1;
            assignments.push(format!("{} = {}", col, placeholder(backend, n)));
            params.push(value.clone());
        }
        let set_clause = if assignments.is_empty() {
            String::new()
        } else {
            format!(", {}", assignments.join(", "))
        };
        // WHERE 的两个绑定：id 与期望版本
        n += 1;
        let id_ph = placeholder(backend, n);
        n += 1;
        let version_ph = placeholder(backend, n);
        let sql = format!(
            "UPDATE {} SET {} = {} + 1{} WHERE {} = {} AND {} = {}",
            self.table,
            version_column,
            version_column,
            set_clause,
            self.id_column,
            id_ph,
            version_column,
            version_ph
        );
        params.push(Value::from(id));
        params.push(Value::from(expected));
        let session = pool.get_session(&self.role).await?;
        let exec = session.execute_with_params(&sql, &params).await?;
        let affected = exec.rows_affected();
        if affected == 0 {
            return Err(DbError::VersionConflict {
                table: self.table.clone(),
                id,
            });
        }
        Ok(affected)
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
        let backend = sea_backend(pool);
        let mut columns = Vec::with_capacity(map.len());
        let mut placeholders = Vec::with_capacity(map.len());
        let mut params = Vec::with_capacity(map.len());
        for (i, (col, value)) in map.iter().enumerate() {
            if !is_safe_identifier(col) {
                return Err(DbError::Config(format!(
                    "repository column must be a safe identifier: '{col}'"
                )));
            }
            columns.push(col.clone());
            placeholders.push(placeholder(backend, i + 1));
            params.push(value.clone());
        }
        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            self.table,
            columns.join(", "),
            placeholders.join(", ")
        );
        let session = pool.get_session(&self.role).await?;
        let exec = session.execute_with_params(&sql, &params).await?;
        Ok(exec.last_insert_id() as i64)
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<T>> {
        let sql = format!(
            "SELECT {} FROM {} WHERE {} = {}",
            self.select_clause(),
            self.table,
            self.id_column,
            placeholder(sea_backend(pool), 1)
        );
        let session = pool.get_session(&self.role).await?;
        let rows = session
            .query_rows_with_params(&sql, &[serde_json::Value::from(id)])
            .await?;
        match rows.into_iter().next() {
            Some(row) => Ok(Some(serde_json::from_value(row).map_err(|e| {
                DbError::Config(format!("entity deserialize failed: {e}"))
            })?)),
            None => Ok(None),
        }
    }

    async fn find_all(&self, pool: &DbPool, limit: u64, offset: u64) -> DbResult<Vec<T>> {
        let backend = sea_backend(pool);
        let sql = format!(
            "SELECT {} FROM {} ORDER BY {} LIMIT {} OFFSET {}",
            self.select_clause(),
            self.table,
            self.id_column,
            placeholder(backend, 1),
            placeholder(backend, 2)
        );
        let session = pool.get_session(&self.role).await?;
        let rows = session
            .query_rows_with_params(
                &sql,
                &[
                    serde_json::Value::from(limit),
                    serde_json::Value::from(offset),
                ],
            )
            .await?;
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
        let backend = sea_backend(pool);
        let mut assignments = Vec::with_capacity(map.len());
        let mut params = Vec::with_capacity(map.len() + 1);
        let mut n = 0usize;
        for (col, value) in &map {
            if !is_safe_identifier(col) {
                return Err(DbError::Config(format!(
                    "repository column must be a safe identifier: '{col}'"
                )));
            }
            if col == &self.id_column {
                continue; // 主键不参与 SET
            }
            n += 1;
            assignments.push(format!("{} = {}", col, placeholder(backend, n)));
            params.push(value.clone());
        }
        if assignments.is_empty() {
            return Err(DbError::Config(
                "repository update requires at least one non-id column".to_string(),
            ));
        }
        n += 1;
        let sql = format!(
            "UPDATE {} SET {} WHERE {} = {}",
            self.table,
            assignments.join(", "),
            self.id_column,
            placeholder(backend, n)
        );
        params.push(serde_json::Value::from(id));
        let session = pool.get_session(&self.role).await?;
        let exec = session.execute_with_params(&sql, &params).await?;
        Ok(exec.rows_affected())
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<u64> {
        let sql = format!(
            "DELETE FROM {} WHERE {} = {}",
            self.table,
            self.id_column,
            placeholder(sea_backend(pool), 1)
        );
        let session = pool.get_session(&self.role).await?;
        let exec = session
            .execute_with_params(&sql, &[serde_json::Value::from(id)])
            .await?;
        Ok(exec.rows_affected())
    }

    async fn count(&self, pool: &DbPool) -> DbResult<u64> {
        // 注：sqlite 方言的 query_rows 按主表列内省构造行对象，
        // `SELECT COUNT(*) AS cnt` 的聚合列不在表列清单内会被丢弃，
        // 故以主键列全量取回后计行数（MVP 口径，大表请配合分页/上游聚合）。
        let sql = format!("SELECT {} FROM {}", self.id_column, self.table);
        let session = pool.get_session(&self.role).await?;
        let rows = session.query_rows_with_params(&sql, &[]).await?;
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

        /// 乐观锁更新（转发 [`JsonRepository::update_if_version`]）
        impl $repo {
            /// 带版本条件的比较并置换：见 [`JsonRepository::update_if_version`]
            ///（宏按实体生成；未用到乐观锁的实例化点会触发 dead_code，故放行）
            #[allow(dead_code)]
            pub async fn update_if_version(
                &self,
                pool: &$crate::database::DbPool,
                id: i64,
                entity: &$entity,
                version_column: &str,
            ) -> $crate::foundation::DbResult<u64> {
                $crate::database::repository::JsonRepository::new($table)?
                    $(.with_id_column($id)?)?
                    .update_if_version(pool, id, entity, version_column)
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
        assert_eq!(
            sql_literal(&Value::Null, SqlBackend::Standard).unwrap(),
            "NULL"
        );
        assert_eq!(
            sql_literal(&Value::Bool(true), SqlBackend::Standard).unwrap(),
            "TRUE"
        );
        assert_eq!(
            sql_literal(&serde_json::json!(42), SqlBackend::Standard).unwrap(),
            "42"
        );
        // 单引号加倍转义
        assert_eq!(
            sql_literal(&serde_json::json!("O'Brien"), SqlBackend::Standard).unwrap(),
            "'O''Brien'"
        );
        // 数组/对象 → JSON 字符串
        let lit = sql_literal(&serde_json::json!([1, 2]), SqlBackend::Standard).unwrap();
        assert_eq!(lit, "'[1,2]'");
    }

    /// R-repo-001: MySQL 后端必须同时转义反斜杠——仅加倍单引号会被 `\'`
    /// 序列击穿（值以反斜杠结尾时悬空引号改变字面量边界）
    #[test]
    fn test_sql_literal_mysql_backslash_escaping() {
        // 尾随反斜杠值 `test\`：
        // Standard 口径原样保留反斜杠（PG 字面量语义，安全）
        assert_eq!(
            sql_literal(&serde_json::json!("test\\"), SqlBackend::Standard).unwrap(),
            "'test\\'"
        );
        // MySql 口径反斜杠加倍，字面量边界不再被 `\'` 击穿
        assert_eq!(
            sql_literal(&serde_json::json!("test\\"), SqlBackend::MySql).unwrap(),
            "'test\\\\'"
        );
        // `\'` 注入序列：MySql 转义后 `a\\'' -- …` 整体是一个安全字面量
        assert_eq!(
            sql_literal(
                &serde_json::json!("a\\' -- DROP TABLE users"),
                SqlBackend::MySql
            )
            .unwrap(),
            "'a\\\\'' -- DROP TABLE users'"
        );
        // Standard 口径保持现状（反斜杠是字面量）
        assert_eq!(
            sql_literal(&serde_json::json!("a\\b"), SqlBackend::Standard).unwrap(),
            "'a\\b'"
        );
        assert_eq!(
            sql_literal(&serde_json::json!("a\\b"), SqlBackend::MySql).unwrap(),
            "'a\\\\b'"
        );
        // 单引号在 MySql 下依旧加倍
        assert_eq!(
            sql_literal(&serde_json::json!("it's"), SqlBackend::MySql).unwrap(),
            "'it''s'"
        );
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
