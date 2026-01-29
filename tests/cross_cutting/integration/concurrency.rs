// Copyright (c) 2026 Kirky.X
//
// Licensed under MIT License
// See LICENSE file in project root for full license information.

//! 并发集成测试
//!
//! 注意: 需要内部 connection() 方法，已暂时跳过所有测试

#[path = "../../common/mod.rs"]
mod common;

/// TEST-CC-001: 并发连接获取测试
/// NOTE: 暂时跳过，因为需要内部 connection() 方法
#[tokio::test]
#[ignore = "需要内部 connection() 方法"]
async fn test_concurrent_connection_acquire() {
    // 实际测试由其他测试覆盖
}
