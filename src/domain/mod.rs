// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Domain 层 - 领域模块
//!
//! 提供业务领域相关的能力，可依赖 foundation 层和第三方库

#[cfg(feature = "permission")]
pub mod permission;

pub mod migration;

#[cfg(feature = "audit")]
pub mod audit;
