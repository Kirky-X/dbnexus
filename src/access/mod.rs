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
#[cfg(feature = "sql-parser")]
pub mod sql_parser;

// Re-exports: security
#[cfg(feature = "sql-parser")]
pub use security::{DdlGuard, DdlValidationResult};
pub use security::{MaskType, SensitiveError, SensitiveMasker, SensitiveResult};

// Re-exports: permission
#[cfg(feature = "permission")]
pub use permission::{
    MemoryPermissionProvider, PermissionAction, PermissionConfig, PermissionContext, PermissionProvider,
    PermissionProviderError, RolePolicy, TablePermission, YamlPermissionProvider,
};

// Re-exports: authentication
#[cfg(feature = "authentication")]
pub use authentication::{
    AuthCredentials, AuthError, AuthResult, AuthenticationManager, JwtClaims, JwtManager, PasswordHasher, TokenType,
    User,
};

// Re-exports: sql_parser
#[cfg(all(feature = "sql-parser", not(feature = "permission")))]
pub use sql_parser::PermissionAction;
#[cfg(feature = "sql-parser")]
pub use sql_parser::SqlParser;

// Re-exports: permission_engine
// 注意：Engine* 别名仅在 crate root (lib.rs) 导出，此处不再重复导出以避免双重路径（HIGH-002 修复）
