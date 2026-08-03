// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 可靠性模块 — 运行时容错能力
//!
//! 提供重试、故障转移等可靠性增强功能。

#[cfg(feature = "retry")]
pub mod retry;

#[cfg(feature = "retry")]
pub use retry::{RetryError, RetryExecutor, RetryPolicy};

#[cfg(feature = "retry")]
pub use retry::is_idempotent_operation;
