// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Authentication module implementation details.
//!
//! Contains impl blocks extracted from [`super`].

use super::*;

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

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
