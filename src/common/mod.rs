// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 公共类型模块
//!
//! 提供跨模块共享的类型和错误定义

pub mod error;

pub use error::{DbNexusError, DbNexusResult, ErrorCategory, QueryErrorReport};
