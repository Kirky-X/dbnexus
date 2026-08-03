// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 分布式 ID 生成器 — Snowflake 默认实现
//!
//! 64 位 ID 布局：1 bit 符号 + 41 bits 时间戳 + 10 bits machine_id + 12 bits 序列号

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// IdComponents — ID 解析结果
// ============================================================================

/// ID 组成成分
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdComponents {
    /// 时间戳（毫秒，自定义 epoch 起算）
    pub timestamp_ms: u64,
    /// 机器 ID（0-1023）
    pub machine_id: u32,
    /// 序列号（0-4095）
    pub sequence: u32,
}

// ============================================================================
// DistributedIdGenerator trait
// ============================================================================

/// 分布式 ID 生成器 trait
pub trait DistributedIdGenerator: Send + Sync {
    /// 生成下一个 ID
    fn next_id(&self) -> u64;
    /// 解析 ID 组成
    fn parse_id(&self, id: u64) -> IdComponents;
}

// ============================================================================
// SnowflakeIdGenerator
// ============================================================================

/// Snowflake ID 生成器
///
/// 线程安全无锁实现（AtomicU64 + AtomicU32）。
///
/// # ID 布局
///
/// ```text
/// 0                   1                   2                   3
/// 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |0|                    timestamp (41 bits)                      |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |          machine_id (10 bits)     |    sequence (12 bits)     |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// ```
pub struct SnowflakeIdGenerator {
    machine_id: u32,
    epoch: u64,
    /// 组合原子变量：高 41 bits 时间戳 + 低 12 bits 序列号
    /// 通过 fetch_add 原子递增保证并发唯一性
    ts_seq: AtomicU64,
}

impl SnowflakeIdGenerator {
    /// 创建生成器
    ///
    /// # 参数
    /// - `machine_id`: 机器 ID（0-1023）
    /// - `epoch`: 自定义纪元（毫秒，UNIX 时间戳起算）
    ///
    /// # 错误
    /// machine_id 超过 1023 时返回错误
    pub fn new(machine_id: u32, epoch: u64) -> Result<Self, String> {
        if machine_id > 1023 {
            return Err(format!("machine_id must be 0-1023, got {machine_id}"));
        }
        Ok(Self {
            machine_id,
            epoch,
            ts_seq: AtomicU64::new(0),
        })
    }

    /// 获取当前时间戳（毫秒，自定义 epoch 起算）
    fn current_timestamp(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64
            - self.epoch
    }

    /// spin-wait 直到下一毫秒
    fn wait_next_millis(&self, last_ts: u64) -> u64 {
        let mut ts = self.current_timestamp();
        let mut spins = 0;
        while ts <= last_ts {
            std::hint::spin_loop();
            ts = self.current_timestamp();
            spins += 1;
            if spins > 100_000 {
                break;
            }
        }
        ts
    }
}

impl DistributedIdGenerator for SnowflakeIdGenerator {
    fn next_id(&self) -> u64 {
        loop {
            let timestamp = self.current_timestamp();
            let current = self.ts_seq.load(Ordering::SeqCst);
            let last_ts = current >> 12;

            if timestamp < last_ts {
                // 时钟回拨：spin-wait 最多 1ms
                let waited = self.wait_next_millis(last_ts);
                if waited < last_ts {
                    return 0; // 仍然回拨，返回 0 表示错误
                }
                continue;
            }

            // 计算新的 ts_seq 值
            let new_ts_seq = if timestamp > last_ts {
                // 新毫秒：序列号重置为 0
                timestamp << 12
            } else {
                // 同一毫秒：递增序列号
                let seq = (current & 0xFFF) + 1;
                if seq > 0xFFF {
                    // 序列号溢出：等待下一毫秒后重试
                    self.wait_next_millis(last_ts);
                    continue;
                }
                (last_ts << 12) | seq
            };

            // CAS: 原子更新 ts_seq
            match self
                .ts_seq
                .compare_exchange(current, new_ts_seq, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => {
                    let seq = new_ts_seq & 0xFFF;
                    return (timestamp << 22) | ((self.machine_id as u64) << 12) | seq;
                }
                Err(_) => {
                    // CAS 失败：其他线程已更新，重试
                    continue;
                }
            }
        }
    }

    fn parse_id(&self, id: u64) -> IdComponents {
        let timestamp_ms = id >> 22;
        let machine_id = ((id >> 12) & 0x3FF) as u32;
        let sequence = (id & 0xFFF) as u32;

        IdComponents {
            timestamp_ms,
            machine_id,
            sequence,
        }
    }
}
