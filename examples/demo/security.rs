// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 安全功能示例
//!
//! 展示 dbnexus 的安全功能：
//! - DdlGuard: DDL 语句安全验证，防止危险数据库操作
//! - SensitiveMasker: 敏感数据脱敏，支持手机号、邮箱、身份证、银行卡等
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example security --features "sqlite"
//! ```

use dbnexus::security::{DdlGuard, DdlValidationResult, MaskType, SensitiveMasker};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔒 DBNexus 安全功能示例\n");
    println!("========================================");

    // ============================================
    // 第一部分：DdlGuard - DDL 安全守卫
    // ============================================
    println!("\n【第一部分】DDL 安全守卫 (DdlGuard)\n");
    println!("------------------------------------------");
    println!("DdlGuard 使用 AST 解析验证 DDL SQL 语句的安全性，");
    println!("防止 DROP DATABASE、TRUNCATE 等危险操作。\n");

    let guard = DdlGuard::new();

    // 1. 测试允许的 DDL 操作
    println!("1️⃣ 允许的 DDL 操作:");
    println!("------------------------------------------");

    let allowed_operations = vec![
        "CREATE TABLE users (id INT PRIMARY KEY, name TEXT)",
        "ALTER TABLE users ADD COLUMN email TEXT",
        "CREATE INDEX idx_name ON users (name)",
        "CREATE VIEW active_users AS SELECT * FROM users WHERE active = true",
        "DROP INDEX idx_name",
        "DROP VIEW old_users",
        "TRUNCATE TABLE temp_data",
        "SELECT 1",
    ];

    for sql in allowed_operations {
        let result = guard.validate(sql);
        match &result {
            Ok(DdlValidationResult::Allowed) => {
                println!("  ✅ 允许: {}", truncate_sql(sql));
            }
            Ok(DdlValidationResult::Forbidden(_)) => {
                println!("  ❌ 意外拒绝: {} - [原因已记录]", truncate_sql(sql));
            }
            Ok(DdlValidationResult::ParseError(msg)) => {
                println!("  ⚠️  解析错误: {} - {}", truncate_sql(sql), msg);
            }
            Err(msg) => {
                println!("  ❌ 错误: {} - {}", truncate_sql(sql), msg);
            }
        }
    }

    // 2. 测试禁止的 DDL 操作
    println!("\n2️⃣ 禁止的 DDL 操作:");
    println!("------------------------------------------");

    let forbidden_operations = vec![
        ("DROP DATABASE production", "危险：删除整个数据库"),
        ("DROP TABLE users", "危险：删除表"),
        ("DROP ALL TABLES", "危险：批量删除表"),
        ("TRUNCATE users", "TRUNCATE 不在白名单中"),
        ("DELETE FROM users", "DELETE 不在白名单中"),
        ("INSERT INTO users VALUES (1)", "INSERT 不在白名单中"),
        ("UPDATE users SET name = 'test'", "UPDATE 不在白名单中"),
        ("", "空语句"),
        ("   \n\t  ", "空白语句"),
    ];

    for (sql, reason) in forbidden_operations {
        let result = guard.validate(sql);
        match &result {
            Ok(DdlValidationResult::Allowed) => {
                println!("  ❌ 意外允许: {}", truncate_sql(sql));
            }
            Ok(DdlValidationResult::Forbidden(_)) => {
                println!("  ✅ 正确拒绝: {} - {}", truncate_sql(sql), reason);
            }
            Ok(DdlValidationResult::ParseError(msg)) => {
                println!("  ✅ 解析失败: {} - {}", truncate_sql(sql), msg);
            }
            Err(msg) => {
                println!("  ✅ 错误处理: {} - {}", truncate_sql(sql), msg);
            }
        }
    }

    // 3. 测试大小写不敏感
    println!("\n3️⃣ 大小写不敏感测试:");
    println!("------------------------------------------");

    let case_tests = vec![
        "drop database production",
        "DROP TABLE users",
        "Create Table test (id int)",
    ];

    for sql in case_tests {
        let result = guard.validate(sql);
        let status = match &result {
            Ok(DdlValidationResult::Allowed) => "允许",
            Ok(DdlValidationResult::Forbidden(_)) => "拒绝",
            _ => "其他",
        };
        println!("  - {}: {}", truncate_sql(sql), status);
    }

    // 4. 测试 SQL 注入防护
    println!("\n4️⃣ SQL 注入防护测试:");
    println!("------------------------------------------");

    let injection_tests = vec![
        "DROP DATABASE production; --",
        "DROP TABLE users; SELECT * FROM passwords",
        "1; DROP DATABASE production",
    ];

    for sql in injection_tests {
        let result = guard.validate(sql);
        match &result {
            Ok(DdlValidationResult::Allowed) => {
                println!("  ❌ 意外允许: {}", truncate_sql(sql));
            }
            Ok(DdlValidationResult::Forbidden(_)) => {
                println!("  ✅ 已拦截: {}", truncate_sql(sql));
            }
            _ => {}
        }
    }

    // ============================================
    // 第二部分：SensitiveMasker - 敏感数据脱敏
    // ============================================
    println!("\n========================================");
    println!("【第二部分】敏感数据脱敏 (SensitiveMasker)\n");
    println!("------------------------------------------");
    println!("SensitiveMasker 提供多种敏感数据脱敏策略，");
    println!("支持手机号、邮箱、身份证、银行卡、姓名、地址等。\n");

    let _masker = SensitiveMasker::new();

    // 1. 手机号脱敏
    println!("1️⃣ 手机号脱敏 (保留前3后4):");
    println!("------------------------------------------");

    let phones = vec![
        "13812345678",
        "138 1234 5678",
        "138-1234-5678",
        "+86 138 1234 5678",
    ];

    for phone in phones {
        match SensitiveMasker::mask(phone, MaskType::Phone) {
            Ok(masked) => {
                println!("  {} -> {}", phone, masked);
            }
            Err(e) => {
                println!("  {} -> 错误: {}", phone, e);
            }
        }
    }

    // 2. 邮箱脱敏
    println!("\n2️⃣ 邮箱脱敏 (保留前2字符和域名):");
    println!("------------------------------------------");

    let emails = vec![
        "test@example.com",
        "user.name@company.org",
        "ab@c.com",
        "verylongemailaddress@gmail.com",
    ];

    for email in emails {
        match SensitiveMasker::mask(email, MaskType::Email) {
            Ok(masked) => {
                println!("  {} -> {}", email, masked);
            }
            Err(e) => {
                println!("  {} -> 错误: {}", email, e);
            }
        }
    }

    // 3. 身份证脱敏
    println!("\n3️⃣ 身份证脱敏 (保留前4后4):");
    println!("------------------------------------------");

    let id_cards = vec![
        "110101199001011234",  // 18位
        "110101900101123",     // 15位
    ];

    for id in id_cards {
        match SensitiveMasker::mask(id, MaskType::IdCard) {
            Ok(masked) => {
                println!("  {} -> {}", id, masked);
            }
            Err(e) => {
                println!("  {} -> 错误: {}", id, e);
            }
        }
    }

    // 4. 银行卡脱敏
    println!("\n4️⃣ 银行卡脱敏 (保留前4后4):");
    println!("------------------------------------------");

    let bank_cards = vec![
        "6222021234567890",       // 16位
        "6222021234567890123",    // 19位
        "6222 0234 5678 9012",    // 带空格
    ];

    for card in bank_cards {
        match SensitiveMasker::mask(card, MaskType::BankCard) {
            Ok(masked) => {
                println!("  {} -> {}", card, masked);
            }
            Err(e) => {
                println!("  {} -> 错误: {}", card, e);
            }
        }
    }

    // 5. 姓名脱敏
    println!("\n5️⃣ 姓名脱敏 (保留姓氏):");
    println!("------------------------------------------");

    let names = vec![
        "张三",
        "李某某",
        "欧阳明月",
        "阿凡提",
    ];

    for name in names {
        match SensitiveMasker::mask(name, MaskType::Name) {
            Ok(masked) => {
                println!("  {} -> {}", name, masked);
            }
            Err(e) => {
                println!("  {} -> 错误: {}", name, e);
            }
        }
    }

    // 6. 地址脱敏
    println!("\n6️⃣ 地址脱敏 (保留省市区):");
    println!("------------------------------------------");

    let addresses = vec![
        "北京市朝阳区某某街道123号",
        "上海市浦东新区世纪大道1000号",
        "某某小区5栋601室",
    ];

    for address in addresses {
        match SensitiveMasker::mask(address, MaskType::Address) {
            Ok(masked) => {
                println!("  {} -> {}", truncate_sql(address), masked);
            }
            Err(e) => {
                println!("  {} -> 错误: {}", truncate_sql(address), e);
            }
        }
    }

    // 7. 自定义脱敏
    println!("\n7️⃣ 自定义脱敏 (指定保留位数):");
    println!("------------------------------------------");

    let custom_tests = vec![
        ("1234567890", 3, 3),   // 保留前3后3
        ("ABCDEFGH", 2, 2),     // 保留前2后2
        ("ID12345", 2, 4),      // 保留前2后4
    ];

    for (data, prefix, suffix) in custom_tests {
        let mask_type = MaskType::Custom {
            keep_prefix: prefix,
            keep_suffix: suffix,
        };
        match SensitiveMasker::mask(data, mask_type) {
            Ok(masked) => {
                println!("  {} (保留前{}后{}) -> {}", data, prefix, suffix, masked);
            }
            Err(e) => {
                println!("  {} -> 错误: {}", data, e);
            }
        }
    }

    // 8. 错误处理
    println!("\n8️⃣ 错误处理测试:");
    println!("------------------------------------------");

    let error_tests = vec![
        ("123", MaskType::Phone, "过短的手机号"),
        ("invalid", MaskType::Email, "无效邮箱"),
        ("12345", MaskType::IdCard, "无效身份证长度"),
        ("1234567", MaskType::BankCard, "过短的银行卡号"),
    ];

    for (data, mask_type, desc) in error_tests {
        match SensitiveMasker::mask(data, mask_type) {
            Ok(masked) => {
                println!("  ❌ {}: 意外成功 -> {}", desc, masked);
            }
            Err(e) => {
                println!("  ✅ {}: 正确报错 - {}", desc, e);
            }
        }
    }

    // ============================================
    // 总结
    // ============================================
    println!("\n========================================");
    println!("✨ 安全功能示例运行完成！");
    println!("========================================\n");

    println!("💡 DdlGuard 安全特性:");
    println!("  - 使用 AST 解析，比字符串匹配更安全");
    println!("  - 白名单机制，只允许明确的 DDL 操作");
    println!("  - 大小写不敏感，防止大小写绕过");
    println!("  - 拦截 DROP DATABASE/TRUNCATE 等危险操作\n");

    println!("💡 SensitiveMasker 脱敏类型:");
    println!("  - Phone: 手机号 138****5678");
    println!("  - Email: 邮箱 te****@example.com");
    println!("  - IdCard: 身份证 1101****1234");
    println!("  - BankCard: 银行卡 6222****7890");
    println!("  - Name: 姓名 张**");
    println!("  - Address: 地址 保留省市区");
    println!("  - Custom: 自定义保留位数\n");

    println!("💡 使用场景:");
    println!("  - DdlGuard: 数据库中间件、DDL 防火墙");
    println!("  - SensitiveMasker: 审计日志、API 响应、数据导出");

    Ok(())
}

/// 截断过长的 SQL 语句用于显示
fn truncate_sql(sql: &str) -> String {
    let max_len = 50;
    if sql.len() > max_len {
        format!("{}...", &sql[..max_len])
    } else {
        sql.to_string()
    }
}
