// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 统一错误类型

use thiserror::Error;

/// DBNexus 顶层统一错误类型
#[derive(Debug, Error)]
pub enum DbNexusError {
    /// 权限错误
    #[cfg(feature = "permission")]
    #[error(transparent)]
    Permission(#[from] crate::domain::permission::PermissionError),

    /// 权限配置错误
    #[cfg(feature = "permission")]
    #[error(transparent)]
    PermissionConfig(#[from] crate::domain::permission::PermissionConfigError),

    /// 不支持的数据库协议
    #[error("Unsupported database scheme in URL: {0}")]
    UnsupportedDatabaseScheme(String),
}

/// DBNexus 统一结果类型
pub type DbNexusResult<T> = Result<T, DbNexusError>;
