// Copyright (c) 2026 Kirky.X
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
use dbnexus::{SagaAction, SagaError, SagaExecutionResult, SagaOrchestrator, SagaStep, ShardConfig, ShardRouter};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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
                "{}: 扣款失败（模拟故障）",
                self.name
            )))
        } else {
            self.executed.store(true, Ordering::SeqCst);
            println!("    ✅ [正向] {} 执行成功", self.name);
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
        println!("    🔙 [补偿] {} 执行成功", self.name);
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
                "{}: 入账失败（模拟故障）",
                self.name
            )))
        } else {
            self.executed.store(true, Ordering::SeqCst);
            println!("    ✅ [正向] {} 执行成功", self.name);
            Ok(())
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

fn print_result(result: &SagaExecutionResult) {
    println!();
    println!("  执行结果:");
    println!("  - saga_id   : {}", result.saga_id);
    println!("  - 成功      : {}", result.success);
    println!("  - 状态      : {:?}", result.status);
    println!("  - 已完成步骤: {:?}", result.completed_steps);
    println!("  - 已补偿步骤: {:?}", result.compensated_steps);
    if let Some(ref failure) = result.failure {
        println!("  - 失败步骤  : {}", failure.step_name);
        println!("  - 失败原因  : {}", failure.error);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🔗 DBNexus 分布式事务 Saga 编排器示例");
    println!("========================================\n");

    // ============================================
    // 1. 创建分片路由器（Saga 需要 ShardRouter）
    // ============================================
    println!("--- 1. 创建分片路由器 ---\n");

    let config = ShardConfig::new("hash", 2, "shard", "sqlite::memory:");
    let router = ShardRouter::with_config(&config).await?;
    let router = Arc::new(router);

    println!("  ✓ 路由器创建成功");
    println!("  - 分片数: {}", router.total_shards());
    println!("  - 已初始化池: {:?}", router.initialized_shards());
    println!();

    // ============================================
    // 2. 成功场景：跨分片转账全部成功
    // ============================================
    println!("--- 2. 成功场景：跨分片转账 ---\n");
    println!("  场景: 用户 A (shard 0) → 用户 B (shard 1) 转账 100 元\n");

    let debit_executed = Arc::new(AtomicBool::new(false));
    let credit_executed = Arc::new(AtomicBool::new(false));

    let steps = vec![
        SagaStep {
            name: "扣款-账户A".to_string(),
            shard_id: 0,
            action: Box::new(DebitAction {
                name: "扣款-账户A".to_string(),
                should_fail: false,
                executed: Arc::clone(&debit_executed),
            }),
            compensation: Box::new(RefundAction {
                name: "退款-账户A".to_string(),
            }),
        },
        SagaStep {
            name: "入账-账户B".to_string(),
            shard_id: 1,
            action: Box::new(CreditAction {
                name: "入账-账户B".to_string(),
                should_fail: false,
                executed: Arc::clone(&credit_executed),
            }),
            compensation: Box::new(RefundAction {
                name: "退款-账户B".to_string(),
            }),
        },
    ];

    let orchestrator = SagaOrchestrator::new(Arc::clone(&router));
    let result = orchestrator.execute_saga(steps).await;

    print_result(&result);
    println!();

    // 查看 Saga 日志
    if let Some(log) = orchestrator.get_saga_log(&result.saga_id) {
        println!("  Saga 日志:");
        println!("  - saga_id: {}", log.saga_id);
        println!("  - 状态   : {:?}", log.status);
        for step_log in &log.steps {
            println!(
                "    - {} (shard {}): action_success={}",
                step_log.name, step_log.shard_id, step_log.action_success
            );
        }
    }
    println!();

    // ============================================
    // 3. 失败场景：第二步失败触发补偿
    // ============================================
    println!("--- 3. 失败场景：入账失败触发补偿 ---\n");
    println!("  场景: 扣款成功，但入账失败 → 自动补偿扣款\n");

    let steps = vec![
        SagaStep {
            name: "扣款-账户A".to_string(),
            shard_id: 0,
            action: Box::new(DebitAction {
                name: "扣款-账户A".to_string(),
                should_fail: false,
                executed: Arc::new(AtomicBool::new(false)),
            }),
            compensation: Box::new(RefundAction {
                name: "退款-账户A".to_string(),
            }),
        },
        SagaStep {
            name: "入账-账户B".to_string(),
            shard_id: 1,
            action: Box::new(CreditAction {
                name: "入账-账户B".to_string(),
                should_fail: true, // 模拟入账失败
                executed: Arc::new(AtomicBool::new(false)),
            }),
            compensation: Box::new(RefundAction {
                name: "退款-账户B".to_string(),
            }),
        },
    ];

    let orchestrator2 = SagaOrchestrator::new(Arc::clone(&router));
    let result2 = orchestrator2.execute_saga(steps).await;

    print_result(&result2);
    println!();

    // ============================================
    // 4. Saga 状态流转说明
    // ============================================
    println!("--- 4. Saga 状态流转 ---\n");
    println!("  ┌─────────┐    全部成功    ┌───────────┐");
    println!("  │ Running │ ──────────────→ │ Completed │");
    println!("  └────┬────┘                └───────────┘");
    println!("       │ 某步失败");
    println!("       ▼");
    println!("  ┌─────────────┐  补偿完成  ┌─────────┐");
    println!("  │ Compensating│ ──────────→│ Failed  │");
    println!("  └─────────────┘            └─────────┘");
    println!();

    // ============================================
    // 5. SagaError 类型
    // ============================================
    println!("--- 5. SagaError 类型 ---\n");
    println!("  ┌──────────────────────┬──────────────────────────────────┐");
    println!("  │ 变体                 │ 说明                             │");
    println!("  ├──────────────────────┼──────────────────────────────────┤");
    println!("  │ ExecutionFailed      │ 正向动作执行失败                 │");
    println!("  │ CompensationFailed   │ 补偿动作执行失败                 │");
    println!("  │ Timeout              │ Saga 执行超时                    │");
    println!("  └──────────────────────┴──────────────────────────────────┘");
    println!();

    println!("========================================");
    println!("✨ 分布式事务 Saga 示例完成！");
    println!("========================================");
    println!("\n📚 关键 API:");
    println!("  - SagaOrchestrator::new(router)");
    println!("  - SagaStep {{ name, shard_id, action, compensation }}");
    println!("  - orchestrator.execute_saga(steps) -> SagaExecutionResult");
    println!("  - orchestrator.get_saga_log(saga_id) -> Option<SagaLog>");
    Ok(())
}
