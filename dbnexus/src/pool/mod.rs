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

use crate::config::{DbConfig, DbResult};
use sea_orm::ExecResult;
use async_trait::async_trait;

/// 连接池抽象 trait
///
/// 定义连接池的通用接口，便于测试和替换实现
#[async_trait]
pub trait ConnectionPool: Send + Sync {
    /// 获取会话
    ///
    /// # Arguments
    ///
    /// * `role` - 用户角色
    ///
    /// # Returns
    ///
    /// 返回数据库会话
    async fn get_session(&self, role: &str) -> DbResult<Session>;

    /// 获取连接池状态
    ///
    /// # Returns
    ///
    /// 返回连接池状态信息
    fn status(&self) -> PoolStatus;

    /// 获取配置
    ///
    /// # Returns
    ///
    /// 返回连接池配置
    fn config(&self) -> &DbConfig;
}

/// 数据库会话抽象 trait
///
/// 定义数据库会话的通用接口，便于测试和替换实现
#[async_trait]
pub trait DatabaseSession: Send + Sync {
    /// 执行 SQL（带权限检查）
    ///
    /// # Arguments
    ///
    /// * `sql` - SQL 语句
    ///
    /// # Returns
    ///
    /// 返回执行结果
    async fn execute(&mut self, sql: &str) -> DbResult<ExecResult>;

    /// 执行原始 SQL（不带权限检查）
    ///
    /// # Arguments
    ///
    /// * `sql` - SQL 语句
    ///
    /// # Returns
    ///
    /// 返回执行结果
    async fn execute_raw(&self, sql: &str) -> DbResult<ExecResult>;

    /// 执行原始 DDL（仅限管理员）
    ///
    /// # Arguments
    ///
    /// * `sql` - DDL SQL 语句
    ///
    /// # Returns
    ///
    /// 返回执行结果
    async fn execute_raw_ddl(&self, sql: &str) -> DbResult<ExecResult>;

    /// 开始事务
    ///
    /// # Returns
    ///
    /// 返回成功或错误
    async fn begin_transaction(&mut self) -> DbResult<()>;

    /// 提交事务
    ///
    /// # Returns
    ///
    /// 返回成功或错误
    async fn commit(&mut self) -> DbResult<()>;

    /// 回滚事务
    ///
    /// # Returns
    ///
    /// 返回成功或错误
    async fn rollback(&mut self) -> DbResult<()>;

    /// 获取角色
    ///
    /// # Returns
    ///
    /// 返回用户角色
    fn role(&self) -> &str;

    /// 是否在事务中
    ///
    /// # Returns
    ///
    /// 返回是否在事务中
    fn is_in_transaction(&self) -> bool;
}