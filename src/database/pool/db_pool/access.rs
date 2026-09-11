// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! T425 大文件拆分：自 db_pool.rs 按职责纯移动的 impl 块（行为不变）。

use super::*;

impl DbPool {
    /// 设置缓存提供者（DI 注入点）
    ///
    /// 允许外部注入缓存实现，覆盖默认的内置缓存。
    /// 仅在 `cache` 特性启用时可用。
    #[cfg(any(feature = "cache", feature = "oxcache-integration"))]
    pub fn set_cache_provider(
        &mut self,
        provider: Arc<dyn crate::domain::DbCacheProvider + Send + Sync>,
    ) {
        // ArcSwapOption::store 原子替换，无锁且线程安全。
        // Session 持有的 Arc<DbPoolInner> 读端通过 load() 自动看到最新值。
        self.inner.cache_provider.store(Some(Arc::new(provider)));
    }

    /// 注入统一 DDL 守卫策略（T416：白名单/干跑/审计经 `DdlGuardPolicy` 端口）
    ///
    /// 注入后全部 DDL 执行路径（`execute_raw_ddl` / DuckDB 安全门）经该策略校验；
    /// 未注入时使用内置 AST 白名单守卫。
    #[cfg(feature = "sql-parser")]
    pub fn set_ddl_guard(&self, guard: std::sync::Arc<dyn DdlGuardPolicy>) {
        *self
            .inner
            .ddl_guard
            .write()
            .expect("ddl_guard lock poisoned") = Some(guard);
    }

    /// 启用语句级 prepared statement LRU 缓存（T420）
    ///
    /// 启用后 `Session::execute_cached` 经缓存记录语句就绪状态与命中指标；
    /// 未启用时该路径等价于 `execute_raw`。
    #[cfg(feature = "prepare-cache")]
    pub fn enable_prepare_cache(&self, capacity: usize) {
        *self
            .inner
            .prepare_cache
            .write()
            .expect("prepare_cache lock poisoned") =
            Some(std::sync::Arc::new(
                crate::database::pool::prepare_cache::PoolPrepareCache::new(capacity),
            ));
    }

    /// prepare 缓存统计快照（未启用缓存时返回 None）
    #[cfg(feature = "prepare-cache")]
    pub fn prepare_cache_stats(&self) -> Option<crate::database::pool::PrepareCacheStats> {
        self.inner
            .prepare_cache
            .read()
            .expect("prepare_cache lock poisoned")
            .as_ref()
            .map(|cache| cache.stats())
    }

    /// 获取缓存提供者引用
    ///
    /// 返回当前注入的缓存提供者，如果未注入则返回 `None`。
    #[cfg(any(feature = "cache", feature = "oxcache-integration"))]
    pub fn cache_provider(
        &self,
    ) -> Option<Arc<Arc<dyn crate::domain::DbCacheProvider + Send + Sync>>> {
        self.inner.cache_provider.load().clone()
    }

    /// 从池中获取 Session（带 metrics 支持）
    ///
    /// # Arguments
    ///
    /// * `role` - 角色名称，必须在权限配置中定义
    ///
    /// # Errors
    ///
    /// 如果角色未在权限配置中定义，返回错误
    ///
    /// # 安全警告
    ///
    /// 此方法接受角色字符串参数，调用者应确保：
    /// - 已通过其他方式验证用户身份（如JWT Token、API Key等）
    /// - 角色字符串来自可信来源，而非直接的用户输入
    /// - 在生产环境中不要硬编码角色字符串
    /// - 建议结合 `dbnexus::authentication::AuthenticationManager` 使用
    ///
    /// # 示例
    ///
    /// ```rust,no_run,ignore
    /// use dbnexus::{DbPool, authentication::AuthenticationManager};
    ///
    /// // 安全用法：先验证Token，再从Token中提取角色
    /// let auth_manager = AuthenticationManager::new(&jwt_secret)?;
    /// let claims = auth_manager.verify_token(token)?;
    /// let session = pool.get_session(&claims.role).await?;
    ///
    /// // 不安全用法：直接使用用户输入
    /// // let session = pool.get_session(user_input_role).await?; // 不要这样做！
    /// ```
    ///
    pub async fn get_session(&self, role: &str) -> DbResult<Session> {
        // 验证角色名称
        #[cfg(feature = "permission")]
        self.validate_role_name(role).await?;

        let connection = self.acquire_connection().await?;
        let session = Session::new(connection, self.inner.clone(), role.to_string());

        Ok(session)
    }

    /// 统一行查询 API（T401）：以指定角色执行 SELECT 并返回数据行
    ///
    /// 内部经 `get_session` → `Session::query_rows`，共享解析/权限/慢查询口径；
    /// 非 SELECT 或未授权表将返回权限错误。
    pub async fn query_rows(&self, sql: &str, role: &str) -> DbResult<Vec<serde_json::Value>> {
        let session = self.get_session(role).await?;
        session.query_rows(sql).await
    }

    /// T403/T404：注入数据保护配置（字段脱敏 + RLS 谓词）
    #[cfg(feature = "data-protection")]
    pub async fn set_data_protection(
        &self,
        dp: crate::access::data_protection::DataProtection,
    ) {
        *self.inner.data_protection.write().await = dp;
    }

    /// T405 前置：运行时替换权限配置（角色策略缓存同步换装）
    ///
    /// 供配置热重载（confers watch）与测试注入使用。
    #[cfg(feature = "permission")]
    pub async fn set_permission_config(
        &self,
        config: crate::access::permission::PermissionConfig,
    ) -> DbResult<()> {
        for (role_name, policy) in &config.roles {
            let _ = self.inner.policy_cache.set(role_name, policy).await;
        }
        self.inner.permission_config.store(Some(Arc::new(config)));
        Ok(())
    }

    /// 验证角色名称是否在权限配置中定义
    ///
    /// 仅在权限配置文件存在且成功加载时验证角色。
    /// 如果没有配置权限文件，使用安全默认策略（仅允许 admin/system 角色）。
    ///
    /// **显性化（v0.3.0 修复）**：未配置权限文件时输出 warn 日志，明确说明
    /// 正在使用安全默认策略，提醒用户配置权限文件以启用完整角色验证。
    #[cfg(feature = "permission")]
    pub(super) async fn validate_role_name(&self, role: &str) -> DbResult<()> {
        // 无锁读取权限配置（ArcSwap COW — 读取是完全无锁的 CAS 操作）
        let permission_config = self.inner.permission_config.load();

        // 检查权限配置是否存在（用户是否显式配置了权限文件）
        if permission_config.is_none() {
            // 没有配置权限文件时，使用安全默认策略
            // 只允许预定义的安全角色，防止未授权访问
            let safe_roles = ["admin", "system"];
            if !safe_roles.contains(&role) {
                return Err(DbError::Permission(format!(
                    "Role '{}' is not allowed without explicit permission configuration. Allowed roles: {}",
                    role,
                    safe_roles.join(", ")
                )));
            }
            return Ok(());
        }

        // 检查角色是否存在
        if permission_config
            .as_ref()
            .is_some_and(|c| c.get_role_policy(role).is_none())
        {
            // 角色不存在
            return Err(DbError::Permission(format!(
                "Role '{}' is not defined in permission configuration",
                role
            )));
        }

        Ok(())
    }

}
