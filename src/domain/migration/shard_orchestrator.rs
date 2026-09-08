// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 分片迁移编排 — 对 N 个分片执行统一迁移的协调器

use std::path::Path;
use std::sync::Arc;

use futures::stream::{FuturesUnordered, StreamExt};

use crate::database::sharding::ShardRouter;

// ============================================================================
// 类型定义
// ============================================================================

/// 单分片迁移结果
#[derive(Debug, Clone)]
pub struct ShardMigrationResult {
    /// 分片 ID
    pub shard_id: u32,
    /// 是否成功
    pub success: bool,
    /// 已应用的迁移版本
    pub applied_versions: Vec<u32>,
    /// 错误信息
    pub error: Option<String>,
}

/// 全局迁移结果
#[derive(Debug, Clone)]
pub struct OrchestratedMigrationResult {
    /// 总分片数
    pub total_shards: u32,
    /// 成功分片数
    pub success_count: u32,
    /// 失败分片列表
    pub failed_shards: Vec<ShardMigrationResult>,
    /// 所有分片结果
    pub results: Vec<ShardMigrationResult>,
}

// ============================================================================
// ShardMigrationOrchestrator
// ============================================================================

/// 分片迁移编排器
///
/// 遍历所有分片连接池，对每个分片执行相同迁移文件。
/// 支持并行和串行模式，部分失败不阻断其他分片。
///
/// 注意：分片级迁移执行尚未接入 `MigrationExecutor`，当前为占位实现，
/// `orchestrate_migration` 会对每个分片返回 `success: false` 的失败结果。
pub struct ShardMigrationOrchestrator {
    router: Arc<ShardRouter>,
    parallel: bool,
}

impl ShardMigrationOrchestrator {
    /// 创建编排器
    pub fn new(router: Arc<ShardRouter>, parallel: bool) -> Self {
        Self { router, parallel }
    }

    /// 执行跨分片迁移编排
    ///
    /// 当前为占位实现：不会对任何分片执行迁移，返回的每个分片结果
    /// 均为 `success: false` 且 `error` 说明尚未实现。
    pub async fn orchestrate_migration(
        &self,
        _migrations_dir: &Path,
    ) -> OrchestratedMigrationResult {
        let shards = self.router.all_shards();
        let total_shards = shards.len() as u32;

        if self.parallel {
            self.orchestrate_parallel(shards, total_shards).await
        } else {
            self.orchestrate_serial(shards, total_shards).await
        }
    }

    async fn orchestrate_parallel(
        &self,
        shards: Vec<&crate::database::sharding::ShardInfo>,
        total_shards: u32,
    ) -> OrchestratedMigrationResult {
        let mut futures = FuturesUnordered::new();

        for shard_info in shards {
            let shard_id = shard_info.shard_id;
            futures.push(async move {
                // 占位实现：分片级迁移尚未接入 MigrationExecutor，未执行任何迁移。
                // 返回明确的失败结果，避免向调用方报告虚假成功。
                ShardMigrationResult {
                    shard_id,
                    success: false,
                    applied_versions: Vec::new(),
                    error: Some(
                        "分片迁移编排尚未实现：未接入 MigrationExecutor，该分片未执行任何迁移"
                            .to_string(),
                    ),
                }
            });
        }

        let mut results = Vec::new();
        while let Some(result) = futures.next().await {
            results.push(result);
        }

        Self::summarize(total_shards, results)
    }

    async fn orchestrate_serial(
        &self,
        shards: Vec<&crate::database::sharding::ShardInfo>,
        total_shards: u32,
    ) -> OrchestratedMigrationResult {
        let mut results = Vec::new();

        for shard_info in shards {
            let shard_id = shard_info.shard_id;
            results.push(ShardMigrationResult {
                shard_id,
                success: false,
                applied_versions: Vec::new(),
                error: Some(
                    "分片迁移编排尚未实现：未接入 MigrationExecutor，该分片未执行任何迁移"
                        .to_string(),
                ),
            });
        }

        Self::summarize(total_shards, results)
    }

    fn summarize(
        total_shards: u32,
        results: Vec<ShardMigrationResult>,
    ) -> OrchestratedMigrationResult {
        let success_count = results.iter().filter(|r| r.success).count() as u32;
        let failed_shards: Vec<_> = results.iter().filter(|r| !r.success).cloned().collect();

        OrchestratedMigrationResult {
            total_shards,
            success_count,
            failed_shards,
            results,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个注册了 `shard_count` 个分片信息的路由器（无需真实连接池）
    fn make_router(shard_count: u32) -> Arc<ShardRouter> {
        let mut router = ShardRouter::with_strategy("yearly", shard_count);
        for shard_id in 0..shard_count {
            router.register_shard(
                shard_id,
                format!("shard_{}", shard_id),
                format!("sqlite://shard_{}.db", shard_id),
            );
        }
        Arc::new(router)
    }

    /// 并行编排为占位实现：每个分片返回 success=false 且 error 非空（虚假成功已修复）
    #[tokio::test]
    async fn test_orchestrate_parallel_reports_unimplemented_failure() {
        let orchestrator = ShardMigrationOrchestrator::new(make_router(2), true);
        let result = orchestrator
            .orchestrate_migration(Path::new("./migrations"))
            .await;

        assert_eq!(result.total_shards, 2);
        assert_eq!(result.results.len(), 2);
        assert_eq!(result.success_count, 0, "占位实现不应报告任何成功分片");
        assert_eq!(result.failed_shards.len(), 2);
        for shard in &result.results {
            assert!(!shard.success);
            assert!(shard.error.is_some(), "error 应说明尚未实现");
            assert!(
                shard.error.as_deref().unwrap().contains("尚未实现"),
                "实际错误: {:?}",
                shard.error
            );
            assert!(shard.applied_versions.is_empty());
        }
    }

    /// 串行编排同样返回诚实失败
    #[tokio::test]
    async fn test_orchestrate_serial_reports_unimplemented_failure() {
        let orchestrator = ShardMigrationOrchestrator::new(make_router(3), false);
        let result = orchestrator
            .orchestrate_migration(Path::new("./migrations"))
            .await;

        assert_eq!(result.total_shards, 3);
        assert_eq!(result.results.len(), 3);
        assert_eq!(result.success_count, 0);
        assert_eq!(result.failed_shards.len(), 3);
        for shard in &result.results {
            assert!(!shard.success);
            assert!(shard.error.is_some());
        }
    }
}
