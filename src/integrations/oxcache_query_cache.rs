// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! `OxcacheQueryCache` — 查询缓存装饰器（`oxcache-integration` feature）
//!
//! 把 oxcache [`CacheBackend`] 装配到 [`DbPool`] 的参数化查询通道上：
//! `query_cached` 以安全上下文 + SQL + 绑定参数 + 表版本戳派生缓存 key
//! （SHA-256），命中直接返回缓存行集，未命中穿透执行并回填；
//! `invalidate_table` 在写路径后按表戳版本，使提及该表的缓存项自然失效。
//!
//! # 契约
//!
//! - **key 派生**：`SHA-256(安全上下文 + 表版本原始字节 + SQL + 参数
//!   JSON + 数据保护策略世代)`——安全上下文（role/namespace）、SQL、
//!   参数、任一提及表被失效、`set_data_protection` 运行时换装，任一
//!   变化都派生新 key（旧条目交给后端 TTL/LRU 淘汰）。
//!   表版本以**原始字节**进哈希（绕开任何文本解码的多对一路径），
//!   `invalidate_table` 写十进制字符串（可读且单射）
//! - **安全上下文隔离**：行集内容依赖执行时安全上下文（RLS 谓词/脱敏/
//!   权限过滤），role 与 namespace 必须参与 key——共享同一后端的实例
//!   若安全上下文不同（admin vs 受限角色、租户 A vs B），穿透回填的
//!   条目互不可见；跨进程共享后端（redis 等）时跨实例隔离由相同机制
//!   保证
//! - **策略换装失效**：`data-protection` feature 下 key 派生纳入池的
//!   数据保护策略世代，`set_data_protection` 换装后旧策略下回填的行集
//!   （旧 RLS 谓词/脱敏出口）不再可命中——运行时收紧或放宽策略都即时
//!   生效，不依赖调用方手动失效
//! - **表名显式声明**：`tables` 参数由调用方给出而非从 SQL 解析，且
//!   fail-closed——空表集与非法表名（非 `[A-Za-z0-9_.]` 白名单）构建期
//!   拒绝：漏传 tables 的查询永不受失效影响，即潜在无限脏读
//! - **失效语义**：`invalidate_table` 以纳秒时间戳十进制串写表版本
//!   （单射、免读改写竞态），幂等且对未知表成功；版本读取失败显性
//!   报错（把失效探测失败当未失效会静默脏读）
//! - **版本条目存活不变量**：版本键与数据条目同策略共存亡（都不设
//!   per-entry TTL 时同受全局 TTL/容量淘汰支配）；版本键数量 = 表数
//!   （远小于数据条目）且每次 `query_cached` 都访问（热，逐出概率远低
//!   于数据条目）——**容量规划要求 capacity ≥ 表数 + 数据条目工作集**，
//!   病态小容量（capacity < 表数）不受支持
//! - **缓存写失败显性化**：穿透执行成功但回填失败时返回
//!   [`DbError::Cache`]（行数据不丢，查询幂等可安全重试），不静默吞掉
//! - **默认零行为变化**：不装饰时 `DbPool` 查询通道完全不受影响；本
//!   feature 未启用时无任何编译面

use std::sync::Arc;
use std::time::Duration;

use oxcache::backend::CacheBackend;
use sha2::{Digest, Sha256};

use crate::database::pool::DbPool;
use crate::foundation::{DbError, DbResult};
use crate::i18n;

const KEY_PREFIX: &str = "qcache:q:v1";
const TABLE_VERSION_PREFIX: &str = "qcache:t:v1";
const ZERO_VERSION: &[u8] = b"0";
const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

/// 一次 `query_cached` 的结果
#[derive(Debug, Clone)]
pub struct CachedQuery {
    /// 行集（与 `Session::query_rows_with_params` 同构）
    pub rows: Vec<serde_json::Value>,
    /// 是否来自缓存（false = 穿透执行并已回填）
    pub from_cache: bool,
}

/// 查询缓存装饰器：oxcache 后端 × [`DbPool`] 参数化查询
///
/// # 示例
///
/// ```ignore
/// let qc = OxcacheQueryCache::new(pool, cache_backend);
/// // N+1 点查：二轮起全部命中
/// let cq = qc.query_cached(
///     "SELECT id, name FROM users WHERE id = ?",
///     &[serde_json::json!(user_id)],
///     &["users"],
/// ).await?;
/// // 写路径后失效该表
/// qc.invalidate_table("users").await?;
/// ```
///
/// 共享后端的多实例必须以 role/namespace 区分安全上下文（见模块契约）。
#[derive(Clone)]
pub struct OxcacheQueryCache {
    pool: Arc<DbPool>,
    cache: Arc<dyn CacheBackend + Send + Sync>,
    role: String,
    namespace: String,
    default_ttl: Option<Duration>,
}

impl OxcacheQueryCache {
    /// 以池与 oxcache 后端构建装饰器（查询角色默认 `admin`，命名空间为空）
    #[must_use]
    pub fn new(pool: Arc<DbPool>, cache: Arc<dyn CacheBackend + Send + Sync>) -> Self {
        Self {
            pool,
            cache,
            role: "admin".to_string(),
            namespace: String::new(),
            default_ttl: None,
        }
    }

    /// 指定穿透执行使用的会话角色（默认 `admin`）
    ///
    /// role 参与 key 派生：不同角色的行集可能被 RLS/脱敏/权限过滤成不同
    /// 内容，跨角色不得共享条目。
    #[must_use]
    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.role = role.into();
        self
    }

    /// 指定命名空间（多租户/多池共享后端时隔离条目；默认空）
    ///
    /// namespace 参与 key 派生：不同租户的数据行不同，共享后端必须以
    /// namespace 隔离。
    #[must_use]
    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = namespace.into();
        self
    }

    /// 指定缓存条目默认 TTL（后端侧过期；未设置则由后端策略决定）
    #[must_use]
    pub fn with_default_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = Some(ttl);
        self
    }

    /// 装饰的池（供写路径取会话）
    #[must_use]
    pub fn pool(&self) -> &Arc<DbPool> {
        &self.pool
    }

    /// 参数化查询的缓存优先执行
    ///
    /// - `tables`：SQL 涉及的表（显式声明，非空，白名单校验——见模块
    ///   契约）；任一表在本次调用与上次回填之间被 `invalidate_table`
    ///   过，即视为不同 key
    /// - 命中：反序列化缓存行集返回（`from_cache = true`），数据库零往返
    /// - 穿透：经 `Session::query_rows_with_params` 执行并回填
    ///   （`from_cache = false`），TTL 取本次参数（缺省用默认 TTL）
    pub async fn query_cached(
        &self,
        sql: &str,
        params: &[serde_json::Value],
        tables: &[&str],
    ) -> DbResult<CachedQuery> {
        self.query_cached_with_ttl(sql, params, tables, None).await
    }

    /// [`Self::query_cached`] 的显式 TTL 变体（覆盖默认 TTL；`None` 且未
    /// 设置默认时由后端策略决定过期）
    pub async fn query_cached_with_ttl(
        &self,
        sql: &str,
        params: &[serde_json::Value],
        tables: &[&str],
        ttl: Option<Duration>,
    ) -> DbResult<CachedQuery> {
        let key = self.derive_key(sql, params, tables).await?;
        let ttl = ttl.or(self.default_ttl);

        if let Some(bytes) = self.cache.get(&key).await.map_err(|e| {
            DbError::Cache(i18n::t(
                "query-cache-get-failed",
                &[("error", e.to_string())],
            ))
        })? {
            let rows = serde_json::from_slice(&bytes).map_err(|e| {
                DbError::Cache(i18n::t(
                    "query-cache-decode-failed",
                    &[("error", e.to_string())],
                ))
            })?;
            return Ok(CachedQuery {
                rows,
                from_cache: true,
            });
        }

        let session = self.pool.get_session(&self.role).await?;
        let rows = session.query_rows_with_params(sql, params).await?;
        let bytes = serde_json::to_vec(&rows).map_err(|e| {
            DbError::Cache(i18n::t(
                "query-cache-encode-failed",
                &[("error", e.to_string())],
            ))
        })?;
        self.cache
            .set(key.into(), Arc::new(bytes), ttl)
            .await
            .map_err(|e| {
                DbError::Cache(i18n::t(
                    "query-cache-fill-failed",
                    &[("error", e.to_string())],
                ))
            })?;
        Ok(CachedQuery {
            rows,
            from_cache: false,
        })
    }

    /// 写路径失效：使提及 `table` 的所有缓存项失效
    ///
    /// 以纳秒时间戳**十进制字符串**覆盖该表版本键——原始字节进哈希前
    /// 无任何文本解码，编码单射（相邻时间戳必派生不同 key，失效不静默
    /// no-op）；后续 `query_cached` 读到新版本即派生新 key，旧条目交由
    /// 后端淘汰。幂等；对未缓存过的表同样成功。
    pub async fn invalidate_table(&self, table: &str) -> DbResult<()> {
        validate_table_name(table)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| {
                DbError::Cache(i18n::t(
                    "query-cache-clock-before-epoch",
                    &[("error", e.to_string())],
                ))
            })?
            .as_nanos()
            .to_string();
        self.cache
            .set(
                Arc::from(format!("{TABLE_VERSION_PREFIX}:{table}").as_str()),
                Arc::new(now.into_bytes()),
                None,
            )
            .await
            .map_err(|e| {
                DbError::Cache(i18n::t(
                    "query-cache-invalidate-failed",
                    &[("error", e.to_string())],
                ))
            })
    }

    /// key 派生：`SHA-256(安全上下文 + 表版本原始字节 + SQL + 参数 JSON)`
    ///
    /// fail-closed：空表集/非法表名拒绝；版本批量读取失败显性报错
    /// （失效探测失败当未失效会静默脏读）；版本以原始字节进哈希，
    /// 不经任何文本解码（多对一解码会让失效碰撞回旧 key）。
    async fn derive_key(
        &self,
        sql: &str,
        params: &[serde_json::Value],
        tables: &[&str],
    ) -> DbResult<String> {
        if tables.is_empty() {
            return Err(DbError::Config(i18n::t_simple(
                "query-cache-tables-required",
            )));
        }
        for table in tables {
            validate_table_name(table)?;
        }

        // 版本键一次批量读取（远程后端省 N-1 次往返；默认实现逐个语义一致）
        let version_keys: Vec<String> = tables
            .iter()
            .map(|t| format!("{TABLE_VERSION_PREFIX}:{t}"))
            .collect();
        let versions = self.cache.get_many(&version_keys).await.map_err(|e| {
            DbError::Cache(i18n::t(
                "query-cache-version-read-failed",
                &[("error", e.to_string())],
            ))
        })?;
        // 后端契约：get_many 返回值与请求键一一对应；zip 按短侧截断会
        // 静默丢弃尾部表的版本维度（该表失效探测失明 → 潜在脏读），
        // debug 构建在此钉住契约破坏
        debug_assert_eq!(
            versions.len(),
            tables.len(),
            "get_many must return one entry per requested key"
        );

        let mut hasher = Sha256::new();
        // 长度前缀框架：每个变长字段先写 8 字节小端长度再写内容——编码
        // 单射，字段值含换行/分隔字节时不同 (role, namespace, sql) 组合
        // 不可能产生相同哈希输入（换行标签拼接可被字段值注入混淆）
        // 安全上下文维度：role/namespace 不同即不同 key（行集内容依赖
        // 执行时 RLS/脱敏/权限上下文，跨上下文命中即越权读取）
        push_field(&mut hasher, self.role.as_bytes());
        push_field(&mut hasher, self.namespace.as_bytes());
        // 数据保护策略世代：set_data_protection 运行时换装 bump 世代，
        // 旧策略下回填的行集（旧 RLS 谓词/脱敏出口）不得被新策略下的
        // 查询命中（feature 关闭时行集无策略依赖，无此维度）
        #[cfg(feature = "data-protection")]
        push_field(
            &mut hasher,
            &self.pool.data_protection_epoch().to_le_bytes(),
        );
        // 表版本以原始字节进哈希：绕开文本解码多对一路径（lossy 解码曾
        // 使相邻纳秒时间戳碰撞出相同版本串，失效静默 no-op）
        for (table, version) in tables.iter().zip(&versions) {
            push_field(&mut hasher, table.as_bytes());
            push_field(&mut hasher, version.as_deref().unwrap_or(ZERO_VERSION));
        }
        push_field(&mut hasher, sql.as_bytes());
        let params_json = serde_json::to_vec(params).map_err(|e| {
            DbError::Cache(i18n::t(
                "query-cache-params-encode-failed",
                &[("error", e.to_string())],
            ))
        })?;
        push_field(&mut hasher, &params_json);
        let digest = hasher.finalize();

        // hex 编码查表零中间分配（命中热路径每调用必经）
        let mut key = String::with_capacity(KEY_PREFIX.len() + 1 + digest.len() * 2);
        key.push_str(KEY_PREFIX);
        key.push(':');
        for byte in digest {
            key.push(HEX_CHARS[(byte >> 4) as usize] as char);
            key.push(HEX_CHARS[(byte & 0x0f) as usize] as char);
        }
        Ok(key)
    }
}

/// 哈希字段写入（8 字节小端长度前缀 + 内容）——长度前缀使变长字段
/// 序列化单射，字段值不限定字符集
fn push_field(hasher: &mut Sha256, field: &[u8]) {
    hasher.update((field.len() as u64).to_le_bytes());
    hasher.update(field);
}

/// 表名白名单校验（对齐 `copy` 模块标识符口径：字母/数字/下划线/点，
/// 首字符字母或下划线，不允许连续点）——表名拼接版本键命名空间，
/// 放行任意字符会把冒号/空白注入键空间
fn validate_table_name(table: &str) -> DbResult<()> {
    let valid = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '.';
    let first_ok = table
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    if !first_ok || !table.chars().all(valid) || table.contains("..") {
        return Err(DbError::Config(i18n::t(
            "query-cache-table-name-invalid",
            &[("table", table.to_string())],
        )));
    }
    Ok(())
}
