// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 认证模块
//!
//! 提供基于 JWT 的用户认证功能，包括：
//! - 密码哈希和验证
//! - JWT Token 生成和验证
//! - 用户认证流程

pub mod jwt;
pub mod models;
pub mod password;

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

impl AuthenticationManager {
    /// 创建新的认证管理器
    ///
    /// # 参数
    ///
    /// * `jwt_secret` - JWT 签名密钥（建议从环境变量读取）
    pub fn new(jwt_secret: &[u8]) -> Self {
        Self {
            password_hasher: PasswordHasher::new(),
            jwt_manager: JwtManager::new(jwt_secret),
            users: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 使用自定义配置创建认证管理器
    pub fn with_config(jwt_secret: &[u8], access_expiration_secs: u64, refresh_expiration_secs: u64) -> Self {
        Self {
            password_hasher: PasswordHasher::new(),
            jwt_manager: JwtManager::with_expiration(jwt_secret, access_expiration_secs, refresh_expiration_secs),
            users: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 添加或更新用户（直接插入已哈希的 User）
    ///
    /// 适用于测试/迁移场景。生产用户注册应使用 [`register_user`](Self::register_user)，
    /// 后者会验证密码强度并自动哈希。
    pub async fn add_user(&self, user: User) -> AuthResult<()> {
        let mut users = self.users.write().await;
        users.insert(user.username.clone(), user);
        Ok(())
    }

    /// 注册新用户（验证密码强度 + 哈希 + 存储）
    ///
    /// 与 [`add_user`](Self::add_user) 的区别：接收明文密码，内部执行
    /// `validate_strength → hash → insert` 完整流程。适用于用户注册场景。
    ///
    /// # 参数
    ///
    /// * `username` - 用户名（同时作为内部用户 ID）
    /// * `password` - 明文密码（需通过强度检查：≥8 字符 + 含字母 + 含数字）
    /// * `role` - 用户角色
    ///
    /// # 错误
    ///
    /// 密码强度不足时返回 `AuthError::PasswordHash`
    pub async fn register_user(&self, username: &str, password: &str, role: &str) -> AuthResult<()> {
        // 1. 验证密码强度
        self.password_hasher.validate_strength(password)?;

        // 2. 哈希密码
        let password_hash = self.password_hasher.hash(password)?;

        // 3. 构造 User 并存储
        let user = User {
            id: username.to_string(),
            username: username.to_string(),
            password_hash,
            role: role.to_string(),
            email: None,
            created_at: None,
        };

        let mut users = self.users.write().await;
        users.insert(username.to_string(), user);
        Ok(())
    }

    /// 用户认证
    ///
    /// 验证用户凭据并生成 JWT Token
    pub async fn authenticate(&self, credentials: AuthCredentials) -> AuthResult<String> {
        // 1. 验证用户名
        let users = self.users.read().await;
        let user = users.get(&credentials.username).ok_or(AuthError::InvalidCredentials)?;

        // 2. 验证密码
        self.password_hasher
            .verify(&credentials.password, &user.password_hash)?;

        // 3. 生成 JWT token
        let token = self
            .jwt_manager
            .generate_token(&user.id, &user.username, &user.role, TokenType::Access)?;

        Ok(token)
    }

    /// 验证 JWT Token
    pub fn verify_token(&self, token: &str) -> AuthResult<JwtClaims> {
        self.jwt_manager.verify_token(token)
    }

    /// 刷新访问令牌
    pub fn refresh_token(&self, refresh_token: &str) -> AuthResult<String> {
        self.jwt_manager.refresh_access_token(refresh_token)
    }

    /// 获取用户信息
    pub async fn get_user(&self, username: &str) -> AuthResult<User> {
        let users = self.users.read().await;
        users
            .get(username)
            .cloned()
            .ok_or_else(|| AuthError::UserNotFound(username.to_string()))
    }

    /// 删除用户
    pub async fn remove_user(&self, username: &str) -> AuthResult<()> {
        let mut users = self.users.write().await;
        users
            .remove(username)
            .map(|_| ())
            .ok_or_else(|| AuthError::UserNotFound(username.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_manager() -> AuthenticationManager {
        let manager = AuthenticationManager::new(b"test-secret-key");

        // 添加测试用户
        let password_hash = manager.password_hasher.hash("TestPassword123").unwrap();
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
            password: "TestPassword123".to_string(),
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
            password: "WrongPassword".to_string(),
        };

        let result = manager.authenticate(credentials).await;
        assert!(matches!(result, Err(AuthError::InvalidCredentials)));
    }

    #[tokio::test]
    async fn test_authenticate_failure_user_not_found() {
        let manager = create_test_manager().await;

        let credentials = AuthCredentials {
            username: "nonexistent".to_string(),
            password: "TestPassword123".to_string(),
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
            password: "TestPassword123".to_string(),
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
        let result = manager.register_user("u1", "Short1", "user").await;
        assert!(
            matches!(result, Err(AuthError::PasswordHash(_))),
            "short password should be rejected"
        );
    }

    #[tokio::test]
    async fn test_register_user_no_letter_rejected() {
        let manager = AuthenticationManager::new(b"secret");
        // 无字母
        let result = manager.register_user("u2", "12345678", "user").await;
        assert!(
            matches!(result, Err(AuthError::PasswordHash(_))),
            "password without letter should be rejected"
        );
    }

    #[tokio::test]
    async fn test_register_user_no_digit_rejected() {
        let manager = AuthenticationManager::new(b"secret");
        // 无数字
        let result = manager.register_user("u3", "OnlyLetters", "user").await;
        assert!(
            matches!(result, Err(AuthError::PasswordHash(_))),
            "password without digit should be rejected"
        );
    }

    #[tokio::test]
    async fn test_register_user_strong_password_succeeds_and_authenticates() {
        let manager = AuthenticationManager::new(b"secret");
        manager.register_user("alice", "SecurePass123", "admin").await.unwrap();

        // 注册后应能认证
        let token = manager
            .authenticate(AuthCredentials {
                username: "alice".to_string(),
                password: "SecurePass123".to_string(),
            })
            .await
            .unwrap();
        let claims = manager.verify_token(&token).unwrap();
        assert_eq!(claims.username, "alice");
        assert_eq!(claims.role, "admin");
    }
}
