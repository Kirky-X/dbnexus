// Copyright (c) 2025-2026 Kirky.X🌠
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
    println!("🆔 DBNexus distributed ID generator example");
    println!("========================================\n");

    // 创建 Snowflake ID 生成器
    println!("--- 1. create a generator ---\n");

    let machine_id = 42;
    let epoch = 1_700_000_000_000; // 自定义纪元：2023-11-14T22:13:20Z

    let generator = SnowflakeIdGenerator::new(machine_id, epoch).expect("valid machine_id");
    println!("  ✓ generator created");
    println!("  - machine_id : {}", machine_id);
    println!("  - epoch      : {} (custom epoch)", epoch);
    println!();

    // 批量生成 ID
    println!("--- 2. batch generate IDs ---\n");

    let count = 10;
    let mut ids = Vec::with_capacity(count);

    for i in 0..count {
        let id = generator.next_id().expect("generate ID");
        ids.push(id);
        println!("  [{:>2}] ID = {}", i, id);
    }
    println!();

    // 解析 ID 组成
    println!("--- 3. parse the ID structure ---\n");

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

    let sample = generator.parse_id(ids[0]);
    println!("  detailed breakdown (first ID):");
    println!(
        "  - timestamp_ms : {} ms (counted from the custom epoch)",
        sample.timestamp_ms
    );
    println!("  - machine_id   : {}", sample.machine_id);
    println!("  - sequence     : {}", sample.sequence);
    println!();

    // 唯一性验证
    println!("--- 4. uniqueness verification ---\n");

    let large_count = 10_000;
    let mut unique_ids = HashSet::with_capacity(large_count);
    let mut duplicates = 0;

    for _ in 0..large_count {
        let id = generator.next_id().expect("generate ID");
        if !unique_ids.insert(id) {
            duplicates += 1;
        }
    }

    println!("  - generated count: {}", large_count);
    println!("  - unique count : {}", unique_ids.len());
    println!("  - duplicate count: {}", duplicates);
    println!(
        "  - conclusion    : {}",
        if duplicates == 0 {
            "✅ zero duplicates, IDs globally unique"
        } else {
            "❌ duplicates found!"
        }
    );
    println!();

    // 多线程并发生成
    println!("--- 5. multithreaded concurrent generation ---\n");

    let thread_count = 4;
    let ids_per_thread = 2_500;
    let generator = Arc::new(SnowflakeIdGenerator::new(1, epoch).expect("valid"));
    let mut handles = Vec::new();

    for t in 0..thread_count {
        let id_gen = Arc::clone(&generator);
        handles.push(thread::spawn(move || {
            let mut local_ids = Vec::with_capacity(ids_per_thread);
            for _ in 0..ids_per_thread {
                local_ids.push(id_gen.next_id().expect("generate ID"));
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
            "  - thread {}: generated {} IDs, locally unique {}",
            thread_id,
            ids_per_thread,
            local_unique.len()
        );
        total_generated += ids_per_thread;
        all_ids.extend(local_unique);
    }

    println!();
    println!("  - total generated: {}", total_generated);
    println!("  - globally unique : {}", all_ids.len());
    println!(
        "  - cross-thread duplicates: {}",
        total_generated - all_ids.len()
    );
    println!(
        "  - conclusion    : {}",
        if all_ids.len() == total_generated {
            "✅ zero duplicates across threads"
        } else {
            "❌ cross-thread duplicates found!"
        }
    );
    println!();

    // 错误场景：machine_id 溢出
    println!("--- 6. error scenarios ---\n");

    match SnowflakeIdGenerator::new(1024, epoch) {
        Ok(_) => println!("  ❌ should not have succeeded"),
        Err(e) => println!("  ✓ machine_id=1024 correctly rejected: {}", e),
    }

    match SnowflakeIdGenerator::new(9999, epoch) {
        Ok(_) => println!("  ❌ should not have succeeded"),
        Err(e) => println!("  ✓ machine_id=9999 correctly rejected: {}", e),
    }

    // machine_id=1023 是合法最大值
    match SnowflakeIdGenerator::new(1023, epoch) {
        Ok(_) => println!("  ✓ machine_id=1023 (max legal value) created"),
        Err(e) => println!("  ❌ should not have failed: {}", e),
    }
    println!();

    // ID 布局说明
    println!("--- 7. Snowflake ID layout ---\n");
    println!("  ┌───┬──────────────────────────────┬───────────────┬──────────────┐");
    println!("  │ 0 │     timestamp (41 bits)      │ machine (10b) │ seq (12 bit) │");
    println!("  └───┴──────────────────────────────┴───────────────┴──────────────┘");
    println!();
    println!("  - sign bit   : 1 bit (always 0, guaranteeing a positive number)");
    println!("  - timestamp: 41 bits ≈ 69 years (ms precision, from the custom epoch)");
    println!("  - machine_id: 10 bits (0-1023, 1024 nodes supported)");
    println!("  - sequence  : 12 bits (0-4095, 4096 IDs per ms per node)");
    println!();

    println!("========================================");
    println!("✨ Distributed ID generator example complete!");
    println!("========================================");
    println!("\n📚 Key APIs:");
    println!("  - SnowflakeIdGenerator::new(machine_id, epoch)");
    println!("  - generator.next_id() -> Result<u64, SnowflakeError>");
    println!("  - generator.parse_id(id) -> IdComponents");
}
