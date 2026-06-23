// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 连接池基础模块
//!
//! 提供数据库连接池的基础设施能力

mod config;
mod error;
mod impl_;
mod interface;
mod types;

pub use config::PoolConfig;
pub use error::{PoolConfigError, PoolError};
pub use interface::{PoolConnector, PoolLifecycle, PoolReader, PoolWriter};
pub use types::{Connection, PoolStatus, Session};

/// 标准工厂函数
pub async fn new(config: PoolConfig) -> Result<impl PoolConnector, PoolConfigError> {
    config.validate()?;
    impl_::default::DbPool::connect(config).await
}

/// 内存实现工厂函数（测试用）
pub fn new_in_memory() -> impl PoolConnector {
    impl_::memory::MemoryPool::new()
}
