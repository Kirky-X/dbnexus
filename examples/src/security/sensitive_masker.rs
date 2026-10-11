// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 敏感数据脱敏示例
//!
//! 展示如何使用 SensitiveMasker 对不同类型的敏感数据进行脱敏：
//! - 手机号脱敏（保留前3后4）
//! - 邮箱脱敏（保留前2字符和域名）
//! - 身份证脱敏（保留前4后4）
//! - 银行卡脱敏（保留前4后4）
//! - 姓名脱敏（保留姓氏）
//! - 地址脱敏（保留省市）
//! - 自定义脱敏（指定保留前后位数）
//! - 错误处理（无效输入）
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example sensitive_masker
//! ```

use dbnexus::{MaskType, SensitiveError, SensitiveMasker};

/// 打印脱敏结果
fn print_mask(label: &str, data: &str, mask_type: MaskType) {
    match SensitiveMasker::mask(data, mask_type) {
        Ok(masked) => {
            println!("  ✓ [{}]", label);
            println!("    original: {}", data);
            println!("    masked: {}", masked);
        }
        Err(err) => {
            println!("  ✗ [{}] failed", label);
            println!("    original: {}", data);
            println!("    error: {}", err);
        }
    }
}

fn main() {
    println!("========================================");
    println!("🔒 DBNexus sensitive data masking example");
    println!("========================================\n");

    let masker = SensitiveMasker::new();
    println!("✓ SensitiveMasker instance created\n");
    let _ = masker; // 验证 new() 可用，实际使用静态方法 mask()

    // ============================================
    // 1. 手机号脱敏（保留前3后4）
    // ============================================
    println!("--- 1. phone number masking (keeps first 3 and last 4) ---");
    print_mask("standard 11 digits", "13812345678", MaskType::Phone);
    print_mask("with spaces", "138 1234 5678", MaskType::Phone);
    print_mask("with hyphens", "138-1234-5678", MaskType::Phone);
    print_mask("international format", "+86 138 1234 5678", MaskType::Phone);
    println!();

    // ============================================
    // 2. 邮箱脱敏（保留前2字符和域名）
    // ============================================
    println!("--- 2. email masking (keeps the first 2 chars and the domain) ---");
    print_mask("standard email", "test@example.com", MaskType::Email);
    print_mask("long email", "alice.longname@company.org", MaskType::Email);
    print_mask("short email", "a@b.com", MaskType::Email);
    print_mask("single character", "x@y.io", MaskType::Email);
    println!();

    // ============================================
    // 3. 身份证脱敏（保留前4后4）
    // ============================================
    println!("--- 3. ID card masking (keeps first 4 and last 4) ---");
    print_mask("18-digit ID card", "110101199001011234", MaskType::IdCard);
    print_mask("15-digit ID card", "110101900101123", MaskType::IdCard);
    print_mask("ends with X", "11010119900101123X", MaskType::IdCard);
    println!();

    // ============================================
    // 4. 银行卡脱敏（保留前4后4）
    // ============================================
    println!("--- 4. bank card masking (keeps first 4 and last 4) ---");
    print_mask("16-digit bank card", "6222021234567890", MaskType::BankCard);
    print_mask(
        "19-digit bank card",
        "6222021234567890123",
        MaskType::BankCard,
    );
    print_mask("with spaces", "6222 0212 3456 7890", MaskType::BankCard);
    println!();

    // ============================================
    // 5. 姓名脱敏（保留姓氏）
    // ============================================
    println!("--- 5. name masking (keeps the surname) ---");
    print_mask("two-character name", "张三", MaskType::Name);
    print_mask("three-character name", "李某某", MaskType::Name);
    print_mask("four-character name", "欧阳明月", MaskType::Name);
    print_mask("English name", "AliceSmith", MaskType::Name);
    println!();

    // ============================================
    // 6. 地址脱敏（保留省市）
    // ============================================
    println!("--- 6. address masking (keeps province/city) ---");
    print_mask(
        "standard address",
        "北京市朝阳区某某街道123号",
        MaskType::Address,
    );
    print_mask(
        "full province/city",
        "广东省深圳市南山区科技园路1号",
        MaskType::Address,
    );
    print_mask(
        "no administrative region",
        "某某街道123号456室",
        MaskType::Address,
    );
    println!();

    // ============================================
    // 7. 自定义脱敏（指定保留前后位数）
    // ============================================
    println!("--- 7. custom masking ---");
    print_mask(
        "keep first 2 last 2",
        "1234567890",
        MaskType::Custom {
            keep_prefix: 2,
            keep_suffix: 2,
        },
    );
    print_mask(
        "keep first 4 last 4",
        "ABCDEFGH12345678",
        MaskType::Custom {
            keep_prefix: 4,
            keep_suffix: 4,
        },
    );
    print_mask(
        "keep first 0 last 4",
        "1234567890",
        MaskType::Custom {
            keep_prefix: 0,
            keep_suffix: 4,
        },
    );
    print_mask(
        "keep more than the length",
        "123",
        MaskType::Custom {
            keep_prefix: 2,
            keep_suffix: 2,
        },
    );
    println!();

    // ============================================
    // 8. 错误处理（无效输入）
    // ============================================
    println!("--- 8. error handling (invalid input) ---");

    let invalid_cases: [(&str, &str, MaskType); 5] = [
        ("phone number too short", "123", MaskType::Phone),
        ("email has no @", "invalid", MaskType::Email),
        ("ID card length error", "12345", MaskType::IdCard),
        ("bank card too short", "1234567", MaskType::BankCard),
        ("empty name", "", MaskType::Name),
    ];

    for (label, data, mask_type) in &invalid_cases {
        match SensitiveMasker::mask(data, *mask_type) {
            Ok(_) => println!("  ⚠ [{}] should have failed but succeeded", label),
            Err(err) => {
                println!("  ✓ [{}] correctly failed: {}", label, err);
                // 验证错误类型
                match err {
                    SensitiveError::InvalidInput(_) => {
                        println!("    (error type: InvalidInput ✓)");
                    }
                    SensitiveError::MaskingFailed(_) => {
                        println!("    (error type: MaskingFailed)");
                    }
                    _ => println!("    (other error types)"),
                }
            }
        }
    }
    println!();

    // ============================================
    // 9. Default trait 演示
    // ============================================
    println!("--- 9. Default trait ---");
    let default_masker: SensitiveMasker = SensitiveMasker;
    let _ = default_masker;
    let result = SensitiveMasker::mask("13812345678", MaskType::Phone).unwrap();
    println!(
        "  ✓ created via Default + static method call: 13812345678 → {}",
        result
    );
    println!();

    // ============================================
    // 10. 批量脱敏场景（模拟用户数据导出）
    // ============================================
    println!("--- 10. batch masking scenario (user data export) ---");
    let user_records: &[(&str, &str, &str, &str, &str)] = &[
        (
            "张三",
            "13812345678",
            "zhangsan@example.com",
            "110101199001011234",
            "北京市朝阳区某某路1号",
        ),
        (
            "李某某",
            "13987654321",
            "li@example.com",
            "110101198501052345",
            "上海市浦东新区张江路2号",
        ),
        (
            "欧阳明月",
            "13700001111",
            "ouyang@company.org",
            "11010120001231234X",
            "广东省深圳市南山区科苑路3号",
        ),
    ];

    println!(
        "  {:<8} {:<14} {:<24} {:<20} {:<24}",
        "name", "phone number", "email", "ID card", "address"
    );
    println!("  {}", "-".repeat(94));
    for (name, phone, email, id_card, address) in user_records {
        let masked_name = SensitiveMasker::mask(name, MaskType::Name).unwrap_or_default();
        let masked_phone = SensitiveMasker::mask(phone, MaskType::Phone).unwrap_or_default();
        let masked_email = SensitiveMasker::mask(email, MaskType::Email).unwrap_or_default();
        let masked_id = SensitiveMasker::mask(id_card, MaskType::IdCard).unwrap_or_default();
        let masked_addr = SensitiveMasker::mask(address, MaskType::Address).unwrap_or_default();
        println!(
            "  {:<8} {:<14} {:<24} {:<20} {:<24}",
            masked_name, masked_phone, masked_email, masked_id, masked_addr
        );
    }

    println!("\n========================================");
    println!("✨ Sensitive data masking example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - SensitiveMasker::new()           - create a masker instance");
    println!("  - SensitiveMasker::default()       - create via the Default trait");
    println!("  - SensitiveMasker::mask(data, t)   - static masking method");
    println!("  - MaskType::Phone                  - phone number (keeps first 3 and last 4)");
    println!("  - MaskType::Email                  - email (keeps the first 2 chars + domain)");
    println!("  - MaskType::IdCard                 - ID card (keeps first 4 and last 4)");
    println!("  - MaskType::BankCard               - bank card (keeps first 4 and last 4)");
    println!("  - MaskType::Name                   - name (keeps the surname)");
    println!("  - MaskType::Address                - address (keeps province/city)");
    println!("  - MaskType::Custom{{keep_prefix,..}} - custom masking");
    println!("  - SensitiveError::InvalidInput     - invalid input error");
}
