// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 数据 API 网关雏形
//!
//! 实体 → JSON 查询端点生成器：为声明的表端点（表/列白名单 + 分页 + 过滤 +
//! 排序）提供可被上层 Web 框架（如 sdforge）直接包装为 HTTP 端点的
//! 异步查询处理器。
//!
//! # 安全口径
//!
//! - **执行角色必须显式声明**：`DataApiGateway::new` 不带角色（`None`），
//!   查询一律 fail-closed 报错；须经 `with_role` 显式指定角色。
//!   admin 角色会同时绕过表级 RBAC 与 RLS 谓词注入，只能显式 opt-in；
//! - **归属判定**：网关不做隐式归属隔离——`get`/`list` 只按主键与调用方
//!   给出的过滤条件取数；多租户场景须用 `get_owned`/`list_owned` 传入
//!   来自认证上下文的 [`OwnerScope`]，归属值不得取自请求参数；
//! - **列白名单**：SELECT 仅返回声明暴露的列；过滤/排序列同样必须
//!   在白名单内，违规返回 `DbError::Permission`（可映射 HTTP 403/400）；
//! - **标识符校验**：表名/列名一律 `is_safe_identifier` 白名单校验；
//! - **值传递**：过滤值经 `sql_literal` 标准转义（复用仓储实现），
//!   并沿用 `query_rows`/`execute_raw` 的整条注入防御链；
//! - **分页上限**：`max_page_size` 硬上限，超出自动钳制。
//!
//! # 示例
//!
//! ```ignore
//! let gateway = DataApiGateway::new(pool.clone())
//!     .with_role("reader")
//!     .register("users", TableEndpoint::new("t_users", &["id", "name"])?, )?;
//! let resp = gateway.list("users", &ListRequest {
//!     page: 1, page_size: 20,
//!     filters: vec![Filter::eq("name", "Alice")],
//!     order: Some(("id", OrderDirection::Desc)),
//! }).await?;
//! // resp.items: Vec<serde_json::Value>；resp.total
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

use crate::database::DbPool;
use crate::database::identifier::is_safe_identifier;
use crate::database::repository::sql_literal;
use crate::foundation::{DbError, DbResult};

/// 单表查询端点声明
#[derive(Debug, Clone)]
pub struct TableEndpoint {
    /// 物理表名
    pub table: String,
    /// 暴露列白名单（SELECT 投影与过滤/排序的准入清单）
    pub columns: Vec<String>,
    /// 可排序列白名单（默认与 columns 相同）
    pub orderable: Option<Vec<String>>,
    /// 单页行数硬上限
    pub max_page_size: u64,
    /// 默认页大小
    pub default_page_size: u64,
}

impl TableEndpoint {
    /// 声明端点
    ///
    /// # Errors
    ///
    /// 表名或任一列名不是安全标识符时返回 `DbError::Config`
    pub fn new(table: &str, columns: &[&str]) -> DbResult<Self> {
        if !is_safe_identifier(table) {
            return Err(DbError::Config(format!(
                "data-api table name must be a safe identifier: '{table}'"
            )));
        }
        let columns: Vec<String> = columns.iter().map(|c| c.to_string()).collect();
        if columns.is_empty() || columns.iter().any(|c| !is_safe_identifier(c)) {
            return Err(DbError::Config(
                "data-api columns must be non-empty safe identifiers".to_string(),
            ));
        }
        Ok(Self {
            table: table.to_string(),
            columns,
            orderable: None,
            max_page_size: 100,
            default_page_size: 20,
        })
    }

    /// 收窄可排序列白名单
    ///
    /// # Errors
    ///
    /// 排序列不是安全标识符或不在列白名单内时返回 `DbError::Config`
    pub fn with_orderable(mut self, orderable: &[&str]) -> DbResult<Self> {
        let orderable: Vec<String> = orderable.iter().map(|c| c.to_string()).collect();
        if orderable.is_empty()
            || orderable
                .iter()
                .any(|c| !is_safe_identifier(c) || !self.columns.contains(c))
        {
            return Err(DbError::Config(
                "data-api orderable columns must be safe identifiers within the column whitelist"
                    .to_string(),
            ));
        }
        self.orderable = Some(orderable);
        Ok(self)
    }

    /// 调整分页上限与默认页大小
    pub fn with_page_limits(mut self, max_page_size: u64, default_page_size: u64) -> Self {
        self.max_page_size = max_page_size.max(1);
        self.default_page_size = default_page_size.max(1).min(self.max_page_size);
        self
    }

    fn orderable_columns(&self) -> &[String] {
        self.orderable.as_deref().unwrap_or(&self.columns)
    }
}

/// 过滤比较算子
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOp {
    /// 相等
    Eq,
    /// 不等
    Ne,
    /// 小于
    Lt,
    /// 小于等于
    Le,
    /// 大于
    Gt,
    /// 大于等于
    Ge,
    /// 子串包含（SQL LIKE %value%，值内 %/_ 不做通配转义，MVP 口径）
    Contains,
}

impl FilterOp {
    fn as_sql(self) -> &'static str {
        match self {
            FilterOp::Eq => "=",
            FilterOp::Ne => "!=",
            FilterOp::Lt => "<",
            FilterOp::Le => "<=",
            FilterOp::Gt => ">",
            FilterOp::Ge => ">=",
            // Contains 不经 as_sql（以 instr() 形态表达，见 list() 注释）
            FilterOp::Contains => "LIKE",
        }
    }
}

/// 过滤条件
#[derive(Debug, Clone)]
pub struct Filter {
    /// 过滤列（必须在端点列白名单内）
    pub column: String,
    /// 比较算子
    pub op: FilterOp,
    /// 比较值
    pub value: Value,
}

impl Filter {
    /// 等值过滤
    pub fn eq(column: &str, value: impl Into<Value>) -> Self {
        Self {
            column: column.to_string(),
            op: FilterOp::Eq,
            value: value.into(),
        }
    }

    /// 子串包含过滤
    pub fn contains(column: &str, value: impl Into<Value>) -> Self {
        Self {
            column: column.to_string(),
            op: FilterOp::Contains,
            value: value.into(),
        }
    }
}

/// 排序方向
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderDirection {
    /// 升序
    Asc,
    /// 降序
    Desc,
}

/// 列表查询请求
#[derive(Debug, Clone, Default)]
pub struct ListRequest {
    /// 页码（1 起）
    pub page: u64,
    /// 页大小（0 = 使用端点默认页大小）
    pub page_size: u64,
    /// 过滤条件（全部以 AND 连接）
    pub filters: Vec<Filter>,
    /// 排序（单列 MVP）
    pub order: Option<(String, OrderDirection)>,
}

/// 列表查询响应（可直接序列化为 JSON 响应体）
#[derive(Debug, Clone, Serialize)]
pub struct ListResponse {
    /// 当页数据行（仅白名单列）
    pub items: Vec<Value>,
    /// 页码
    pub page: u64,
    /// 实际页大小
    pub page_size: u64,
    /// 过滤后总行数
    pub total: u64,
}

/// 归属作用域：端点的归属列与归属值
///
/// 归属值来自认证上下文（如租户 ID / 用户 ID），**不得**取自请求参数，
/// 否则归属判定形同虚设。
#[derive(Debug, Clone)]
pub struct OwnerScope {
    /// 归属列（如 `tenant_id`）
    column: String,
    /// 归属值（如租户 ID）
    value: String,
}

impl OwnerScope {
    /// 构造归属作用域
    ///
    /// # Errors
    ///
    /// 归属列名不是安全标识符时返回 `DbError::Config`
    pub fn new(column: &str, value: &str) -> DbResult<Self> {
        if !is_safe_identifier(column) {
            return Err(DbError::Config(format!(
                "data-api owner column must be a safe identifier: '{column}'"
            )));
        }
        Ok(Self {
            column: column.to_string(),
            value: value.to_string(),
        })
    }

    /// 归属谓词（列名经白名单校验，值经 `sql_literal` 转义）
    fn clause(&self, backend: crate::database::repository::SqlBackend) -> DbResult<String> {
        let literal = sql_literal(&Value::String(self.value.clone()), backend)?;
        Ok(format!("{} = {}", self.column, literal))
    }
}

/// 数据 API 网关
///
/// 持有池与端点注册表；全部查询以显式声明的角色执行（角色未声明时
/// 查询 fail-closed 报错，不以 admin 兜底）。
pub struct DataApiGateway {
    pool: Arc<DbPool>,
    role: Option<String>,
    endpoints: HashMap<String, TableEndpoint>,
}

impl DataApiGateway {
    /// 创建网关（执行角色未声明：查询前须经 `with_role` 指定角色）
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self {
            pool,
            role: None,
            endpoints: HashMap::new(),
        }
    }

    /// 指定查询执行角色
    pub fn with_role(mut self, role: &str) -> Self {
        self.role = Some(role.to_string());
        self
    }

    /// 查询执行角色（未声明 → fail-closed）
    fn query_role(&self) -> DbResult<&str> {
        self.role.as_deref().ok_or_else(|| {
            DbError::Config(
                "data-api query role is not configured; call with_role() before querying"
                    .to_string(),
            )
        })
    }

    /// 注册端点
    ///
    /// # Errors
    ///
    /// 端点名不是安全标识符时返回 `DbError::Config`
    pub fn register(mut self, name: &str, endpoint: TableEndpoint) -> DbResult<Self> {
        if !is_safe_identifier(name) {
            return Err(DbError::Config(format!(
                "data-api endpoint name must be a safe identifier: '{name}'"
            )));
        }
        self.endpoints.insert(name.to_string(), endpoint);
        Ok(self)
    }

    /// 已注册端点名（无序）
    pub fn endpoint_names(&self) -> Vec<&str> {
        self.endpoints.keys().map(|s| s.as_str()).collect()
    }

    /// 端点清单（供 sdforge 生成 OpenAPI/路由表）
    ///
    /// 输出含**内部实现面**：物理表名、列白名单、执行角色名。它是构建期
    /// 喂给代码生成器的契约，不是给客户端的响应体——把返回值直接挂到
    /// 对外路由等于公开数据库 schema 与权限模型。
    pub fn manifest(&self) -> Value {
        let endpoints: Vec<Value> = self
            .endpoints
            .iter()
            .map(|(name, ep)| {
                serde_json::json!({
                    "name": name,
                    "table": ep.table,
                    "columns": ep.columns,
                    "orderable": ep.orderable_columns(),
                    "max_page_size": ep.max_page_size,
                    "default_page_size": ep.default_page_size,
                })
            })
            .collect();
        serde_json::json!({ "role": self.role, "endpoints": endpoints })
    }

    fn endpoint(&self, name: &str) -> DbResult<&TableEndpoint> {
        self.endpoints
            .get(name)
            .ok_or_else(|| DbError::Config(format!("unknown data-api endpoint: '{name}'")))
    }

    /// 行投影后过滤：sqlite 行内省方言会把全部表列补回（未投影列为 Null），
    /// 网关在出口裁剪至白名单列，保证未暴露列（名与值）不出现
    fn project(&self, ep: &TableEndpoint, rows: Vec<Value>) -> Vec<Value> {
        rows.into_iter()
            .map(|row| match row {
                Value::Object(map) => Value::Object(
                    map.into_iter()
                        .filter(|(key, _)| ep.columns.contains(key))
                        .collect(),
                ),
                other => other,
            })
            .collect()
    }

    /// 按主键查询单行（仅白名单列）
    ///
    /// 不做归属判定：返回的行可能属于其他租户/用户。多租户场景须改用
    /// [`Self::get_owned`]（归属值来自认证上下文）。
    pub async fn get(&self, name: &str, id: i64) -> DbResult<Option<Value>> {
        self.get_with_scope(name, id, None).await
    }

    /// 按主键查询单行并强制归属谓词
    ///
    /// 归属列与归属值来自 [`OwnerScope`]（认证上下文），拼入 WHERE 后
    /// 与主键条件同时成立才返回行。
    pub async fn get_owned(
        &self,
        name: &str,
        id: i64,
        scope: &OwnerScope,
    ) -> DbResult<Option<Value>> {
        self.get_with_scope(name, id, Some(scope)).await
    }

    async fn get_with_scope(
        &self,
        name: &str,
        id: i64,
        scope: Option<&OwnerScope>,
    ) -> DbResult<Option<Value>> {
        let ep = self.endpoint(name)?;
        let role = self.query_role()?;
        let mut sql = format!(
            "SELECT {} FROM {} WHERE id = {}",
            ep.columns.join(", "),
            ep.table,
            id
        );
        if let Some(scope) = scope {
            let backend = crate::database::repository::resolve_sql_backend(&self.pool);
            sql.push_str(" AND ");
            sql.push_str(&scope.clause(backend)?);
        }
        let rows = self.pool.query_rows(&sql, role).await?;
        Ok(self.project(ep, rows).into_iter().next())
    }

    /// 列表查询：白名单投影 + 过滤 + 排序 + 分页（含总数）
    ///
    /// 不做归属判定：过滤条件完全来自调用方（`req.filters`）。多租户场景
    /// 须改用 [`Self::list_owned`]。
    pub async fn list(&self, name: &str, req: &ListRequest) -> DbResult<ListResponse> {
        self.list_with_scope(name, req, None).await
    }

    /// 列表查询并强制归属谓词
    ///
    /// 归属列与归属值来自 [`OwnerScope`]（认证上下文），与调用方过滤条件
    /// 以 AND 组合。
    pub async fn list_owned(
        &self,
        name: &str,
        req: &ListRequest,
        scope: &OwnerScope,
    ) -> DbResult<ListResponse> {
        self.list_with_scope(name, req, Some(scope)).await
    }

    async fn list_with_scope(
        &self,
        name: &str,
        req: &ListRequest,
        scope: Option<&OwnerScope>,
    ) -> DbResult<ListResponse> {
        let ep = self.endpoint(name)?;
        let role = self.query_role()?;

        // 分页参数校验与钳制
        if req.page == 0 {
            return Err(DbError::Config("data-api page starts at 1".to_string()));
        }
        let page_size = if req.page_size == 0 {
            ep.default_page_size
        } else {
            req.page_size.min(ep.max_page_size)
        };

        // 过滤条件：列必须白名单内（违规 → Permission，可映射 403）
        let mut where_clauses: Vec<String> = Vec::new();
        for filter in &req.filters {
            if !ep.columns.contains(&filter.column) {
                return Err(DbError::Permission(format!(
                    "data-api column '{}' is not exposed for endpoint '{}'",
                    filter.column, name
                )));
            }
            if filter.op == FilterOp::Contains {
                // 注：LIKE '%word%' 形态会被注入防护链的 %..% 变量检测保守拦截
                // （parse_single 直接失败），故用 instr() 表达子串包含
                // （sqlite/mysql；pg 可经 strpos 扩展，MVP 以 sqlite 为准）
                let text = match &filter.value {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                where_clauses.push(format!(
                    "instr({}, {}) > 0",
                    filter.column,
                    sql_literal(
                        &Value::String(text),
                        crate::database::repository::resolve_sql_backend(&self.pool)
                    )?
                ));
                continue;
            }
            let literal = sql_literal(
                &filter.value,
                crate::database::repository::resolve_sql_backend(&self.pool),
            )?;
            where_clauses.push(format!(
                "{} {} {}",
                filter.column,
                filter.op.as_sql(),
                literal
            ));
        }
        let where_sql = if where_clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", where_clauses.join(" AND "))
        };
        // 归属谓词与调用方过滤条件以 AND 组合（归属值来自认证上下文）
        let owner_sql = match scope {
            Some(scope) => {
                let backend = crate::database::repository::resolve_sql_backend(&self.pool);
                Some(scope.clause(backend)?)
            }
            None => None,
        };
        let where_sql = match owner_sql {
            Some(owner) if where_sql.is_empty() => format!(" WHERE {owner}"),
            Some(owner) => format!("{where_sql} AND {owner}"),
            None => where_sql,
        };

        // 排序：列必须在可排序列白名单内
        let order_sql = match &req.order {
            Some((column, direction)) => {
                let allowed = ep.orderable_columns().contains(column);
                if !allowed {
                    return Err(DbError::Permission(format!(
                        "data-api column '{}' is not orderable for endpoint '{}'",
                        column, name
                    )));
                }
                let dir = match direction {
                    OrderDirection::Asc => "ASC",
                    OrderDirection::Desc => "DESC",
                };
                format!(" ORDER BY {column} {dir}")
            }
            None => String::new(),
        };

        // 总数（MVP：主列全量取回后计行数——sqlite 行内省方言不保留聚合列）
        let count_sql = format!("SELECT {} FROM {}{}", ep.columns[0], ep.table, where_sql);
        let total = self.pool.query_rows(&count_sql, role).await?.len() as u64;

        let offset = (req.page - 1) * page_size;
        let sql = format!(
            "SELECT {} FROM {}{}{} LIMIT {} OFFSET {}",
            ep.columns.join(", "),
            ep.table,
            where_sql,
            order_sql,
            page_size,
            offset
        );
        let items = self.project(ep, self.pool.query_rows(&sql, role).await?);

        Ok(ListResponse {
            items,
            page: req.page,
            page_size,
            total,
        })
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_table_endpoint_constructor_validations() {
        // 表名非法
        assert!(TableEndpoint::new("9bad", &["a"]).is_err());
        // 列集为空 / 含非法列
        assert!(TableEndpoint::new("t9", &[]).is_err());
        assert!(TableEndpoint::new("t9", &["a", "bad col"]).is_err());
        // 合法构造
        assert!(TableEndpoint::new("t9", &["a", "b"]).is_ok());
    }

    #[test]
    fn test_table_endpoint_orderable_whitelist() {
        let endpoint = TableEndpoint::new("t9", &["a", "b"]).unwrap();
        // 可排序列必须在列白名单内
        assert!(endpoint.clone().with_orderable(&["c"]).is_err());
        assert!(endpoint.clone().with_orderable(&[]).is_err());
        assert!(endpoint.clone().with_orderable(&["a"]).is_ok());
        // 非法标识符拒绝
        assert!(endpoint.with_orderable(&["a;"]).is_err());
    }

    #[test]
    fn test_filter_op_as_sql_matrix() {
        // Contains 走 instr() 不经 as_sql；其余操作符逐一断言
        assert_eq!(FilterOp::Eq.as_sql(), "=");
        assert_eq!(FilterOp::Ne.as_sql(), "!=");
        assert_eq!(FilterOp::Lt.as_sql(), "<");
        assert_eq!(FilterOp::Le.as_sql(), "<=");
        assert_eq!(FilterOp::Gt.as_sql(), ">");
        assert_eq!(FilterOp::Ge.as_sql(), ">=");
    }

    #[test]
    fn test_order_direction_and_request_defaults() {
        // ListRequest 默认值契约：page 0/page_size 0 表示用端点默认
        let req = ListRequest::default();
        assert_eq!(req.page, 0);
        assert_eq!(req.page_size, 0);
        assert!(req.filters.is_empty());
        assert!(req.order.is_none());

        let f = Filter::eq("a", 1);
        assert_eq!(f.column, "a");
        let f = Filter::contains("name", "x");
        assert_eq!(f.op, FilterOp::Contains);
    }

    /// list 全流程：分页钳制 / 过滤白名单 / 排序白名单 / Contains 走 instr
    // sqlite 临时文件库依赖嵌入式驱动：无 sqlite 组合不可运行
    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_list_flow_filters_order_pagination() {
        // 内存库每连接独立：用临时文件库保证建表与查询同库
        // 文件名带进程号：失败残留的库文件不得被下一次运行复用
        let db_path = std::env::temp_dir().join(format!(
            "dbnexus_data_api_{}_{}.db",
            std::process::id(),
            std::time::Instant::now().elapsed().as_nanos()
        ));
        let _ = std::fs::remove_file(&db_path);
        let pool = std::sync::Arc::new(
            DbPool::new(&format!("sqlite:{}?mode=rwc", db_path.display()))
                .await
                .expect("pool"),
        );
        let admin = pool.get_session("admin").await.expect("admin");
        admin
            .execute_raw_ddl("CREATE TABLE t9 (id INTEGER PRIMARY KEY, name TEXT)")
            .await
            .expect("create");
        for i in 1..=5 {
            admin
                .execute_with_params(
                    "INSERT INTO t9 (id, name) VALUES (?, ?)",
                    &[serde_json::json!(i), serde_json::json!(format!("user{i}"))],
                )
                .await
                .expect("insert");
        }

        let endpoint = TableEndpoint::new("t9", &["id", "name"])
            .unwrap()
            .with_orderable(&["id"])
            .unwrap();
        let gateway = DataApiGateway::new(pool)
            .with_role("admin")
            .register("t9_list", endpoint)
            .expect("register");

        // page=0 拒绝
        let req = ListRequest {
            page: 0,
            ..Default::default()
        };
        assert!(gateway.list("t9_list", &req).await.is_err());

        // 过滤列不在白名单 → Permission
        let req = ListRequest {
            page: 1,
            filters: vec![Filter::eq("secret", 1)],
            ..Default::default()
        };
        let err = gateway.list("t9_list", &req).await.unwrap_err();
        assert!(matches!(err, DbError::Permission(_)), "got {err:?}");

        // 排序列不在可排序白名单 → Config
        let req = ListRequest {
            page: 1,
            order: Some(("name".to_string(), OrderDirection::Asc)),
            ..Default::default()
        };
        assert!(gateway.list("t9_list", &req).await.is_err());

        // 正常查询：等值过滤 + 升序 + 分页
        let req = ListRequest {
            page: 2,
            page_size: 2,
            filters: vec![Filter::contains("name", "user")],
            order: Some(("id".to_string(), OrderDirection::Asc)),
        };
        let resp = gateway.list("t9_list", &req).await.expect("list");
        assert_eq!(resp.total, 5);
        assert_eq!(resp.page, 2);
        assert_eq!(resp.page_size, 2);
        assert_eq!(resp.items.len(), 2, "page 2 should have 2 remaining rows");
        let _ = std::fs::remove_file(&db_path);
    }

    /// 查询执行角色：未声明时 manifest 不报告角色，`with_role` 后报告新角色
    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_with_role_updates_manifest_role() {
        let pool = std::sync::Arc::new(DbPool::new("sqlite::memory:").await.expect("pool"));
        let endpoint = TableEndpoint::new("t9", &["id"]).unwrap();
        let gateway = DataApiGateway::new(pool)
            .register("t9_list", endpoint)
            .expect("register");

        assert!(
            gateway.manifest()["role"].is_null(),
            "the query role must not be declared by default"
        );

        let gateway = gateway.with_role("analyst");
        assert_eq!(
            gateway.manifest()["role"],
            "analyst",
            "with_role must be reflected in the endpoint manifest"
        );
    }

    /// 未声明执行角色时查询 fail-closed（不以 admin 兜底）
    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_query_without_role_is_rejected() {
        let pool = std::sync::Arc::new(DbPool::new("sqlite::memory:").await.expect("pool"));
        let endpoint = TableEndpoint::new("t9", &["id"]).unwrap();
        let gateway = DataApiGateway::new(pool)
            .register("t9_list", endpoint)
            .expect("register");

        let err = gateway.get("t9_list", 1).await.unwrap_err();
        assert!(
            matches!(err, DbError::Config(_)),
            "querying without a declared role must be rejected, got {err:?}"
        );
    }
}
