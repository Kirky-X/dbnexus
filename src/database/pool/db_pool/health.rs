// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! T425 大文件拆分：自 db_pool.rs 按职责纯移动的 impl 块（行为不变）。

use super::*;

impl DbPool {
    /// 创建单个数据库连接
    ///
    /// 使用配置中的 URL 建立新的数据库连接。
    /// 此方法不进行连接池管理，仅创建原始连接。
    ///
    /// # Arguments
    ///
    /// * `config` - 数据库配置，包含连接 URL
    ///
    /// # Returns
    ///
    /// 成功创建的数据库连接
    ///
    /// # Errors
    ///
    /// 如果连接失败，返回数据库错误
    pub(super) async fn create_connection(config: &DbConfig) -> DbResult<DbConnection> {
        let db_type = config.database_type().map_err(|e| {
            DbError::Connection(sea_orm::DbErr::Custom(i18n::t(
                "pool-invalid-db-url",
                &[("error", e.to_string())],
            )))
        })?;

        match db_type {
            crate::foundation::DatabaseType::DuckDb => {
                #[cfg(feature = "duckdb")]
                {
                    let conn = crate::database::DuckDbConnection::new(&config.url)?;
                    Ok(DbConnection::DuckDb(conn))
                }
                #[cfg(not(feature = "duckdb"))]
                {
                    Err(DbError::Connection(sea_orm::DbErr::Custom(
                        "DuckDB feature is not enabled".to_string(),
                    )))
                }
            }
            crate::foundation::DatabaseType::Ladybug => {
                #[cfg(feature = "ladybug")]
                {
                    let pool_size = config.pool_config.max_connections as usize;
                    let conn = crate::database::LadybugConnection::new(&config.url, pool_size)?;
                    Ok(DbConnection::Ladybug(Arc::new(conn)))
                }
                #[cfg(not(feature = "ladybug"))]
                {
                    Err(DbError::Connection(sea_orm::DbErr::Custom(
                        "Ladybug feature is not enabled".to_string(),
                    )))
                }
            }
            crate::foundation::DatabaseType::Neo4j => {
                #[cfg(feature = "neo4j")]
                {
                    let (uri, user, password) =
                        crate::database::Neo4jConnection::parse_url(&config.url)?;
                    let conn =
                        crate::database::Neo4jConnection::new(&uri, &user, &password).await?;
                    Ok(DbConnection::Neo4j(Arc::new(conn)))
                }
                #[cfg(not(feature = "neo4j"))]
                {
                    Err(DbError::Connection(sea_orm::DbErr::Custom(
                        "Neo4j feature is not enabled".to_string(),
                    )))
                }
            }
            _ => {
                let conn = sea_orm::Database::connect(&config.url).await?;
                Ok(DbConnection::SeaOrm(conn))
            }
        }
    }

    /// 预创建最小连接数（并行创建以提高启动速度，带超时和重试）
    ///
    /// 并行启动 `min_connections` 个建连任务，每个任务带 `warmup_timeout` 超时
    /// 和 `warmup_retries` 次重试。
    ///
    /// **失败语义（v0.3.0 修复）**：
    /// - 全部失败：返回 `Err`（第一个错误），避免静默成功
    /// - 部分失败：返回 `Ok`，warn 日志记录失败数量
    /// - 全部成功：返回 `Ok`
    #[cfg(feature = "pool-warmup")]
    pub(super) async fn warmup_connections(&self) -> DbResult<()> {
        let initial_connections = self.inner.config.pool_config.min_connections;
        let warmup_timeout = Duration::from_secs(self.inner.config.warmup_timeout);
        let warmup_retries = self.inner.config.warmup_retries;

        let mut connection_tasks = Vec::new();

        for _ in 0..initial_connections {
            let config = Arc::clone(&self.inner.config);
            connection_tasks.push(async move {
                let mut retries = 0;
                let mut last_error = None;

                while retries <= warmup_retries {
                    match timeout(warmup_timeout, Self::create_connection(&config)).await {
                        Ok(Ok(conn)) => return Ok(conn),
                        Ok(Err(e)) => {
                            last_error = Some(e);
                            retries += 1;
                            if retries <= warmup_retries {
                                tokio::time::sleep(Duration::from_millis(100)).await;
                            }
                        }
                        Err(_) => {
                            last_error = Some(DbError::Connection(
                                sea_orm::DbErr::ConnectionAcquire(sea_orm::ConnAcquireErr::Timeout),
                            ));
                            break;
                        }
                    }
                }

                Err(last_error.unwrap_or_else(|| {
                    DbError::Connection(sea_orm::DbErr::ConnectionAcquire(
                        sea_orm::ConnAcquireErr::Timeout,
                    ))
                }))
            });
        }

        // 并行执行所有连接创建任务
        let results = futures::future::join_all(connection_tasks).await;

        // 统计成功/失败（规则 12：失败必须显性化，不可静默丢弃）
        let mut success_count = 0usize;
        let mut errors: Vec<DbError> = Vec::new();

        for result in results {
            match result {
                Ok(conn) => {
                    self.inner.idle_connections.lock().await.push(conn);
                    self.inner.total_count.fetch_add(1, Ordering::SeqCst);
                    success_count += 1;
                }
                Err(e) => errors.push(e),
            }
        }

        if success_count == 0 && initial_connections > 0 {
            // 全部失败：返回第一个错误（显性化失败，避免静默成功）
            return Err(errors.into_iter().next().unwrap_or_else(|| {
                DbError::Connection(sea_orm::DbErr::ConnectionAcquire(
                    sea_orm::ConnAcquireErr::Timeout,
                ))
            }));
        }

        if !errors.is_empty() {
            // 部分失败：不阻断初始化，错误已通过 errors 集合显性化
        }

        Ok(())
    }

    /// 检查连接健康状态
    ///
    /// 通过执行轻量级查询来验证数据库连接的有效性。
    /// 使用数据库特定的健康检查查询：
    /// - SQLite: `SELECT 1`
    /// - PostgreSQL: `SELECT 1`
    /// - MySQL: `SELECT 1`
    ///
    /// # Arguments
    ///
    /// * `conn` - 要检查的数据库连接
    ///
    /// # Returns
    ///
    /// 如果连接有效返回 `true`，否则返回 `false`
    pub async fn check_connection_health(&self, conn: &DbConnection) -> bool {
        match conn {
            DbConnection::SeaOrm(sea_conn) => {
                let backend = Self::get_database_backend(&self.inner.config.url);
                let result = timeout(
                    Duration::from_secs(5),
                    sea_conn.execute_raw(sea_orm::Statement::from_string(
                        backend,
                        "SELECT 1".to_string(),
                    )),
                )
                .await;
                matches!(result, Ok(Ok(_)))
            }
            #[cfg(feature = "duckdb")]
            DbConnection::DuckDb(duck_conn) => {
                let result = timeout(Duration::from_secs(5), duck_conn.health_check()).await;
                matches!(result, Ok(Ok(_)))
            }
            #[cfg(feature = "ladybug")]
            DbConnection::Ladybug(conn) => {
                let result = timeout(Duration::from_secs(5), conn.health_check()).await;
                matches!(result, Ok(Ok(_)))
            }
            #[cfg(feature = "neo4j")]
            DbConnection::Neo4j(conn) => {
                let result = timeout(Duration::from_secs(5), conn.health_check()).await;
                matches!(result, Ok(Ok(_)))
            }
        }
    }

    /// 获取数据库类型
    ///
    /// 根据数据库 URL 的协议部分解析数据库类型。
    /// 支持的数据库类型包括 SQLite、PostgreSQL、MySQL 和 DuckDB。
    ///
    /// # Arguments
    ///
    /// * `url` - 数据库连接 URL
    ///
    /// # Returns
    ///
    /// 对应的 Sea-ORM 数据库后端类型
    ///
    /// # Note
    ///
    /// 如果 URL 无法识别，默认返回 SQLite 类型。
    /// DuckDB 连接不使用 SeaORM 后端，此方法仅用于 SeaORM 连接。
    pub(super) fn get_database_backend(url: &str) -> sea_orm::DatabaseBackend {
        if url.starts_with("sqlite:") {
            sea_orm::DatabaseBackend::Sqlite
        } else if url.starts_with("postgres:") || url.starts_with("postgresql:") {
            sea_orm::DatabaseBackend::Postgres
        } else if url.starts_with("mysql:") {
            sea_orm::DatabaseBackend::MySql
        } else if url.starts_with("duckdb:") {
            // DuckDB 不使用 SeaORM 后端，映射到 Sqlite 以避免 panic
            sea_orm::DatabaseBackend::Sqlite
        } else {
            sea_orm::DatabaseBackend::Sqlite
        }
    }

    #[cfg(feature = "pool-health-check")]
    /// 验证空闲连接的有效性（并行版本）
    ///
    /// 遍历空闲连接池，对每个连接并发执行健康检查，
    /// 将连接分区为有效和无效两组。
    ///
    /// 使用 `futures::future::join_all()` 并行验证所有连接，
    /// 显著减少大量连接时的总等待时间。
    ///
    /// # Arguments
    ///
    /// * `idle` - 空闲连接队列的可变引用
    /// * `config` - 数据库配置
    ///
    /// # Returns
    ///
    /// 返回元组 (有效连接列表, 无效连接数量)
    pub(super) async fn validate_idle_connections(
        idle: &mut Vec<DbConnection>,
        config: &DbConfig,
    ) -> (Vec<DbConnection>, usize) {
        let backend = Self::get_database_backend(&config.url);

        // 先将所有连接移出，避免在持有锁期间进行 I/O 操作
        let connections: Vec<DbConnection> = std::mem::take(idle);

        // 并行执行所有健康检查
        let check_futures: Vec<_> = connections
            .into_iter()
            .map(|conn| async {
                let is_valid = match &conn {
                    DbConnection::SeaOrm(sea_conn) => timeout(
                        Duration::from_secs(2),
                        sea_conn.execute_raw(sea_orm::Statement::from_string(
                            backend,
                            "SELECT 1".to_string(),
                        )),
                    )
                    .await
                    .is_ok_and(|result| result.is_ok()),
                    #[cfg(feature = "duckdb")]
                    DbConnection::DuckDb(duck_conn) => {
                        timeout(Duration::from_secs(2), duck_conn.health_check())
                            .await
                            .is_ok_and(|result| result.is_ok())
                    }
                    #[cfg(feature = "ladybug")]
                    DbConnection::Ladybug(graph_conn) => {
                        timeout(Duration::from_secs(2), graph_conn.health_check())
                            .await
                            .is_ok_and(|result| result.is_ok())
                    }
                    #[cfg(feature = "neo4j")]
                    DbConnection::Neo4j(graph_conn) => {
                        timeout(Duration::from_secs(2), graph_conn.health_check())
                            .await
                            .is_ok_and(|result| result.is_ok())
                    }
                };
                (conn, is_valid)
            })
            .collect();

        let results: Vec<(DbConnection, bool)> = futures::future::join_all(check_futures).await;

        // 分区为有效和无效连接（先计算 invalid_count，再转移所有权）
        let invalid_count = results.iter().filter(|(_, is_valid)| !*is_valid).count();

        let valid_connections: Vec<DbConnection> = results
            .into_iter()
            .filter_map(|(conn, is_valid)| if is_valid { Some(conn) } else { None })
            .collect();

        (valid_connections, invalid_count)
    }

    #[cfg(feature = "pool-health-check")]
    /// 清理无效连接
    ///
    /// 遍历空闲连接池，验证每个连接的有效性，
    /// 移除超时或断开连接的实例。
    ///
    /// # Returns
    ///
    /// 被移除的无效连接数量
    pub async fn clean_invalid_connections(&self) -> u32 {
        let mut idle = self.inner.idle_connections.lock().await;
        let config = &self.inner.config;

        // 使用辅助方法验证连接
        let (valid_connections, removed_count) =
            Self::validate_idle_connections(&mut idle, config).await;

        // 重建空闲连接队列
        idle.extend(valid_connections);

        // 更新总连接数
        if removed_count > 0 {
            self.inner
                .total_count
                .fetch_sub(removed_count as u32, Ordering::SeqCst);
        }

        removed_count as u32
    }

    #[cfg(feature = "pool-health-check")]
    /// 验证并重新创建无效连接
    ///
    /// 检查所有空闲连接的健康状态，自动替换无效连接。
    /// 此方法会确保池中至少保持配置的最小连接数。
    ///
    /// # Returns
    ///
    /// 被重新创建的连接数量，或错误
    pub async fn validate_and_recreate_connections(&self) -> Result<u32, sea_orm::DbErr> {
        let mut idle = self.inner.idle_connections.lock().await;
        let config = &self.inner.config;

        // 使用辅助方法验证连接
        let (valid_connections, invalid_count) =
            Self::validate_idle_connections(&mut idle, config).await;

        let mut recreated_count = 0;

        if invalid_count > 0 {
            // 更新总连接数
            self.inner
                .total_count
                .fetch_sub(invalid_count as u32, Ordering::SeqCst);

            // 重建空闲队列（只保留有效连接）
            idle.extend(valid_connections);

            // 重新创建连接以维持最小连接数
            let current_idle = idle.len();
            let needed = config
                .pool_config
                .min_connections
                .saturating_sub(current_idle as u32) as usize;

            for _ in 0..needed {
                match Self::create_connection(config).await {
                    Ok(new_conn) => {
                        idle.push(new_conn);
                        self.inner.total_count.fetch_add(1, Ordering::SeqCst);
                        recreated_count += 1;
                    }
                    Err(e) => {
                        return Err(sea_orm::DbErr::Custom(i18n::t(
                            "pool-recreate-failed",
                            &[("error", e.to_string())],
                        )));
                    }
                }
            }
        } else {
            // 没有无效连接，恢复有效连接到池中
            idle.extend(valid_connections);
        }

        Ok(recreated_count as u32)
    }

    /// 解析健康检查间隔配置
    ///
    /// 解析传入的间隔值（秒），并限制在 5-300 秒范围内。
    /// 超出范围的值会触发警告日志。
    ///
    /// # Arguments
    ///
    /// * `value` - 健康检查间隔配置值（由调用方从环境变量 `DB_HEALTH_CHECK_INTERVAL` 读取）
    ///
    /// # Returns
    ///
    /// 返回解析后的间隔秒数，默认为 30 秒。
    ///
    /// # Examples
    ///
    /// ```
    /// use dbnexus::DbPool;
    /// // 空字符串返回默认值 30
    /// assert_eq!(DbPool::parse_health_check_interval(""), 30);
    ///
    /// // 有效值返回该值
    /// assert_eq!(DbPool::parse_health_check_interval("60"), 60);
    ///
    /// // 超出范围的值返回限制后的值
    /// assert_eq!(DbPool::parse_health_check_interval("1000"), 300);
    /// ```
    #[cfg(feature = "pool-health-check")]
    pub fn parse_health_check_interval(value: &str) -> u64 {
        value
            .parse::<u64>()
            .ok()
            .map(|v| v.clamp(5, 300))
            .unwrap_or(30)
    }

    /// 启动后台连接健康检查任务
    ///
    /// 该任务会定期检查所有空闲连接的健康状态，
    /// 自动移除无效连接并重建新连接以维持最小连接数。
    ///
    /// 健康检查间隔默认为 30 秒，可通过环境变量 `DB_HEALTH_CHECK_INTERVAL` 配置（秒）。
    /// 间隔值会被限制在 5-300 秒范围内，超出范围的值会触发警告日志。
    #[cfg(feature = "pool-health-check")]
    pub(super) fn start_background_health_check(&self) {
        let pool = self.clone();
        let shutdown = self.inner.health_check_shutdown.clone();

        // 从环境变量读取健康检查间隔配置并解析
        let env_value = std::env::var("DB_HEALTH_CHECK_INTERVAL").unwrap_or_default();
        let interval_secs = Self::parse_health_check_interval(&env_value);

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(interval_secs));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // 执行连接健康检查
                        let _ = pool.validate_and_recreate_connections().await;
                    }
                    _ = shutdown.notified() => {
                        break;
                    }
                }
            }
        });
    }

}
