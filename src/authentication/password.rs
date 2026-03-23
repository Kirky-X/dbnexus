// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 密码哈希和验证模块

use super::models::{AuthError, AuthResult};
use bcrypt::{hash, verify, DEFAULT_COST};

/// 密码哈希器
pub struct PasswordHasher;

impl PasswordHasher {
    /// 创建新的密码哈希器
    pub fn new() -> Self {
        Self
    }

    /// 哈希密码
    pub fn hash(&self, password: &str) -> AuthResult<String> {
        hash(password, DEFAULT_COST)
            .map_err(|e| AuthError::PasswordHash(e.to_string()))
    }

    /// 验证密码
    pub fn verify(&self, password: &str, hash: &str) -> AuthResult<()> {
        let is_valid = verify(password, hash)
            .map_err(|_| AuthError::InvalidCredentials)?;

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
                "Password must be at least 8 characters".to_string()
            ));
        }

        // 检查是否包含字母
        if !password.chars().any(|c| c.is_alphabetic()) {
            return Err(AuthError::PasswordHash(
                "Password must contain at least one letter".to_string()
            ));
        }

        // 检查是否包含数字
        if !password.chars().any(|c| c.is_numeric()) {
            return Err(AuthError::PasswordHash(
                "Password must contain at least one digit".to_string()
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

    #[test]
    fn test_hash_password() {
        let hasher = PasswordHasher::new();
        let password = "TestPassword123";

        let hash = hasher.hash(password).unwrap();
        assert_ne!(hash, password);
        assert!(hash.starts_with("$2b$"));
    }

    #[test]
    fn test_verify_password() {
        let hasher = PasswordHasher::new();
        let password = "TestPassword123";

        let hash = hasher.hash(password).unwrap();
        assert!(hasher.verify(password, &hash).is_ok());
        assert!(hasher.verify("WrongPassword", &hash).is_err());
    }

    #[test]
    fn test_password_strength() {
        let hasher = PasswordHasher::new();

        // 太短
        assert!(hasher.validate_strength("Short1").is_err());

        // 缺少数字
        assert!(hasher.validate_strength("OnlyLetters").is_err());

        // 缺少字母
        assert!(hasher.validate_strength("12345678").is_err());

        // 有效密码
        assert!(hasher.validate_strength("ValidPass123").is_ok());
    }

    #[test]
    fn test_hash_and_verify_consistency() {
        let hasher = PasswordHasher::new();
        let passwords = vec![
            "Password123",
            "SecurePass456",
            "MySecret@789",
        ];

        for password in passwords {
            let hash = hasher.hash(password).unwrap();
            assert!(hasher.verify(password, &hash).is_ok());
            assert!(hasher.verify(&password.to_uppercase(), &hash).is_err());
        }
    }
}
