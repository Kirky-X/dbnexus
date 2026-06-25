// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 安全模块
//!
//! 提供数据库操作的安全验证功能。

#[cfg(feature = "sql-parser")]
mod ddl_guard;
mod sensitive;

#[cfg(feature = "sql-parser")]
pub use ddl_guard::{DdlGuard, DdlValidationResult};
pub use sensitive::{MaskType, SensitiveError, SensitiveMasker, SensitiveResult};
