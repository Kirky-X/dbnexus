// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Session 模块
//!
//! 提供数据库会话管理，包括事务、权限检查和读写分离

use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(any(feature = "ladybug", feature = "neo4j"))]
use std::collections::HashMap;

// DbPool 测试导入的 cfg 必须与实际使用点一致：
// - graph_tests / vuln_0005_tests：ladybug
// - vuln_0001_tests：permission + sqlite（测试用 sqlite::memory: 真实建池）
// - session_basic_tests（仅 sqlite）使用全限定 super::super::DbPool，不依赖本导入。
// 若只门控 permission（不含 sqlite），permission 单开组合下 vuln_0001_tests 整体
// 被 cfg 掉，导入悬空触发 unused import 警告。
#[cfg(all(
    test,
    any(feature = "ladybug", all(feature = "permission", feature = "sqlite"))
))]
use super::DbPool;
#[cfg(feature = "permission")]
use super::audit::audit_admin_bypass;
use super::db_pool::DbPoolInner;
use super::{DatabaseConnection, DbConnection};
#[cfg(all(feature = "sql-parser", feature = "permission"))]
use crate::access::SqlParser;
#[cfg(all(feature = "sql-parser", not(feature = "permission")))]
use crate::access::SqlParser;
#[cfg(feature = "sql-parser")]
use crate::access::is_ddl_operation;
#[cfg(feature = "sql-parser")]
use crate::access::{DdlGuard, DdlGuardPolicy, DdlValidationResult};
// SqlOperationType 仅在 permission 权限检查路径（parse_operation_async 结果映射）使用
#[cfg(all(feature = "sql-parser", feature = "permission"))]
use crate::access::SqlOperationType;
#[cfg(feature = "permission")]
use crate::access::{PermissionAction, PermissionContext, TableAccessDecision};
use crate::foundation::{DbError, DbResult};
use crate::i18n;
#[cfg(feature = "metrics")]
use crate::observability::MetricsCollector;
use async_trait::async_trait;

// 导入 Sea-ORM 的事务 trait 和连接 trait
use sea_orm::{ConnectionTrait, DatabaseTransaction, ExecResult, TransactionTrait};
#[cfg(any(feature = "ladybug", feature = "neo4j"))]
use tokio::sync::Mutex;
use tokio::sync::RwLock;

// 大文件拆分：按职责纯移动的子模块（行为不变）
mod duckdb;
mod execute;
mod graph;
mod metrics;
#[cfg(test)]
mod tests;
mod transaction;

/// Session 内部可变状态
///
/// 使用 Mutex 包装需要内部可变性的字段，支持 `&self` 方法签名
struct SessionState {
    /// 事务对象（用于真实的事务管理）
    ///
    /// 性能优化：使用 `Arc<DatabaseTransaction>` 而非 `DatabaseTransaction`，
    /// 因为 sea-orm 的 `DatabaseTransaction` 未实现 `Clone`，使用 `Arc` 包装后
    /// 可在 `execute_raw` 中短锁 clone 后锁外执行 async DB 操作，避免持锁 await。
    transaction: Option<Arc<DatabaseTransaction>>,

    /// 图数据库事务对象（ladybug/neo4j feature 启用时可用）
    ///
    /// 使用 `Box<dyn GraphTransaction + Send>` 存储图事务句柄。
    /// `GraphTransaction::commit/rollback` 消耗 `self`，因此使用 `Option` 存储，
    /// take 出来后调用。
    #[cfg(any(feature = "ladybug", feature = "neo4j"))]
    graph_transaction: Option<Box<dyn crate::database::graph::GraphTransaction + Send>>,

    /// 图事务是否被 poison
    ///
    /// 当 `execute_cypher` 在事务内 await 期间 panic 时，take→put back 中断，
    /// 事务句柄丢失。设置此标记后，后续图操作返回错误，防止在事务外执行。
    #[cfg(any(feature = "ladybug", feature = "neo4j"))]
    graph_txn_poisoned: bool,

    /// 最后写操作时间（用于读写分离）
    ///
    /// LD-1 误报说明（架构审查）：审查曾标记"`Option<Instant>` 存在原子时序问题"为
    /// LOW 架构问题。此为误报：`last_write` 由外层 `state: Mutex<SessionState>` 保护，
    /// 所有读写均持锁（`mark_write` 写、`should_use_master` 读、`commit/rollback` 清除），
    /// 不存在无锁并发访问，无需使用 `AtomicInstant`。`Mutex<SessionState>` 的串行化
    /// 保证 `last_write` 的 read-modify-write 是原子的。改用原子操作反而是过度工程化。
    last_write: Option<Instant>,
}

/// serde_json 值 → sea-orm 绑定值
///
/// Null 绑定为无类型参数（列类型由引擎推断）；数组/对象序列化为
/// JSON 字符串（与仓储字面量时代的存储形态一致）。
fn json_to_sea_value(v: &serde_json::Value) -> sea_orm::Value {
    match v {
        serde_json::Value::Null => None::<String>.into(),
        serde_json::Value::Bool(b) => (*b).into(),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into()
            } else {
                n.as_f64().unwrap_or(0.0).into()
            }
        }
        serde_json::Value::String(s) => s.clone().into(),
        other => serde_json::to_string(other)
            .unwrap_or_else(|_| "null".to_string())
            .into(),
    }
}

/// 语句构造：空参走 `from_string`（与既有路径完全一致），带参走绑定
fn build_statement(
    backend: sea_orm::DatabaseBackend,
    sql: String,
    params: &[serde_json::Value],
) -> sea_orm::Statement {
    if params.is_empty() {
        sea_orm::Statement::from_string(backend, sql)
    } else {
        sea_orm::Statement::from_sql_and_values(backend, sql, params.iter().map(json_to_sea_value))
    }
}

/// 事务隔离级别（四档，映射到 sea-orm `IsolationLevel`）
///
/// 各后端的实际支持度不同（如 SQLite 只有可串行化语义、MySQL 默认
/// REPEATABLE READ）：请求的级别由底层引擎尽力落实，SQLite 上会
/// 映射为引擎默认语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbIsolationLevel {
    /// 读未提交
    ReadUncommitted,
    /// 读已提交
    ReadCommitted,
    /// 可重复读
    RepeatableRead,
    /// 可串行化
    Serializable,
}

impl DbIsolationLevel {
    /// 映射到 sea-orm 隔离级别
    pub fn into_sea_orm(self) -> sea_orm::IsolationLevel {
        match self {
            DbIsolationLevel::ReadUncommitted => sea_orm::IsolationLevel::ReadUncommitted,
            DbIsolationLevel::ReadCommitted => sea_orm::IsolationLevel::ReadCommitted,
            DbIsolationLevel::RepeatableRead => sea_orm::IsolationLevel::RepeatableRead,
            DbIsolationLevel::Serializable => sea_orm::IsolationLevel::Serializable,
        }
    }
}

/// Session 结构
pub struct Session {
    /// 数据库连接（统一枚举：SeaORM 或 DuckDB）
    connection: Option<DbConnection>,

    /// 连接池内部状态（用于归还连接）
    pool_inner: Arc<DbPoolInner>,

    /// 角色
    role: String,

    /// 是否 admin 角色（`new` 时预计算的快照；`role` 与 `pool_inner.admin_role`
    /// 构造后均不可变，热路径（每条语句的权限检查）免字符串比较）
    is_admin: bool,

    /// 权限上下文
    #[cfg(feature = "permission")]
    permission_ctx: PermissionContext,

    /// 内部可变状态（事务和写操作时间）
    state: RwLock<SessionState>,

    /// 图操作互斥锁（防止并发 `execute_cypher` 在 take → put back 窗口绕过事务）
    ///
    /// `Box<dyn GraphTransaction>` 不可 clone，图事务采用
    /// take → 锁外 await → put back 模式。若无互斥，并发 `execute_cypher` 会在
    /// take 后的 await 窗口内看到 `graph_transaction` 为 `None`，落入"直接在连接上
    /// 执行"分支，破坏事务隔离。此锁将图操作串行化，确保 put back 后才允许下一个 take。
    #[cfg(any(feature = "ladybug", feature = "neo4j"))]
    graph_op_mutex: Mutex<()>,

    /// 指标收集器（可选，用于 metrics 特性）
    #[cfg(feature = "metrics")]
    metrics_collector: Option<Arc<MetricsCollector>>,
}

impl Session {
    /// 创建新的 Session
    pub(crate) fn new(
        connection: DbConnection,
        pool_inner: Arc<DbPoolInner>,
        role: String,
    ) -> Self {
        #[cfg(feature = "permission")]
        let permission_ctx = PermissionContext::new(role.clone(), pool_inner.policy_cache.clone());

        let is_admin = role == pool_inner.admin_role;

        #[cfg(feature = "metrics")]
        let metrics = pool_inner
            .metrics_collector
            .read()
            .expect("metrics_collector lock")
            .clone();

        Session {
            connection: Some(connection),
            pool_inner,
            role,
            is_admin,
            #[cfg(feature = "permission")]
            permission_ctx,
            state: RwLock::new(SessionState {
                transaction: None,
                #[cfg(any(feature = "ladybug", feature = "neo4j"))]
                graph_transaction: None,
                #[cfg(any(feature = "ladybug", feature = "neo4j"))]
                graph_txn_poisoned: false,
                last_write: None,
            }),
            #[cfg(any(feature = "ladybug", feature = "neo4j"))]
            graph_op_mutex: Mutex::new(()),
            #[cfg(feature = "metrics")]
            metrics_collector: metrics,
        }
    }

    /// 获取角色
    pub fn role(&self) -> &str {
        &self.role
    }

    /// 获取权限上下文
    #[cfg(feature = "permission")]
    pub fn permission_ctx(&self) -> &PermissionContext {
        &self.permission_ctx
    }

    /// 标记为写操作
    pub async fn mark_write(&self) {
        let mut state = self.state.write().await;
        state.last_write = Some(Instant::now());
    }

    /// 检查权限
    #[cfg(feature = "permission")]
    pub async fn check_permission(
        &self,
        table: &str,
        operation: &PermissionAction,
    ) -> Result<(), DbError> {
        // Admin 角色绕过权限检查（拥有完全控制权）
        // vuln-0001 修复：admin bypass 仍记录审计事件（进程级审计环）以保留审计链
        if self.is_admin {
            audit_admin_bypass(&self.role, table, operation);
            return Ok(());
        }

        check_table_or_error(&self.permission_ctx, table, operation).await
    }

    /// 是否在事务中
    ///
    /// 图事务或关系型事务任一存在都返回 true。
    pub async fn is_in_transaction(&self) -> bool {
        let state = self.state.read().await;
        #[cfg(any(feature = "ladybug", feature = "neo4j"))]
        {
            state.graph_transaction.is_some() || state.transaction.is_some()
        }
        #[cfg(not(any(feature = "ladybug", feature = "neo4j")))]
        {
            state.transaction.is_some()
        }
    }
}

impl Session {
    /// 是否应该使用主库（基于读写分离配置）
    pub async fn should_use_master(&self) -> bool {
        let state = self.state.read().await;
        // 如果在事务中，必须使用主库
        if state.transaction.is_some() {
            return true;
        }

        // 如果配置了读写分离且有写操作，使用主库
        state
            .last_write
            .map(|t| t.elapsed() < Duration::from_secs(5))
            .unwrap_or(false)
    }

    /// 获取 SeaORM 连接引用（仅内部宏和测试使用）
    ///
    /// 用户应通过 Entity 的 CRUD 方法进行数据库操作，不应直接调用此方法。
    /// 此方法从 `DbConnection` 枚举中提取 SeaORM 连接，若为 DuckDB 连接则返回错误。
    ///
    /// # 安全性
    ///
    /// 此方法确保连接在使用前是可用的。如果连接已被释放（不应发生），
    /// 将返回错误。Session 的生命周期管理确保连接始终可用。
    pub fn connection(&self) -> Result<&DatabaseConnection, DbError> {
        self.connection
            .as_ref()
            .ok_or_else(|| {
                DbError::Config(
                    "Connection not available - Session may have been invalidated".to_string(),
                )
            })?
            .as_sea_orm()
    }

    /// 创建迁移执行器（仅内部使用）
    ///
    /// 用于迁移功能，将底层连接包装成 MigrationExecutor
    #[cfg(feature = "migration")]
    pub fn create_migration_executor(
        &self,
        db_type: crate::foundation::DatabaseType,
    ) -> Result<super::MigrationExecutor, DbError> {
        let conn = self.connection()?.clone();
        Ok(super::MigrationExecutor::new(conn, db_type))
    }
}

#[cfg(all(feature = "sql-parser", feature = "permission"))]
fn is_invalid_table_name(table_name: &str) -> bool {
    let table_name = table_name.trim();
    if table_name.is_empty() {
        return true;
    }

    for part in table_name.split('.') {
        let part = part.trim();
        if part.is_empty() {
            return true;
        }

        let unquoted = part
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .or_else(|| part.strip_prefix('`').and_then(|s| s.strip_suffix('`')))
            .or_else(|| part.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
            .unwrap_or(part)
            .trim();

        if unquoted.is_empty() {
            return true;
        }
    }

    false
}

/// 构造权限拒绝错误
///
/// 统一 "Permission denied for {action} on {table}" 错误消息格式，
/// 避免在多处调用点重复 `DbError::Permission(format!(...))` 模板。
#[cfg(feature = "permission")]
fn permission_denied(
    action: &(impl std::fmt::Display + ?Sized),
    table: &(impl std::fmt::Display + ?Sized),
) -> DbError {
    log::warn!(
        "permission denied: action={} table={}",
        sanitize_log_field(&action.to_string()),
        sanitize_log_field(&table.to_string())
    );
    DbError::Permission(i18n::t(
        "session-permission-denied",
        &[("action", action.to_string()), ("table", table.to_string())],
    ))
}

/// 构造速率限制拒绝错误（HTTP 429 语义，区别于策略拒绝的
/// [`permission_denied`]；`retry_after` 供 HTTP 层映射 `Retry-After` 头。
/// 亚秒建议按秒向上取整：默认档填充速率 2/s 给出 500ms 建议，截断为 0
/// 会被当作「后端未提供」丢弃，导致 Retry-After 系统性缺失）
#[cfg(feature = "permission")]
fn rate_limited_error(retry_after: Option<Duration>) -> DbError {
    log::warn!(
        "rate limited: table access throttled (retry_after={:?})",
        retry_after
    );
    DbError::RateLimited {
        retry_after_secs: retry_after
            .filter(|d| !d.is_zero())
            .map(|d| d.as_secs() + u64::from(d.subsec_nanos() != 0)),
    }
}

/// 限流感知的表访问检查：策略拒绝 → 403 语义，限流拒绝 → 429 语义
#[cfg(feature = "permission")]
async fn check_table_or_error(
    ctx: &PermissionContext,
    table: &str,
    action: &PermissionAction,
) -> Result<(), DbError> {
    match ctx.check_table_access_decision(table, action).await {
        TableAccessDecision::Allowed => Ok(()),
        TableAccessDecision::Denied => Err(permission_denied(action, table)),
        TableAccessDecision::RateLimited { retry_after } => Err(rate_limited_error(retry_after)),
    }
}

/// 剔除控制字符（<0x20 与 0x7F，含换行/回车/ESC）与 Unicode 格式字符
/// （零宽 U+200B-200F、双向控制 U+202A-202E、双向隔离 U+2066-2069、BOM
/// U+FEFF），防止日志注入：表名/角色名等字段源自用户 SQL 解析结果或调用
/// 方传入，原样写入会让攻击者伪造日志行、注入 ANSI 序列或经不可见字符
/// 视觉重排误导日志审阅者（OWASP Logging Cheat Sheet / Trojan Source）。
#[cfg(feature = "permission")]
fn sanitize_log_field(input: &str) -> String {
    input
        .chars()
        .filter(|c| {
            !c.is_control()
                && !matches!(
                    c,
                    '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}'
                )
        })
        .collect()
}

/// 判断是否为写操作（Insert/Update/Delete）
#[cfg(feature = "permission")]
fn is_write_action(action: &PermissionAction) -> bool {
    matches!(
        action,
        PermissionAction::Insert | PermissionAction::Update | PermissionAction::Delete
    )
}

/// vuln-0005 修复：Cypher 注入防护检查
///
/// 对原始 Cypher 语句进行多层安全检查，拒绝明显危险的输入。
/// 这是参数化查询之外的第二道防线（defense in depth）：
/// - 参数化查询防止值注入
/// - 此函数防止语句结构注入（多语句、注释混淆、危险过程调用）
///
/// # 检查项
///
/// 1. **长度限制**：超过 10KB（10_240 字节）的 Cypher 拒绝（防止 DoS / 端口扫描 payload）
/// 2. **多语句**：除末尾分号外的 `;` 拒绝（防止 `MATCH ...; DELETE ...` 多语句注入）
/// 3. **行注释**：`//`（非 URL scheme）拒绝（防止注释掉后续安全检查）
/// 4. **块注释**：`/* */` 拒绝（防止注释绕过权限检查片段）
/// 5. **危险过程**：`CALL apoc.` 等管理员过程拒绝（防止提权 / 文件系统访问）
///
/// # 参数
///
/// * `cypher` - 待检查的 Cypher 语句
///
/// # 返回
///
/// - `Ok(())` 表示通过安全检查
/// - `Err(DbError::Permission(...))` 表示检测到危险模式
///
/// # Errors
///
/// 检测到危险模式时返回 `DbError::Permission`，错误消息描述具体原因。
#[cfg(any(feature = "ladybug", feature = "neo4j"))]
fn validate_cypher_safety(cypher: &str) -> DbResult<()> {
    // 1. 长度限制：10KB（10_240 字节）
    const MAX_CYPHER_BYTES: usize = 10_240;
    if cypher.len() > MAX_CYPHER_BYTES {
        return Err(DbError::Permission(format!(
            "Cypher query exceeds maximum length ({} bytes, got {} bytes) - potential DoS payload",
            MAX_CYPHER_BYTES,
            cypher.len()
        )));
    }

    // 2. 多语句检测：除末尾分号外的 `;`
    //
    // 末尾分号允许（部分客户端习惯以 `;` 结尾），但中间的 `;` 视为多语句注入。
    let trimmed = cypher.trim();
    let inner = trimmed.trim_end_matches(';').trim();
    if inner.contains(';') {
        return Err(DbError::Permission(
            "Cypher query contains multiple statements (';' inside query) - potential injection"
                .to_string(),
        ));
    }

    // 3. 行注释检测：`//`（排除 URL scheme 如 `http://`、`https://`）
    //
    // Cypher 不支持 `//` 行注释（OpenCypher 标准用 `//` 是合法注释，但极少在正常查询中使用）。
    // 检测策略：查找 `//` 出现位置，若前一个字符不是字母（排除 URL scheme）则拒绝。
    if let Some(pos) = cypher.find("//") {
        let is_url_scheme = pos > 0 && {
            let prev = cypher.as_bytes()[pos - 1];
            prev.is_ascii_alphabetic()
        };
        if !is_url_scheme {
            return Err(DbError::Permission(
                "Cypher query contains line comment '//' - potential injection".to_string(),
            ));
        }
    }

    // 4/5. 块注释与危险过程检测经统一注入引擎（规则表合并 + 去重）
    let findings = crate::access::InjectionEngine::global().scan_graph(cypher);
    // 块注释（`/* */`）：任一标记命中即拒绝（与合并前第 4 步口径一致）
    if findings
        .iter()
        .any(|rule| rule.category == crate::access::RuleCategory::Comment)
    {
        return Err(DbError::Permission(
            "Cypher query contains block comment '/* */' - potential injection".to_string(),
        ));
    }
    // 危险过程调用：`CALL apoc.`（APOC 是 Neo4j 管理员过程库，可执行系统操作）；
    // 其他危险过程（如 `dbms.`、`db.`）也在黑名单中，防止提权 / 系统访问
    if let Some(rule) = findings
        .iter()
        .find(|rule| rule.category == crate::access::RuleCategory::GraphProcedure)
    {
        return Err(DbError::Permission(format!(
            "Cypher query calls dangerous procedure ('{}') - potential privilege escalation",
            rule.pattern
        )));
    }

    Ok(())
}

impl Drop for Session {
    fn drop(&mut self) {
        // 说明：图事务通过级联 Drop 处理
        //
        // `state: Mutex<SessionState>` 被 drop 时，`SessionState::graph_transaction`
        // 也会被 drop，触发 `LadybugTransaction::drop`（actor 模式自动 ROLLBACK）
        // 或 `Neo4jTransaction::drop`（spawn rollback task）。
        //
        // 如果 `execute_cypher` 正在执行（graph_txn 被 take 出来在 await 中），
        // Session drop 会导致 future drop，局部变量 `graph_txn` 也会被 drop。
        //
        // 归还连接到池（直接通过 DbPoolInner 操作，无需 Arc<DbPool>）
        if let Some(conn) = self.connection.take() {
            DbPoolInner::release_connection(&self.pool_inner, conn);
        }
    }
}

/// 基于 SqlParser 的表名提取（vuln-0003 修复）
///
/// 使用 sqlparser AST 解析提取 SQL 语句的表名，替代朴素字符串匹配。
/// 正确处理字符串字面量、注释、子查询等复杂 SQL 语法，防止权限检查绕过。
///
/// # 参数
///
/// * `sql` - SQL 语句
///
/// # 返回
///
/// - `Some(table_name)` - 成功提取表名
/// - `None` - 解析失败、不支持的语句类型（DDL/DCL/Transaction）或无表名
///
/// # 行为说明
///
/// - 使用全局共享 `SqlParser` 单例，避免重复创建 parser + 缓存
/// - 解析失败时返回 `None`，调用方应跳过表级权限检查（由下游 `execute_raw`
///   的 SqlParser 检查提供防御纵深）
/// - 派生表（subquery in FROM）返回 `None`（无具名基表）
///
/// # 安全性
///
/// 此函数是 vuln-0003 修复的核心，替代了可被绕过的 `extract_table_name`。
/// 当 `permission` feature 启用时，`sql-parser` feature 被强制启用
/// （Cargo.toml: `permission = ["sql-parser", ...]`），因此此函数始终可用。
#[cfg(all(feature = "permission", feature = "sql-parser"))]
async fn extract_table_name_via_parser(sql: &str) -> Option<String> {
    let parser = SqlParser::shared().await;
    match parser.parse_operation_async(sql).await {
        Ok(Some((table, _))) => {
            if table.is_empty() || is_invalid_table_name(&table) {
                None
            } else {
                Some(table)
            }
        }
        Ok(None) => None,
        Err(_) => None,
    }
}

// 实现 DatabaseSession trait
#[async_trait]
impl super::DatabaseSession for Session {
    async fn execute(&self, sql: &str) -> crate::DbResult<ExecResult> {
        Ok(self.execute(sql).await?)
    }

    async fn execute_raw(&self, sql: &str) -> crate::DbResult<ExecResult> {
        Ok(self.execute_raw(sql).await?)
    }

    async fn execute_raw_ddl(&self, sql: &str) -> crate::DbResult<ExecResult> {
        Ok(self.execute_raw_ddl(sql).await?)
    }

    async fn begin_transaction(&self) -> crate::DbResult<()> {
        Ok(self.begin_transaction().await?)
    }

    async fn commit(&self) -> crate::DbResult<()> {
        Ok(self.commit().await?)
    }

    async fn rollback(&self) -> crate::DbResult<()> {
        Ok(self.rollback().await?)
    }

    fn role(&self) -> &str {
        self.role()
    }

    async fn is_in_transaction(&self) -> bool {
        self.is_in_transaction().await
    }
}
