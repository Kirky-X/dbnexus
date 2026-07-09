// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

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
            println!("    原始: {}", data);
            println!("    脱敏: {}", masked);
        }
        Err(err) => {
            println!("  ✗ [{}] 失败", label);
            println!("    原始: {}", data);
            println!("    错误: {}", err);
        }
    }
}

fn main() {
    println!("========================================");
    println!("🔒 DBNexus 敏感数据脱敏示例");
    println!("========================================\n");

    let masker = SensitiveMasker::new();
    println!("✓ SensitiveMasker 实例创建成功\n");
    let _ = masker; // 验证 new() 可用，实际使用静态方法 mask()

    // ============================================
    // 1. 手机号脱敏（保留前3后4）
    // ============================================
    println!("--- 1. 手机号脱敏（保留前3后4） ---");
    print_mask("标准11位", "13812345678", MaskType::Phone);
    print_mask("带空格", "138 1234 5678", MaskType::Phone);
    print_mask("带横线", "138-1234-5678", MaskType::Phone);
    print_mask("国际格式", "+86 138 1234 5678", MaskType::Phone);
    println!();

    // ============================================
    // 2. 邮箱脱敏（保留前2字符和域名）
    // ============================================
    println!("--- 2. 邮箱脱敏（保留前2字符和域名） ---");
    print_mask("标准邮箱", "test@example.com", MaskType::Email);
    print_mask("长邮箱", "alice.longname@company.org", MaskType::Email);
    print_mask("短邮箱", "a@b.com", MaskType::Email);
    print_mask("单字符", "x@y.io", MaskType::Email);
    println!();

    // ============================================
    // 3. 身份证脱敏（保留前4后4）
    // ============================================
    println!("--- 3. 身份证脱敏（保留前4后4） ---");
    print_mask("18位身份证", "110101199001011234", MaskType::IdCard);
    print_mask("15位身份证", "110101900101123", MaskType::IdCard);
    print_mask("带X结尾", "11010119900101123X", MaskType::IdCard);
    println!();

    // ============================================
    // 4. 银行卡脱敏（保留前4后4）
    // ============================================
    println!("--- 4. 银行卡脱敏（保留前4后4） ---");
    print_mask("16位银行卡", "6222021234567890", MaskType::BankCard);
    print_mask("19位银行卡", "6222021234567890123", MaskType::BankCard);
    print_mask("带空格", "6222 0212 3456 7890", MaskType::BankCard);
    println!();

    // ============================================
    // 5. 姓名脱敏（保留姓氏）
    // ============================================
    println!("--- 5. 姓名脱敏（保留姓氏） ---");
    print_mask("两字姓名", "张三", MaskType::Name);
    print_mask("三字姓名", "李某某", MaskType::Name);
    print_mask("四字姓名", "欧阳明月", MaskType::Name);
    print_mask("英文姓名", "AliceSmith", MaskType::Name);
    println!();

    // ============================================
    // 6. 地址脱敏（保留省市）
    // ============================================
    println!("--- 6. 地址脱敏（保留省市） ---");
    print_mask("标准地址", "北京市朝阳区某某街道123号", MaskType::Address);
    print_mask("省市完整", "广东省深圳市南山区科技园路1号", MaskType::Address);
    print_mask("无行政区划", "某某街道123号456室", MaskType::Address);
    println!();

    // ============================================
    // 7. 自定义脱敏（指定保留前后位数）
    // ============================================
    println!("--- 7. 自定义脱敏 ---");
    print_mask(
        "保留前2后2",
        "1234567890",
        MaskType::Custom {
            keep_prefix: 2,
            keep_suffix: 2,
        },
    );
    print_mask(
        "保留前4后4",
        "ABCDEFGH12345678",
        MaskType::Custom {
            keep_prefix: 4,
            keep_suffix: 4,
        },
    );
    print_mask(
        "保留前0后4",
        "1234567890",
        MaskType::Custom {
            keep_prefix: 0,
            keep_suffix: 4,
        },
    );
    print_mask(
        "保留超过长度",
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
    println!("--- 8. 错误处理（无效输入） ---");

    let invalid_cases: [(&str, &str, MaskType); 5] = [
        ("手机号过短", "123", MaskType::Phone),
        ("邮箱无@", "invalid", MaskType::Email),
        ("身份证长度错误", "12345", MaskType::IdCard),
        ("银行卡过短", "1234567", MaskType::BankCard),
        ("姓名为空", "", MaskType::Name),
    ];

    for (label, data, mask_type) in &invalid_cases {
        match SensitiveMasker::mask(data, *mask_type) {
            Ok(_) => println!("  ⚠ [{}] 应失败但成功了", label),
            Err(err) => {
                println!("  ✓ [{}] 正确失败: {}", label, err);
                // 验证错误类型
                match err {
                    SensitiveError::InvalidInput(_) => {
                        println!("    (错误类型: InvalidInput ✓)");
                    }
                    SensitiveError::MaskingFailed(_) => {
                        println!("    (错误类型: MaskingFailed)");
                    }
                    _ => println!("    (其他错误类型)"),
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
    println!("  ✓ Default 创建 + 静态方法调用: 13812345678 → {}", result);
    println!();

    // ============================================
    // 10. 批量脱敏场景（模拟用户数据导出）
    // ============================================
    println!("--- 10. 批量脱敏场景（用户数据导出） ---");
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
        "姓名", "手机号", "邮箱", "身份证", "地址"
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
    println!("✨ 敏感数据脱敏示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - SensitiveMasker::new()           - 创建脱敏器实例");
    println!("  - SensitiveMasker::default()       - Default trait 创建");
    println!("  - SensitiveMasker::mask(data, t)   - 静态方法脱敏");
    println!("  - MaskType::Phone                  - 手机号（保留前3后4）");
    println!("  - MaskType::Email                  - 邮箱（保留前2字符+域名）");
    println!("  - MaskType::IdCard                 - 身份证（保留前4后4）");
    println!("  - MaskType::BankCard               - 银行卡（保留前4后4）");
    println!("  - MaskType::Name                   - 姓名（保留姓氏）");
    println!("  - MaskType::Address                - 地址（保留省市）");
    println!("  - MaskType::Custom{{keep_prefix,..}} - 自定义脱敏");
    println!("  - SensitiveError::InvalidInput     - 无效输入错误");
}
