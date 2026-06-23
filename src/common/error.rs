// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 统一错误类型

use thiserror::Error;

/// DBNexus 顶层统一错误类型
#[derive(Debug, Error)]
pub enum DbNexusError {
    /// 连接池错误
    #[cfg(feature = "pool")]
    #[error(transparent)]
    Pool(#[from] crate::foundation::pool::PoolError),

    /// 连接池配置错误
    #[cfg(feature = "pool")]
    #[error(transparent)]
    PoolConfig(#[from] crate::foundation::pool::PoolConfigError),

    /// 权限错误
    #[cfg(feature = "permission")]
    #[error(transparent)]
    Permission(#[from] crate::domain::permission::PermissionError),

    /// 权限配置错误
    #[cfg(feature = "permission")]
    #[error(transparent)]
    PermissionConfig(#[from] crate::domain::permission::PermissionConfigError),
}

/// DBNexus 统一结果类型
pub type DbNexusResult<T> = Result<T, DbNexusError>;
