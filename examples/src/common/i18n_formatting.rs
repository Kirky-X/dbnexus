// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 国际化（i18n）格式化示例
//!
//! 演示 [`DbI18nFormatter`] 的 locale 感知格式化能力：
//! - 数字格式化（千位分隔符、小数点）
//! - 行数格式化（locale 敏感的数字分组）
//! - 迁移消息（复数规则感知）
//! - 日期格式化（locale 敏感的日期显示）
//! - 复数类别查询
//! - 字符串排序（locale 敏感的 collation）
//!
//! i18n 模块基于 ICU4X 2.x，是核心特性，始终可用。
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example i18n_formatting
//! ```

use std::cmp::Ordering;

use dbnexus::{DbI18nFormatter, I18nError};

// 辅助函数

/// 演示指定 locale 的数字格式化
fn demo_number_formatting(fmt: &DbI18nFormatter, locale: &str) -> Result<(), I18nError> {
    println!("  Locale: {}", locale);

    let numbers = [42.0, 1_234.56, 1_234_567.89, 0.001];
    for n in numbers {
        let formatted = fmt.format_number(n)?;
        println!("    {:>15} → {}", n, formatted);
    }

    let counts = [0u64, 1, 999, 1_000_000, 999_999_999];
    for c in counts {
        let formatted = fmt.format_row_count(c)?;
        println!("    row count {:>12} → {}", c, formatted);
    }

    Ok(())
}

/// 演示指定 locale 的迁移消息
fn demo_migration_messages(fmt: &DbI18nFormatter, locale: &str) -> Result<(), I18nError> {
    println!("  Locale: {}", locale);
    for count in [0, 1, 2, 5, 100] {
        let msg = fmt.format_migration_message(count)?;
        let plural = fmt.plural_category(count)?;
        println!("    count={:>3} → \"{}\" (plural: {})", count, msg, plural);
    }
    Ok(())
}

// 主函数

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🌍 DBNexus internationalization (i18n) formatting example");
    println!("========================================\n");

    // 创建不同 locale 的格式化器
    println!("--- 1. create formatters ---\n");

    let locales = ["en-US", "zh-CN", "de-DE", "ja-JP", "fr-FR"];
    let mut formatters = Vec::new();

    for locale in &locales {
        match DbI18nFormatter::new(locale) {
            Ok(fmt) => {
                println!("  ✓ {} created", locale);
                formatters.push((locale.to_string(), fmt));
            }
            Err(e) => println!("  ✗ {} creation failed: {}", locale, e),
        }
    }

    // 数字格式化（locale 敏感）
    println!("\n--- 2. number formatting ---\n");

    for (locale, fmt) in &formatters {
        demo_number_formatting(fmt, locale)?;
        println!();
    }

    // 迁移消息（复数规则）
    println!("--- 3. migration messages (plural rules) ---\n");

    for (locale, fmt) in &formatters {
        demo_migration_messages(fmt, locale)?;
        println!();
    }

    // 日期格式化
    println!("--- 4. date formatting ---\n");

    let dates = [(2026, 1, 15), (2026, 7, 4), (2026, 12, 25)];

    for (locale, fmt) in &formatters {
        println!("  Locale: {}", locale);
        for (year, month, day) in dates {
            match fmt.format_timestamp(year, month, day) {
                Ok(formatted) => {
                    println!("    {:04}-{:02}-{:02} → {}", year, month, day, formatted)
                }
                Err(e) => println!("    {:04}-{:02}-{:02} → error: {}", year, month, day, e),
            }
        }
        println!();
    }

    // 字符串排序（locale 敏感 collation）
    println!("--- 5. string ordering ---\n");

    let words = ["banana", "apple", "cherry", "date", "éclair"];

    for (locale, fmt) in &formatters {
        println!("  Locale: {} — before sorting: {:?}", locale, words);
        let mut sorted = words.to_vec();
        sorted.sort_by(|a, b| fmt.compare_strings(a, b).unwrap_or(Ordering::Equal));
        println!("  after sorting: {:?}", sorted);
        println!();
    }

    // 错误处理
    println!("--- 6. error handling ---\n");

    // 无效 locale
    match DbI18nFormatter::new("not-a-valid-locale!!!") {
        Ok(_) => println!("  ✗ unexpected success"),
        Err(I18nError::InvalidLocale { input, reason }) => {
            println!("  ✓ invalid locale error handling:");
            println!("    input:  {}", input);
            println!("    reason: {}", reason);
        }
        Err(e) => println!("  ✗ unexpected error type: {:?}", e),
    }

    // 非有限数字
    let fmt = DbI18nFormatter::new("en-US")?;
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        match fmt.format_number(value) {
            Ok(_) => println!("  ✗ unexpected success: {}", value),
            Err(I18nError::InvalidNumber { input, reason }) => {
                println!("  ✓ non-finite number error: {} → {}", input, reason);
            }
            Err(e) => println!("  ✗ unexpected error: {:?}", e),
        }
    }

    // 无效日期
    match fmt.format_timestamp(2026, 13, 32) {
        Ok(_) => println!("  ✗ unexpected success"),
        Err(I18nError::DateError(reason)) => {
            println!("  ✓ invalid date error: {}", reason);
        }
        Err(e) => println!("  ✓ other date error: {:?}", e),
    }

    println!("\n========================================");
    println!("✨ Internationalization formatting example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - DbI18nFormatter::new(locale)           create a locale formatter");
    println!("  - format_number(f64)                    locale-sensitive number formatting");
    println!("  - format_row_count(u64)                 row count formatting");
    println!("  - format_migration_message(u64)         plural-rule-aware message");
    println!("  - format_timestamp(year, month, day)    locale-sensitive date formatting");
    println!("  - plural_category(u64)                  query the plural category");
    println!("  - compare_strings(a, b)                 locale-sensitive string ordering");

    Ok(())
}
