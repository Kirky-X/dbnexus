// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 配置管理模块
//!
//! 纯数据结构，配置加载由 confers 库接管
//!
//! # 主要类型
//!
//! - [`DbConfig`] - 数据库配置结构体
//! - [`PoolConfig`] - 连接池配置
//! - [`CacheConfig`] - 缓存配置
//! - [`ConfigError`] - 配置相关错误类型
//!
//! # 使用方式
//!
//! 通过 confers 库加载配置，然后使用 `DbConfig::from_confers()` 创建配置实例：
//!
//! ```rust,ignore
//! use confers::ConfigProvider;
//! use dbnexus::config::DbConfig;
//!
//! let provider = /* confers provider */;
//! let config = DbConfig::from_confers(&provider)?;
//! ```

mod types;

pub use types::{
    CacheConfig, ConfigError, DatabaseType, DbConfig, DbError,
    DbResult, PoolConfig,
};
