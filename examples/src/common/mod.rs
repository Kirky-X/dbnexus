// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in the project root for full license information.

#![allow(dead_code)]

//! 示例共享模块
//!
//! 提供跨示例复用的辅助代码，避免在每个示例中重复 DbPool 构造、
//! 实体定义、DDL 创建和权限上下文设置等样板代码。
//!
//! 每个示例二进制通过 `#[path = "../common/mod.rs"] mod common;` 引入本模块。

pub mod db;
pub mod ddl;
pub mod entities;
pub mod permissions;
