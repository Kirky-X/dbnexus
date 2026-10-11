// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 结构化错误报告示例
//!
//! 演示 [`QueryErrorReport`] 与 [`ErrorCategory`] 的使用：
//! - 构造 4 类错误报告（Permission / InjectionRisk / SyntaxError / ShardConflict）
//! - 链式 `with_table` / `with_operation` 设置上下文
//! - [`Display`](std::fmt::Display) 输出格式
//! - 通过 `From<DbNexusError>` 自动转换
//! - 模拟分片冲突场景：调用 [`ShardRouter::enforce_shard_binding`]
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example error_handling --features "sqlite,sharding,permission"
//! ```

use dbnexus::{ErrorCategory, QueryErrorReport, ShardConfig, ShardRouter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🚨 DBNexus structured error report example");
    println!("========================================\n");

    // ============================================
    // Permission 类别
    // ============================================
    println!("--- 1. ErrorCategory::Permission ---\n");
    let perm_err = QueryErrorReport::new(
        ErrorCategory::Permission,
        "Role 'viewer' is not allowed to DELETE on table 'users'",
        "Grant DELETE permission to role 'viewer' or use a role with elevated privileges",
    )
    .with_table("users")
    .with_operation("DELETE");

    println!("{}", perm_err);
    println!();

    // ============================================
    // InjectionRisk 类别
    // ============================================
    println!("--- 2. ErrorCategory::InjectionRisk ---\n");
    let injection_err = QueryErrorReport::new(
        ErrorCategory::InjectionRisk,
        "SQL contains UNION-based injection pattern: '1=1 UNION SELECT password FROM credentials'",
        "Use parameterized queries (? placeholders) and validate user input length and charset",
    )
    .with_table("users")
    .with_operation("SELECT");

    println!("{}", injection_err);
    println!();

    // ============================================
    // SyntaxError 类别
    // ============================================
    println!("--- 3. ErrorCategory::SyntaxError ---\n");
    let syntax_err = QueryErrorReport::new(
        ErrorCategory::SyntaxError,
        "Failed to parse SQL: unexpected token 'FORM' (did you mean 'FROM'?)",
        "Check SQL keyword spelling and consult the SQL grammar reference",
    )
    .with_operation("SELECT");

    println!("{}", syntax_err);
    println!();

    // ============================================
    // ShardConflict 类别（来自 ShardRouter::enforce_shard_binding）
    // ============================================
    println!("--- 4. ErrorCategory::ShardConflict (via ShardRouter) ---\n");

    // 配置 4 个分片但不创建实际连接池（仅用于路由计算）
    let config = ShardConfig::new("hash", 4, "shard", "sqlite:./data/{shard}.db");
    let router = ShardRouter::with_config_sync(&config);

    // 选一个 shard_key 并取得绑定分片
    let bound_key = "user_42";
    let bound_shard_id = router.shard_id_for_key(bound_key);
    println!(
        "  binding: shard_key='{}' → shard_id={}",
        bound_key, bound_shard_id
    );

    // 用一个落在不同分片的 shard_key 触发冲突
    // 通过遍历找出一个冲突的 key（保证示例可重现）
    let conflict_key = (0..1000)
        .map(|i| format!("user_{}", i))
        .find(|k| router.shard_id_for_key(k) != bound_shard_id)
        .expect("should find a conflict key in 1000 attempts");

    println!(
        "  conflict: shard_key='{}' → shard_id={}",
        conflict_key,
        router.shard_id_for_key(&conflict_key)
    );

    let shard_err = router
        .enforce_shard_binding(bound_shard_id, &conflict_key)
        .expect_err("enforce_shard_binding should detect cross-shard conflict");

    println!();
    println!("{}", shard_err);
    println!();

    // ============================================
    // 字段访问与断言
    // ============================================
    println!("--- 5. field access and assertions ---\n");
    println!("  perm_err.category    = {}", perm_err.category);
    println!("  perm_err.table       = {:?}", perm_err.table);
    println!("  perm_err.operation   = {:?}", perm_err.operation);
    println!("  shard_err.category   = {}", shard_err.category);
    assert_eq!(perm_err.category, ErrorCategory::Permission);
    assert_eq!(injection_err.category, ErrorCategory::InjectionRisk);
    assert_eq!(syntax_err.category, ErrorCategory::SyntaxError);
    assert_eq!(shard_err.category, ErrorCategory::ShardConflict);
    println!("  ✓ all 4 category assertions passed");

    // ============================================
    // Display 与 Error trait 兼容性
    // ============================================
    println!("\n--- 6. std::error::Error trait compatibility ---\n");
    let err_boxed: Box<dyn std::error::Error> = Box::new(perm_err.clone());
    println!("  Box<dyn Error> display: {}", err_boxed);
    println!("  source() = {:?}", err_boxed.source());

    println!("\n========================================");
    println!("✨ Structured error report example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - QueryErrorReport::new(category, message, suggestion)");
    println!("  - .with_table(name) / .with_operation(op)   chainable context setters");
    println!("  - ErrorCategory: Permission | InjectionRisk | SyntaxError | ShardConflict");
    println!("  - ShardRouter::enforce_shard_binding(expected, key) auto-reports ShardConflict");
    println!("  - impl From<DbNexusError> for QueryErrorReport  automatic conversion");
    println!("  - impl std::error::Error  standard error-handling chain compatibility");

    Ok(())
}
