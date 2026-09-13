// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 数据 API 网关雏形
//!
//! 实体 → JSON 查询端点生成器：为声明的表端点（表/列白名单 + 分页 + 过滤 +
//! 排序）提供可被上层 Web 框架（如 sdforge）直接包装为 HTTP 端点的
//! 异步查询处理器。
//!
//! # 安全口径
//!
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
use crate::database::repository::{is_safe_identifier, sql_literal};
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

/// 数据 API 网关
///
/// 持有池与端点注册表；全部查询以声明的角色执行（默认 admin，
/// 生产部署建议换成最小权限角色）。
pub struct DataApiGateway {
    pool: Arc<DbPool>,
    role: String,
    endpoints: HashMap<String, TableEndpoint>,
}

impl DataApiGateway {
    /// 创建网关
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self {
            pool,
            role: "admin".to_string(),
            endpoints: HashMap::new(),
        }
    }

    /// 指定查询执行角色
    pub fn with_role(mut self, role: &str) -> Self {
        self.role = role.to_string();
        self
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
    pub async fn get(&self, name: &str, id: i64) -> DbResult<Option<Value>> {
        let ep = self.endpoint(name)?;
        let sql = format!(
            "SELECT {} FROM {} WHERE id = {}",
            ep.columns.join(", "),
            ep.table,
            id
        );
        let rows = self.pool.query_rows(&sql, &self.role).await?;
        Ok(self.project(ep, rows).into_iter().next())
    }

    /// 列表查询：白名单投影 + 过滤 + 排序 + 分页（含总数）
    pub async fn list(&self, name: &str, req: &ListRequest) -> DbResult<ListResponse> {
        let ep = self.endpoint(name)?;

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
                    sql_literal(&Value::String(text))?
                ));
                continue;
            }
            let literal = sql_literal(&filter.value)?;
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
        let total = self.pool.query_rows(&count_sql, &self.role).await?.len() as u64;

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
        let items = self.project(ep, self.pool.query_rows(&sql, &self.role).await?);

        Ok(ListResponse {
            items,
            page: req.page,
            page_size,
            total,
        })
    }
}
