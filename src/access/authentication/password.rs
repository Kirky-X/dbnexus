// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 密码哈希和验证模块

use super::models::{AuthError, AuthResult};
use bcrypt::{DEFAULT_COST, hash, verify};

/// 密码哈希器
pub struct PasswordHasher;

impl PasswordHasher {
    /// 创建新的密码哈希器
    pub fn new() -> Self {
        Self
    }

    /// 哈希密码
    pub fn hash(&self, password: &str) -> AuthResult<String> {
        hash(password, DEFAULT_COST).map_err(|e| AuthError::PasswordHash(e.to_string()))
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
    pub fn validate_strength(&self, password: &str) -> AuthResult<()> {
        if password.len() < 8 {
            return Err(AuthError::PasswordHash(
                "Password must be at least 8 characters".to_string(),
            ));
        }

        // 检查是否包含字母
        if !password.chars().any(|c| c.is_alphabetic()) {
            return Err(AuthError::PasswordHash(
                "Password must contain at least one letter".to_string(),
            ));
        }

        // 检查是否包含数字
        if !password.chars().any(|c| c.is_numeric()) {
            return Err(AuthError::PasswordHash(
                "Password must contain at least one digit".to_string(),
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

    const TEST_PASSWORD: &str = "TestPassword123";
    const TEST_WRONG_PASSWORD: &str = "WrongPassword";
    const TEST_VALID_PASSWORD: &str = "ValidPass123";
    const TEST_WEAK_SHORT: &str = "Short1";
    const TEST_WEAK_NO_LETTER: &str = "12345678";
    const TEST_WEAK_NO_DIGIT: &str = "OnlyLetters";
    const TEST_PASSWORD_A: &str = "Password123";
    const TEST_PASSWORD_B: &str = "SecurePass456";
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

        // 有效密码
        assert!(hasher.validate_strength(TEST_VALID_PASSWORD).is_ok());
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
