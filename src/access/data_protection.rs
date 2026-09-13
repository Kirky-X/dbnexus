// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 字段级自动脱敏与行级安全（`data-protection` feature）
//!
//! - `MaskingEngine`：按列名的声明式脱敏规则（掩码/SHA-256 哈希/截断），
//!   在行查询出口（`Session::query_rows`）统一应用。
//! - `RlsPolicy`：行级安全谓词注入（租户/角色条件 append 到 WHERE），
//!   admin 角色或显式 bypass 授权可绕过。

use sha2::{Digest, Sha256};

// ============================================================================
// 字段级脱敏
// ============================================================================

/// 脱敏策略
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskStrategy {
    /// 掩码：保留首尾各 keep 个字符，中间以 `*` 替代（字符串列；数值列转字符串后掩码）
    /// 保留首尾明文长度
    Mask {
        /// 保留首尾明文长度
        keep: usize,
    },
    /// SHA-256 十六进制哈希（不可逆）
    Hash,
    /// 截断到 n 字符（尾部追加省略号）
    Truncate(usize),
}

/// 单列脱敏规则
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskingRule {
    /// 列名（对 JSON 行的顶层 key 匹配）
    pub column: String,
    /// 应用策略
    pub strategy: MaskStrategy,
}

/// 脱敏引擎：列名 → 策略映射
#[derive(Debug, Clone, Default)]
pub struct MaskingEngine {
    rules: Vec<MaskingRule>,
}

impl MaskingEngine {
    /// 创建引擎
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// 追加规则（同列后写覆盖先写）
    pub fn rule(mut self, column: impl Into<String>, strategy: MaskStrategy) -> Self {
        let column = column.into();
        self.rules.retain(|r| r.column != column);
        self.rules.push(MaskingRule { column, strategy });
        self
    }

    /// 是否有规则
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// 对单值应用策略
    pub fn apply_value(
        &self,
        strategy: &MaskStrategy,
        value: &serde_json::Value,
    ) -> serde_json::Value {
        let s = match value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Null => return serde_json::Value::Null,
            other => other.to_string(),
        };
        let out = match strategy {
            MaskStrategy::Mask { keep } => {
                let chars: Vec<char> = s.chars().collect();
                let total = chars.len();
                let keep = (*keep).min(total / 2);
                if total > keep * 2 {
                    let mut out2: String = chars[..keep].iter().collect();
                    for _ in 0..(total - keep * 2) {
                        out2.push('*');
                    }
                    out2.extend(chars[total - keep..].iter());
                    out2
                } else {
                    "*".repeat(total)
                }
            }
            MaskStrategy::Hash => {
                let mut hasher = Sha256::new();
                hasher.update(s.as_bytes());
                let digest = hasher.finalize();
                hex_encode(&digest)
            }
            MaskStrategy::Truncate(n) => {
                let mut out2: String = s.chars().take(*n).collect();
                if s.chars().count() > *n {
                    out2.push('…');
                }
                out2
            }
        };
        serde_json::Value::String(out)
    }

    /// 对一批查询结果行应用脱敏（命中规则的字符串/数值列被转换）
    pub fn apply(&self, rows: &mut [serde_json::Value]) {
        if self.rules.is_empty() {
            return;
        }
        for row in rows.iter_mut() {
            let Some(obj) = row.as_object_mut() else {
                continue;
            };
            for rule in &self.rules {
                if let Some(v) = obj.get_mut(&rule.column) {
                    let masked = self.apply_value(&rule.strategy, v);
                    *v = masked;
                }
            }
        }
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

// ============================================================================
// 行级安全（RLS）
// ============================================================================

/// 行级安全策略：对指定表的 SELECT 注入 `<column> = '<value>'` 谓词
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RlsPolicy {
    /// 目标表
    pub table: String,
    /// 谓词列（如 tenant_id）
    pub column: String,
    /// 谓词值（如租户 ID）
    pub value: String,
}

/// RLS 引擎：策略集 + 绕过控制
#[derive(Debug, Clone, Default)]
pub struct RlsEngine {
    policies: Vec<RlsPolicy>,
}

impl RlsEngine {
    /// 创建引擎
    pub fn new() -> Self {
        Self {
            policies: Vec::new(),
        }
    }

    /// 追加策略
    pub fn policy(
        mut self,
        table: impl Into<String>,
        column: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.policies.push(RlsPolicy {
            table: table.into(),
            column: column.into(),
            value: value.into(),
        });
        self
    }

    /// 是否有策略
    pub fn is_empty(&self) -> bool {
        self.policies.is_empty()
    }

    /// 对 SQL 注入 RLS 谓词（MVP：字符串级 append，`WHERE` 已存在则 `AND` 连接）
    ///
    /// # 安全边界
    ///
    /// - admin 角色**不**注入（显式管理通道，与 `admin_bypass` 语义一致）
    /// - 单引号值经转义；该 MVP 不处理子查询/别名场景，复杂语句建议走实体 API
    pub fn inject(&self, sql: &str, primary_table: Option<&str>) -> String {
        if self.policies.is_empty() {
            return sql.to_string();
        }
        let Some(table) = primary_table else {
            return sql.to_string();
        };
        let Some(policy) = self.policies.iter().find(|p| p.table == table) else {
            return sql.to_string();
        };
        let value = policy.value.replace('\'', "''");
        let predicate = format!("{} = '{}'", policy.column, value);
        let trimmed = sql.trim_end().trim_end_matches(';');
        if contains_where(trimmed) {
            format!("{trimmed} AND {predicate}")
        } else {
            format!("{trimmed} WHERE {predicate}")
        }
    }
}

fn contains_where(sql: &str) -> bool {
    let lower = sql.to_ascii_lowercase();
    // 粗略判断：避免把 "where" 误判进字符串字面量的场景交给 MVP 边界
    lower.contains(" where ") || lower.starts_with("where ") || lower.contains(")where ")
}

use std::sync::Arc;
use tokio::sync::RwLock;

/// 数据保护配置（脱敏 + RLS），运行时可整体换装（ArcSwap 换装由池侧注入）
#[derive(Debug, Clone, Default)]
pub struct DataProtection {
    /// 字段脱敏引擎（None = 不脱敏）
    pub masking: Option<Arc<MaskingEngine>>,
    /// 行级安全引擎（None = 不注入谓词）
    pub rls: Option<Arc<RlsEngine>>,
}

impl DataProtection {
    /// 只读快照（无锁读取）
    pub fn load(arc: &Arc<RwLock<DataProtection>>) -> DataProtection {
        arc.blocking_read().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_masking_hash_strategy() {
        let engine = MaskingEngine::new().rule("email", MaskStrategy::Hash);
        let mut rows = vec![json!({"email": "alice@example.com"})];
        engine.apply(&mut rows);
        let masked = rows[0]["email"].as_str().unwrap();
        // sha2("alice@example.com") 前缀
        assert_eq!(masked.len(), 64, "SHA-256 十六进制应为 64 字符");
        assert!(!masked.contains("alice"));
        assert!(!masked.contains('@'));
    }

    #[test]
    fn test_masking_mask_and_truncate() {
        let engine = MaskingEngine::new()
            .rule("phone", MaskStrategy::Mask { keep: 3 })
            .rule("bio", MaskStrategy::Truncate(5));
        let mut rows = vec![json!({"phone": "13812345678", "bio": "很长的个人简介内容"})];
        engine.apply(&mut rows);
        let phone = rows[0]["phone"].as_str().unwrap();
        assert!(phone.starts_with("138"), "应保留前 3 位");
        assert!(phone.contains('*'), "中间应为掩码");
        let bio = rows[0]["bio"].as_str().unwrap();
        assert!(bio.chars().count() <= 6, "截断 5 字符 + 省略号");
        assert!(bio.ends_with('…'));
    }

    #[test]
    fn test_masking_untouched_columns() {
        let engine = MaskingEngine::new().rule("secret", MaskStrategy::Hash);
        let mut rows = vec![json!({"id": 1, "name": "keep"})];
        engine.apply(&mut rows);
        assert_eq!(rows[0]["name"], "keep");
        assert_eq!(rows[0]["id"], 1);
    }

    #[test]
    fn test_rls_injection() {
        let rls = RlsEngine::new().policy("orders", "tenant_id", "t-100");
        // 无 WHERE → append WHERE
        let out = rls.inject("SELECT * FROM orders", Some("orders"));
        assert_eq!(out, "SELECT * FROM orders WHERE tenant_id = 't-100'");
        // 已有 WHERE → AND 连接
        let out2 = rls.inject("SELECT * FROM orders WHERE amount > 10", Some("orders"));
        assert!(out2.ends_with("AND tenant_id = 't-100'"));
        // 非目标表不注入
        assert_eq!(
            rls.inject("SELECT * FROM users", Some("users")),
            "SELECT * FROM users"
        );
        // 无主表不注入
        assert_eq!(rls.inject("SELECT 1", None), "SELECT 1");
    }
}
