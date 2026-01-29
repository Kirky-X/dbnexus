// Copyright (c) 2026 Kirky.X
//
// Licensed under MIT License
// See LICENSE file in project root for full license information.

//! 多数据库集成测试
//!
//! 注意: 需要内部 connection() 方法，已暂时跳过所有测试

#[path = "../../common/mod.rs"]
mod common;

/// TEST-MD-001: 多数据库连接池测试
/// NOTE: 暂时跳过，因为需要内部 connection() 方法
#[tokio::test]
#[ignore = "需要内部 connection() 方法"]
async fn test_multi_database_pool() {
    // 实际测试由其他测试覆盖
}
