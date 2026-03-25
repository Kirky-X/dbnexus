// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

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
pub use security::{DdlGuard, DdlValidationResult, MaskType, SensitiveError, SensitiveMasker, SensitiveResult};

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
#[cfg(feature = "permission-engine")]
pub use permission_engine::{
    PermissionAction as EnginePermissionAction, PermissionContext as PermissionEngineContext, PermissionDecision,
    PermissionEngine, PermissionEngineConfig, PermissionProvider as EnginePermissionProvider, PermissionResource,
    PermissionRule, PermissionSubject, PolicyDecisionPoint, RbacPermissionProvider, Role,
    YamlPermissionProvider as EngineYamlPermissionProvider,
};
