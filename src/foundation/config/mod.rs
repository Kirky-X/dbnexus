// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 配置管理模块
//!
//! 纯数据结构，配置加载通过 serde 直接反序列化
//!
//! # 主要类型
//!
//! - [`DbConfig`] - 数据库配置结构体
//! - [`PoolConfig`] - 连接池配置
//! - `CacheConfig` - 缓存配置
//! - [`ConfigError`] - 配置相关错误类型
//!
//! # 使用方式
//!
//! 通过 `serde_yaml_ng` / `serde_json` 直接反序列化创建配置实例：
//!
//! ```rust,ignore
//! use dbnexus::DbConfig;
//!
//! // 从 YAML 字符串加载（需要启用 `yaml` feature）
//! let yaml = r#"
//! url: "sqlite::memory:"
//! max_connections: 20
//! "#;
//! let config = DbConfig::from_yaml_str(yaml)?;
//!
//! // 从 JSON 字符串加载
//! let json = r#"{"url":"sqlite::memory:","max_connections":20}"#;
//! let config = DbConfig::from_json_str(json)?;
//! ```

mod types;

pub use types::{CacheConfig, ConfigError, DatabaseType, DbConfig, PoolConfig};
// 注意: DbError 和 DbResult 已迁移到 crate::error 模块
// 请使用 crate::error::{DbError, DbResult} 代替
