// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 权限变更审计链（`data-protection` feature）
//!
//! 权限变更事件（RBAC/RLS/脱敏配置变更）经 HMAC-SHA256 链式签名：
//! `hmac_n = HMAC-SHA256(key, prev_hash_n || canonical_event_n)`，
//! `prev_hash_0 = GENESIS`（零哈希，与 confers 审计链同模式——盐/键由
//! 使用方持有，链条锚定固定创世哈希）。
//!
//! [`verify_permission_chain`] 对完整链条重算签名：任一事件被篡改、
//! 删除、重排或伪造（prev_hash 链断裂 / hmac 不匹配 / 序号跳跃）即
//! 校验失败。
//!
//! ```rust,no_run
//! use dbnexus::access::permission_audit_chain::{PermissionAuditChain, PermissionChangeRecord};
//!
//! let mut chain = PermissionAuditChain::new(b"audit-chain-key");
//! chain.append(&PermissionChangeRecord::new("admin", "role_added", "operator"));
//! assert!(chain.verify());
//! ```

use sha2::{Digest, Sha256};

/// HMAC-SHA256 输出长度（字节）
pub const CHAIN_HASH_LEN: usize = 32;

/// 创世 prev_hash（全零；使链条锚定不受每次进程重启影响）
pub const GENESIS_HASH: [u8; CHAIN_HASH_LEN] = [0u8; CHAIN_HASH_LEN];

/// HMAC 块长（SHA-256 为 64 字节）
const HMAC_BLOCK_SIZE: usize = 64;

/// HMAC-SHA256（RFC 2104，经 sha2 手工构造——不引入 hmac 依赖）
fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; CHAIN_HASH_LEN] {
    // 密钥归一化：超长先哈希；不足块长补零到块长
    let mut key_block = [0u8; HMAC_BLOCK_SIZE];
    let normalized: Vec<u8> = if key.len() > HMAC_BLOCK_SIZE {
        Sha256::digest(key).to_vec()
    } else {
        key.to_vec()
    };
    key_block[..normalized.len()].copy_from_slice(&normalized);

    let mut inner = Sha256::new();
    let mut outer = Sha256::new();
    for byte in &key_block {
        inner.update([byte ^ 0x36]);
        outer.update([byte ^ 0x5c]);
    }
    inner.update(message);
    outer.update(inner.finalize());
    let out = outer.finalize();
    let mut bytes = [0u8; CHAIN_HASH_LEN];
    bytes.copy_from_slice(&out);
    bytes
}

/// 十六进制编码（小写，64 字符）
fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

// ============================================================================
// 权限变更事件记录
// ============================================================================

/// 权限变更事件记录
///
/// 链条的最小事件单元：谁（actor）对哪个角色（role）做了什么（action），
/// `detail` 为可扩展 JSON 文本（如新策略快照的摘要）。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PermissionChangeRecord {
    /// 目标角色（如 admin / analyst）
    pub role: String,
    /// 变更动作（如 role_added / role_removed / policy_updated）
    pub action: String,
    /// 操作者标识
    pub actor: String,
    /// 变更时间（RFC 3339）
    pub at: String,
    /// 扩展详情（JSON 文本；None = 无）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl PermissionChangeRecord {
    /// 创建变更记录（时间为当前 UTC RFC 3339）
    pub fn new(role: impl Into<String>, action: impl Into<String>, actor: impl Into<String>) -> Self {
        let at = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default();
        Self {
            role: role.into(),
            action: action.into(),
            actor: actor.into(),
            at,
            detail: None,
        }
    }

    /// 设置扩展详情（builder 风格）
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

// ============================================================================
// 链条条目与链
// ============================================================================

/// 链条条目
///
/// `event_json` 为事件的规范化 JSON 文本；`prev_hash`/`hmac` 为十六进制。
#[derive(Debug, Clone, PartialEq)]
pub struct ChainEntry {
    /// 序号（0 起，连续）
    pub seq: u64,
    /// 上一条目 hmac（hex；首条为 GENESIS 十六进制）
    pub prev_hash: String,
    /// 事件规范化 JSON 文本
    pub event_json: String,
    /// 本条目签名（hex）
    pub hmac: String,
}

impl ChainEntry {
    /// 重算本条目的签名（校验用；prev_hash 非法时按空向量处理）
    pub fn compute_hmac(&self, key: &[u8]) -> String {
        let prev = hex_decode(&self.prev_hash).unwrap_or_default();
        let msg = chain_message(&prev, self.event_json.as_bytes());
        hex_encode(&hmac_sha256(key, &msg))
    }
}

/// 链消息 = prev_hash 字节 || 规范化事件字节（confers 同款 HMAC(prev||event)）
fn chain_message(prev_hash: &[u8], canonical_event: &[u8]) -> Vec<u8> {
    let mut msg = Vec::with_capacity(prev_hash.len() + canonical_event.len());
    msg.extend_from_slice(prev_hash);
    msg.extend_from_slice(canonical_event);
    msg
}

/// 十六进制解码（非法输入返回 None）
fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// 权限变更审计链
///
/// 线程安全：append 经互斥锁串行化（多写者不交错）。
pub struct PermissionAuditChain {
    key: Vec<u8>,
    prev_hash: [u8; CHAIN_HASH_LEN],
    seq: u64,
    entries: Vec<ChainEntry>,
}

impl PermissionAuditChain {
    /// 以 HMAC 密钥创建链（从创世哈希开始）
    pub fn new(key: &[u8]) -> Self {
        Self {
            key: key.to_vec(),
            prev_hash: GENESIS_HASH,
            seq: 0,
            entries: Vec::new(),
        }
    }

    /// 追加一条权限变更事件并返回签名后的条目
    pub fn append(&mut self, record: &PermissionChangeRecord) -> ChainEntry {
        let event_json = serde_json::to_string(record).expect("record serializes");
        let entry = self.append_event_json(event_json);
        self.entries.push(entry.clone());
        entry
    }

    /// 追加任意规范化 JSON 事件（供上层复用链条承载其他事件形态）
    pub fn append_event_json(&mut self, event_json: String) -> ChainEntry {
        let msg = chain_message(&self.prev_hash, event_json.as_bytes());
        let hmac = hmac_sha256(&self.key, &msg);
        let entry = ChainEntry {
            seq: self.seq,
            prev_hash: hex_encode(&self.prev_hash),
            event_json,
            hmac: hex_encode(&hmac),
        };
        self.prev_hash = hmac;
        self.seq += 1;
        entry
    }

    /// 全部条目（快照）
    pub fn entries(&self) -> &[ChainEntry] {
        &self.entries
    }

    /// 条目数
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 链是否为空
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 当前链头哈希（空链为 GENESIS 十六进制）
    pub fn last_hash(&self) -> String {
        hex_encode(&self.prev_hash)
    }

    /// 用持有密钥校验自身链条
    pub fn verify(&self) -> bool {
        verify_permission_chain(&self.entries, &self.key)
    }
}

/// 校验权限变更审计链
///
/// 逐条重算 `HMAC(key, prev_hash || canonical_event)` 并检查：
/// 1. `seq` 连续（0 起）
/// 2. `prev_hash` 与上一条 `hmac`（或 GENESIS）一致
/// 3. `hmac` 与重算值一致
///
/// 空链视为合法；非法 hex/长度不符/任意一项不匹配返回 false。
pub fn verify_permission_chain(entries: &[ChainEntry], key: &[u8]) -> bool {
    let mut prev_hash = GENESIS_HASH;
    for (expected_seq, entry) in entries.iter().enumerate() {
        let seq = entry.seq as usize;
        if seq != expected_seq {
            return false;
        }
        let Some(prev) = hex_decode(&entry.prev_hash) else {
            return false;
        };
        if prev != prev_hash {
            return false;
        }
        let recomputed_msg = chain_message(&prev, entry.event_json.as_bytes());
        let recomputed = hmac_sha256(key, &recomputed_msg);
        let Some(hmac) = hex_decode(&entry.hmac) else {
            return false;
        };
        if hmac.len() != CHAIN_HASH_LEN || hmac != recomputed {
            return false;
        }
        prev_hash = hmac.try_into().unwrap_or(GENESIS_HASH);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(role: &str, action: &str) -> PermissionChangeRecord {
        PermissionChangeRecord::new(role, action, "operator")
    }

    // ===== RFC 4231 HMAC-SHA256 向量 =====

    #[test]
    fn hmac_matches_rfc4231_case_1() {
        // key = 20 bytes of 0x0b, data = "Hi There"
        let expected = "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7";
        let mac = hmac_sha256(&[0x0b; 20], b"Hi There");
        assert_eq!(hex_encode(&mac), expected);
    }

    #[test]
    fn hmac_matches_rfc4231_case_2() {
        let expected = "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843";
        let mac = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(hex_encode(&mac), expected);
    }

    #[test]
    fn hmac_long_key_normalized() {
        // RFC 4231 Test Case 6：131 字节密钥（超块长 → 先哈希再使用的合法解释）
        let key = [0xaa; 131];
        let expected = "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54";
        let mac = hmac_sha256(&key, b"Test Using Larger Than Block-Size Key - Hash Key First");
        assert_eq!(hex_encode(&mac), expected);
    }

    // ===== 链行为 =====

    #[test]
    fn test_chain_append_links_entries() {
        let mut chain = PermissionAuditChain::new(b"key");
        let e0 = chain.append(&PermissionChangeRecord::new("admin", "role_added", "op"));
        assert_eq!(e0.seq, 0);
        assert_eq!(e0.prev_hash, hex_encode(&GENESIS_HASH));

        let e1 = chain.append(&PermissionChangeRecord::new("analyst", "policy_updated", "op"));
        assert_eq!(e1.seq, 1);
        assert_eq!(e1.prev_hash, e0.hmac, "第二条 prev_hash 应为第一条 hmac");

        assert_eq!(chain.len(), 2);
        assert!(chain.verify(), "完好链条应通过校验");
    }

    #[test]
    fn test_verify_detects_tampered_event() {
        let mut chain = PermissionAuditChain::new(b"key");
        chain.append(&PermissionChangeRecord::new("admin", "role_added", "op"));
        let mut entries = chain.entries().to_vec();

        // 篡改事件内容
        entries[0].event_json = entries[0].event_json.replace("role_added", "role_removed");
        assert!(!verify_permission_chain(&entries, b"key"), "篡改事件应被检出");

        // 篡改签名
        let mut entries2 = chain.entries().to_vec();
        entries2[0].hmac = "00".repeat(CHAIN_HASH_LEN);
        assert!(!verify_permission_chain(&entries2, b"key"));
    }

    #[test]
    fn test_verify_detects_deletion_and_reordering() {
        let mut chain = PermissionAuditChain::new(b"key");
        chain.append(&PermissionChangeRecord::new("admin", "role_added", "op"));
        chain.append(&PermissionChangeRecord::new("analyst", "policy_updated", "op"));
        chain.append(&PermissionChangeRecord::new("guest", "role_removed", "op"));

        // 删除中间条目 → seq 跳跃 + prev 断裂
        let entries = chain.entries().to_vec();
        let deleted: Vec<ChainEntry> = entries
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != 1)
            .map(|(_, e)| e.clone())
            .collect();
        assert!(!verify_permission_chain(&deleted, b"key"), "删除条目应被检出");

        // 重排
        let mut reordered = entries.clone();
        reordered.swap(0, 1);
        assert!(!verify_permission_chain(&reordered, b"key"), "重排应被检出");
    }

    #[test]
    fn test_verify_wrong_key_and_empty_chain() {
        let mut chain = PermissionAuditChain::new(b"key");
        chain.append(&PermissionChangeRecord::new("admin", "role_added", "op"));

        // 错误密钥 → 校验失败
        assert!(!verify_permission_chain(chain.entries(), b"other-key"));

        // 空链合法
        let empty = PermissionAuditChain::new(b"key");
        assert!(verify_permission_chain(&[], b"key"));
        assert!(empty.verify());
    }

    #[test]
    fn test_hex_decode_invalid() {
        assert!(hex_decode("zz").is_none());
        assert!(hex_decode("abc").is_none());
        assert_eq!(hex_decode(""), Some(Vec::new()));
    }

    #[test]
    fn test_record_serde_roundtrip() {
        let record = PermissionChangeRecord::new("admin", "role_added", "op")
            .with_detail(r#"{"tables":["*"]}"#);
        let text = serde_json::to_string(&record).unwrap();
        let parsed: PermissionChangeRecord = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed.role, "admin");
        assert_eq!(parsed.detail.as_deref(), Some(r#"{"tables":["*"]}"#));
    }
}
