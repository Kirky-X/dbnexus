// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 分布式事务 Saga 编排器示例
//!
//! 演示 `SagaOrchestrator` 的使用：
//! - 定义 `SagaStep`（正向动作 + 补偿动作）
//! - 实现 `SagaAction` trait
//! - 成功场景：所有步骤顺序执行
//! - 失败场景：正向失败后逆序补偿
//! - 查看 `SagaLog` 执行日志
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --bin saga
//! ```

use async_trait::async_trait;
use dbnexus::{
    SagaAction, SagaError, SagaExecutionResult, SagaOrchestrator, SagaStep, ShardConfig,
    ShardRouter,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

// ============================================
// 自定义 Saga 动作
// ============================================

/// 转账动作：从账户 A 扣款
struct DebitAction {
    name: String,
    should_fail: bool,
    executed: Arc<AtomicBool>,
}

#[async_trait]
impl SagaAction for DebitAction {
    async fn execute(&self, _session: &dbnexus::Session) -> Result<(), SagaError> {
        if self.should_fail {
            Err(SagaError::ExecutionFailed(format!(
                "{}: debit failed (simulated fault)",
                self.name
            )))
        } else {
            self.executed.store(true, Ordering::SeqCst);
            println!("    ✅ [forward] {} succeeded", self.name);
            Ok(())
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// 补偿动作：退款回账户 A
struct RefundAction {
    name: String,
}

#[async_trait]
impl SagaAction for RefundAction {
    async fn execute(&self, _session: &dbnexus::Session) -> Result<(), SagaError> {
        println!("    🔙 [compensation] {} succeeded", self.name);
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// 转账动作：向账户 B 入账
struct CreditAction {
    name: String,
    should_fail: bool,
    executed: Arc<AtomicBool>,
}

#[async_trait]
impl SagaAction for CreditAction {
    async fn execute(&self, _session: &dbnexus::Session) -> Result<(), SagaError> {
        if self.should_fail {
            Err(SagaError::ExecutionFailed(format!(
                "{}: credit failed (simulated fault)",
                self.name
            )))
        } else {
            self.executed.store(true, Ordering::SeqCst);
            println!("    ✅ [forward] {} succeeded", self.name);
            Ok(())
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

fn print_result(result: &SagaExecutionResult) {
    println!();
    println!("  Execution result:");
    println!("  - saga_id   : {}", result.saga_id);
    println!("  - succeeded    : {}", result.success);
    println!("  - status      : {:?}", result.status);
    println!("  - completed steps: {:?}", result.completed_steps);
    println!("  - compensated steps: {:?}", result.compensated_steps);
    if let Some(ref failure) = result.failure {
        println!("  - failed step   : {}", failure.step_name);
        println!("  - failure reason : {}", failure.error);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🔗 DBNexus distributed transaction Saga orchestrator example");
    println!("========================================\n");

    // ============================================
    // 创建分片路由器（Saga 需要 ShardRouter）
    // ============================================
    println!("--- 1. create the shard router ---\n");

    let config = ShardConfig::new("hash", 2, "shard", "sqlite::memory:");
    let router = ShardRouter::with_config(&config).await?;
    let router = Arc::new(router);

    println!("  ✓ Router created");
    println!("  - shard count: {}", router.total_shards());
    println!("  - initialized pools: {:?}", router.initialized_shards());
    println!();

    // ============================================
    // 成功场景：跨分片转账全部成功
    // ============================================
    println!("--- 2. success scenario: cross-shard transfer ---\n");
    println!("  Scenario: user A (shard 0) → user B (shard 1) transfer 100\n");

    let debit_executed = Arc::new(AtomicBool::new(false));
    let credit_executed = Arc::new(AtomicBool::new(false));

    let steps = vec![
        SagaStep {
            name: "debit-accountA".to_string(),
            shard_id: 0,
            action: Box::new(DebitAction {
                name: "debit-accountA".to_string(),
                should_fail: false,
                executed: Arc::clone(&debit_executed),
            }),
            compensation: Box::new(RefundAction {
                name: "refund-accountA".to_string(),
            }),
        },
        SagaStep {
            name: "credit-accountB".to_string(),
            shard_id: 1,
            action: Box::new(CreditAction {
                name: "credit-accountB".to_string(),
                should_fail: false,
                executed: Arc::clone(&credit_executed),
            }),
            compensation: Box::new(RefundAction {
                name: "refund-accountB".to_string(),
            }),
        },
    ];

    let orchestrator = SagaOrchestrator::new(Arc::clone(&router));
    let result = orchestrator.execute_saga(steps).await;

    print_result(&result);
    println!();

    // 查看 Saga 日志
    if let Some(log) = orchestrator.get_saga_log(&result.saga_id).await {
        println!("  Saga log:");
        println!("  - saga_id: {}", log.saga_id);
        println!("  - status   : {:?}", log.status);
        for step_log in &log.steps {
            println!(
                "    - {} (shard {}): action_success={}",
                step_log.name, step_log.shard_id, step_log.action_success
            );
        }
    }
    println!();

    // ============================================
    // 失败场景：第二步失败触发补偿
    // ============================================
    println!("--- 3. failure scenario: credit failure triggers compensation ---\n");
    println!("  Scenario: debit succeeded but credit failed → auto-compensate the debit\n");

    let steps = vec![
        SagaStep {
            name: "debit-accountA".to_string(),
            shard_id: 0,
            action: Box::new(DebitAction {
                name: "debit-accountA".to_string(),
                should_fail: false,
                executed: Arc::new(AtomicBool::new(false)),
            }),
            compensation: Box::new(RefundAction {
                name: "refund-accountA".to_string(),
            }),
        },
        SagaStep {
            name: "credit-accountB".to_string(),
            shard_id: 1,
            action: Box::new(CreditAction {
                name: "credit-accountB".to_string(),
                should_fail: true,
                executed: Arc::new(AtomicBool::new(false)),
            }),
            compensation: Box::new(RefundAction {
                name: "refund-accountB".to_string(),
            }),
        },
    ];

    let orchestrator2 = SagaOrchestrator::new(Arc::clone(&router));
    let result2 = orchestrator2.execute_saga(steps).await;

    print_result(&result2);
    println!();

    // ============================================
    // Saga 状态流转说明
    // ============================================
    println!("--- 4. Saga state transitions ---\n");
    println!("  ┌─────────┐    all succeeded    ┌───────────┐");
    println!("  │ Running │ ──────────────→ │ Completed │");
    println!("  └────┬────┘                └───────────┘");
    println!("       │ a step failed");
    println!("       ▼");
    println!("  ┌─────────────┐  compensation done  ┌─────────┐");
    println!("  │ Compensating│ ──────────→│ Failed  │");
    println!("  └─────────────┘            └─────────┘");
    println!();

    // ============================================
    // SagaError 类型
    // ============================================
    println!("--- 5. SagaError types ---\n");
    println!("  ┌──────────────────────┬──────────────────────────────────┐");
    println!("  │ variant              │ description                              │");
    println!("  ├──────────────────────┼──────────────────────────────────┤");
    println!("  │ ExecutionFailed      │ the forward action failed                │");
    println!("  │ CompensationFailed   │ the compensation action failed           │");
    println!("  │ Timeout              │ the Saga execution timed out             │");
    println!("  └──────────────────────┴──────────────────────────────────┘");
    println!();

    println!("========================================");
    println!("✨ Distributed transaction Saga example complete!");
    println!("========================================");
    println!("\n📚 Key APIs:");
    println!("  - SagaOrchestrator::new(router)");
    println!("  - SagaStep {{ name, shard_id, action, compensation }}");
    println!("  - orchestrator.execute_saga(steps) -> SagaExecutionResult");
    println!("  - orchestrator.get_saga_log(saga_id).await -> Option<SagaLog>");
    Ok(())
}
