// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 分布式 ID 生成器示例
//!
//! 演示 `SnowflakeIdGenerator` 的使用：
//! - 创建生成器（machine_id + 自定义 epoch）
//! - 批量生成唯一 ID
//! - 解析 ID 组成（时间戳 / machine_id / 序列号）
//! - 并发安全验证（多线程同时生成）
//! - 错误场景展示（machine_id 溢出）
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --bin distributed_id
//! ```

use dbnexus::{DistributedIdGenerator, IdComponents, SnowflakeIdGenerator};
use std::collections::HashSet;
use std::sync::Arc;
use std::thread;

fn main() {
    println!("========================================");
    println!("🆔 DBNexus 分布式 ID 生成器示例");
    println!("========================================\n");

    // ============================================
    // 1. 创建 Snowflake ID 生成器
    // ============================================
    println!("--- 1. 创建生成器 ---\n");

    let machine_id = 42;
    let epoch = 1_700_000_000_000; // 自定义纪元：2023-11-14T22:13:20Z

    let generator = SnowflakeIdGenerator::new(machine_id, epoch).expect("valid machine_id");
    println!("  ✓ 生成器创建成功");
    println!("  - machine_id : {}", machine_id);
    println!("  - epoch      : {} (自定义纪元)", epoch);
    println!();

    // ============================================
    // 2. 批量生成 ID
    // ============================================
    println!("--- 2. 批量生成 ID ---\n");

    let count = 10;
    let mut ids = Vec::with_capacity(count);

    for i in 0..count {
        let id = generator.next_id().expect("generate ID");
        ids.push(id);
        println!("  [{:>2}] ID = {}", i, id);
    }
    println!();

    // ============================================
    // 3. 解析 ID 组成
    // ============================================
    println!("--- 3. 解析 ID 组成 ---\n");

    println!("  ┌────┬──────────────────┬────────────┬──────────┐");
    println!("  │ #  │ ID               │ timestamp  │ sequence │");
    println!("  ├────┼──────────────────┼────────────┼──────────┤");

    for (i, &id) in ids.iter().enumerate() {
        let components: IdComponents = generator.parse_id(id);
        println!(
            "  │ {:>2} │ {:>16} │ {:>10} │ {:>8} │",
            i, id, components.timestamp_ms, components.sequence
        );
    }

    println!("  └────┴──────────────────┴────────────┴──────────┘");
    println!();

    // 验证解析结果
    let sample = generator.parse_id(ids[0]);
    println!("  详细解析（第一个 ID）:");
    println!("  - timestamp_ms : {} ms (自定义 epoch 起算)", sample.timestamp_ms);
    println!("  - machine_id   : {}", sample.machine_id);
    println!("  - sequence     : {}", sample.sequence);
    println!();

    // ============================================
    // 4. 唯一性验证
    // ============================================
    println!("--- 4. 唯一性验证 ---\n");

    let large_count = 10_000;
    let mut unique_ids = HashSet::with_capacity(large_count);
    let mut duplicates = 0;

    for _ in 0..large_count {
        let id = generator.next_id().expect("generate ID");
        if !unique_ids.insert(id) {
            duplicates += 1;
        }
    }

    println!("  - 生成数量 : {}", large_count);
    println!("  - 唯一数量 : {}", unique_ids.len());
    println!("  - 重复数量 : {}", duplicates);
    println!(
        "  - 结论     : {}",
        if duplicates == 0 {
            "✅ 零重复，ID 全局唯一"
        } else {
            "❌ 存在重复！"
        }
    );
    println!();

    // ============================================
    // 5. 多线程并发生成
    // ============================================
    println!("--- 5. 多线程并发生成 ---\n");

    let thread_count = 4;
    let ids_per_thread = 2_500;
    let generator = Arc::new(SnowflakeIdGenerator::new(1, epoch).expect("valid"));
    let mut handles = Vec::new();

    for t in 0..thread_count {
        let gen = Arc::clone(&generator);
        handles.push(thread::spawn(move || {
            let mut local_ids = Vec::with_capacity(ids_per_thread);
            for _ in 0..ids_per_thread {
                local_ids.push(gen.next_id().expect("generate ID"));
            }
            (t, local_ids)
        }));
    }

    let mut all_ids = HashSet::new();
    let mut total_generated = 0;

    for handle in handles {
        let (thread_id, local_ids) = handle.join().expect("thread should not panic");
        let local_unique: HashSet<u64> = local_ids.into_iter().collect();
        println!(
            "  - 线程 {} : 生成 {} 个 ID，本地唯一 {}",
            thread_id,
            ids_per_thread,
            local_unique.len()
        );
        total_generated += ids_per_thread;
        all_ids.extend(local_unique);
    }

    println!();
    println!("  - 总生成   : {}", total_generated);
    println!("  - 全局唯一 : {}", all_ids.len());
    println!("  - 跨线程重复 : {}", total_generated - all_ids.len());
    println!(
        "  - 结论     : {}",
        if all_ids.len() == total_generated {
            "✅ 多线程并发零重复"
        } else {
            "❌ 存在跨线程重复！"
        }
    );
    println!();

    // ============================================
    // 6. 错误场景：machine_id 溢出
    // ============================================
    println!("--- 6. 错误场景 ---\n");

    match SnowflakeIdGenerator::new(1024, epoch) {
        Ok(_) => println!("  ❌ 不应该成功"),
        Err(e) => println!("  ✓ machine_id=1024 正确拒绝: {}", e),
    }

    match SnowflakeIdGenerator::new(9999, epoch) {
        Ok(_) => println!("  ❌ 不应该成功"),
        Err(e) => println!("  ✓ machine_id=9999 正确拒绝: {}", e),
    }

    // machine_id=1023 是合法最大值
    match SnowflakeIdGenerator::new(1023, epoch) {
        Ok(_) => println!("  ✓ machine_id=1023（最大合法值）创建成功"),
        Err(e) => println!("  ❌ 不应该失败: {}", e),
    }
    println!();

    // ============================================
    // 7. ID 布局说明
    // ============================================
    println!("--- 7. Snowflake ID 布局 ---\n");
    println!("  ┌───┬──────────────────────────────┬───────────────┬──────────────┐");
    println!("  │ 0 │     timestamp (41 bits)      │ machine (10b) │ seq (12 bit) │");
    println!("  └───┴──────────────────────────────┴───────────────┴──────────────┘");
    println!();
    println!("  - 符号位    : 1 bit（始终为 0，保证正数）");
    println!("  - 时间戳    : 41 bits ≈ 69 年（毫秒精度，自定义 epoch 起算）");
    println!("  - machine_id: 10 bits（0-1023，支持 1024 个节点）");
    println!("  - sequence  : 12 bits（0-4095，每毫秒每节点 4096 个 ID）");
    println!();

    println!("========================================");
    println!("✨ 分布式 ID 生成器示例完成！");
    println!("========================================");
    println!("\n📚 关键 API:");
    println!("  - SnowflakeIdGenerator::new(machine_id, epoch)");
    println!("  - generator.next_id() -> Result<u64, SnowflakeError>");
    println!("  - generator.parse_id(id) -> IdComponents");
}
