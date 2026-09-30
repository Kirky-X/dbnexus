// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 大文件拆分：自 session.rs 按职责纯移动的 impl 块（行为不变）。

use super::*;

impl Session {
    /// 执行原始 SQL（带权限检查）
    ///
    /// # 自动重试语义
    ///
    /// `retry` feature 启用且 `DbConfig.retry_policy` 配置时：
    /// - **幂等操作**（SELECT/SHOW/EXPLAIN 前缀，见 `is_idempotent_operation`）
    ///   失败后自动按指数退避重试（至多 `max_retries` 次）
    /// - **写类操作**（INSERT/UPDATE/DELETE/DDL）绝不重试，避免副作用重复
    pub async fn execute_raw(&self, sql: &str) -> DbResult<ExecResult> {
        self.execute_raw_impl(sql, &[]).await
    }

    /// 绑定参数执行 SQL
    ///
    /// 与 [`Self::execute_raw`] 同一条防御链（DDL 拒绝/权限/慢查询/事务感知/重试），
    /// 差异仅在语句构造：非空 `params` 经 `Statement::from_sql_and_values` 绑定，
    /// 占位符按后端书写（PostgreSQL `$N`，SQLite/MySQL `?`）。
    /// `params` 为空时与 `execute_raw` 路径逐字节一致。
    pub async fn execute_with_params(
        &self,
        sql: &str,
        params: &[serde_json::Value],
    ) -> DbResult<ExecResult> {
        self.execute_raw_impl(sql, params).await
    }

    /// 内部：execute_raw 的参数化实现
    async fn execute_raw_impl(
        &self,
        sql: &str,
        #[cfg_attr(not(feature = "sql-parser"), allow(unused_variables))]
        params: &[serde_json::Value],
    ) -> DbResult<ExecResult> {
        #[cfg(feature = "sql-parser")]
        {
            // 检查是否为 DDL 操作
            if is_ddl_operation(sql) {
                return Err(DbError::Permission(
                    "DDL operations are not allowed in this context".to_string(),
                ));
            }
        }

        #[cfg(not(feature = "sql-parser"))]
        {
            let _ = sql;
            Err(DbError::Permission(
                "execute_raw requires the sql-parser feature to be enabled".to_string(),
            ))
        }

        #[cfg(feature = "sql-parser")]
        {
            #[cfg(all(feature = "sql-parser", feature = "permission"))]
            {
                // 解析 SQL 操作类型和表名（使用全局共享单例，避免重复创建 parser + 缓存）
                let parser = SqlParser::shared().await;
                match parser.parse_single(sql).await {
                    Ok(parsed) => {
                        let action = match parsed.operation_type {
                            SqlOperationType::Select => PermissionAction::Select,
                            SqlOperationType::Insert => PermissionAction::Insert,
                            SqlOperationType::Update => PermissionAction::Update,
                            SqlOperationType::Delete => PermissionAction::Delete,
                            // 解析成功但是不支持的语句类型（DDL/DCL/Transaction），
                            // 这些情况需要拒绝执行以确保安全
                            _ => {
                                return Err(DbError::Permission(
                                    "SQL statement requires a valid table name for permission checking".to_string(),
                                ));
                            }
                        };

                        // Admin 角色绕过权限检查（含表名有效性检查：表名检查本就是
                        // 权限检查的一部分，无表语句如 SELECT 1 不触达任何表，对
                        // admin 放行无越权面）；非 admin 保持 fail-closed
                        if self.role == self.pool_inner.admin_role {
                            // admin 有完全权限，跳过检查
                        } else {
                            if parsed.all_table_names.is_empty() {
                                return Err(DbError::Permission(
                                    "Failed to extract table name for permission checking"
                                        .to_string(),
                                ));
                            }
                            for table in &parsed.all_table_names {
                                if table.is_empty() || is_invalid_table_name(table) {
                                    return Err(DbError::Permission(
                                        "Failed to extract table name for permission checking"
                                            .to_string(),
                                    ));
                                }
                            }

                            // 对语句涉及的所有表逐一检查权限
                            // （含 JOIN/子查询表，防止通过关联表越权读写未授权数据）
                            for table in &parsed.all_table_names {
                                if !self.permission_ctx.check_table_access(table, &action).await {
                                    return Err(permission_denied(&action, table));
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // 解析失败：admin role 放行（对齐 Ok(None)/parse 失败路径），
                        // 非 admin role 拒绝（安全默认）。
                        if self.role == self.pool_inner.admin_role {
                            // admin 有完全权限，跳过检查
                        } else {
                            log::warn!(
                                "permission denied: role={} reason=sql-parse-failure (fail-closed)",
                                sanitize_log_field(&self.role)
                            );
                            return Err(DbError::Permission(
                                "Failed to parse SQL statement for permission checking".to_string(),
                            ));
                        }
                    }
                }
            }

            // 慢查询检测——在查询执行前记录起始时间
            #[cfg(all(feature = "metrics", feature = "sql-parser"))]
            let query_start = std::time::Instant::now();

            // 性能优化：短锁 clone Arc<DatabaseTransaction>，锁外执行 async DB 调用
            let tx_opt: Option<Arc<DatabaseTransaction>> = {
                let state = self.state.write().await;
                state.transaction.clone()
            };

            // retry feature: 幂等查询自动重试 + 指数退避
            #[cfg(feature = "retry")]
            {
                if let Some(ref policy) = self.pool_inner.config.retry_policy
                    && crate::reliability::is_idempotent_operation(sql)
                {
                    let mut last_error: Option<DbError> = None;
                    // 首次执行 + 重试循环
                    for attempt in 0..=policy.max_retries {
                        if attempt > 0 {
                            let backoff = Self::calculate_retry_backoff(policy, attempt - 1);
                            tokio::time::sleep(backoff).await;
                        }
                        let backend = if let Some(tx) = tx_opt.as_ref() {
                            use sea_orm::ConnectionTrait;
                            tx.get_database_backend()
                        } else {
                            self.connection()?.get_database_backend()
                        };
                        let stmt = build_statement(backend, sql.to_owned(), params);
                        let result = if let Some(ref tx) = tx_opt {
                            tx.execute_raw(stmt).await.map_err(DbError::Connection)
                        } else {
                            let conn = self.connection()?;
                            conn.execute_raw(stmt).await.map_err(DbError::Connection)
                        };
                        match result {
                            Ok(exec_result) => {
                                // 记录查询指标（含慢查询检测）
                                #[cfg(all(feature = "metrics", feature = "sql-parser"))]
                                self.record_execute_metrics(query_start, true);
                                return Ok(exec_result);
                            }
                            Err(e) => last_error = Some(e),
                        }
                    }
                    // 重试耗尽，记录失败
                    #[cfg(all(feature = "metrics", feature = "sql-parser"))]
                    self.record_execute_metrics(query_start, false);
                    return Err(last_error.unwrap());
                }
            }

            // 无重试路径（retry 未启用或非幂等操作）
            let backend = if let Some(tx) = tx_opt.as_ref() {
                use sea_orm::ConnectionTrait;
                tx.get_database_backend()
            } else {
                self.connection()?.get_database_backend()
            };
            let stmt = build_statement(backend, sql.to_owned(), params);
            let result = if let Some(tx) = tx_opt {
                tx.execute_raw(stmt).await.map_err(DbError::Connection)
            } else {
                let conn = self.connection()?;
                conn.execute_raw(stmt).await.map_err(DbError::Connection)
            };

            // 记录查询指标（含慢查询检测）
            #[cfg(all(feature = "metrics", feature = "sql-parser"))]
            self.record_execute_metrics(query_start, result.is_ok());

            result
        }
    }

    /// 当前读：在活动事务中执行行锁定查询（`SELECT ... FOR UPDATE`）
    ///
    /// 与普通 [`Self::query_rows`]（快照读）不同，本方法要求 Session 已开启
    /// 事务——排他行锁只在事务持有期内有效，非事务下调用一律拒绝，
    /// 避免「拿到锁即释」的假防护。用于「读取-修改-写入」竞态防护
    /// （库存扣减、余额结算等）。
    ///
    /// `FOR UPDATE` 子句由调用方写入 SQL（方言语法各异）；SQLite 不支持
    /// `FOR UPDATE` 语法，测试与 SQLite 场景可传普通 SELECT（事务内执行）。
    ///
    /// # Errors
    ///
    /// - 未开启事务：`DbError::Transaction`
    /// - 非关系型后端（图连接 / DuckDB raw）：`DbError::Unsupported`
    /// - 其余错误透传 [`Self::query_rows`]（含 SELECT-only 与权限检查）
    pub async fn query_rows_for_update(&self, sql: &str) -> DbResult<Vec<serde_json::Value>> {
        // 行锁依赖事务持有期：非事务下一律拒绝
        if !self.is_in_transaction().await {
            return Err(DbError::Transaction(
                "query_rows_for_update requires an active transaction (call begin_transaction first)"
                    .to_string(),
            ));
        }

        // 仅 SeaORM 关系型后端具备行锁语义；raw/图连接显性拒绝
        let is_relational = self
            .connection
            .as_ref()
            .map(|conn| {
                #[cfg(any(feature = "ladybug", feature = "neo4j"))]
                {
                    !conn.is_graph() && conn.as_sea_orm().is_ok()
                }
                #[cfg(not(any(feature = "ladybug", feature = "neo4j")))]
                {
                    conn.as_sea_orm().is_ok()
                }
            })
            .unwrap_or(false);
        if !is_relational {
            return Err(DbError::Unsupported(
                "query_rows_for_update requires a relational (SeaORM) backend".to_string(),
            ));
        }

        // 事务感知执行：query_rows 在活动事务下自动走事务连接
        self.query_rows(sql).await
    }

    /// 统一行查询 API：执行 SELECT 并返回数据行（JSON 对象数组）
    ///
    /// 与 `execute_raw` 共用同一套解析与表级权限检查，但仅允许 SELECT，
    /// 并返回真实数据行（`serde_json::Value` 对象数组）而非 ExecResult，
    /// 供 scatter-gather、数据 API 网关等上层消费。
    ///
    /// # 自动重试语义
    ///
    /// `retry` feature 启用且 `DbConfig.retry_policy` 配置时，行查询失败
    /// 自动按指数退避重试（与 `execute_raw` 幂等路径同口径）。
    pub async fn query_rows(&self, sql: &str) -> DbResult<Vec<serde_json::Value>> {
        self.query_rows_impl(sql, &[]).await
    }

    /// 绑定参数行查询
    ///
    /// 与 [`Self::query_rows`] 同一条防御链与方言处理，差异仅在语句构造：
    /// 非空 `params` 经绑定传递（PostgreSQL `$N`、其余 `?`）。
    /// `params` 为空时与 `query_rows` 路径一致。
    pub async fn query_rows_with_params(
        &self,
        sql: &str,
        params: &[serde_json::Value],
    ) -> DbResult<Vec<serde_json::Value>> {
        self.query_rows_impl(sql, params).await
    }

    /// 内部：query_rows 的参数化实现
    async fn query_rows_impl(
        &self,
        sql: &str,
        #[cfg_attr(not(feature = "sql-parser"), allow(unused_variables))]
        params: &[serde_json::Value],
    ) -> DbResult<Vec<serde_json::Value>> {
        #[cfg(not(feature = "sql-parser"))]
        {
            let _ = sql;
            return Err(DbError::Permission(
                "query_rows requires the sql-parser feature to be enabled".to_string(),
            ));
        }

        #[cfg(feature = "sql-parser")]
        {
            // 与 execute_raw 一致：DDL 一律拒绝
            if is_ddl_operation(sql) {
                return Err(DbError::Permission(
                    "DDL operations are not allowed in this context".to_string(),
                ));
            }

            #[cfg(all(feature = "sql-parser", feature = "permission"))]
            {
                let parser = SqlParser::shared().await;
                match parser.parse_single(sql).await {
                    Ok(parsed) => {
                        // 行查询只允许 SELECT
                        if parsed.operation_type != SqlOperationType::Select {
                            return Err(DbError::Permission(
                                "query_rows only allows SELECT statements".to_string(),
                            ));
                        }
                        // Admin 角色绕过权限检查（含表名有效性检查，同 execute_raw）；
                        // 非 admin 保持 fail-closed 并逐表校验 Select 权限
                        if self.role != self.pool_inner.admin_role {
                            if parsed.all_table_names.is_empty() {
                                return Err(DbError::Permission(
                                    "Failed to extract table name for permission checking"
                                        .to_string(),
                                ));
                            }
                            for table in &parsed.all_table_names {
                                if table.is_empty() || is_invalid_table_name(table) {
                                    return Err(DbError::Permission(
                                        "Failed to extract table name for permission checking"
                                            .to_string(),
                                    ));
                                }
                            }
                            for table in &parsed.all_table_names {
                                if !self
                                    .permission_ctx
                                    .check_table_access(table, &PermissionAction::Select)
                                    .await
                                {
                                    return Err(permission_denied(
                                        &PermissionAction::Select,
                                        table,
                                    ));
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // 解析失败：admin 放行（对齐 execute_raw 路径），非 admin 拒绝（安全默认）
                        if self.role != self.pool_inner.admin_role {
                            log::warn!(
                                "permission denied: role={} reason=sql-parse-failure (fail-closed)",
                                sanitize_log_field(&self.role)
                            );
                            return Err(DbError::Permission(
                                "Failed to parse SQL statement for permission checking".to_string(),
                            ));
                        }
                    }
                }
            }

            // 慢查询检测——与 execute_raw 同口径
            #[cfg(all(feature = "metrics", feature = "sql-parser"))]
            let query_start = std::time::Instant::now();

            // 短锁 clone Arc<DatabaseTransaction>，锁外执行 async DB 调用
            let tx_opt: Option<Arc<DatabaseTransaction>> = {
                let state = self.state.write().await;
                state.transaction.clone()
            };

            // 方言感知执行（SeaORM 2.0 无整行 JSON 提取 API，分方言处理）
            // retry feature——幂等行查询自动重试（与 execute_raw 同口径：
            // RetryPolicy 存在且 SQL 判定为幂等（SELECT/SHOW/EXPLAIN 前缀）时
            // 逐次退避重试；query_rows 仅放行 SELECT，天然幂等）
            #[cfg(feature = "retry")]
            let result = {
                let policy = self
                    .pool_inner
                    .config
                    .retry_policy
                    .as_ref()
                    .filter(|_| crate::reliability::is_idempotent_operation(sql));
                match policy {
                    Some(policy) => {
                        let mut last_error: Option<DbError> = None;
                        let mut success: Option<Vec<serde_json::Value>> = None;
                        for attempt in 0..=policy.max_retries {
                            if attempt > 0 {
                                let backoff = Self::calculate_retry_backoff(policy, attempt - 1);
                                tokio::time::sleep(backoff).await;
                            }
                            match self.query_rows_execute(sql, tx_opt.clone(), params).await {
                                Ok(rows) => {
                                    success = Some(rows);
                                    break;
                                }
                                Err(e) => last_error = Some(e),
                            }
                        }
                        match success {
                            Some(rows) => Ok(rows),
                            None => Err(last_error.unwrap()),
                        }
                    }
                    None => self.query_rows_execute(sql, tx_opt, params).await,
                }
            };

            #[cfg(not(feature = "retry"))]
            let result = self.query_rows_execute(sql, tx_opt, params).await;

            #[cfg(all(feature = "metrics", feature = "sql-parser"))]
            self.record_execute_metrics(query_start, result.is_ok());

            result
        }
    }

    /// 内部：按方言执行行查询并转为 JSON 行
    #[cfg(feature = "sql-parser")]
    ///
    /// - **PostgreSQL**：`SELECT row_to_json(sub.*) FROM (<sql>) sub` 包装，单列 JSON 精确提取
    /// - **SQLite**：主表列经 `pragma_table_info` 内省 + 逐列类型探测（i64→f64→String→Null）
    /// - **MySQL / 图后端 / DuckDB**：MVP 未覆盖，返回明确错误（DuckDB 请使用 `execute_duckdb`）
    async fn query_rows_execute(
        &self,
        sql: &str,
        tx_opt: Option<Arc<DatabaseTransaction>>,
        // pg/sqlite 方言臂都裁掉时（无任何驱动特性）该参数无消费点
        #[cfg_attr(
            not(any(feature = "postgres", feature = "sqlite")),
            allow(unused_variables)
        )]
        params: &[serde_json::Value],
    ) -> DbResult<Vec<serde_json::Value>> {
        use sea_orm::ConnectionTrait;

        // 图后端 / DuckDB 连接不支持 SeaORM 行查询，给出明确错误
        if let Some(conn_arc) = self.connection.as_ref()
            && conn_arc.as_sea_orm().is_err()
        {
            return Err(DbError::Query(
                "query_rows MVP supports SeaORM backends only (postgres/sqlite); \
                 use execute_duckdb or graph APIs for other backends"
                    .to_string(),
            ));
        }

        let backend = if let Some(tx) = tx_opt.as_ref() {
            tx.get_database_backend()
        } else {
            self.connection()?.get_database_backend()
        };

        #[allow(unused_variables)]
        let primary_table: Option<String> = {
            #[cfg(feature = "sql-parser")]
            {
                let parser = SqlParser::shared().await;
                match parser.parse_single(sql).await {
                    Ok(parsed) if parsed.all_table_names.len() == 1 => {
                        Some(parsed.all_table_names[0].clone())
                    }
                    _ => None,
                }
            }
            #[cfg(not(feature = "sql-parser"))]
            {
                let _ = sql;
                None
            }
        };

        // RLS 谓词注入（admin 角色走管理通道不注入；MVP 边界——
        // 无 permission feature 时 primary_table 为 None，注入自动失效）
        // postgres 取数臂按 MVP 设计使用原始 sql 做 row_to_json 包装，
        // 注入产物仅 sqlite 臂消费——sqlite 关闭时该绑定不参与编译使用。
        #[cfg(feature = "data-protection")]
        #[cfg_attr(not(feature = "sqlite"), allow(unused_variables))]
        let sql_for_fetch = {
            let dp = { self.pool_inner.data_protection.read().await.clone() };
            let is_admin = self.role == self.pool_inner.admin_role;
            if !is_admin {
                if let Some(rls) = dp.rls.as_ref() {
                    rls.inject(sql, primary_table.as_deref())
                } else {
                    sql.to_string()
                }
            } else {
                sql.to_string()
            }
        };
        #[cfg(not(feature = "data-protection"))]
        #[cfg_attr(not(feature = "sqlite"), allow(unused_variables))]
        let sql_for_fetch = sql.to_string();

        match backend {
            #[cfg(feature = "postgres")]
            sea_orm::DbBackend::Postgres => {
                // postgres：row_to_json 包装（零列名依赖、类型精确）
                let wrapped = format!(
                    "SELECT row_to_json(sub.*) AS row_data FROM ({}) AS sub",
                    sql.trim_end().trim_end_matches(';')
                );
                let stmt = build_statement(backend, wrapped, params);
                let rows = if let Some(tx) = tx_opt.as_ref() {
                    tx.query_all_raw(stmt).await
                } else {
                    self.connection()?.query_all_raw(stmt).await
                }
                .map_err(DbError::Connection)?;
                let mut out = Vec::with_capacity(rows.len());
                for r in rows {
                    let v = r
                        .try_get::<serde_json::Value>("", "row_data")
                        .map_err(DbError::Connection)?;
                    out.push(v);
                }
                #[cfg(feature = "data-protection")]
                self.apply_masking(&mut out).await;
                Ok(out)
            }
            #[cfg(feature = "sqlite")]
            sea_orm::DbBackend::Sqlite => {
                // sqlite：主表列内省 + 类型探测（MVP：单表查询）
                let table = primary_table.as_deref().ok_or_else(|| {
                    DbError::Query(
                        "query_rows on sqlite (MVP) requires a single-table SELECT statement"
                            .to_string(),
                    )
                })?;
                let cols = self.sqlite_table_columns(table, tx_opt.as_ref()).await?;
                let stmt = build_statement(backend, sql_for_fetch.clone(), params);
                let rows = if let Some(tx) = tx_opt.as_ref() {
                    tx.query_all_raw(stmt).await
                } else {
                    self.connection()?.query_all_raw(stmt).await
                }
                .map_err(DbError::Connection)?;
                #[allow(unused_mut)]
                let mut out: Vec<serde_json::Value> =
                    rows.iter().map(|r| sqlite_row_to_json(r, &cols)).collect();
                #[cfg(feature = "data-protection")]
                self.apply_masking(&mut out).await;
                Ok(out)
            }
            _ => Err(DbError::Query(
                "query_rows MVP supports postgres/sqlite backends only".to_string(),
            )),
        }
        // 所有分支均已 return（postgres/sqlite 成功路径、其他方言 Err）
    }

    /// 查询出口字段脱敏
    #[cfg(feature = "data-protection")]
    async fn apply_masking(&self, rows: &mut [serde_json::Value]) {
        let dp = { self.pool_inner.data_protection.read().await.clone() };
        if let Some(m) = dp.masking.as_ref() {
            m.apply(rows);
        }
    }

    /// 内部（sqlite）：主表列名（pragma_table_info）
    #[cfg(all(feature = "sqlite", feature = "sql-parser"))]
    async fn sqlite_table_columns(
        &self,
        table: &str,
        tx_opt: Option<&Arc<DatabaseTransaction>>,
    ) -> DbResult<Vec<String>> {
        use sea_orm::ConnectionTrait;
        let stmt = sea_orm::Statement::from_string(
            sea_orm::DbBackend::Sqlite,
            format!(
                "SELECT name FROM pragma_table_info('{}')",
                table.replace('\'', "")
            ),
        );
        let rows = if let Some(tx) = tx_opt {
            tx.query_all_raw(stmt).await
        } else {
            self.connection()?.query_all_raw(stmt).await
        }
        .map_err(DbError::Connection)?;
        let mut cols = Vec::with_capacity(rows.len());
        for r in rows {
            let name: Option<String> = r.try_get::<Option<String>>("", "name").ok().flatten();
            if let Some(n) = name {
                cols.push(n);
            }
        }
        Ok(cols)
    }

    /// 计算重试退避时间（retry feature 内部辅助方法）
    #[cfg(feature = "retry")]
    fn calculate_retry_backoff(
        policy: &crate::reliability::RetryPolicy,
        attempt: u32,
    ) -> std::time::Duration {
        use std::time::Duration;
        let base_ms = policy.initial_backoff_ms as f64;
        let backoff_ms = base_ms * policy.multiplier.powi(attempt as i32);
        let capped_ms = backoff_ms.min(policy.max_backoff_ms as f64);
        Duration::from_millis(capped_ms as u64)
    }

    /// 语句级缓存感知执行路径
    ///
    /// 启用池级 prepare 缓存时，先在 LRU 中登记/命中语句就绪状态
    /// （命中指标经 `DbPool::prepare_cache_stats` 观察），再走 `execute_raw`
    /// 的完整防御链执行；未启用缓存时等价于 `execute_raw`。
    #[cfg(feature = "prepare-cache")]
    pub async fn execute_cached(&self, sql: &str) -> DbResult<ExecResult> {
        let cache = self
            .pool_inner
            .prepare_cache
            .read()
            .expect("prepare_cache lock poisoned")
            .clone();
        match cache {
            Some(cache) => {
                let _hit = cache.get_or_prepare(sql, |_| ());
                self.execute_raw(sql).await
            }
            None => self.execute_raw(sql).await,
        }
    }

    /// 统一 DDL 守卫漏斗 —— 全部 DDL 执行路径共用单一入口
    ///
    /// 消费 [`DdlGuardPolicy`] 端口（白名单/干跑/审计统一；默认内置白名单守卫，
    /// 可经 `DbPool::set_ddl_guard` / `DbPoolBuilder::ddl_guard` 注入自定义策略），
    /// 决策到 `DbError` 的映射与既有分散检查保持一致：
    /// - `Allowed` → 放行
    /// - `Forbidden(reason)` → `DbError::Permission`
    /// - `ParseError(error)` → `DbError::Config`
    #[cfg(feature = "sql-parser")]
    pub(crate) fn enforce_ddl_guard(&self, sql: &str) -> DbResult<()> {
        let injected = self
            .pool_inner
            .ddl_guard
            .read()
            .expect("ddl_guard lock poisoned")
            .clone();
        let result = match injected {
            Some(guard) => run_ddl_policy(guard.as_ref(), sql)?,
            None => {
                let guard = DdlGuard::new();
                run_ddl_policy(&guard, sql)?
            }
        };
        match result {
            DdlValidationResult::Allowed => Ok(()),
            DdlValidationResult::Forbidden(reason) => Err(DbError::Permission(i18n::t(
                "session-ddl-not-allowed",
                &[("reason", reason.to_string())],
            ))),
            DdlValidationResult::ParseError(error) => Err(DbError::Config(i18n::t(
                "session-ddl-parse-failed",
                &[("error", error.to_string())],
            ))),
        }
    }

    /// 执行 DDL 操作（允许创建表、删除表等操作）
    ///
    /// 此方法专门用于执行 DDL 操作，绕过常规的 DDL 检查。
    /// 仅用于测试和迁移场景，生产环境应谨慎使用。
    ///
    /// # Arguments
    ///
    /// * `sql` - 要执行的 DDL SQL 语句
    ///
    /// # Returns
    ///
    /// 执行结果
    ///
    /// # Note
    ///
    /// 此方法只允许管理员角色执行，用于测试和迁移场景。
    pub async fn execute_raw_ddl(&self, sql: &str) -> DbResult<ExecResult> {
        // 检查角色白名单（只允许管理员角色执行 DDL）
        if self.role != self.pool_inner.admin_role {
            return Err(DbError::Permission(format!(
                "DDL operations are only allowed for admin role. Current role: '{}', Admin role: '{}'",
                self.role, self.pool_inner.admin_role
            )));
        }

        // DDL 安全验证
        #[cfg(feature = "sql-parser")]
        self.enforce_ddl_guard(sql)?;

        // 执行 SQL
        let conn = self.connection()?;
        conn.execute_unprepared(sql)
            .await
            .map_err(DbError::Connection)
    }

    /// 执行 SQL（带权限检查和操作类型）
    pub async fn execute(&self, sql: &str) -> DbResult<ExecResult> {
        // DDL 检查（sql-parser 启用时）
        #[cfg(feature = "sql-parser")]
        check_ddl_operation(sql)?;

        #[cfg(feature = "permission")]
        {
            let start = Instant::now();
            // 解析 SQL 操作类型和表名
            let parsed = parse_sql_for_permission(sql).await?;
            match parsed {
                Some((table_name, action)) => {
                    // 表名有效性检查
                    if table_name.is_empty() || is_invalid_table_name(&table_name) {
                        return Err(DbError::Permission(
                            "Failed to extract table name for permission checking".to_string(),
                        ));
                    }
                    // 权限检查（含 admin 角色绕过）
                    self.check_permission(&table_name, &action).await?;
                    // 执行 SQL
                    let result = self.execute_raw(sql).await?;
                    // 记录指标并标记写操作
                    self.record_metrics_and_mark_write(&action, start).await;
                    Ok(result)
                }
                None => {
                    // 解析失败或不支持的语句类型，直接执行
                    // （仅 sql-parser 启用时可能出现 None）
                    let result = self.execute_raw(sql).await?;
                    Ok(result)
                }
            }
        }

        #[cfg(not(feature = "permission"))]
        {
            // 执行 SQL
            let result = self.execute_raw(sql).await?;
            Ok(result)
        }
    }

    /// 执行 SQL 并指定操作类型
    #[cfg(feature = "permission")]
    pub async fn execute_with_operation(
        &self,
        sql: &str,
        operation: &PermissionAction,
    ) -> DbResult<ExecResult> {
        let start = Instant::now();

        #[cfg(feature = "sql-parser")]
        {
            // 检查是否为 DDL 操作
            if is_ddl_operation(sql) {
                return Err(DbError::Permission(
                    "DDL operations are not allowed in this context".to_string(),
                ));
            }
        }

        // 提取表名（vuln-0003 修复：使用 SqlParser AST 解析替代朴素字符串匹配）
        let table_name: String = extract_table_name_via_parser(sql).await.unwrap_or_default();

        // 检查权限
        #[cfg(feature = "permission")]
        {
            if !table_name.is_empty()
                && !self
                    .permission_ctx
                    .check_table_access(&table_name, operation)
                    .await
            {
                return Err(permission_denied(operation, &table_name));
            }
        }

        // 执行 SQL
        let result = self.execute_raw(sql).await?;

        // LD-2 修复：复用 record_metrics_and_mark_write 统一 metrics 记录与 mark_write 逻辑，
        // 消除与 execute() 方法中重复的手动展开（record_query_metrics + is_write_action + mark_write）
        self.record_metrics_and_mark_write(operation, start).await;

        Ok(result)
    }

    /// 批量执行 SQL
    ///
    /// # Arguments
    ///
    /// * `sqls` - 要执行的 SQL 语句列表
    ///
    /// # Returns
    ///
    /// 返回执行结果列表
    pub async fn batch_execute(&self, sqls: Vec<&str>) -> DbResult<Vec<DbResult<ExecResult>>> {
        let mut results = Vec::new();

        for sql in sqls {
            let result = self.execute(sql).await;
            results.push(result);
        }

        Ok(results)
    }

    /// 批量执行（带事务）
    ///
    /// 所有操作在一个事务中执行，任一失败则全部回滚
    ///
    /// # Arguments
    ///
    /// * `sqls` - 要执行的 SQL 语句列表
    ///
    /// # Returns
    ///
    /// 返回执行结果列表，任一失败则返回错误
    pub async fn batch_execute_in_transaction(&self, sqls: Vec<&str>) -> DbResult<Vec<ExecResult>> {
        self.begin_transaction().await?;

        // MD-3 修复：用 async block + ? 简化事务执行，消除 last_error + break 命令式风格
        let result: DbResult<Vec<ExecResult>> = async {
            let mut results = Vec::with_capacity(sqls.len());
            for sql in sqls {
                results.push(self.execute_raw(sql).await?);
            }
            Ok(results)
        }
        .await;

        match result {
            Ok(results) => {
                self.commit().await?;
                Ok(results)
            }
            Err(e) => {
                // LD-4 修复：保留原始错误上下文，rollback 失败时组合错误消息（不覆盖原始错误）
                match self.rollback().await {
                    Ok(()) => Err(e),
                    Err(rollback_err) => Err(DbError::Transaction(format!(
                        "batch failed: {}; rollback also failed: {}",
                        e, rollback_err
                    ))),
                }
            }
        }
    }
}

/// sqlite：单行 → JSON 对象（逐列类型探测：i64 → f64 → String → Null）
#[cfg(all(feature = "sqlite", feature = "sql-parser"))]
fn sqlite_row_to_json(row: &sea_orm::QueryResult, cols: &[String]) -> serde_json::Value {
    let mut obj = serde_json::Map::with_capacity(cols.len());
    for name in cols {
        let v = if let Ok(Some(x)) = row.try_get::<Option<i64>>("", name) {
            serde_json::Value::from(x)
        } else if let Ok(Some(x)) = row.try_get::<Option<f64>>("", name) {
            serde_json::Value::from(x)
        } else if let Ok(Some(x)) = row.try_get::<Option<String>>("", name) {
            serde_json::Value::from(x)
        } else {
            serde_json::Value::Null
        };
        obj.insert(name.clone(), v);
    }
    serde_json::Value::Object(obj)
}

/// 对单一守卫策略执行校验并触发审计钩子
///
/// 校验失败（策略自身返回 `Err`）映射为 `DbError::Config`，与既有
/// `session-ddl-validation-error` 语义一致。
#[cfg(feature = "sql-parser")]
fn run_ddl_policy(policy: &dyn DdlGuardPolicy, sql: &str) -> DbResult<DdlValidationResult> {
    let result = policy.validate(sql).map_err(|error| {
        DbError::Config(i18n::t(
            "session-ddl-validation-error",
            &[("error", error.to_string())],
        ))
    })?;
    policy.audit(sql, &result);
    Ok(result)
}

// 消费方均在 all(sql-parser, permission) 门控的表名校验路径内。

/// 检查 DDL 操作，如果 SQL 为 DDL 则返回错误
///
/// 统一 execute / execute_raw / execute_with_operation 中的 DDL 拒绝逻辑。
#[cfg(feature = "sql-parser")]
fn check_ddl_operation(sql: &str) -> DbResult<()> {
    if is_ddl_operation(sql) {
        return Err(DbError::Permission(
            "DDL operations are not allowed in this context".to_string(),
        ));
    }
    Ok(())
}

/// 解析 SQL 操作类型和表名用于权限检查
///
/// 使用 SqlParser AST 解析提取表名和操作类型。
/// 返回 None 表示不支持的语句或解析失败（execute 会跳过权限检查直接执行）。
#[cfg(feature = "permission")]
async fn parse_sql_for_permission(sql: &str) -> DbResult<Option<(String, PermissionAction)>> {
    let parser = SqlParser::shared().await;
    match parser.parse_operation_async(sql).await {
        Ok(Some((table, action))) => Ok(Some((table, action))),
        Ok(None) => Ok(None),
        Err(_) => Ok(None),
    }
}
