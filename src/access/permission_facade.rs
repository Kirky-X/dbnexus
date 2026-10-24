// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 权限统一门面
//!
//! 把 RBAC（前置的角色策略换装）、字段级脱敏与 RLS 行级安全
//! 组合为**单一入口**：一处配置，全局生效——query_rows 出口自动
//! 脱敏、SELECT 自动注入租户谓词、表访问经角色策略校验。
//!
//! # 生效边界
//!
//! - 租户谓词注入**单表**语句：读出口（`query_rows`）与 raw 写出口
//!   （`execute_raw` / `execute_with_params` / `execute` 的 UPDATE/DELETE）
//!   同样受行级隔离——表级权限不替代行级谓词；
//! - 多表语句与集合操作（UNION/INTERSECT/EXCEPT）无法用单表谓词覆盖全部行，
//!   RLS 生效时按权限错误拒绝（fail-closed），不静默放行；语句无法解析时
//!   同样拒绝（无法判定受影响表即不放行）；
//! - 谓词列名经标识符白名单校验；谓词值经后端转义后写为字符串字面量
//!   （字面量在 PostgreSQL 中由列类型决定，text/uuid/int 等列类型通用）；
//! - INSERT 不在行级谓词覆盖范围：租户列由调用方赋值；
//! - admin 角色走管理通道，不注入租户谓词（与 `admin_bypass` 语义一致）。
//!
//! # 示例
//!
//! ```ignore
//! let facade = PermissionFacade::new(&pool);
//! facade.apply(
//!     PermissionFacadeConfig::new()
//!         .with_role("default", RolePolicy { /* orders: Select */ .. })
//!         .with_masking_rule("email", MaskStrategy::Hash)
//!         .with_rls_policy("orders", "tenant_id", "t-100"),
//! ).await?;
//! ```

use crate::database::DbPool;
#[cfg(not(any(feature = "permission", feature = "data-protection")))]
use crate::foundation::DbError;
use crate::foundation::DbResult;

#[cfg(feature = "data-protection")]
use crate::access::data_protection::{DataProtection, MaskStrategy, MaskingEngine, RlsEngine};
#[cfg(feature = "permission")]
use crate::access::permission::{PermissionConfig, RolePolicy};

/// 权限统一配置：RBAC + 脱敏 + RLS 三合一
#[derive(Debug, Default)]
pub struct PermissionFacadeConfig {
    /// RBAC 角色策略（None = 不调整既有角色策略）
    #[cfg(feature = "permission")]
    rbac: Option<PermissionConfig>,
    /// 字段脱敏引擎（None = 不脱敏）
    #[cfg(feature = "data-protection")]
    masking: Option<MaskingEngine>,
    /// 行级安全引擎（None = 不注入谓词）
    #[cfg(feature = "data-protection")]
    rls: Option<RlsEngine>,
}

impl PermissionFacadeConfig {
    /// 空配置（不改变任何既有行为）
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置 RBAC：以角色名 → 策略映射整体换装
    #[cfg(feature = "permission")]
    pub fn with_roles(mut self, roles: std::collections::HashMap<String, RolePolicy>) -> Self {
        self.rbac = Some(PermissionConfig { roles });
        self
    }

    /// 追加一条字段脱敏规则
    #[cfg(feature = "data-protection")]
    pub fn with_masking_rule(mut self, column: &str, strategy: MaskStrategy) -> Self {
        let engine = self.masking.take().unwrap_or_default();
        self.masking = Some(engine.rule(column, strategy));
        self
    }

    /// 追加一条 RLS 租户谓词（table 上注入 `column = 'value'`）
    #[cfg(feature = "data-protection")]
    pub fn with_rls_policy(mut self, table: &str, column: &str, value: &str) -> Self {
        let engine = self.rls.take().unwrap_or_default();
        self.rls = Some(engine.policy(table, column, value));
        self
    }
}

/// 权限统一门面
///
/// 绑定连接池；`apply` 把组合配置一次性换装到池上，对后续全部
/// `query_rows` / 表访问生效（与运行时换装同机制）。
pub struct PermissionFacade<'a> {
    pool: &'a DbPool,
}

impl<'a> PermissionFacade<'a> {
    /// 绑定连接池
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    /// 一处配置，全局生效
    ///
    /// # Errors
    ///
    /// RBAC 角色策略换装失败时返回 `DbError`（脱敏/RLS 为无失败换装）。
    pub async fn apply(&self, config: PermissionFacadeConfig) -> DbResult<()> {
        #[cfg(feature = "permission")]
        if let Some(rbac) = config.rbac {
            self.pool.set_permission_config(rbac).await?;
        }

        #[cfg(feature = "data-protection")]
        self.pool
            .set_data_protection(DataProtection {
                masking: config.masking.map(std::sync::Arc::new),
                rls: config.rls.map(std::sync::Arc::new),
            })
            .await;

        #[cfg(not(any(feature = "permission", feature = "data-protection")))]
        {
            // 两者皆缺时 facade 无可生效面（本模块仅在双 feature 下编译，此块
            // 为 cfg 组合的语义自洽保留）；error 文案与 condition 对应
            let _ = &self.pool;
            return Err(DbError::Config(
                "PermissionFacade requires 'permission' and/or 'data-protection' features"
                    .to_string(),
            ));
        }

        #[allow(unreachable_code)]
        Ok(())
    }
}

// MaskingEngine/RlsEngine 的 Default 约定检查（编译期）：with_masking_rule/with_rls_policy
// 依赖 unwrap_or_default 惰性构造
#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(all(feature = "permission", feature = "data-protection"))]
    #[test]
    fn test_config_builder_accumulates_rules() {
        let config = PermissionFacadeConfig::new()
            .with_masking_rule("email", MaskStrategy::Hash)
            .with_masking_rule("phone", MaskStrategy::Mask { keep: 3 })
            .with_rls_policy("orders", "tenant_id", "t-100")
            .with_rls_policy("audit", "tenant_id", "t-100");
        assert!(config.masking.is_some());
        assert!(config.rls.is_some());
        assert!(config.rbac.is_none());
    }
}
