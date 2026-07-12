// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 认证模块
//!
//! 提供基于 JWT 的用户认证功能，包括：
//! - 密码哈希和验证
//! - JWT Token 生成和验证
//! - 用户认证流程

pub mod jwt;
pub mod models;
pub mod password;

mod auth_impl;

pub use jwt::JwtManager;
pub use models::{AuthCredentials, AuthError, AuthResult, JwtClaims, TokenType, User};
pub use password::PasswordHasher;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 认证管理器
///
/// 统一管理用户认证、Token 生成和验证
pub struct AuthenticationManager {
    password_hasher: PasswordHasher,
    jwt_manager: JwtManager,
    users: Arc<RwLock<HashMap<String, User>>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PASSWORD: &str = "TestPassword123";
    const TEST_WRONG_PASSWORD: &str = "WrongPassword";
    const TEST_STRONG_PASSWORD: &str = "SecurePass123";
    const TEST_WEAK_SHORT: &str = "Short1";
    const TEST_WEAK_NO_LETTER: &str = "12345678";
    const TEST_WEAK_NO_DIGIT: &str = "OnlyLetters";

    async fn create_test_manager() -> AuthenticationManager {
        let manager = AuthenticationManager::new(b"test-secret-key");

        // 添加测试用户
        let password_hash = manager.password_hasher.hash(TEST_PASSWORD).unwrap();
        let user = User {
            id: "user123".to_string(),
            username: "testuser".to_string(),
            password_hash,
            role: "admin".to_string(),
            email: Some("test@example.com".to_string()),
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
        };

        manager.add_user(user).await.unwrap();
        manager
    }

    #[tokio::test]
    async fn test_authenticate_success() {
        let manager = create_test_manager().await;

        let credentials = AuthCredentials {
            username: "testuser".to_string(),
            password: TEST_PASSWORD.to_string(),
        };

        let token = manager.authenticate(credentials).await.unwrap();

        // 验证 token
        let claims = manager.verify_token(&token).unwrap();
        assert_eq!(claims.username, "testuser");
        assert_eq!(claims.role, "admin");
    }

    #[tokio::test]
    async fn test_authenticate_failure_wrong_password() {
        let manager = create_test_manager().await;

        let credentials = AuthCredentials {
            username: "testuser".to_string(),
            password: TEST_WRONG_PASSWORD.to_string(),
        };

        let result = manager.authenticate(credentials).await;
        assert!(matches!(result, Err(AuthError::InvalidCredentials)));
    }

    #[tokio::test]
    async fn test_authenticate_failure_user_not_found() {
        let manager = create_test_manager().await;

        let credentials = AuthCredentials {
            username: "nonexistent".to_string(),
            password: TEST_PASSWORD.to_string(),
        };

        let result = manager.authenticate(credentials).await;
        assert!(matches!(result, Err(AuthError::InvalidCredentials)));
    }

    #[tokio::test]
    async fn test_get_user() {
        let manager = create_test_manager().await;

        let user = manager.get_user("testuser").await.unwrap();
        assert_eq!(user.username, "testuser");
        assert_eq!(user.role, "admin");
    }

    #[tokio::test]
    async fn test_remove_user() {
        let manager = create_test_manager().await;

        manager.remove_user("testuser").await.unwrap();

        let result = manager.get_user("testuser").await;
        assert!(matches!(result, Err(AuthError::UserNotFound(_))));
    }

    #[tokio::test]
    async fn test_token_refresh_flow() {
        let manager = create_test_manager().await;

        // 认证获取访问令牌
        let credentials = AuthCredentials {
            username: "testuser".to_string(),
            password: TEST_PASSWORD.to_string(),
        };
        let _access_token = manager.authenticate(credentials).await.unwrap();

        // 生成刷新令牌
        let refresh_token = manager
            .jwt_manager
            .generate_token("user123", "testuser", "admin", TokenType::Refresh)
            .unwrap();

        // 刷新访问令牌
        let new_access_token = manager.refresh_token(&refresh_token).unwrap();

        let claims = manager.verify_token(&new_access_token).unwrap();
        assert_eq!(claims.username, "testuser");
    }

    // ============================================================================
    // register_user 密码强度验证测试（diting security 修复）
    // ============================================================================

    #[tokio::test]
    async fn test_register_user_weak_password_rejected() {
        let manager = AuthenticationManager::new(b"secret");
        // 太短
        let result = manager.register_user("u1", TEST_WEAK_SHORT, "user").await;
        assert!(
            matches!(result, Err(AuthError::PasswordHash(_))),
            "short password should be rejected"
        );
    }

    #[tokio::test]
    async fn test_register_user_no_letter_rejected() {
        let manager = AuthenticationManager::new(b"secret");
        // 无字母
        let result = manager.register_user("u2", TEST_WEAK_NO_LETTER, "user").await;
        assert!(
            matches!(result, Err(AuthError::PasswordHash(_))),
            "password without letter should be rejected"
        );
    }

    #[tokio::test]
    async fn test_register_user_no_digit_rejected() {
        let manager = AuthenticationManager::new(b"secret");
        // 无数字
        let result = manager.register_user("u3", TEST_WEAK_NO_DIGIT, "user").await;
        assert!(
            matches!(result, Err(AuthError::PasswordHash(_))),
            "password without digit should be rejected"
        );
    }

    #[tokio::test]
    async fn test_register_user_strong_password_succeeds_and_authenticates() {
        let manager = AuthenticationManager::new(b"secret");
        manager
            .register_user("alice", TEST_STRONG_PASSWORD, "admin")
            .await
            .unwrap();

        // 注册后应能认证
        let token = manager
            .authenticate(AuthCredentials {
                username: "alice".to_string(),
                password: TEST_STRONG_PASSWORD.to_string(),
            })
            .await
            .unwrap();
        let claims = manager.verify_token(&token).unwrap();
        assert_eq!(claims.username, "alice");
        assert_eq!(claims.role, "admin");
    }
}
