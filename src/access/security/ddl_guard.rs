// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! DDL 安全守卫模块
//!
//! 提供基于 AST（抽象语法树）的 DDL 语句验证，相比字符串前缀匹配更安全可靠。

use sqlparser::ast::Statement;
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;

/// 允许的 DDL 语句类型白名单
const ALLOWED_DDL_STATEMENTS: &[&str] = &[
    "CreateTable",
    "AlterTable",
    "DropTable",
    "CreateIndex",
    "DropIndex",
    "CreateView",
    // DropView：与 CreateView 成对——能建视图即应能删（test_valid_drop_view 契约）；
    // 6487866 白名单扩展时遗漏，导致 DROP VIEW 被拒
    "DropView",
    "Truncate",
    "Query", // SELECT 查询（用于验证）
    // DML 语句：迁移事务可能混合 DDL+DML（如 ALTER TABLE + UPDATE），
    // admin 路径的 DdlGuard 应放行——表级权限已由 security_gate 前置检查。
    "Insert",
    "Update",
    "Delete",
];

/// DDL 验证结果
#[derive(Debug, Clone)]
pub enum DdlValidationResult {
    /// 验证通过
    Allowed,
    /// 验证失败，包含原因
    Forbidden(String),
    /// SQL 解析失败
    ParseError(String),
}

/// DDL 守卫决策审计记录
#[derive(Debug, Clone)]
pub struct DdlAuditRecord {
    /// 被校验的 SQL
    pub sql: String,
    /// 是否放行
    pub allowed: bool,
    /// 拦截/解析失败原因（放行时为 None）
    pub reason: Option<String>,
}

impl DdlAuditRecord {
    /// 从校验结果构造记录
    fn from_result(sql: &str, result: &DdlValidationResult) -> Self {
        match result {
            DdlValidationResult::Allowed => Self {
                sql: sql.to_string(),
                allowed: true,
                reason: None,
            },
            DdlValidationResult::Forbidden(reason) => Self {
                sql: sql.to_string(),
                allowed: false,
                reason: Some(reason.clone()),
            },
            DdlValidationResult::ParseError(error) => Self {
                sql: sql.to_string(),
                allowed: false,
                reason: Some(error.clone()),
            },
        }
    }
}

/// 统一 DDL 守卫端口
///
/// 白名单（内置 [`DdlGuard`]）、干跑（[`DryRunDdlGuard`]）、审计
/// （[`AuditingDdlGuard`]）经同一入口接入；Session 的全部 DDL 路径
/// （`execute_raw_ddl` / DuckDB 安全门）收敛到单一漏斗消费本端口，
/// 替代此前各执行路径内分散的 `DdlGuard::new()` + 决策映射。
///
/// 支持经 `DbPoolBuilder::ddl_guard` / `DbPool::set_ddl_guard` 注入自定义策略。
pub trait DdlGuardPolicy: Send + Sync {
    /// 验证 SQL 是否可执行（语义与内置白名单守卫一致）
    fn validate(&self, sql: &str) -> Result<DdlValidationResult, String>;

    /// 决策审计钩子：每次校验给出决策后调用（默认 NoOp）
    fn audit(&self, _sql: &str, _result: &DdlValidationResult) {}
}

/// 审计装饰器：把守卫决策流式转发到外部 sink
///
/// 组合任意 [`DdlGuardPolicy`]，决策（放行/拦截/解析失败）实时回调，
/// 供上层接入结构化审计日志/DB 审计存储。
pub struct AuditingDdlGuard {
    inner: std::sync::Arc<dyn DdlGuardPolicy>,
    sink: std::sync::Arc<dyn Fn(&DdlAuditRecord) + Send + Sync>,
}

impl AuditingDdlGuard {
    /// 包装内部策略并指定审计 sink
    pub fn new(
        inner: std::sync::Arc<dyn DdlGuardPolicy>,
        sink: std::sync::Arc<dyn Fn(&DdlAuditRecord) + Send + Sync>,
    ) -> Self {
        Self { inner, sink }
    }
}

impl DdlGuardPolicy for AuditingDdlGuard {
    fn validate(&self, sql: &str) -> Result<DdlValidationResult, String> {
        self.inner.validate(sql)
    }

    fn audit(&self, sql: &str, result: &DdlValidationResult) {
        (self.sink)(&DdlAuditRecord::from_result(sql, result));
    }
}

/// 干跑装饰器：记录全部决策、不改变放行语义
///
/// - 作为守卫注入时行为与内部策略一致，同时留存决策记录（`records()`）；
/// - 独立用于预检：`plan()` 对一组语句给出逐条决策，不执行任何语句。
pub struct DryRunDdlGuard {
    inner: std::sync::Arc<dyn DdlGuardPolicy>,
    records: std::sync::Mutex<Vec<DdlAuditRecord>>,
}

impl DryRunDdlGuard {
    /// 包装内部策略
    pub fn new(inner: std::sync::Arc<dyn DdlGuardPolicy>) -> Self {
        Self {
            inner,
            records: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// 干跑一组语句：逐条决策并记录，不执行
    pub fn plan(&self, statements: &[&str]) -> Vec<DdlAuditRecord> {
        statements
            .iter()
            .map(|sql| {
                let record = match self.inner.validate(sql) {
                    Ok(decision) => DdlAuditRecord::from_result(sql, &decision),
                    Err(error) => DdlAuditRecord {
                        sql: sql.to_string(),
                        allowed: false,
                        reason: Some(error),
                    },
                };
                self.records
                    .lock()
                    .expect("dry-run records")
                    .push(record.clone());
                record
            })
            .collect()
    }

    /// 全部已记录决策（干跑审计视图）
    pub fn records(&self) -> Vec<DdlAuditRecord> {
        self.records.lock().expect("dry-run records").clone()
    }

    /// 语句是否会被放行（干跑单条查询，决策同样记录）
    pub fn would_allow(&self, sql: &str) -> bool {
        matches!(
            DdlGuardPolicy::validate(self, sql),
            Ok(DdlValidationResult::Allowed)
        )
    }
}

impl DdlGuardPolicy for DryRunDdlGuard {
    fn validate(&self, sql: &str) -> Result<DdlValidationResult, String> {
        let result = self.inner.validate(sql);
        let record = match &result {
            Ok(decision) => DdlAuditRecord::from_result(sql, decision),
            Err(error) => DdlAuditRecord {
                sql: sql.to_string(),
                allowed: false,
                reason: Some(error.clone()),
            },
        };
        self.records.lock().expect("dry-run records").push(record);
        result
    }
}

impl DdlGuardPolicy for DdlGuard {
    fn validate(&self, sql: &str) -> Result<DdlValidationResult, String> {
        DdlGuard::validate(self, sql)
    }
}

/// DDL 安全守卫
///
/// 使用 AST 分析验证 DDL SQL 语句的安全性，
/// 替代原有的字符串前缀匹配方案。
pub struct DdlGuard {
    dialect: GenericDialect,
}

impl DdlGuard {
    /// 创建新的 DdlGuard 实例
    pub fn new() -> Self {
        Self {
            dialect: GenericDialect {},
        }
    }

    /// 验证 DDL SQL 是否安全可执行
    ///
    /// 使用 AST 解析进行语义级验证，消除字符串匹配绕过风险。
    ///
    /// # Arguments
    ///
    /// * `sql` - 要验证的 SQL 语句
    ///
    /// # Returns
    ///
    /// * `Ok(DdlValidationResult::Allowed)` - SQL 通过验证
    /// * `Ok(DdlValidationResult::Forbidden(reason))` - SQL 被拦截
    /// * `Err(msg)` - 解析错误
    pub fn validate(&self, sql: &str) -> Result<DdlValidationResult, String> {
        let sql_trimmed = sql.trim();
        if sql_trimmed.is_empty() {
            return Ok(DdlValidationResult::Forbidden(
                "Empty SQL statement".to_string(),
            ));
        }

        // 第一步：检查禁止的字符串模式（捕获 AST 无法检测的注入）
        // 禁用模式表已合并至统一注入引擎（scan_ddl 管线口径不变）
        if let Some(rule) = crate::access::InjectionEngine::global()
            .scan_ddl(sql_trimmed)
            .first()
        {
            return Ok(DdlValidationResult::Forbidden(format!(
                "Contains forbidden pattern: {}",
                rule.pattern
            )));
        }

        // 第二步：AST 解析验证
        let statements = Parser::parse_sql(&self.dialect, sql_trimmed)
            .map_err(|e| format!("Failed to parse SQL: {}", e))?;

        if statements.is_empty() {
            return Ok(DdlValidationResult::Forbidden(
                "Empty SQL statement after parsing".to_string(),
            ));
        }

        // 第三步：检查每条语句是否在白名单中
        for stmt in &statements {
            if !Self::is_allowed_statement(stmt) {
                return Ok(DdlValidationResult::Forbidden(format!(
                    "Statement type '{}' is not in the allowed DDL whitelist",
                    Self::statement_type_name(stmt)
                )));
            }
        }

        Ok(DdlValidationResult::Allowed)
    }

    /// 检查语句类型是否在白名单中
    fn is_allowed_statement(stmt: &Statement) -> bool {
        // 统一走白名单检查（包括 DROP 语句），不再硬编码过滤特定 DROP 类型
        let type_name = Self::statement_type_name(stmt);
        ALLOWED_DDL_STATEMENTS.contains(&type_name.as_str())
    }

    /// 获取语句的类型名称
    fn statement_type_name(stmt: &Statement) -> String {
        match stmt {
            Statement::CreateTable(_) => "CreateTable".to_string(),
            Statement::AlterTable(_) => "AlterTable".to_string(),
            Statement::CreateIndex(_) => "CreateIndex".to_string(),
            Statement::CreateView(_) => "CreateView".to_string(),
            Statement::Drop { object_type, .. } => {
                // DROP 语句需要特殊处理，根据 object_type 返回具体类型
                let type_str = format!("{:?}", object_type);
                if type_str.contains("Table") {
                    "DropTable".to_string()
                } else if type_str.contains("Index") {
                    "DropIndex".to_string()
                } else if type_str.contains("View") {
                    "DropView".to_string()
                } else if type_str.contains("Database") {
                    "DropDatabase".to_string()
                } else {
                    format!("Drop{:?}", object_type)
                }
            }
            Statement::Truncate(_) => "Truncate".to_string(),
            Statement::Query(_) => "Query".to_string(),
            Statement::Insert(_) => "Insert".to_string(),
            Statement::Update(_) => "Update".to_string(),
            Statement::Delete(_) => "Delete".to_string(),
            Statement::Set(_) => "Set".to_string(),
            _ => format!("{:?}", stmt),
        }
    }
}

impl Default for DdlGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guard() -> DdlGuard {
        DdlGuard::new()
    }

    #[test]
    fn test_valid_create_table() {
        let result = guard()
            .validate("CREATE TABLE users (id INT PRIMARY KEY)")
            .unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_valid_create_table_lowercase() {
        let result = guard().validate("create table users (id int)").unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_valid_create_or_replace() {
        // CREATE OR REPLACE 是 sqlparser 规范化的语句，仍解析为 CreateTable
        let result = guard()
            .validate("CREATE OR REPLACE TABLE users (id INT)")
            .unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_valid_alter_table() {
        let result = guard()
            .validate("ALTER TABLE users ADD COLUMN name VARCHAR(255)")
            .unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_valid_create_index() {
        let result = guard()
            .validate("CREATE INDEX idx_name ON users (name)")
            .unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_valid_drop_index() {
        let result = guard().validate("DROP INDEX idx_name").unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_valid_drop_view() {
        let result = guard().validate("DROP VIEW active_users").unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_valid_create_view() {
        let result = guard()
            .validate("CREATE VIEW active_users AS SELECT * FROM users WHERE active = true")
            .unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_valid_select() {
        let result = guard().validate("SELECT 1").unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_drop_database_rejected() {
        let result = guard().validate("DROP DATABASE production").unwrap();
        assert!(matches!(
            result,
            DdlValidationResult::Forbidden(ref msg) if msg.contains("DROP DATABASE")
        ));
    }

    #[test]
    fn test_drop_database_lowercase_rejected() {
        let result = guard().validate("drop database production").unwrap();
        assert!(matches!(
            result,
            DdlValidationResult::Forbidden(ref msg) if msg.contains("forbidden")
        ));
    }

    #[test]
    fn test_drop_all_rejected() {
        let result = guard().validate("DROP ALL TABLES").unwrap();
        assert!(matches!(
            result,
            DdlValidationResult::Forbidden(ref msg) if msg.contains("DROP ALL")
        ));
    }

    #[test]
    fn test_drop_table_allowed_for_admin() {
        // DROP TABLE 在白名单中（admin 路径 DdlGuard 放行，角色检查由 security_gate 前置保证）
        let result = guard().validate("DROP TABLE users").unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_insert_allowed_for_admin() {
        // DML 在白名单中（迁移事务可能混合 DDL+DML）
        let result = guard()
            .validate("INSERT INTO users (id) VALUES (1)")
            .unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_update_allowed_for_admin() {
        let result = guard()
            .validate("UPDATE users SET name = 'test' WHERE id = 1")
            .unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_delete_allowed_for_admin() {
        let result = guard().validate("DELETE FROM users WHERE id = 1").unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_delete_lowercase_allowed_for_admin() {
        let result = guard().validate("delete from users where id = 1").unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_empty_sql() {
        let result = guard().validate("").unwrap();
        assert!(matches!(
            result,
            DdlValidationResult::Forbidden(ref msg) if msg.contains("Empty")
        ));
    }

    #[test]
    fn test_whitespace_sql() {
        let result = guard().validate("   \n\t  ").unwrap();
        assert!(matches!(
            result,
            DdlValidationResult::Forbidden(ref msg) if msg.contains("Empty")
        ));
    }

    use std::sync::Arc;

    /// 拒绝一切的测试策略（验证端口可注入自定义实现）
    struct DenyAllGuard;

    impl DdlGuardPolicy for DenyAllGuard {
        fn validate(&self, _sql: &str) -> Result<DdlValidationResult, String> {
            Ok(DdlValidationResult::Forbidden("deny all".to_string()))
        }
    }

    #[test]
    fn test_policy_trait_object_whitelist_guard() {
        // 内置白名单守卫经 trait 对象使用（Session 漏斗的默认路径）
        let policy: Arc<dyn DdlGuardPolicy> = Arc::new(DdlGuard::new());
        assert!(matches!(
            policy.validate("CREATE TABLE t (id INT)"),
            Ok(DdlValidationResult::Allowed)
        ));
        assert!(matches!(
            policy.validate("DROP DATABASE x"),
            Ok(DdlValidationResult::Forbidden(_))
        ));
    }

    #[test]
    fn test_policy_trait_object_custom_injection() {
        let policy: Arc<dyn DdlGuardPolicy> = Arc::new(DenyAllGuard);
        assert!(matches!(
            policy.validate("CREATE TABLE t (id INT)"),
            Ok(DdlValidationResult::Forbidden(ref msg)) if msg.contains("deny all")
        ));
    }

    #[test]
    fn test_auditing_guard_forwards_decisions_to_sink() {
        let events: Arc<std::sync::Mutex<Vec<DdlAuditRecord>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink_events = events.clone();
        let auditing = AuditingDdlGuard::new(
            Arc::new(DdlGuard::new()),
            Arc::new(move |record: &DdlAuditRecord| {
                sink_events.lock().unwrap().push(DdlAuditRecord {
                    sql: record.sql.clone(),
                    allowed: record.allowed,
                    reason: record.reason.clone(),
                });
            }),
        );

        // 混合决策：放行 + 拦截（对齐 Session 漏斗用法：validate 后触发 audit）
        let allowed = auditing.validate("CREATE TABLE t (id INT)").unwrap();
        auditing.audit("CREATE TABLE t (id INT)", &allowed);
        let forbidden = auditing.validate("DROP DATABASE prod").unwrap();
        auditing.audit("DROP DATABASE prod", &forbidden);

        let events = events.lock().unwrap();
        assert_eq!(events.len(), 2, "audit 钩子应逐决策转发");
        assert!(events[0].allowed);
        assert!(!events[1].allowed);
        assert!(
            events[1]
                .reason
                .as_deref()
                .unwrap()
                .contains("DROP DATABASE")
        );
    }

    #[test]
    fn test_dry_run_guard_records_and_plans() {
        let dry = DryRunDdlGuard::new(Arc::new(DdlGuard::new()));

        // 单条干跑查询
        assert!(dry.would_allow("CREATE TABLE t (id INT)"));
        assert!(!dry.would_allow("DROP DATABASE prod"));
        assert_eq!(dry.records().len(), 2, "would_allow 应记录决策");

        // 批量预检：不执行，逐条给出决策
        let plan = dry.plan(&["ALTER TABLE t ADD COLUMN c INT", "GRANT ALL ON t TO x"]);
        assert_eq!(plan.len(), 2);
        assert!(plan[0].allowed);
        assert!(!plan[1].allowed, "GRANT 不在白名单，应标记拦截");
        assert_eq!(dry.records().len(), 4, "plan 决策并入记录");

        // 作为守卫注入时语义与内部策略一致
        assert!(matches!(
            DdlGuardPolicy::validate(&dry, "CREATE INDEX i ON t (c)"),
            Ok(DdlValidationResult::Allowed)
        ));
    }
}
