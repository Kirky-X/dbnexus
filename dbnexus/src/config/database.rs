//! 数据库配置模块
//!
//! 提供统一的数据库配置管理，支持从环境变量读取配置

use serde::{Deserialize, Serialize};

/// 数据库连接池配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    /// 最大连接数
    pub max_connections: u32,
    /// 最小连接数
    pub min_connections: u32,
    /// 连接空闲超时时间（秒）
    pub idle_timeout: u64,
    /// 连接获取超时时间（毫秒）
    pub acquire_timeout: u64,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 5,
            min_connections: 1,
            idle_timeout: 300,
            acquire_timeout: 5000,
        }
    }
}

/// 数据库类型枚举
#[derive(Debug, Clone, PartialEq)]
pub enum DatabaseType {
    /// PostgreSQL
    Postgres,
    /// MySQL
    MySql,
    /// SQLite
    Sqlite,
}

impl DatabaseType {
    /// 从字符串解析数据库类型
    ///
    /// # Examples
    ///
    /// ```rust
    /// use dbnexus::config::DatabaseType;
    ///
    /// assert_eq!(DatabaseType::parse_database_type("postgres"), DatabaseType::Postgres);
    /// assert_eq!(DatabaseType::parse_database_type("postgresql"), DatabaseType::Postgres);
    /// assert_eq!(DatabaseType::parse_database_type("mysql"), DatabaseType::MySql);
    /// assert_eq!(DatabaseType::parse_database_type("sqlite"), DatabaseType::Sqlite);
    /// assert_eq!(DatabaseType::parse_database_type("unknown"), DatabaseType::Sqlite);
    /// ```
    pub fn parse_database_type(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "postgres" | "postgresql" => DatabaseType::Postgres,
            "mysql" => DatabaseType::MySql,
            "sqlite" => DatabaseType::Sqlite,
            _ => DatabaseType::Sqlite, // 默认使用 SQLite
        }
    }

    /// 获取数据库类型的显示名称
    pub fn as_str(&self) -> &'static str {
        match self {
            DatabaseType::Postgres => "postgres",
            DatabaseType::MySql => "mysql",
            DatabaseType::Sqlite => "sqlite",
        }
    }

    /// 检查是否为真实数据库（非内存数据库）
    pub fn is_real_database(&self) -> bool {
        !matches!(self, DatabaseType::Sqlite)
    }
}

/// 统一的数据库配置
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// 数据库类型
    pub db_type: DatabaseType,
    /// 连接 URL
    pub connection_url: String,
    /// 连接池配置
    pub pool_config: PoolConfig,
}

impl DatabaseConfig {
    /// 从环境变量创建配置
    pub fn from_env() -> Self {
        let db_type_str = std::env::var("TEST_DB_TYPE").unwrap_or_else(|_| "sqlite".to_string());
        let db_type = DatabaseType::parse_database_type(&db_type_str);

        let connection_url = match db_type {
            DatabaseType::Postgres => std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://dbnexus:dbnexus_password@localhost:15432/dbnexus_test".to_string()),
            DatabaseType::MySql => std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "mysql://dbnexus:dbnexus_password@localhost:13306/dbnexus_test".to_string()),
            DatabaseType::Sqlite => "sqlite::memory:".to_string(),
        };

        Self {
            db_type,
            connection_url,
            pool_config: PoolConfig::default(),
        }
    }

    /// 从环境变量创建配置，支持自定义池配置
    pub fn from_env_with_pool_config(pool_config: PoolConfig) -> Self {
        let mut config = Self::from_env();
        config.pool_config = pool_config;
        config
    }

    /// 获取当前数据库类型
    pub fn database_type(&self) -> &DatabaseType {
        &self.db_type
    }

    /// 获取连接 URL
    pub fn connection_url(&self) -> &str {
        &self.connection_url
    }

    /// 获取池配置
    pub fn pool_config(&self) -> &PoolConfig {
        &self.pool_config
    }

    /// 转换为 DbConfig（兼容现有代码）
    pub fn to_db_config(&self, permissions_path: Option<String>) -> crate::config::DbConfig {
        crate::config::DbConfig {
            url: self.connection_url.clone(),
            max_connections: self.pool_config.max_connections,
            min_connections: self.pool_config.min_connections,
            idle_timeout: self.pool_config.idle_timeout,
            acquire_timeout: self.pool_config.acquire_timeout,
            permissions_path,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_type_parse() {
        assert_eq!(DatabaseType::parse_database_type("postgres"), DatabaseType::Postgres);
        assert_eq!(DatabaseType::parse_database_type("postgresql"), DatabaseType::Postgres);
        assert_eq!(DatabaseType::parse_database_type("mysql"), DatabaseType::MySql);
        assert_eq!(DatabaseType::parse_database_type("sqlite"), DatabaseType::Sqlite);
        assert_eq!(DatabaseType::parse_database_type("unknown"), DatabaseType::Sqlite);
    }

    #[test]
    fn test_database_type_is_real_database() {
        assert!(DatabaseType::Postgres.is_real_database());
        assert!(DatabaseType::MySql.is_real_database());
        assert!(!DatabaseType::Sqlite.is_real_database());
    }
}
