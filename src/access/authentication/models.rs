// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 认证相关数据模型

use serde::{Deserialize, Serialize};

/// 认证错误类型
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    /// 无效的凭据（用户名或密码错误）
    #[error("Invalid credentials")]
    InvalidCredentials,

    /// Token 生成失败
    #[error("Token generation failed: {0}")]
    TokenGeneration(String),

    /// 无效的 Token
    #[error("Invalid token")]
    InvalidToken,

    /// Token 已过期
    #[error("Token expired")]
    TokenExpired,

    /// 用户不存在
    #[error("User not found: {0}")]
    UserNotFound(String),

    /// 密码哈希失败
    #[error("Password hash failed: {0}")]
    PasswordHash(String),

    /// 用户存储已达上限
    #[error("User storage limit reached: {0}")]
    UserLimitReached(String),
}

/// 认证结果类型别名
pub type AuthResult<T> = Result<T, AuthError>;

/// 用户信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// 用户 ID
    pub id: String,

    /// 用户名
    pub username: String,

    /// 密码哈希（bcrypt）— 禁止序列化到 JSON，防止凭据泄露
    #[serde(skip)]
    pub password_hash: String,

    /// 用户角色
    pub role: String,

    /// 邮箱地址
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    /// 创建时间（ISO 8601 格式）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>, // 使用 ISO 8601 字符串代替chrono
}

/// 认证凭据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthCredentials {
    /// 用户名
    pub username: String,

    /// 密码 — 禁止序列化，防止明文密码泄露
    #[serde(skip)]
    pub password: String,
}

/// Token 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenType {
    /// 访问令牌（短期有效）
    Access,

    /// 刷新令牌（长期有效）
    Refresh,
}

/// JWT Claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    /// 主题（用户 ID）
    pub sub: String,

    /// 用户名
    pub username: String,

    /// 用户角色
    pub role: String,

    /// 过期时间（Unix 时间戳）
    pub exp: usize,

    /// 签发时间（Unix 时间戳）
    pub iat: usize,

    /// Token 类型
    pub token_type: TokenType,

    /// JWT ID — 唯一标识符，用于单独撤销 token
    pub jti: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_error_display() {
        let err = AuthError::InvalidCredentials;
        assert_eq!(format!("{}", err), "Invalid credentials");
    }

    #[test]
    fn test_user_serialization() {
        let user = User {
            id: "123".to_string(),
            username: "test_user".to_string(),
            password_hash: "hash".to_string(),
            role: "admin".to_string(),
            email: Some("test@example.com".to_string()),
            created_at: None,
        };

        let json = serde_json::to_string(&user).unwrap();
        assert!(json.contains("test_user"));
        // 密码哈希不得出现在序列化输出中（防止凭据泄露）
        assert!(!json.contains("password_hash"));
        assert!(!json.contains("hash"));
    }
}
