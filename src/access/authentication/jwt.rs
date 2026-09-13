// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! JWT Token 管理模块

use super::models::{AuthError, AuthResult, JwtClaims, TokenType};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
// Duration 仅被 oxcache-integration 门控的 compute_remaining_ttl 消费
#[cfg(feature = "oxcache-integration")]
use std::time::Duration;

#[cfg(feature = "oxcache-integration")]
use std::sync::Arc;
#[cfg(feature = "oxcache-integration")]
use crate::domain::DbCacheProvider;

/// JWT 访问令牌默认过期时间（秒）
const ACCESS_TOKEN_EXPIRATION_SECS: u64 = 3600; // 1 hour

/// JWT 刷新令牌默认过期时间（秒）
const REFRESH_TOKEN_EXPIRATION_SECS: u64 = 3600 * 24 * 7; // 7 days

/// 默认有效角色列表
const DEFAULT_VALID_ROLES: &[&str] = &["admin", "user", "readonly", "readwrite"];

/// jti 全局计数器
static JTI_COUNTER: AtomicU64 = AtomicU64::new(0);

/// 撤销集合容量上限。
///
/// 超过此上限后，新插入会先淘汰过期条目，仍达上限则逐出 Instant 最旧条目。
/// 语义边界：极端场景下单实例内 >10,000 个未过期的已轮换 jti 同时存在时，
/// 最旧条目可能被逐出，理论上允许其重放。逐出优先过期项 + 上限足够大，风险可接受。
const MAX_REVOKED_JTIS: usize = 10_000;

/// JWT 管理器
pub struct JwtManager {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    access_expiration_secs: u64,
    refresh_expiration_secs: u64,
    /// 有效角色白名单，防止 `generate_token` 注入任意角色
    valid_roles: HashSet<String>,
    /// 已撤销的 refresh token jti 集合（refresh token rotation 保护）
    /// 值类型 `Instant` 记录插入时刻，用于过期淘汰。
    revoked_refresh_jtis: Mutex<HashMap<String, Instant>>,
    /// 可选的分布式撤销缓存（经 `oxcache-integration` feature 启用）。
    ///
    /// 注入后，撤销操作同时写入本地 HashMap + 远程缓存（key=jti，ttl=令牌剩余有效期），
    /// 验证时先查本地集合并命中后短路，未命中则查远程缓存。
    /// 未注入时行为不变（纯本地 HashMap）。
    #[cfg(feature = "oxcache-integration")]
    revocation_cache: Option<Arc<dyn DbCacheProvider + Send + Sync>>,
}

impl JwtManager {
    /// 创建新的 JWT 管理器
    ///
    /// # 参数
    ///
    /// * `secret` - JWT 签名密钥（建议从环境变量读取，至少 32 字节 / 256 bits）
    ///
    /// # 错误
    ///
    /// 密钥短于 32 字节时返回 `AuthError::TokenGeneration`。
    pub fn new(secret: &[u8]) -> AuthResult<Self> {
        // HS256 要求至少 256 bits（32 字节）密钥
        if secret.len() < 32 {
            return Err(AuthError::TokenGeneration(format!(
                "JWT secret must be at least 32 bytes (256 bits) for HS256, got {} bytes",
                secret.len()
            )));
        }
        Ok(Self {
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
            access_expiration_secs: ACCESS_TOKEN_EXPIRATION_SECS,
            refresh_expiration_secs: REFRESH_TOKEN_EXPIRATION_SECS,
            valid_roles: DEFAULT_VALID_ROLES.iter().map(|s| s.to_string()).collect(),
            revoked_refresh_jtis: Mutex::new(HashMap::new()),
            #[cfg(feature = "oxcache-integration")]
            revocation_cache: None,
        })
    }

    /// 使用自定义过期时间创建 JWT 管理器
    ///
    /// # 错误
    ///
    /// 密钥短于 32 字节时返回 `AuthError::TokenGeneration`。
    pub fn with_expiration(
        secret: &[u8],
        access_expiration_secs: u64,
        refresh_expiration_secs: u64,
    ) -> AuthResult<Self> {
        if secret.len() < 32 {
            return Err(AuthError::TokenGeneration(format!(
                "JWT secret must be at least 32 bytes (256 bits) for HS256, got {} bytes",
                secret.len()
            )));
        }
        Ok(Self {
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
            access_expiration_secs,
            refresh_expiration_secs,
            valid_roles: DEFAULT_VALID_ROLES.iter().map(|s| s.to_string()).collect(),
            revoked_refresh_jtis: Mutex::new(HashMap::new()),
            #[cfg(feature = "oxcache-integration")]
            revocation_cache: None,
        })
    }

    /// 添加自定义有效角色
    ///
    /// 扩展角色白名单，允许 `generate_token` 接受自定义角色。
    pub fn add_valid_role(&mut self, role: String) {
        self.valid_roles.insert(role);
    }

    /// 注入分布式撤销缓存。
    ///
    /// 注入后，撤销操作同时写入本地 HashMap + 远程缓存（key=jti，ttl=令牌剩余有效期），
    /// 验证时先查本地集合，未命中则查远程缓存。
    #[cfg(feature = "oxcache-integration")]
    pub fn with_revocation_cache(
        &mut self,
        cache: Arc<dyn DbCacheProvider + Send + Sync>,
    ) -> &mut Self {
        self.revocation_cache = Some(cache);
        self
    }

    /// 生成 JWT Token
    ///
    /// # 错误
    ///
    /// - `role` 不在有效角色白名单中时返回 `AuthError::TokenGeneration`
    pub fn generate_token(
        &self,
        user_id: &str,
        username: &str,
        role: &str,
        token_type: TokenType,
    ) -> AuthResult<String> {
        // H-1: 角色白名单验证，防止注入任意角色
        if !self.valid_roles.contains(role) {
            return Err(AuthError::TokenGeneration(format!(
                "Invalid role '{}'. Valid roles: {:?}",
                role,
                self.valid_roles.iter().collect::<Vec<_>>()
            )));
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| AuthError::TokenGeneration("System time error".to_string()))?
            .as_secs() as usize;

        let expiration = match token_type {
            TokenType::Access => now + self.access_expiration_secs as usize,
            TokenType::Refresh => now + self.refresh_expiration_secs as usize,
        };

        // M-1: 生成唯一 jti（全局计数器 + 时间戳）
        let jti_count = JTI_COUNTER.fetch_add(1, Ordering::SeqCst);
        let jti = format!("{}-{}-{}", user_id, now, jti_count);

        let claims = JwtClaims {
            sub: user_id.to_string(),
            username: username.to_string(),
            role: role.to_string(),
            exp: expiration,
            iat: now,
            token_type,
            jti,
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| AuthError::TokenGeneration(e.to_string()))
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
    /// 同时检查 token 是否已被撤销（refresh token rotation 保护）。
    pub async fn verify_refresh_token(&self, token: &str) -> AuthResult<JwtClaims> {
        let claims = self.verify_token(token)?;
        if claims.token_type != TokenType::Refresh {
            return Err(AuthError::InvalidToken);
        }
        // H-3: 检查 refresh token 是否已被撤销
        // 先查本地集合（短路），未命中再查远程缓存
        if let Ok(revoked) = self.revoked_refresh_jtis.lock()
            && revoked.contains_key(&claims.jti)
        {
            return Err(AuthError::InvalidToken);
        }
        // 查远程撤销缓存（异步）
        #[cfg(feature = "oxcache-integration")]
        if let Some(ref cache) = self.revocation_cache {
            if let Ok(Some(_)) = cache.get(&format!("revoked_jti:{}", claims.jti)).await {
                return Err(AuthError::InvalidToken);
            }
        }
        Ok(claims)
    }

    /// 刷新访问令牌（带 refresh token rotation）
    ///
    /// 刷新成功后自动撤销旧的 refresh token，防止重放攻击。
    pub async fn refresh_access_token(&self, refresh_token: &str) -> AuthResult<String> {
        let claims = self.verify_refresh_token(refresh_token).await?;

        // H-3: 撤销旧 refresh token（refresh token rotation）
        if let Ok(mut revoked) = self.revoked_refresh_jtis.lock() {
            Self::evict_revoked_entries(&mut revoked, self.refresh_expiration_secs);
            revoked.insert(claims.jti.clone(), Instant::now());
        }

        // 同步写入远程撤销缓存（TTL = 令牌剩余有效期）
        #[cfg(feature = "oxcache-integration")]
        if let Some(ref cache) = self.revocation_cache {
            let cache_key = format!("revoked_jti:{}", claims.jti);
            let remaining_ttl = self.compute_remaining_ttl(&claims);
            // 异步写入不阻塞刷新路径（fire-and-forget，失败时本地 HashMap 已保护）
            let cache_clone = cache.clone();
            tokio::spawn(async move {
                let _ = cache_clone
                    .set(&cache_key, vec![1], Some(remaining_ttl))
                    .await;
            });
        }

        self.generate_token(
            &claims.sub,
            &claims.username,
            &claims.role,
            TokenType::Access,
        )
    }

    /// 计算令牌剩余有效期。
    #[cfg(feature = "oxcache-integration")]
    fn compute_remaining_ttl(&self, claims: &JwtClaims) -> Duration {
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let exp_secs = claims.exp as u64;
        if exp_secs > now_secs {
            Duration::from_secs(exp_secs - now_secs)
        } else {
            Duration::ZERO
        }
    }

    /// 淘汰撤销集合中的过期条目，并在达上限时逐出最旧条目。
    ///
    /// 淘汰策略：
    /// 1. 移除 `inserted_at + token_duration < now` 的条目（超过 refresh token 有效期无保留价值）
    /// 2. 若仍达 `MAX_REVOKED_JTIS` 上限，移除 `Instant` 最旧的条目
    fn evict_revoked_entries(revoked: &mut HashMap<String, Instant>, refresh_expiration_secs: u64) {
        let now = Instant::now();
        let token_duration = std::time::Duration::from_secs(refresh_expiration_secs);
        revoked.retain(|_, inserted_at| now.duration_since(*inserted_at) < token_duration);

        // 淘汰后仍达上限，循环移除 Instant 最旧的条目直到低于上限
        while revoked.len() >= MAX_REVOKED_JTIS {
            let oldest_key = revoked
                .iter()
                .min_by_key(|(_, instant)| *instant)
                .map(|(key, _)| key.clone());
            match oldest_key {
                Some(key) => { revoked.remove(&key); }
                None => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET: &[u8] = b"test-secret-key-for-testing-32bx"; // 32 bytes minimum for HS256

    #[test]
    fn test_generate_and_verify_token() {
        let manager = JwtManager::new(TEST_SECRET).expect("valid secret");

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
        let manager = JwtManager::new(TEST_SECRET).expect("valid secret");

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
        let manager = JwtManager::new(TEST_SECRET).expect("valid secret");

        let result = manager.verify_token("invalid.token.here");
        assert!(matches!(result, Err(AuthError::InvalidToken)));
    }

    #[tokio::test]
    async fn test_refresh_token() {
        let manager = JwtManager::new(TEST_SECRET).expect("valid secret");

        let refresh_token = manager
            .generate_token("user123", "testuser", "admin", TokenType::Refresh)
            .unwrap();

        let new_access_token = manager.refresh_access_token(&refresh_token).await.unwrap();

        let claims = manager.verify_token(&new_access_token).unwrap();
        assert_eq!(claims.sub, "user123");
        assert_eq!(claims.token_type, TokenType::Access);
    }

    #[test]
    fn test_custom_expiration() {
        let manager = JwtManager::with_expiration(TEST_SECRET, 60, 3600).expect("valid secret");

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
        let manager = JwtManager::new(TEST_SECRET).expect("valid secret");
        let access_token = manager
            .generate_token("user1", "alice", "admin", TokenType::Access)
            .unwrap();
        let claims = manager.verify_access_token(&access_token).unwrap();
        assert_eq!(claims.token_type, TokenType::Access);
    }

    #[test]
    fn test_verify_access_token_rejects_refresh() {
        let manager = JwtManager::new(TEST_SECRET).expect("valid secret");
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

    #[tokio::test]
    async fn test_verify_refresh_token_accepts_refresh() {
        let manager = JwtManager::new(TEST_SECRET).expect("valid secret");
        let refresh_token = manager
            .generate_token("user1", "alice", "admin", TokenType::Refresh)
            .unwrap();
        let claims = manager.verify_refresh_token(&refresh_token).await.unwrap();
        assert_eq!(claims.token_type, TokenType::Refresh);
    }

    #[tokio::test]
    async fn test_verify_refresh_token_rejects_access() {
        let manager = JwtManager::new(TEST_SECRET).expect("valid secret");
        let access_token = manager
            .generate_token("user1", "alice", "admin", TokenType::Access)
            .unwrap();
        // access token 不应用作 refresh token
        let result = manager.verify_refresh_token(&access_token).await;
        assert!(
            matches!(result, Err(AuthError::InvalidToken)),
            "access token should be rejected by verify_refresh_token"
        );
    }

    // ============================================================================
    // HIGH-001: JwtManager 构造函数短密钥拒绝测试
    // ============================================================================

    /// 19 字节密钥（<32），`new` 应返回 Err 而非 panic
    #[test]
    fn test_new_rejects_short_secret() {
        let short_secret = b"dbnexus-demo-secret"; // 19 bytes
        assert_eq!(short_secret.len(), 19);
        match JwtManager::new(short_secret) {
            Err(AuthError::TokenGeneration(ref msg)) => {
                assert!(msg.contains("32"), "error should mention 32 bytes, got: {}", msg);
                assert!(msg.contains("19"), "error should mention 19 bytes, got: {}", msg);
            }
            other => panic!("expected Err(TokenGeneration), got Ok or wrong error variant: {}", 
                match other { Ok(_) => "Ok(...)".to_string(), Err(e) => format!("Err({})", e) }),
        }
    }

    /// 19 字节密钥（<32），`with_expiration` 应返回 Err 而非 panic
    #[test]
    fn test_with_expiration_rejects_short_secret() {
        let short_secret = b"dbnexus-demo-secret"; // 19 bytes
        assert_eq!(short_secret.len(), 19);
        match JwtManager::with_expiration(short_secret, 60, 3600) {
            Err(AuthError::TokenGeneration(ref msg)) => {
                assert!(msg.contains("32"), "error should mention 32 bytes, got: {}", msg);
                assert!(msg.contains("19"), "error should mention 19 bytes, got: {}", msg);
            }
            other => panic!("expected Err(TokenGeneration), got Ok or wrong error variant: {}", 
                match other { Ok(_) => "Ok(...)".to_string(), Err(e) => format!("Err({})", e) }),
        }
    }

    // ============================================================================
    // HIGH-002: 撤销集合有界性测试
    // ============================================================================

    /// 向撤销集合插入超过 MAX_REVOKED_JTIS 的条目后，集合长度应有上限，
    /// 且最早插入的条目应已被逐出。
    #[test]
    fn test_revoked_jtis_bounded() {
        let manager = JwtManager::new(TEST_SECRET).expect("valid secret");
        let mut revoked = manager.revoked_refresh_jtis.lock().unwrap();

        // 插入 MAX_REVOKED_JTIS + 100 条不同 jti
        let total = MAX_REVOKED_JTIS + 100;
        for i in 0..total {
            revoked.insert(format!("test-jti-{}", i), Instant::now());
        }

        // 调用淘汰逻辑
        JwtManager::evict_revoked_entries(&mut revoked, manager.refresh_expiration_secs);

        // 集合长度应不超过上限
        assert!(
            revoked.len() <= MAX_REVOKED_JTIS,
            "revoked set should be bounded to MAX_REVOKED_JTIS ({}), got {}",
            MAX_REVOKED_JTIS,
            revoked.len()
        );

        // 最早插入的条目应已被逐出
        assert!(
            !revoked.contains_key("test-jti-0"),
            "earliest inserted jti should have been evicted"
        );
    }

    // ============================================================================
    // HIGH-002: 轮换重放防护测试
    // ============================================================================

    /// refresh token 刷新成功后，旧 refresh token 应被撤销，
    /// 再次用旧 token 调 verify_refresh_token 必须返回 Err。
    #[tokio::test]
    async fn test_refresh_token_rotation_revokes_old_token() {
        let manager = JwtManager::new(TEST_SECRET).expect("valid secret");

        // 签发 refresh token
        let refresh_token = manager
            .generate_token("user1", "alice", "admin", TokenType::Refresh)
            .expect("generate refresh token should succeed");

        // 刷新成功（内部会撤销旧 refresh token）
        let _new_access = manager
            .refresh_access_token(&refresh_token)
            .await
            .expect("refresh should succeed");

        // 旧 refresh token 应已被撤销
        let result = manager.verify_refresh_token(&refresh_token).await;
        assert!(
            matches!(result, Err(AuthError::InvalidToken)),
            "old refresh token should be revoked after rotation, got {:?}",
            result.map(|_| "Ok(...)".to_string())
        );
    }
}
