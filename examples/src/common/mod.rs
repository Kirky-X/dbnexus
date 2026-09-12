// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT

//! 示例共享模块
//!
//! 提供跨示例复用的辅助代码，避免在每个示例中重复 DbPool 构造、
//! 实体定义、DDL 创建和权限上下文设置等样板代码。
//!
//! 每个示例二进制只使用共享助手的一个子集，模块级伞形豁免避免逐项
//! 标记在数十个示例二进制间重复；新增助手无需各自标注。
//! 每个示例二进制通过 `#[path = "../common/mod.rs"] mod common;` 引入本模块。

#![allow(dead_code)]

pub mod db;
pub mod ddl;
pub mod entities;
pub mod permissions;
