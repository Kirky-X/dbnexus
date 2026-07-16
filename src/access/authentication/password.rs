// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 密码哈希和验证模块

use super::models::{AuthError, AuthResult};
use bcrypt::{hash, verify};

/// vuln-0004 修复：bcrypt cost factor 提升至 12（原 DEFAULT_COST 可能随版本变化）
const BCRYPT_COST: u32 = 12;

/// 密码最小长度（vuln-0004：从 8 提升至 12）
const MIN_PASSWORD_LEN: usize = 12;

/// 密码最大长度（防止 bcrypt 72 字节截断导致的密码碰撞）
const MAX_PASSWORD_LEN: usize = 72;

/// 常见弱密码黑名单（top 100+，vuln-0004 修复）
///
/// 来源：基于公开的常见密码泄露统计（如 RockYou、HaveIBeenPwned Top 100）。
/// 即使满足复杂度要求，黑名单中的密码也必须被拒绝。
/// 包含两类：
/// 1. 简单弱密码（会被复杂度检查拦截，但保留作 defense-in-depth）
/// 2. 满足复杂度但仍常见的密码（如 "Password123!"）
const COMMON_PASSWORDS: &[&str] = &[
    // 简单弱密码
    "123456",
    "123456789",
    "12345678",
    "1234567",
    "1234567890",
    "12345",
    "1234",
    "123456789a",
    "123456a",
    "123321",
    "654321",
    "666666",
    "888888",
    "000000",
    "111111",
    "222222",
    "333333",
    "444444",
    "555555",
    "777777",
    "999999",
    "abc123",
    "abcdef",
    "abc1234",
    "abcab123",
    "qwerty",
    "qwerty123",
    "qwerty1",
    "qwerty12",
    "qazwsx",
    "qweasd",
    "q1w2e3r4",
    "qwer1234",
    "asdfgh",
    "zxcvbn",
    "zxcvbnm",
    "1q2w3e4r",
    "1q2w3e",
    "1qaz2wsx",
    "password",
    "password1",
    "password12",
    "password123",
    "passw0rd",
    "passw0rd1",
    "pass1234",
    "passwd",
    "p@ssw0rd",
    "p@ssword",
    "admin",
    "admin123",
    "administrator",
    "root",
    "root123",
    "toor",
    "superuser",
    "letmein",
    "letmein1",
    "welcome",
    "welcome1",
    "welcome123",
    "monkey",
    "monkey123",
    "monkey1313",
    "dragon",
    "dragon123",
    "master",
    "master123",
    "login",
    "login123",
    "princess",
    "princess1",
    "football",
    "football123",
    "baseball",
    "baseball123",
    "soccer",
    "soccer123",
    "hockey",
    "hockey123",
    "jordan",
    "jordan23",
    "jordan123",
    "michael",
    "michael1",
    "daniel",
    "daniel123",
    "andrew",
    "andrew1",
    "joshua",
    "joshua1",
    "harley",
    "harley1",
    "robert",
    "robert1",
    "thomas",
    "thomas1",
    "jennifer",
    "jennifer1",
    "secret",
    "secret1",
    "secret123",
    "test",
    "test123",
    "test1234",
    "testtest",
    "iloveyou",
    "iloveyou1",
    "iloveyou2",
    "trustno1",
    // 满足复杂度但仍常见的密码（大小写不敏感匹配）
    "password123!",
    "password1!@#",
    "welcome123!@",
    "admin123!@#",
    "qwerty123!@#",
    "letmein123!@",
    "welcome@1234",
    "admin@12345",
    "passw0rd!@#",
    "password@123",
    "abc123!@#$",
    "welcome@123",
    "admin!@#$%",
    "root@12345",
    "test@12345",
];

/// 密码哈希器
pub struct PasswordHasher;

impl PasswordHasher {
    /// 创建新的密码哈希器
    pub fn new() -> Self {
        Self
    }

    /// 哈希密码
    ///
    /// 使用 bcrypt 算法，cost factor 为 12（vuln-0004 修复）。
    pub fn hash(&self, password: &str) -> AuthResult<String> {
        hash(password, BCRYPT_COST).map_err(|e| AuthError::PasswordHash(e.to_string()))
    }

    /// 验证密码
    pub fn verify(&self, password: &str, hash: &str) -> AuthResult<()> {
        let is_valid = verify(password, hash).map_err(|_| AuthError::InvalidCredentials)?;

        if is_valid {
            Ok(())
        } else {
            Err(AuthError::InvalidCredentials)
        }
    }

    /// 验证密码强度
    ///
    /// vuln-0004 修复：增强密码策略
    /// - 最小长度 12 字符
    /// - 必须包含大写字母、小写字母、数字、特殊字符
    /// - 不在常见弱密码黑名单中（大小写不敏感）
    /// - 最大长度 72 字符（bcrypt 限制）
    pub fn validate_strength(&self, password: &str) -> AuthResult<()> {
        // 长度检查
        if password.len() < MIN_PASSWORD_LEN {
            return Err(AuthError::PasswordHash(format!(
                "Password must be at least {} characters",
                MIN_PASSWORD_LEN
            )));
        }

        if password.len() > MAX_PASSWORD_LEN {
            return Err(AuthError::PasswordHash(format!(
                "Password must not exceed {} characters (bcrypt truncation limit)",
                MAX_PASSWORD_LEN
            )));
        }

        // 必须包含大写字母
        if !password.chars().any(|c| c.is_uppercase()) {
            return Err(AuthError::PasswordHash(
                "Password must contain at least one uppercase letter".to_string(),
            ));
        }

        // 必须包含小写字母
        if !password.chars().any(|c| c.is_lowercase()) {
            return Err(AuthError::PasswordHash(
                "Password must contain at least one lowercase letter".to_string(),
            ));
        }

        // 必须包含数字
        if !password.chars().any(|c| c.is_numeric()) {
            return Err(AuthError::PasswordHash(
                "Password must contain at least one digit".to_string(),
            ));
        }

        // 必须包含特殊字符
        if !password.chars().any(|c| !c.is_alphanumeric()) {
            return Err(AuthError::PasswordHash(
                "Password must contain at least one special character".to_string(),
            ));
        }

        // 常见弱密码黑名单检查（大小写不敏感）
        let password_lower = password.to_lowercase();
        if COMMON_PASSWORDS.contains(&password_lower.as_str()) {
            return Err(AuthError::PasswordHash(
                "Password is too common and easily guessable".to_string(),
            ));
        }

        Ok(())
    }
}

impl Default for PasswordHasher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PASSWORD: &str = "TestPassword123!";
    const TEST_WRONG_PASSWORD: &str = "WrongPassword!";
    const TEST_VALID_PASSWORD: &str = "ValidPass@123";
    const TEST_WEAK_SHORT: &str = "Short1!";
    const TEST_WEAK_NO_LETTER: &str = "12345678!@#$";
    const TEST_WEAK_NO_DIGIT: &str = "OnlyLetters!@";
    const TEST_WEAK_NO_UPPER: &str = "onlylowercase1!";
    const TEST_WEAK_NO_LOWER: &str = "ONLYUPPERCASE1!";
    const TEST_WEAK_NO_SPECIAL: &str = "Password123456";
    const TEST_WEAK_COMMON: &str = "Password123!";
    const TEST_PASSWORD_A: &str = "PasswordA@123";
    const TEST_PASSWORD_B: &str = "SecurePass@456";
    const TEST_PASSWORD_C: &str = "MySecret@789";

    #[test]
    fn test_hash_password() {
        let hasher = PasswordHasher::new();
        let password = TEST_PASSWORD;

        let hash = hasher.hash(password).unwrap();
        assert_ne!(hash, password);
        assert!(hash.starts_with("$2b$"));
    }

    #[test]
    fn test_verify_password() {
        let hasher = PasswordHasher::new();
        let password = TEST_PASSWORD;

        let hash = hasher.hash(password).unwrap();
        assert!(hasher.verify(password, &hash).is_ok());
        assert!(hasher.verify(TEST_WRONG_PASSWORD, &hash).is_err());
    }

    #[test]
    fn test_password_strength() {
        let hasher = PasswordHasher::new();

        // 太短
        assert!(hasher.validate_strength(TEST_WEAK_SHORT).is_err());

        // 缺少数字
        assert!(hasher.validate_strength(TEST_WEAK_NO_DIGIT).is_err());

        // 缺少字母
        assert!(hasher.validate_strength(TEST_WEAK_NO_LETTER).is_err());

        // 缺少大写字母
        assert!(hasher.validate_strength(TEST_WEAK_NO_UPPER).is_err());

        // 缺少小写字母
        assert!(hasher.validate_strength(TEST_WEAK_NO_LOWER).is_err());

        // 缺少特殊字符
        assert!(hasher.validate_strength(TEST_WEAK_NO_SPECIAL).is_err());

        // 常见弱密码（即使满足复杂度）
        assert!(hasher.validate_strength(TEST_WEAK_COMMON).is_err());

        // 有效密码
        assert!(hasher.validate_strength(TEST_VALID_PASSWORD).is_ok());
    }

    /// vuln-0004 回归测试：旧策略下 "password1" 仅 8 字符 + 1 字母 + 1 数字即可通过，
    /// 新策略必须拒绝。
    #[test]
    fn test_vuln_0004_password1_rejected() {
        let hasher = PasswordHasher::new();
        // "password1" 在旧策略下可通过（8 字符 + 字母 + 数字）
        assert!(
            hasher.validate_strength("password1").is_err(),
            "password1 must be rejected by enhanced policy"
        );
    }

    /// vuln-0004 回归测试：bcrypt cost factor 必须为 12
    #[test]
    fn test_vuln_0004_bcrypt_cost_12() {
        let hasher = PasswordHasher::new();
        let hash = hasher.hash(TEST_VALID_PASSWORD).unwrap();
        // bcrypt hash 格式: $2b$<cost>$<salt+hash>
        // cost 应为 12
        let cost_str = hash.split('$').nth(2).expect("hash should have cost field");
        assert_eq!(cost_str, "12", "bcrypt cost factor must be 12");
    }

    /// vuln-0004 回归测试：常见弱密码黑名单
    #[test]
    fn test_vuln_0004_common_password_blacklist() {
        let hasher = PasswordHasher::new();
        // "Password123!" 满足复杂度（13 字符 + 大小写 + 数字 + 特殊字符）
        // 但 "password123!" 在黑名单中，必须被拒绝
        assert!(
            hasher.validate_strength("Password123!").is_err(),
            "Password123! is in blacklist and must be rejected"
        );
        // "Welcome123!@" 满足复杂度但在黑名单中
        assert!(
            hasher.validate_strength("Welcome123!@").is_err(),
            "Welcome123!@ is in blacklist and must be rejected"
        );
        // 不在黑名单中的强密码应通过
        assert!(
            hasher.validate_strength("Xy7#kQ9$mL2p").is_ok(),
            "Xy7#kQ9$mL2p should pass (not in blacklist)"
        );
    }

    #[test]
    fn test_hash_and_verify_consistency() {
        let hasher = PasswordHasher::new();
        let passwords = vec![TEST_PASSWORD_A, TEST_PASSWORD_B, TEST_PASSWORD_C];

        for password in passwords {
            let hash = hasher.hash(password).unwrap();
            assert!(hasher.verify(password, &hash).is_ok());
            assert!(hasher.verify(&password.to_uppercase(), &hash).is_err());
        }
    }
}
