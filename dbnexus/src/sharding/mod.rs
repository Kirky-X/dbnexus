//! 分片管理模块
//!
//! 提供数据库分片功能，支持多种分片策略：
//! - `Yearly`: 按年分片
//! - `Monthly`: 按月分片
//! - `Daily`: 按天分片
//! - `Hash`: 哈希分片
//!
//! # Example
//!
//! ```ignore
//! use dbnexus::sharding::{ShardRouter, ShardConfig};
//!
//! let config = ShardConfig::new("yearly", 12, "order", "postgresql://localhost/{shard}");
//! let mut router = ShardRouter::with_config(&config);
//! ```

use chrono::{DateTime, Datelike, Utc};
use std::collections::HashMap;

/// 分片策略 trait
pub trait ShardingStrategy: Send + Sync {
    /// 根据时间和总分片数计算分片 ID
    fn calculate(&self, timestamp: DateTime<Utc>, total_shards: u32) -> u32;
    
    /// 获取策略名称
    fn name(&self) -> &'static str;
    
    /// 验证分片 ID 是否有效
    fn is_valid_shard_id(&self, shard_id: u32, total_shards: u32) -> bool;
    
    /// 获取当前时间对应的分片 ID
    fn current_shard(&self, total_shards: u32) -> u32;
    
    /// 克隆策略到 Box
    fn boxed_clone(&self) -> Box<dyn ShardingStrategy>;
}

/// 年分片策略
/// 按年份划分，如 2024 年对应 shard_id = 2024 % total_shards
#[derive(Debug, Clone, Copy)]
pub struct YearlyStrategy;

impl YearlyStrategy {
    /// 创建新的年分片策略实例
    pub fn new() -> Self {
        Self
    }
}

impl Default for YearlyStrategy {
    fn default() -> Self {
        Self
    }
}

impl ShardingStrategy for YearlyStrategy {
    fn calculate(&self, timestamp: DateTime<Utc>, total_shards: u32) -> u32 {
        let year = timestamp.year() as u32;
        year % total_shards
    }
    
    fn name(&self) -> &'static str {
        "yearly"
    }
    
    fn is_valid_shard_id(&self, shard_id: u32, _total_shards: u32) -> bool {
        // 年分片 ID 通常是年份本身，所以只需要基本验证
        shard_id > 0
    }
    
    fn current_shard(&self, total_shards: u32) -> u32 {
        self.calculate(Utc::now(), total_shards)
    }
    
    fn boxed_clone(&self) -> Box<dyn ShardingStrategy> {
        Box::new(*self)
    }
}

/// 月分片策略
/// 按年月划分，如 2024年1月对应 shard_id = (2024 * 12 + 1) % total_shards
#[derive(Debug, Clone, Copy)]
pub struct MonthlyStrategy;

impl MonthlyStrategy {
    /// 创建新的月分片策略实例
    pub fn new() -> Self {
        Self
    }
}

impl Default for MonthlyStrategy {
    fn default() -> Self {
        Self
    }
}

impl ShardingStrategy for MonthlyStrategy {
    fn calculate(&self, timestamp: DateTime<Utc>, total_shards: u32) -> u32 {
        let year_month = timestamp.year() as u32 * 12 + timestamp.month();
        year_month % total_shards
    }
    
    fn name(&self) -> &'static str {
        "monthly"
    }
    
    fn is_valid_shard_id(&self, shard_id: u32, total_shards: u32) -> bool {
        shard_id < total_shards
    }
    
    fn current_shard(&self, total_shards: u32) -> u32 {
        self.calculate(Utc::now(), total_shards)
    }
    
    fn boxed_clone(&self) -> Box<dyn ShardingStrategy> {
        Box::new(*self)
    }
}

/// 日分片策略
/// 按日期划分
#[derive(Debug, Clone, Copy)]
pub struct DailyStrategy;

impl DailyStrategy {
    /// 创建新的日分片策略实例
    pub fn new() -> Self {
        Self
    }
}

impl Default for DailyStrategy {
    fn default() -> Self {
        Self
    }
}

impl ShardingStrategy for DailyStrategy {
    fn calculate(&self, timestamp: DateTime<Utc>, total_shards: u32) -> u32 {
        let days = timestamp.num_days_from_ce();
        days as u32 % total_shards
    }
    
    fn name(&self) -> &'static str {
        "daily"
    }
    
    fn is_valid_shard_id(&self, shard_id: u32, total_shards: u32) -> bool {
        shard_id < total_shards
    }
    
    fn current_shard(&self, total_shards: u32) -> u32 {
        self.calculate(Utc::now(), total_shards)
    }
    
    fn boxed_clone(&self) -> Box<dyn ShardingStrategy> {
        Box::new(*self)
    }
}

/// 哈希分片策略
/// 使用一致性哈希将数据均匀分布到各分片
#[derive(Debug, Clone, Copy)]
pub struct HashStrategy;

impl HashStrategy {
    /// 创建新的哈希分片策略实例
    pub fn new() -> Self {
        Self
    }
}

impl Default for HashStrategy {
    fn default() -> Self {
        Self
    }
}

impl ShardingStrategy for HashStrategy {
    fn calculate(&self, timestamp: DateTime<Utc>, total_shards: u32) -> u32 {
        use std::hash::{Hash, Hasher};
        use twox_hash::XxHash64;
        
        let mut hasher = XxHash64::default();
        timestamp.to_rfc3339().as_bytes().hash(&mut hasher);
        let hash = hasher.finish();
        (hash % total_shards as u64) as u32
    }
    
    fn name(&self) -> &'static str {
        "hash"
    }
    
    fn is_valid_shard_id(&self, shard_id: u32, total_shards: u32) -> bool {
        shard_id < total_shards
    }
    
    fn current_shard(&self, total_shards: u32) -> u32 {
        self.calculate(Utc::now(), total_shards)
    }
    
    fn boxed_clone(&self) -> Box<dyn ShardingStrategy> {
        Box::new(*self)
    }
}

/// 根据字符串创建分片策略
pub fn create_strategy(name: &str) -> Box<dyn ShardingStrategy> {
    match name.to_lowercase().as_str() {
        "yearly" | "year" => Box::new(YearlyStrategy),
        "monthly" | "month" => Box::new(MonthlyStrategy),
        "daily" | "day" => Box::new(DailyStrategy),
        "hash" => Box::new(HashStrategy),
        _ => Box::new(YearlyStrategy), // 默认使用年分片
    }
}

/// 分片信息
#[derive(Debug, Clone)]
pub struct ShardInfo {
    /// 分片 ID
    pub shard_id: u32,
    /// 分片名称
    pub name: String,
    /// 连接配置
    pub connection_string: String,
}

/// 分片路由器
pub struct ShardRouter {
    /// 总分片数
    total_shards: u32,
    /// 分片策略
    strategy: Box<dyn ShardingStrategy>,
    /// 分片配置映射
    shards: HashMap<u32, ShardInfo>,
}

impl Clone for ShardRouter {
    fn clone(&self) -> Self {
        Self {
            total_shards: self.total_shards,
            strategy: self.strategy.boxed_clone(),
            shards: self.shards.clone(),
        }
    }
}

impl ShardRouter {
    /// 创建新的分片路由器
    pub fn new<S: ShardingStrategy + 'static>(strategy: S, total_shards: u32) -> Self {
        Self {
            total_shards,
            strategy: Box::new(strategy),
            shards: HashMap::new(),
        }
    }
    
    /// 创建基于字符串策略的路由器
    pub fn with_strategy(strategy: &str, total_shards: u32) -> Self {
        Self {
            total_shards,
            strategy: create_strategy(strategy),
            shards: HashMap::new(),
        }
    }
    
    /// 使用配置创建路由器
    pub fn with_config(config: &ShardConfig) -> Self {
        let mut router = Self::with_strategy(&config.strategy, config.total_shards);
        
        for (shard_id, connection_string) in config.generate_all_connections() {
            router.register_shard(
                shard_id,
                format!("{}_{}", config.prefix, shard_id),
                connection_string,
            );
        }
        
        router
    }
    
    /// 注册分片
    pub fn register_shard(&mut self, shard_id: u32, name: String, connection_string: String) {
        self.shards.insert(shard_id, ShardInfo {
            shard_id,
            name,
            connection_string,
        });
    }
    
    /// 根据时间戳路由到分片
    pub fn route(&self, timestamp: DateTime<Utc>) -> Option<&ShardInfo> {
        let shard_id = self.strategy.calculate(timestamp, self.total_shards);
        self.shards.get(&shard_id)
    }
    
    /// 根据时间戳和关键字路由到分片（用于更均匀的分布）
    pub fn route_with_key(&self, timestamp: DateTime<Utc>, key: &str) -> Option<&ShardInfo> {
        let shard_id = self.calculate_shard(timestamp, key);
        self.shards.get(&shard_id)
    }
    
    /// 计算分片 ID（不依赖注册的分片）
    pub fn calculate_shard(&self, timestamp: DateTime<Utc>, key: &str) -> u32 {
        if key.is_empty() {
            self.strategy.calculate(timestamp, self.total_shards)
        } else {
            // 组合时间和关键字的哈希
            use std::hash::{Hash, Hasher};
            use twox_hash::XxHash64;
            
            let mut hasher = XxHash64::default();
            timestamp.to_rfc3339().as_bytes().hash(&mut hasher);
            key.as_bytes().hash(&mut hasher);
            let hash = hasher.finish();
            (hash % self.total_shards as u64) as u32
        }
    }
    
    /// 获取所有分片
    pub fn all_shards(&self) -> Vec<&ShardInfo> {
        self.shards.values().collect()
    }
    
    /// 获取分片策略名称
    pub fn strategy_name(&self) -> &'static str {
        self.strategy.name()
    }
    
    /// 获取总分片数
    pub fn total_shards(&self) -> u32 {
        self.total_shards
    }
}

/// 分片配置
#[derive(Debug, Clone)]
pub struct ShardConfig {
    /// 策略名称
    pub strategy: String,
    /// 总分片数
    pub total_shards: u32,
    /// 分片名称前缀
    pub prefix: String,
    /// 基础连接字符串模板
    pub connection_template: String,
}

impl Default for ShardConfig {
    fn default() -> Self {
        Self {
            strategy: "yearly".to_string(),
            total_shards: 12,
            prefix: "db".to_string(),
            connection_template: "sqlite:./data/{shard}.db".to_string(),
        }
    }
}

impl ShardConfig {
    /// 创建分片配置
    pub fn new(strategy: &str, total_shards: u32, prefix: &str, connection_template: &str) -> Self {
        Self {
            strategy: strategy.to_string(),
            total_shards,
            prefix: prefix.to_string(),
            connection_template: connection_template.to_string(),
        }
    }
    
    /// 生成连接字符串
    pub fn generate_connection_string(&self, shard_id: u32) -> String {
        self.connection_template
            .replace("{shard}", &format!("{}_{}", self.prefix, shard_id))
            .replace("{prefix}", &self.prefix)
            .replace("{id}", &shard_id.to_string())
    }
    
    /// 生成所有分片的连接字符串
    pub fn generate_all_connections(&self) -> Vec<(u32, String)> {
        (0..self.total_shards)
            .map(|id| (id, self.generate_connection_string(id)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    
    #[test]
    fn test_yearly_strategy() {
        let strategy = YearlyStrategy;
        let dt = Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap();
        
        assert_eq!(strategy.calculate(dt, 12), 2024 % 12);
        assert_eq!(strategy.name(), "yearly");
    }
    
    #[test]
    fn test_monthly_strategy() {
        let strategy = MonthlyStrategy;
        let dt = Utc.with_ymd_and_hms(2024, 3, 15, 0, 0, 0).unwrap();
        
        // month() returns 1-indexed: March = 3
        // 2024 * 12 + 3 = 24291
        assert_eq!(strategy.calculate(dt, 100), 24291 % 100);
        assert_eq!(strategy.name(), "monthly");
    }
    
    #[test]
    fn test_daily_strategy() {
        let strategy = DailyStrategy;
        let dt = Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap();
        
        let days = dt.num_days_from_ce();
        assert_eq!(strategy.calculate(dt, 100), days as u32 % 100);
        assert_eq!(strategy.name(), "daily");
    }
    
    #[test]
    fn test_shard_router() {
        let mut router = ShardRouter::with_strategy("yearly", 12);
        
        // 2024 % 12 = 8, so register shard 8
        router.register_shard(8, "db_2024".to_string(), "sqlite:./data/db_2024.db".to_string());
        router.register_shard(5, "db_2025".to_string(), "sqlite:./data/db_2025.db".to_string());
        
        let dt = Utc.with_ymd_and_hms(2024, 6, 15, 0, 0, 0).unwrap();
        let calculated_shard = router.calculate_shard(dt, "");
        let shard = router.route(dt);
        
        assert!(shard.is_some(), "Expected shard to be Some, but got None. Calculated shard: {}", calculated_shard);
        assert_eq!(shard.unwrap().name, "db_2024");
    }
    
    #[test]
    fn test_shard_config() {
        let config = ShardConfig::new("yearly", 12, "order", "postgresql://localhost/{shard}");
        
        assert_eq!(config.generate_connection_string(4), "postgresql://localhost/order_4");
        assert_eq!(config.strategy, "yearly");
    }
    
    #[test]
    fn test_router_with_config() {
        let config = ShardConfig::new("yearly", 4, "data", "postgresql://localhost/{shard}");
        let router = ShardRouter::with_config(&config);
        
        assert_eq!(router.total_shards(), 4);
        assert_eq!(router.all_shards().len(), 4);
        assert_eq!(router.strategy_name(), "yearly");
    }
}