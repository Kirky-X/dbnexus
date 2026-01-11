// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 连接池管理模块
//!
//! 提供数据库连接池的创建、管理和自动修正功能

mod session;
mod db_pool;

pub use db_pool::{DbPool, DatabaseConnection, PoolStatus};
pub use session::Session;

// 导入 Sea-ORM 的事务 trait 和连接 trait
pub use sea_orm::{ConnectionTrait, TransactionTrait};