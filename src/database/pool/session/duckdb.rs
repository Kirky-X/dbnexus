// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 大文件拆分：自 session.rs 按职责纯移动的 impl 块（行为不变）。

use super::*;

impl Session {
    /// DuckDB 路径统一安全门（DDL 拦截 + SQL 注入检测 + 权限校验）
    ///
    /// `execute_duckdb` / `execute_duckdb_raw` 及其参数化变体的共享防御链：
    /// - 非 DML 语句（DDL）→ 仅 admin 角色通过 DdlGuard AST 验证后放行
    /// - DML/查询 → admin 角色直接执行；非 admin 角色需通过表级权限检查
    /// - 无法解析的语句 → admin 放行（支持 `SELECT 1` 等健康检查），非 admin 拒绝
    ///
    /// 返回 `Ok(())` 表示语句已通过安全门，调用方可继续执行。
    ///
    /// 门控含 `duckdb`：本方法的全部调用者（`execute_duckdb*` 系列）均为
    /// `#[cfg(feature = "duckdb")]`。若只门控 `sql-parser`，则
    /// "sql-parser 开 + duckdb 关"组合下本方法成为死代码（触发
    /// `dead_code` 警告，下游 CI 的 `-D warnings` 会失败）。
    #[cfg(all(feature = "sql-parser", feature = "duckdb"))]
    async fn duckdb_security_gate(&self, sql: &str) -> DbResult<()> {
        if is_ddl_operation(sql) {
            // DDL：仅 admin 角色（对齐 execute_raw_ddl 的角色白名单）
            if !self.is_admin {
                return Err(DbError::Permission(format!(
                    "DDL operations are only allowed for admin role in DuckDB context. Current role: '{}', Admin role: '{}'",
                    self.role, self.pool_inner.admin_role
                )));
            }
            // 统一守卫漏斗（白名单/干跑/审计经 DdlGuardPolicy 端口）
            return self.enforce_ddl_guard(sql);
        }

        // 非 DDL：表级权限检查
        #[cfg(feature = "permission")]
        {
            let parser = SqlParser::shared().await;
            match parser.parse_operation_async(sql).await {
                Ok(Some((table_name, action))) => {
                    if table_name.is_empty() || is_invalid_table_name(&table_name) {
                        return Err(DbError::Permission(
                            "Failed to extract table name for permission checking".to_string(),
                        ));
                    }
                    if !self.is_admin {
                        super::check_table_or_error(&self.permission_ctx, &table_name, &action)
                            .await?;
                    }
                }
                Ok(None) => {
                    // admin role 对无法解析的语句直接执行（支持 SELECT 1 等无表名健康检查）；
                    // 非 admin role 拒绝（安全默认：无法解析则无法做权限检查）。
                    if !self.is_admin {
                        return Err(DbError::Permission(
                            "SQL statement requires a valid table name for permission checking"
                                .to_string(),
                        ));
                    }
                }
                Err(_) => {
                    // 解析失败（如 SQL 含 INFORMATION_SCHEMA 被注入检测拦截）：
                    // admin role 放行（对齐 Ok(None) 路径——admin 拥有完全控制权）；
                    // 非 admin role 拒绝（安全默认：无法解析则无法做权限检查）。
                    if !self.is_admin {
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
        #[cfg(not(feature = "permission"))]
        {
            let _ = sql;
        }
        Ok(())
    }

    /// 执行参数化 DuckDB 查询（仅 DuckDB 连接可用）
    ///
    /// 与 [`Self::execute_duckdb`] 相同的安全防御链，但通过 prepared statement
    /// 传递绑定参数——数据库不会将参数值解析为 SQL 代码，从根本上防止 SQL 注入。
    /// **所有携带外部输入的 DuckDB 查询必须走本方法**，禁止 format!/拼接组装 SQL。
    ///
    /// # 参数
    ///
    /// * `sql` - 含 `?` 占位符的 SELECT 语句
    /// * `params` - 按占位符顺序排列的绑定值
    ///
    /// # 返回
    ///
    /// 查询结果行列表
    #[cfg(feature = "duckdb")]
    pub async fn execute_duckdb_with_params(
        &self,
        sql: &str,
        params: Vec<crate::database::DuckValue>,
    ) -> DbResult<Vec<crate::database::DuckDbRow>> {
        #[cfg(feature = "sql-parser")]
        self.duckdb_security_gate(sql).await?;

        #[cfg(not(feature = "sql-parser"))]
        {
            let _ = sql;
            return Err(DbError::Permission(
                "execute_duckdb_with_params requires the sql-parser feature to be enabled for security checks"
                    .to_string(),
            ));
        }

        #[cfg(feature = "sql-parser")]
        {
            let conn = self
                .connection
                .as_ref()
                .ok_or_else(|| DbError::Config("Connection not available".to_string()))?;
            let duck_conn = conn.as_duckdb()?;
            duck_conn.query_with_params(sql, params).await
        }
    }

    /// 执行参数化 DuckDB DML 语句（仅 DuckDB 连接可用）
    ///
    /// 与 [`Self::execute_duckdb_raw`] 相同的安全防御链，但通过 prepared statement
    /// 传递绑定参数，从根本上防止 SQL 注入。**所有携带外部输入的 INSERT/UPDATE/DELETE
    /// 必须走本方法**，禁止 format!/拼接组装 SQL。DDL 不携带参数，继续用
    /// [`Self::execute_duckdb_raw`]。
    ///
    /// # 参数
    ///
    /// * `sql` - 含 `?` 占位符的 DML 语句
    /// * `params` - 按占位符顺序排列的绑定值
    ///
    /// # 返回
    ///
    /// 受影响的行数信息
    #[cfg(feature = "duckdb")]
    pub async fn execute_duckdb_raw_with_params(
        &self,
        sql: &str,
        params: Vec<crate::database::DuckValue>,
    ) -> DbResult<crate::database::DuckDbExecResult> {
        #[cfg(feature = "sql-parser")]
        self.duckdb_security_gate(sql).await?;

        #[cfg(not(feature = "sql-parser"))]
        {
            let _ = sql;
            return Err(DbError::Permission(
                "execute_duckdb_raw_with_params requires the sql-parser feature to be enabled for security checks"
                    .to_string(),
            ));
        }

        #[cfg(feature = "sql-parser")]
        {
            let conn = self
                .connection
                .as_ref()
                .ok_or_else(|| DbError::Config("Connection not available".to_string()))?;
            let duck_conn = conn.as_duckdb()?;
            duck_conn.execute_with_params(sql, params).await
        }
    }

    /// 在单个事务中原子执行多条参数化 DuckDB 语句（仅 DuckDB 连接可用）
    ///
    /// DuckDB 路径的 Session 级事务由本方法提供（`begin_transaction` 仅支持 SeaORM 后端）：
    /// 在**同一条**底层连接上按顺序执行 `BEGIN → 语句序列 → COMMIT`，
    /// 任一语句失败整体 ROLLBACK，保证多语句原子性（如级联删除）。
    /// 每条语句均先通过 [`Self::execute_duckdb_raw_with_params`] 同款安全门。
    ///
    /// # 参数
    ///
    /// * `statements` - `(sql, params)` 有序序列，全部在同一事务内执行
    ///
    /// # 返回
    ///
    /// 各语句的执行结果（顺序与输入一致）；任一失败即整体回滚并返回错误
    #[cfg(feature = "duckdb")]
    pub async fn execute_duckdb_transaction(
        &self,
        statements: Vec<(String, Vec<crate::database::DuckValue>)>,
    ) -> DbResult<Vec<crate::database::DuckDbExecResult>> {
        #[cfg(feature = "sql-parser")]
        {
            for (sql, _) in &statements {
                self.duckdb_security_gate(sql).await?;
            }
        }

        #[cfg(not(feature = "sql-parser"))]
        {
            let _ = &statements;
            return Err(DbError::Permission(
                "execute_duckdb_transaction requires the sql-parser feature to be enabled for security checks"
                    .to_string(),
            ));
        }

        #[cfg(feature = "sql-parser")]
        {
            let conn = self
                .connection
                .as_ref()
                .ok_or_else(|| DbError::Config("Connection not available".to_string()))?;
            let duck_conn = conn.as_duckdb()?;
            duck_conn.execute_transaction(statements).await
        }
    }

    /// 执行 DuckDB 查询（仅 DuckDB 连接可用）
    ///
    /// 当 Session 持有 DuckDB 连接时，通过此方法执行 SQL 查询并返回结果行。
    /// 若持有 SeaORM 连接则返回错误。
    ///
    /// # 参数
    ///
    /// * `sql` - 要执行的 SQL 查询语句（SELECT）
    ///
    /// # 返回
    ///
    /// 查询结果行列表
    #[cfg(feature = "duckdb")]
    pub async fn execute_duckdb(&self, sql: &str) -> DbResult<Vec<crate::database::DuckDbRow>> {
        // 安全检查：与 execute_raw 一致的防御链（DDL 拦截 + SQL 注入检测 + 权限校验）
        #[cfg(feature = "sql-parser")]
        {
            if is_ddl_operation(sql) {
                return Err(DbError::Permission(
                    "DDL operations are not allowed in DuckDB query context".to_string(),
                ));
            }
        }

        #[cfg(not(feature = "sql-parser"))]
        {
            let _ = sql;
            Err(DbError::Permission(
                "execute_duckdb requires the sql-parser feature to be enabled for security checks"
                    .to_string(),
            ))
        }

        #[cfg(feature = "sql-parser")]
        {
            #[cfg(all(feature = "sql-parser", feature = "permission"))]
            {
                let parser = SqlParser::shared().await;
                match parser.parse_operation_async(sql).await {
                    Ok(Some((table_name, action))) => {
                        if table_name.is_empty() || is_invalid_table_name(&table_name) {
                            return Err(DbError::Permission(
                                "Failed to extract table name for permission checking".to_string(),
                            ));
                        }
                        if !self.is_admin {
                            super::check_table_or_error(&self.permission_ctx, &table_name, &action)
                                .await?;
                        }
                    }
                    Ok(None) => {
                        // admin role 对无法解析的语句直接执行（对齐 execute 的 None 路径），
                        // 支持 SELECT 1 / SELECT 1 AS health 等无表名健康检查查询；
                        // 非 admin role 拒绝（安全默认：无法解析则无法做权限检查）。
                        if !self.is_admin {
                            return Err(DbError::Permission(
                                "SQL statement requires a valid table name for permission checking"
                                    .to_string(),
                            ));
                        }
                    }
                    Err(_) => {
                        // 解析失败：admin role 放行（对齐 Ok(None) 路径），非 admin 拒绝。
                        if !self.is_admin {
                            return Err(DbError::Permission(
                                "Failed to parse SQL statement for permission checking".to_string(),
                            ));
                        }
                    }
                }
            }

            let conn = self
                .connection
                .as_ref()
                .ok_or_else(|| DbError::Config("Connection not available".to_string()))?;
            let duck_conn = conn.as_duckdb()?;
            duck_conn.query(sql).await
        }
    }

    /// 执行 DuckDB DDL/DML 语句（仅 DuckDB 连接可用）
    ///
    /// 当 Session 持有 DuckDB 连接时，通过此方法执行 CREATE/INSERT/UPDATE/DELETE 等语句。
    /// 若持有 SeaORM 连接则返回错误。
    ///
    /// # 参数
    ///
    /// * `sql` - 要执行的 SQL 语句（DDL/DML）
    ///
    /// # 返回
    ///
    /// 受影响的行数信息
    #[cfg(feature = "duckdb")]
    pub async fn execute_duckdb_raw(
        &self,
        sql: &str,
    ) -> DbResult<crate::database::DuckDbExecResult> {
        // 安全检查：与 execute_raw_ddl 对齐 —— admin role 通过 DdlGuard 验证后允许 DDL，
        // 非 admin role 拒绝 DDL。DuckDB 是分析型数据库，admin 需要能创建表/视图，
        // 与 SeaORM 路径的 execute_raw_ddl 行为保持一致。
        #[cfg(feature = "sql-parser")]
        {
            if is_ddl_operation(sql) {
                if self.is_admin {
                    // 统一守卫漏斗 —— admin role 通过守卫验证后直接执行，
                    // 不再走 parse_operation 权限检查（DDL 语句无法被
                    // parse_operation_async 正确解析，会返回 Err）
                    self.enforce_ddl_guard(sql)?;
                    let conn = self
                        .connection
                        .as_ref()
                        .ok_or_else(|| DbError::Config("Connection not available".to_string()))?;
                    let duck_conn = conn.as_duckdb()?;
                    return duck_conn.execute(sql).await;
                } else {
                    return Err(DbError::Permission(format!(
                        "DDL operations are only allowed for admin role in DuckDB context. Current role: '{}', Admin role: '{}'",
                        self.role, self.pool_inner.admin_role
                    )));
                }
            }
        }

        #[cfg(not(feature = "sql-parser"))]
        {
            let _ = sql;
            Err(DbError::Permission(
                "execute_duckdb_raw requires the sql-parser feature to be enabled for security checks".to_string(),
            ))
        }

        #[cfg(feature = "sql-parser")]
        {
            #[cfg(all(feature = "sql-parser", feature = "permission"))]
            {
                let parser = SqlParser::shared().await;
                match parser.parse_operation_async(sql).await {
                    Ok(Some((table_name, action))) => {
                        if table_name.is_empty() || is_invalid_table_name(&table_name) {
                            return Err(DbError::Permission(
                                "Failed to extract table name for permission checking".to_string(),
                            ));
                        }
                        if !self.is_admin {
                            super::check_table_or_error(&self.permission_ctx, &table_name, &action)
                                .await?;
                        }
                    }
                    Ok(None) => {
                        // admin role 对无法解析的语句直接执行（对齐 execute 的 None 路径），
                        // 非 admin role 拒绝（安全默认：无法解析则无法做权限检查）。
                        if !self.is_admin {
                            return Err(DbError::Permission(
                                "SQL statement requires a valid table name for permission checking"
                                    .to_string(),
                            ));
                        }
                    }
                    Err(_) => {
                        // 解析失败：admin role 放行（对齐 Ok(None) 路径），非 admin 拒绝。
                        if !self.is_admin {
                            return Err(DbError::Permission(
                                "Failed to parse SQL statement for permission checking".to_string(),
                            ));
                        }
                    }
                }
            }

            let conn = self
                .connection
                .as_ref()
                .ok_or_else(|| DbError::Config("Connection not available".to_string()))?;
            let duck_conn = conn.as_duckdb()?;
            duck_conn.execute(sql).await
        }
    }
}
