// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! MCP Server 适配器（rmcp stdio）
//!
//! 三工具全部经 `DataApiGateway` / `Session` 既有防御链，零旁路：
//! 执行角色显式声明、列白名单、分页钳制、逐表 RBAC/RLS/脱敏均生效。

use std::collections::HashMap;
use std::sync::Arc;

use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, ContentBlock, ListToolsResult, PaginatedRequestParams,
    ServerCapabilities, ServerConfig, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer};

use crate::database::data_api::{DataApiGateway, Filter, ListRequest, OwnerScope, TableEndpoint};
use crate::database::identifier::is_safe_identifier;
use crate::foundation::{DbError, DbResult};
use crate::{DbPool, Session};

/// MCP Server 装配配置
#[derive(Debug, Clone)]
pub struct McpServerConfig {
    /// 执行角色（必填；空串构造失败 = fail-closed）
    pub role: String,
    /// 注册端点清单（name → TableEndpoint）
    pub endpoints: Vec<(String, TableEndpoint)>,
    /// 归属范围（多租户场景，可空）
    pub owner_scope: Option<OwnerScope>,
}

/// 受权限约束的 MCP Server
pub struct DbnexusMcpServer {
    gateway: DataApiGateway,
    pool: Arc<DbPool>,
    role: String,
    endpoints: HashMap<String, TableEndpoint>,
    owner_scope: Option<OwnerScope>,
}

impl DbnexusMcpServer {
    /// 构造：角色为空 / 端点名或列名非法 → Err
    pub fn new(pool: Arc<DbPool>, config: McpServerConfig) -> DbResult<Self> {
        if config.role.trim().is_empty() {
            return Err(DbError::Config("mcp execution role is required".into()));
        }
        for (name, endpoint) in &config.endpoints {
            if !is_safe_identifier(name) || !is_safe_identifier(&endpoint.table) {
                return Err(DbError::Config(format!(
                    "mcp endpoint name/table must be a safe identifier: '{name}'"
                )));
            }
        }
        let mut gateway = DataApiGateway::new(Arc::clone(&pool)).with_role(&config.role);
        for (name, endpoint) in &config.endpoints {
            gateway = gateway.register(name, endpoint.clone())?;
        }
        let endpoints = config.endpoints.into_iter().collect();
        Ok(Self {
            gateway,
            pool,
            role: config.role,
            endpoints,
            owner_scope: config.owner_scope,
        })
    }

    /// 配置只读引用（测试/观测用）
    pub fn role(&self) -> &str {
        &self.role
    }

    /// 会话（explain 判定用）
    async fn session(&self) -> DbResult<Session> {
        Ok(self.pool.get_session(&self.role).await?)
    }

    /// dbnexus_query：filters 三元组 (column, op, value) → ListResponse JSON
    async fn tool_query(&self, args: &serde_json::Value) -> Result<String, String> {
        let table = args
            .get("table")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "table is required".to_string())?;
        let endpoint = self
            .endpoints
            .get(table)
            .ok_or_else(|| "endpoint not registered".to_string())?;

        let mut filters = Vec::new();
        if let Some(list) = args.get("filters").and_then(|v| v.as_array()) {
            for item in list {
                let triple = item
                    .as_array()
                    .ok_or_else(|| "invalid filter tuple".to_string())?;
                if triple.len() != 3 {
                    return Err("invalid filter tuple".into());
                }
                let column = triple[0]
                    .as_str()
                    .ok_or_else(|| "invalid filter column".to_string())?;
                let op = triple[1]
                    .as_str()
                    .ok_or_else(|| "invalid filter op".to_string())?;
                let value = triple[2].clone();
                if !endpoint.columns.iter().any(|c| c == column) {
                    return Err(format!("column not in endpoint whitelist: {column}"));
                }
                let filter = match op {
                    "eq" => Filter::eq(column, value),
                    "contains" => Filter::contains(column, value.as_str().unwrap_or_default()),
                    other => return Err(format!("unsupported filter op: {other}")),
                };
                filters.push(filter);
            }
        }

        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(endpoint.default_page_size)
            .min(endpoint.max_page_size);
        let req = ListRequest {
            page: 1,
            page_size: limit,
            filters,
            order: None,
        };
        let response = match &self.owner_scope {
            Some(scope) => self.gateway.list_owned(table, &req, scope).await,
            None => self.gateway.list(table, &req).await,
        }
        .map_err(|e| format!("query denied: {}", e.message()))?;
        serde_json::to_string(&response).map_err(|e| e.to_string())
    }

    /// dbnexus_schema：返回端点声明元数据（未注册表统一错误）
    fn tool_schema(&self, args: &serde_json::Value) -> Result<String, String> {
        let table = args
            .get("table")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "table is required".to_string())?;
        let endpoint = self
            .endpoints
            .get(table)
            .ok_or_else(|| "endpoint not registered".to_string())?;
        serde_json::to_string(&serde_json::json!({
            "table": endpoint.table,
            "columns": endpoint.columns,
            "orderable": endpoint.orderable.as_deref().unwrap_or(&endpoint.columns),
            "max_page_size": endpoint.max_page_size,
            "default_page_size": endpoint.default_page_size,
        }))
        .map_err(|e| e.to_string())
    }

    /// dbnexus_explain：权限检查路径预演（不执行 SQL）
    async fn tool_explain(&self, args: &serde_json::Value) -> Result<String, String> {
        let sql = args
            .get("sql")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "sql is required".to_string())?;
        let session = self.session().await.map_err(|e| e.message())?;
        let explanation = session.explain_access(sql).await.map_err(|e| e.message())?;
        serde_json::to_string(&explanation).map_err(|e| e.to_string())
    }

    /// 同步执行工具（ServerHandler::call_tool 的桥接）
    async fn dispatch_tool(&self, name: &str, args: &serde_json::Value) -> Result<String, String> {
        match name {
            "dbnexus_query" => self.tool_query(args).await,
            "dbnexus_schema" => self.tool_schema(args),
            "dbnexus_explain" => self.tool_explain(args).await,
            other => Err(format!("unknown tool: {other}")),
        }
    }
}

fn tool_schema_object(
    properties: serde_json::Value,
    required: &[&str],
) -> Arc<serde_json::Map<String, serde_json::Value>> {
    let mut schema = serde_json::Map::new();
    schema.insert("type".into(), serde_json::json!("object"));
    schema.insert("properties".into(), properties);
    schema.insert("required".into(), serde_json::json!(required));
    Arc::new(schema)
}

fn declare_tools() -> Vec<Tool> {
    vec![
        Tool::new(
            "dbnexus_query",
            "Query a registered table with whitelisted filters and bounded pagination",
            tool_schema_object(
                serde_json::json!({
                    "table": { "type": "string", "description": "registered endpoint name" },
                    "filters": { "type": "array", "items": { "type": "array" }, "description": "[[column, eq|contains, value], ...]" },
                    "limit": { "type": "integer", "minimum": 1 }
                }),
                &["table"],
            ),
        ),
        Tool::new(
            "dbnexus_schema",
            "Return declared metadata of a registered endpoint",
            tool_schema_object(
                serde_json::json!({ "table": { "type": "string" } }),
                &["table"],
            ),
        ),
        Tool::new(
            "dbnexus_explain",
            "Preview the permission-check path of a SQL statement without executing it",
            tool_schema_object(serde_json::json!({ "sql": { "type": "string" } }), &["sql"]),
        ),
    ]
}

impl ServerHandler for DbnexusMcpServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: declare_tools(),
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let args = request.arguments.unwrap_or_default();
        let value = serde_json::Value::Object(args);
        match self.dispatch_tool(&request.name, &value).await {
            Ok(text) => Ok(CallToolResponse::Complete(
                rmcp::model::CallToolResult::success(vec![ContentBlock::text(text)]),
            )),
            Err(message) => Ok(CallToolResponse::Complete(
                rmcp::model::CallToolResult::error(vec![ContentBlock::text(message)]),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(role: &str) -> McpServerConfig {
        McpServerConfig {
            role: role.into(),
            endpoints: vec![(
                "orders".into(),
                TableEndpoint::new("orders", &["id", "tenant_id"]).unwrap(),
            )],
            owner_scope: None,
        }
    }

    #[tokio::test]
    async fn test_empty_role_fails_closed() {
        let pool = Arc::new(DbPool::new("sqlite::memory:").await.unwrap());
        match DbnexusMcpServer::new(pool, config("")) {
            Err(DbError::Config(_)) => {}
            _ => panic!("empty role must fail closed"),
        }
    }

    #[tokio::test]
    async fn test_illegal_endpoint_name_rejected() {
        let pool = Arc::new(DbPool::new("sqlite::memory:").await.unwrap());
        let mut cfg = config("reader");
        cfg.endpoints[0].0 = "bad;name".into();
        match DbnexusMcpServer::new(pool, cfg) {
            Err(DbError::Config(_)) => {}
            _ => panic!("illegal endpoint name must be rejected"),
        }
    }

    #[tokio::test]
    async fn test_construction_and_schema() {
        let pool = Arc::new(DbPool::new("sqlite::memory:").await.unwrap());
        let server = DbnexusMcpServer::new(pool, config("reader")).unwrap();
        assert_eq!(server.role(), "reader");
        let schema = server
            .tool_schema(&serde_json::json!({ "table": "orders" }))
            .unwrap();
        assert!(schema.contains("tenant_id"));
        let missing = server
            .tool_schema(&serde_json::json!({ "table": "secrets" }))
            .unwrap_err();
        assert_eq!(missing, "endpoint not registered");
    }

    #[tokio::test]
    async fn test_query_column_whitelist_rejection() {
        let pool = Arc::new(DbPool::new("sqlite::memory:").await.unwrap());
        let server = DbnexusMcpServer::new(pool, config("reader")).unwrap();
        let err = server
            .tool_query(&serde_json::json!({
                "table": "orders",
                "filters": [["secret", "eq", "x"]]
            }))
            .await
            .unwrap_err();
        assert!(err.contains("not in endpoint whitelist"), "{err}");
    }

    #[test]
    fn test_three_tools_declared() {
        let tools = declare_tools();
        assert_eq!(tools.len(), 3);
        assert_eq!(tools[0].name, "dbnexus_query");
        assert_eq!(tools[1].name, "dbnexus_schema");
        assert_eq!(tools[2].name, "dbnexus_explain");
    }
}
