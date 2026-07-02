// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! JWT Token 管理模块

use super::models::{AuthError, AuthResult, JwtClaims, TokenType};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use std::time::{SystemTime, UNIX_EPOCH};

/// JWT 访问令牌默认过期时间（秒）
const ACCESS_TOKEN_EXPIRATION_SECS: u64 = 3600; // 1 hour

/// JWT 刷新令牌默认过期时间（秒）
const REFRESH_TOKEN_EXPIRATION_SECS: u64 = 3600 * 24 * 7; // 7 days

/// JWT 管理器
pub struct JwtManager {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    access_expiration_secs: u64,
    refresh_expiration_secs: u64,
}

impl JwtManager {
    /// 创建新的 JWT 管理器
    ///
    /// # 参数
    ///
    /// * `secret` - JWT 签名密钥（建议从环境变量读取）
    pub fn new(secret: &[u8]) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
            access_expiration_secs: ACCESS_TOKEN_EXPIRATION_SECS,
            refresh_expiration_secs: REFRESH_TOKEN_EXPIRATION_SECS,
        }
    }

    /// 使用自定义过期时间创建 JWT 管理器
    pub fn with_expiration(secret: &[u8], access_expiration_secs: u64, refresh_expiration_secs: u64) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
            access_expiration_secs,
            refresh_expiration_secs,
        }
    }

    /// 生成 JWT Token
    pub fn generate_token(
        &self,
        user_id: &str,
        username: &str,
        role: &str,
        token_type: TokenType,
    ) -> AuthResult<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| AuthError::TokenGeneration("System time error".to_string()))?
            .as_secs() as usize;

        let expiration = match token_type {
            TokenType::Access => now + self.access_expiration_secs as usize,
            TokenType::Refresh => now + self.refresh_expiration_secs as usize,
        };

        let claims = JwtClaims {
            sub: user_id.to_string(),
            username: username.to_string(),
            role: role.to_string(),
            exp: expiration,
            iat: now,
            token_type,
        };

        encode(&Header::default(), &claims, &self.encoding_key).map_err(|e| AuthError::TokenGeneration(e.to_string()))
    }

    /// 验证 JWT Token（不校验 token_type）
    ///
    /// 使用 `leeway = 0` 严格过期检查（无宽限时间），确保 token 过期后立即失效。
    /// 对于安全敏感的数据库中间件，严格的过期语义优于 jsonwebtoken 默认的 60 秒宽限。
    /// 分布式时钟漂移应通过 NTP 同步解决，而非依赖 leeway。
    ///
    /// **注意**：此方法不校验 `token_type`，调用方无法区分 Access/Refresh token。
    /// 安全敏感场景应使用 [`verify_access_token`](Self::verify_access_token) 或
    /// [`verify_refresh_token`](Self::verify_refresh_token)。
    pub fn verify_token(&self, token: &str) -> AuthResult<JwtClaims> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.leeway = 0;
        decode::<JwtClaims>(token, &self.decoding_key, &validation)
            .map(|data| data.claims)
            .map_err(|e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
                _ => AuthError::InvalidToken,
            })
    }

    /// 验证 Access Token（校验 token_type == Access）
    ///
    /// 在 [`verify_token`](Self::verify_token) 基础上额外校验 `token_type`，
    /// 确保 refresh token 不能用作 access token（防止权限提升）。
    pub fn verify_access_token(&self, token: &str) -> AuthResult<JwtClaims> {
        let claims = self.verify_token(token)?;
        if claims.token_type != TokenType::Access {
            return Err(AuthError::InvalidToken);
        }
        Ok(claims)
    }

    /// 验证 Refresh Token（校验 token_type == Refresh）
    ///
    /// 在 [`verify_token`](Self::verify_token) 基础上额外校验 `token_type`，
    /// 确保 access token 不能用作 refresh token（防止 token 混用）。
    pub fn verify_refresh_token(&self, token: &str) -> AuthResult<JwtClaims> {
        let claims = self.verify_token(token)?;
        if claims.token_type != TokenType::Refresh {
            return Err(AuthError::InvalidToken);
        }
        Ok(claims)
    }

    /// 刷新访问令牌
    pub fn refresh_access_token(&self, refresh_token: &str) -> AuthResult<String> {
        let claims = self.verify_refresh_token(refresh_token)?;
        self.generate_token(&claims.sub, &claims.username, &claims.role, TokenType::Access)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET: &[u8] = b"test-secret-key-for-testing";

    #[test]
    fn test_generate_and_verify_token() {
        let manager = JwtManager::new(TEST_SECRET);

        let token = manager
            .generate_token("user123", "testuser", "admin", TokenType::Access)
            .unwrap();

        let claims = manager.verify_token(&token).unwrap();
        assert_eq!(claims.sub, "user123");
        assert_eq!(claims.username, "testuser");
        assert_eq!(claims.role, "admin");
    }

    #[test]
    fn test_token_types() {
        let manager = JwtManager::new(TEST_SECRET);

        let access_token = manager
            .generate_token("user123", "testuser", "admin", TokenType::Access)
            .unwrap();

        let refresh_token = manager
            .generate_token("user123", "testuser", "admin", TokenType::Refresh)
            .unwrap();

        let access_claims = manager.verify_token(&access_token).unwrap();
        let refresh_claims = manager.verify_token(&refresh_token).unwrap();

        assert_eq!(access_claims.token_type, TokenType::Access);
        assert_eq!(refresh_claims.token_type, TokenType::Refresh);
    }

    #[test]
    fn test_invalid_token() {
        let manager = JwtManager::new(TEST_SECRET);

        let result = manager.verify_token("invalid.token.here");
        assert!(matches!(result, Err(AuthError::InvalidToken)));
    }

    #[test]
    fn test_refresh_token() {
        let manager = JwtManager::new(TEST_SECRET);

        let refresh_token = manager
            .generate_token("user123", "testuser", "admin", TokenType::Refresh)
            .unwrap();

        let new_access_token = manager.refresh_access_token(&refresh_token).unwrap();

        let claims = manager.verify_token(&new_access_token).unwrap();
        assert_eq!(claims.sub, "user123");
        assert_eq!(claims.token_type, TokenType::Access);
    }

    #[test]
    fn test_custom_expiration() {
        let manager = JwtManager::with_expiration(TEST_SECRET, 60, 3600);

        let token = manager
            .generate_token("user123", "testuser", "admin", TokenType::Access)
            .unwrap();

        let claims = manager.verify_token(&token).unwrap();
        assert!(claims.exp > claims.iat); // 应该有有效期
    }

    // ============================================================================
    // verify_access_token / verify_refresh_token token_type 校验测试（diting security 修复）
    // ============================================================================

    #[test]
    fn test_verify_access_token_accepts_access() {
        let manager = JwtManager::new(TEST_SECRET);
        let access_token = manager
            .generate_token("user1", "alice", "admin", TokenType::Access)
            .unwrap();
        let claims = manager.verify_access_token(&access_token).unwrap();
        assert_eq!(claims.token_type, TokenType::Access);
    }

    #[test]
    fn test_verify_access_token_rejects_refresh() {
        let manager = JwtManager::new(TEST_SECRET);
        let refresh_token = manager
            .generate_token("user1", "alice", "admin", TokenType::Refresh)
            .unwrap();
        // refresh token 不应用作 access token
        let result = manager.verify_access_token(&refresh_token);
        assert!(
            matches!(result, Err(AuthError::InvalidToken)),
            "refresh token should be rejected by verify_access_token"
        );
    }

    #[test]
    fn test_verify_refresh_token_accepts_refresh() {
        let manager = JwtManager::new(TEST_SECRET);
        let refresh_token = manager
            .generate_token("user1", "alice", "admin", TokenType::Refresh)
            .unwrap();
        let claims = manager.verify_refresh_token(&refresh_token).unwrap();
        assert_eq!(claims.token_type, TokenType::Refresh);
    }

    #[test]
    fn test_verify_refresh_token_rejects_access() {
        let manager = JwtManager::new(TEST_SECRET);
        let access_token = manager
            .generate_token("user1", "alice", "admin", TokenType::Access)
            .unwrap();
        // access token 不应用作 refresh token
        let result = manager.verify_refresh_token(&access_token);
        assert!(
            matches!(result, Err(AuthError::InvalidToken)),
            "access token should be rejected by verify_refresh_token"
        );
    }
}
