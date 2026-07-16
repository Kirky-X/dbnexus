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
    /// vuln-0002 修复：验证 `password_hash` 为非空且符合 bcrypt 格式。
    ///
    /// 适用于测试/迁移场景。生产用户注册应使用 [`register_user`](Self::register_user)，
    /// 后者会验证密码强度并自动哈希。
    ///
    /// # 错误
    ///
    /// - `password_hash` 为空时返回 `AuthError::PasswordHash`
    /// - `password_hash` 不符合 bcrypt 格式时返回 `AuthError::PasswordHash`
    pub async fn add_user(&self, user: User) -> AuthResult<()> {
        validate_bcrypt_hash(&user.password_hash)?;
        let mut users = self.users.write().await;
        users.insert(user.username.clone(), user);
        Ok(())
    }

    /// 添加或更新用户（跳过密码哈希验证）
    ///
    /// **HD-3 文档明确**：此方法可见性为 `pub(crate)`，**不对外暴露**。
    /// 仅限 crate 内部迁移/测试场景使用（如批量导入历史用户、测试 fixture 构造）。
    /// 外部代码必须使用 [`add_user`](Self::add_user)（验证 bcrypt 格式）或
    /// [`register_user`](Self::register_user)（验证密码强度 + 哈希 + 存储）。
    ///
    /// `pub(crate)` 可见性是 HD-3 修复的硬性约束：确保外部调用方无法绕过
    /// `add_user` 的 bcrypt 格式验证（vuln-0002），从源头杜绝无效哈希进入存储。
    /// 若未来需要对外暴露，必须先增加等价的安全验证，不得直接放宽可见性。
    ///
    /// **安全警告**：此方法不验证 `password_hash` 格式，仅限内部迁移/测试使用。
    ///
    /// # Safety（语义层面）
    ///
    /// 调用方必须确保 `user.password_hash` 是有效的 bcrypt 哈希，
    /// 否则后续 `authenticate()` 会因 `bcrypt::verify` 失败而拒绝该用户登录。
    #[doc(hidden)]
    pub(crate) async fn add_user_unchecked(&self, user: User) -> AuthResult<()> {
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

        // 3. 构造 User 并存储（使用 add_user_unchecked 因为密码已由 validate_strength 验证，
        //    且 password_hash 来自 self.password_hasher.hash() 一定是有效 bcrypt 格式）
        let user = User {
            id: username.to_string(),
            username: username.to_string(),
            password_hash,
            role: role.to_string(),
            email: None,
            created_at: None,
        };

        self.add_user_unchecked(user).await
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

/// 验证 bcrypt 哈希格式（vuln-0002 修复）
///
/// bcrypt 哈希格式：`$2<a|b|y>$<cost>$<22-char-salt><31-char-hash>`
/// 总长度固定为 60 字符。
///
/// # 参数
///
/// * `hash` - 待验证的密码哈希字符串
///
/// # 错误
///
/// - 哈希为空时返回 `AuthError::PasswordHash`
/// - 哈希不以 `$2a$`/`$2b$`/`$2y$` 开头时返回 `AuthError::PasswordHash`
/// - 哈希长度不为 60 时返回 `AuthError::PasswordHash`
/// - cost 字段非数字时返回 `AuthError::PasswordHash`
fn validate_bcrypt_hash(hash: &str) -> AuthResult<()> {
    if hash.is_empty() {
        return Err(AuthError::PasswordHash("password_hash must not be empty".to_string()));
    }

    // bcrypt 哈希固定长度为 60
    if hash.len() != 60 {
        return Err(AuthError::PasswordHash(format!(
            "password_hash must be a valid bcrypt hash (60 chars, got {})",
            hash.len()
        )));
    }

    // 必须以 $2a$、$2b$ 或 $2y$ 开头
    let prefix = &hash[..4];
    if !matches!(prefix, "$2a$" | "$2b$" | "$2y$") {
        return Err(AuthError::PasswordHash(
            "password_hash must start with $2a$, $2b$, or $2y$".to_string(),
        ));
    }

    // cost 字段（第 4-5 位）必须为两位数字
    let cost_str = &hash[4..6];
    if !cost_str.chars().all(|c| c.is_ascii_digit()) {
        return Err(AuthError::PasswordHash(
            "password_hash cost factor must be numeric".to_string(),
        ));
    }

    // 第 6 位必须是 $
    if !hash[6..7].starts_with('$') {
        return Err(AuthError::PasswordHash(
            "password_hash format invalid: missing $ after cost".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_bcrypt_hash_valid() {
        // 真实 bcrypt 哈希
        let hasher = PasswordHasher::new();
        let hash = hasher.hash("TestPass@123").unwrap();
        assert!(validate_bcrypt_hash(&hash).is_ok());
    }

    #[test]
    fn test_validate_bcrypt_hash_empty() {
        assert!(validate_bcrypt_hash("").is_err());
    }

    #[test]
    fn test_validate_bcrypt_hash_plaintext() {
        assert!(validate_bcrypt_hash("plaintext").is_err());
        assert!(validate_bcrypt_hash("password123").is_err());
    }

    #[test]
    fn test_validate_bcrypt_hash_wrong_prefix() {
        // 错误的前缀
        let fake_hash = "$3a$12$01234567890123456789012345678901234567890123456789";
        assert!(validate_bcrypt_hash(fake_hash).is_err());
    }

    #[test]
    fn test_validate_bcrypt_hash_wrong_length() {
        // 正确前缀但长度不对
        let short_hash = "$2b$12$short";
        assert!(validate_bcrypt_hash(short_hash).is_err());
    }

    #[test]
    fn test_validate_bcrypt_hash_non_numeric_cost() {
        // cost 字段非数字
        let fake_hash = "$2b$ab$0123456789012345678901234567890123456789012345678";
        assert!(validate_bcrypt_hash(fake_hash).is_err());
    }

    /// vuln-0002 回归测试：add_user 必须拒绝空 password_hash
    #[tokio::test]
    async fn test_vuln_0002_add_user_rejects_empty_hash() {
        let mgr = AuthenticationManager::new(b"secret");
        let user = User {
            id: "u1".to_string(),
            username: "test".to_string(),
            password_hash: "".to_string(),
            role: "user".to_string(),
            email: None,
            created_at: None,
        };
        let result = mgr.add_user(user).await;
        assert!(
            matches!(result, Err(AuthError::PasswordHash(_))),
            "add_user must reject empty password_hash"
        );
    }

    /// vuln-0002 回归测试：add_user 必须拒绝非 bcrypt 格式的 password_hash
    #[tokio::test]
    async fn test_vuln_0002_add_user_rejects_plaintext_hash() {
        let mgr = AuthenticationManager::new(b"secret");
        let user = User {
            id: "u1".to_string(),
            username: "test".to_string(),
            password_hash: "plaintext_password".to_string(),
            role: "user".to_string(),
            email: None,
            created_at: None,
        };
        let result = mgr.add_user(user).await;
        assert!(
            matches!(result, Err(AuthError::PasswordHash(_))),
            "add_user must reject non-bcrypt password_hash"
        );
    }

    /// vuln-0002 回归测试：add_user 接受有效 bcrypt 哈希
    #[tokio::test]
    async fn test_vuln_0002_add_user_accepts_valid_bcrypt() {
        let mgr = AuthenticationManager::new(b"secret");
        let hasher = PasswordHasher::new();
        let hash = hasher.hash("ValidPass@123").unwrap();
        let user = User {
            id: "u1".to_string(),
            username: "test".to_string(),
            password_hash: hash,
            role: "user".to_string(),
            email: None,
            created_at: None,
        };
        let result = mgr.add_user(user).await;
        assert!(result.is_ok(), "add_user should accept valid bcrypt hash");
    }

    /// vuln-0002 回归测试：add_user_unchecked 不验证 password_hash
    #[tokio::test]
    async fn test_vuln_0002_add_user_unchecked_skips_validation() {
        let mgr = AuthenticationManager::new(b"secret");
        let user = User {
            id: "u1".to_string(),
            username: "test".to_string(),
            password_hash: "anything".to_string(),
            role: "user".to_string(),
            email: None,
            created_at: None,
        };
        let result = mgr.add_user_unchecked(user).await;
        assert!(
            result.is_ok(),
            "add_user_unchecked should skip validation for internal use"
        );
    }

    // ===== HD-3 测试：add_user vs add_user_unchecked 职责清晰分离 =====

    /// HD-3 测试：add_user 必须验证 bcrypt 格式（公共 API 职责）
    ///
    /// 同一无效 password_hash，add_user 必须拒绝，add_user_unchecked 必须接受。
    /// 验证两者职责清晰分离：公共 API 强制安全，内部 API 跳过验证。
    #[tokio::test]
    async fn test_hd3_add_user_vs_unchecked_role_separation() {
        let mgr = AuthenticationManager::new(b"secret");

        // 构造无效 password_hash 的 User
        let make_user = || User {
            id: "u1".to_string(),
            username: "test_hd3".to_string(),
            password_hash: "invalid-not-bcrypt".to_string(),
            role: "user".to_string(),
            email: None,
            created_at: None,
        };

        // add_user（公共 API）必须拒绝无效 bcrypt 哈希
        let result = mgr.add_user(make_user()).await;
        assert!(
            matches!(result, Err(AuthError::PasswordHash(_))),
            "HD-3: add_user (public API) must reject invalid bcrypt hash"
        );

        // add_user_unchecked（pub(crate) 内部 API）跳过验证，接受任意 hash
        let result = mgr.add_user_unchecked(make_user()).await;
        assert!(
            result.is_ok(),
            "HD-3: add_user_unchecked (pub(crate) internal) should skip validation"
        );
    }

    /// HD-3 测试：add_user_unchecked 接受的 user 可被 get_user 读回
    ///
    /// 验证 pub(crate) 内部 API 写入的数据可被公共读取 API 读回，
    /// 确保迁移场景（如批量导入）的数据完整性。
    #[tokio::test]
    async fn test_hd3_add_user_unchecked_data_round_trip() {
        let mgr = AuthenticationManager::new(b"secret");

        // 内部 API 写入（模拟迁移场景）
        let user = User {
            id: "migrated-1".to_string(),
            username: "migrated_user".to_string(),
            password_hash: "$2b$12$somefakebutnonemptyhashplaceholder012345678901234567890123456".to_string(),
            role: "user".to_string(),
            email: Some("migrated@example.com".to_string()),
            created_at: None,
        };
        mgr.add_user_unchecked(user)
            .await
            .expect("internal write should succeed");

        // 公共 API 读回
        let read = mgr
            .get_user("migrated_user")
            .await
            .expect("get_user should find migrated user");
        assert_eq!(read.id, "migrated-1");
        assert_eq!(read.role, "user");
        assert_eq!(read.email.as_deref(), Some("migrated@example.com"));
    }
}
