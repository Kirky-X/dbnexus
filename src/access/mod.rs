// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Access 模块
//!
//! 提供身份认证、权限控制、SQL 安全等功能

#[cfg(feature = "permission")]
pub mod permission;
pub mod security;

// 单文件模块
#[cfg(feature = "authentication")]
pub mod authentication;
#[cfg(feature = "permission-engine")]
pub mod permission_engine;
/// T417：统一注入检测引擎（无外部依赖；关系型管线方法按 sql-parser 门控）
pub mod injection_engine;
#[cfg(feature = "sql-parser")]
pub mod sql_parser;

/// T403/T404：字段级脱敏与行级安全（data-protection feature）
#[cfg(feature = "data-protection")]
pub mod data_protection;

/// T409：权限变更审计链（HMAC-SHA256 链式签名，data-protection feature）
#[cfg(feature = "data-protection")]
pub mod permission_audit_chain;

/// T421：权限统一门面（RBAC + 脱敏 + RLS 单一入口）
#[cfg(all(feature = "permission", feature = "data-protection"))]
pub mod permission_facade;

// Re-exports: security
#[cfg(feature = "sql-parser")]
pub use security::{AuditingDdlGuard, DdlAuditRecord, DdlGuard, DdlGuardPolicy, DdlValidationResult, DryRunDdlGuard};
pub use injection_engine::{InjectionEngine, InjectionRule, RuleCategory};
pub use security::{MaskType, SensitiveError, SensitiveMasker, SensitiveResult};

// Re-exports: permission
#[cfg(all(feature = "permission", any(feature = "ladybug", feature = "neo4j")))]
pub use permission::GraphPermissionContext;
#[cfg(feature = "permission")]
pub use permission::{
    AdvancedRbacProvider, CacheStats, MemoryPermissionProvider, PermissionAction, PermissionCache,
    PermissionCacheConfig, PermissionCheckStats, PermissionCheckStatsSnapshot, PermissionConfig,
    PermissionContext, PermissionError, PermissionProvider, PermissionProviderError, RateLimiter,
    RbacProvider, RefreshablePermissionProvider, RolePolicy, TablePermission,
    YamlPermissionProvider,
};

// Re-exports: authentication
#[cfg(feature = "authentication")]
pub use authentication::{
    AuthCredentials, AuthError, AuthResult, AuthenticationManager, JwtClaims, JwtManager,
    PasswordHasher, TokenType, User,
};

// Re-exports: sql_parser
#[cfg(all(feature = "sql-parser", not(feature = "permission")))]
pub use sql_parser::PermissionAction;
#[cfg(feature = "sql-parser")]
pub use sql_parser::{SqlOperationType, SqlParser, contains_sql_injection, is_ddl_operation};

// Re-exports: permission_engine
// 注意：Engine* 别名仅在 crate root (lib.rs) 导出，此处不再重复导出以避免双重路径（HIGH-002 修复）
