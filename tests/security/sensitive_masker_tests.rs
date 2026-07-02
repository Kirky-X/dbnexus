// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! SensitiveMasker 外部测试（T085-T087）
//!
//! 通过公共 API `SensitiveMasker::mask(data, mask_type)` 测试各类脱敏场景。
//! 覆盖正常输入、短输入、无效输入、边界条件。

use dbnexus::{MaskType, SensitiveError, SensitiveMasker};

// ============================================================================
// T086: 脱敏测试
// ============================================================================

/// TEST-MASK-001: 邮箱脱敏
#[test]
fn test_mask_email() {
    let result = SensitiveMasker::mask("alice@example.com", MaskType::Email);
    assert!(result.is_ok(), "mask email should succeed: {:?}", result.err());
    let masked = result.unwrap();
    assert!(masked.contains("@"), "masked email should retain @");
    assert!(masked.contains("example.com"), "domain should be retained");
    assert!(masked.contains("*"), "masked part should contain asterisks");
}

/// TEST-MASK-002: 短邮箱脱敏边界
#[test]
fn test_mask_email_short() {
    // 极短邮箱（用户名仅 1 字符）
    let result = SensitiveMasker::mask("a@b.co", MaskType::Email);
    // 短邮箱可能返回错误或部分掩码，关键是函数不 panic
    match result {
        Ok(masked) => assert!(masked.contains("@"), "short email should retain @"),
        Err(SensitiveError::InvalidInput(_)) => { /* 短邮箱拒绝是可接受的 */ }
        Err(e) => panic!("unexpected error for short email: {:?}", e),
    }
}

/// TEST-MASK-003: 手机号脱敏
#[test]
fn test_mask_phone() {
    let result = SensitiveMasker::mask("13812345678", MaskType::Phone);
    assert!(result.is_ok(), "mask phone should succeed: {:?}", result.err());
    let masked = result.unwrap();
    assert!(masked.starts_with("138"), "should retain first 3 digits");
    assert!(masked.ends_with("5678"), "should retain last 4 digits");
    assert!(masked.contains("*"), "should contain asterisks");
}

/// TEST-MASK-004: 无效手机号脱敏
#[test]
fn test_mask_phone_invalid() {
    // 太短的手机号
    let result = SensitiveMasker::mask("123", MaskType::Phone);
    assert!(result.is_err(), "invalid phone should return error, got: {:?}", result);
    match result {
        Err(SensitiveError::InvalidInput(_)) => { /* expected */ }
        Err(e) => panic!("expected InvalidInput, got {:?}", e),
        Ok(v) => panic!("expected error, got {}", v),
    }
}

/// TEST-MASK-005: 身份证号脱敏
#[test]
fn test_mask_id_card() {
    // 18 位身份证
    let result = SensitiveMasker::mask("110101199001011234", MaskType::IdCard);
    assert!(result.is_ok(), "mask id card should succeed: {:?}", result.err());
    let masked = result.unwrap();
    assert!(masked.starts_with("1101"), "should retain first 4 chars");
    assert!(masked.ends_with("1234"), "should retain last 4 chars");
    assert!(masked.contains("*"), "should contain asterisks");
}

/// TEST-MASK-006: 银行卡号脱敏（任务要求 mask_credit_card，实际为 BankCard）
#[test]
fn test_mask_credit_card() {
    // 16 位银行卡号
    let result = SensitiveMasker::mask("6222021234567890", MaskType::BankCard);
    assert!(result.is_ok(), "mask bank card should succeed: {:?}", result.err());
    let masked = result.unwrap();
    assert!(masked.starts_with("6222"), "should retain first 4 digits");
    assert!(masked.ends_with("7890"), "should retain last 4 digits");
    assert!(masked.contains("*"), "should contain asterisks");
}

/// TEST-MASK-008: Unicode 邮箱本地部分不 panic（回归测试）
///
/// `mask_email` 原实现用 `local.len()`（字节数）和 `&local[..prefix_len]`（字节切片），
/// 对非 ASCII 本地部分（如中文、emoji）按字节切片会 panic。
/// 修复后应使用 `chars()` 安全处理 Unicode。
#[test]
fn test_mask_email_unicode_local_part() {
    // 中文本地部分（每个字符 3 字节）
    let result = SensitiveMasker::mask("测试@example.com", MaskType::Email);
    assert!(result.is_ok(), "unicode email should succeed: {:?}", result.err());
    let masked = result.unwrap();
    assert!(masked.ends_with("@example.com"), "domain should be retained");
    assert!(masked.contains('*'), "should contain asterisks");

    // 单字符中文本地部分
    let result = SensitiveMasker::mask("测@test.com", MaskType::Email);
    assert!(
        result.is_ok(),
        "single unicode char email should succeed: {:?}",
        result.err()
    );

    // emoji 本地部分（4 字节字符）
    let result = SensitiveMasker::mask("😀@emoji.com", MaskType::Email);
    assert!(result.is_ok(), "emoji email should succeed: {:?}", result.err());
}

/// TEST-MASK-007: 自定义脱敏
#[test]
fn test_mask_custom() {
    let result = SensitiveMasker::mask(
        "sensitive_data_here",
        MaskType::Custom {
            keep_prefix: 4,
            keep_suffix: 4,
        },
    );
    assert!(result.is_ok(), "mask custom should succeed: {:?}", result.err());
    let masked = result.unwrap();
    assert!(masked.starts_with("sens"), "should retain first 4 chars");
    assert!(masked.ends_with("here"), "should retain last 4 chars");
    assert!(masked.contains("*"), "should contain asterisks");
}
