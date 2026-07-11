// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Authentication 集成测试（T079-T084）
//!
//! 通过公共 API 测试 JWT 签发/验证、密码哈希、AuthenticationManager 认证流程。
//! 覆盖正常路径、过期、篡改、无效签名、错误密码、未知用户等场景。

use dbnexus::{AuthCredentials, AuthError, AuthenticationManager, JwtManager, PasswordHasher, TokenType, User};

// ============================================================================
// 辅助函数
// ============================================================================

const TEST_SECRET: &[u8] = b"test_secret_key_for_integration_tests_2026";
const ALT_SECRET: &[u8] = b"different_secret_key_for_signature_mismatch";

const TEST_PASSWORD: &str = "Password123";
const TEST_WRONG_PASSWORD: &str = "WrongPassword";
const TEST_SECURE_PASSWORD: &str = "MySecurePassword123";
const TEST_CORRECT_PASSWORD: &str = "CorrectPassword123";
const TEST_WRONG_PASSWORD_456: &str = "WrongPassword456";
const TEST_OLD_PASSWORD: &str = "OldPassword123";
const TEST_NEW_PASSWORD: &str = "NewPassword456";

fn make_user(username: &str, password: &str, role: &str) -> User {
    let hash = PasswordHasher::new().hash(password).expect("hash should succeed");
    User {
        id: format!("uid_{}", username),
        username: username.to_string(),
        password_hash: hash,
        role: role.to_string(),
        email: Some(format!("{}@example.com", username)),
        created_at: Some("2026-07-03T00:00:00Z".to_string()),
    }
}

async fn make_manager_with_user() -> AuthenticationManager {
    let mgr = AuthenticationManager::new(TEST_SECRET);
    mgr.add_user(make_user("alice", TEST_PASSWORD, "admin"))
        .await
        .expect("add_user should succeed");
    mgr
}

// ============================================================================
// T080: JWT 签发与验证集成测试
// ============================================================================

/// TEST-AUTH-JWT-001: JWT 签发与验证往返
#[tokio::test]
async fn test_jwt_issue_and_verify() {
    let mgr = JwtManager::new(TEST_SECRET);

    let token = mgr
        .generate_token("user_1", "alice", "admin", TokenType::Access)
        .expect("generate_token should succeed");
    assert!(!token.is_empty(), "token must be non-empty");

    let claims = mgr.verify_token(&token).expect("verify_token should succeed");
    assert_eq!(claims.sub, "user_1");
    assert_eq!(claims.username, "alice");
    assert_eq!(claims.role, "admin");
    assert_eq!(claims.token_type, TokenType::Access);
    assert!(claims.exp > claims.iat, "exp must be after iat");
}

/// TEST-AUTH-JWT-002: JWT 过期后验证应返回 TokenExpired
#[tokio::test]
async fn test_jwt_expired() {
    // 使用 1 秒过期时间创建 manager
    let mgr = JwtManager::with_expiration(TEST_SECRET, 1, 1);
    let token = mgr
        .generate_token("user_2", "bob", "user", TokenType::Access)
        .expect("generate_token should succeed");

    // 等待过期
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let result = mgr.verify_token(&token);
    assert!(result.is_err(), "expired token should fail verification");
    assert!(
        matches!(result, Err(AuthError::TokenExpired)),
        "expected TokenExpired, got {:?}",
        result
    );
}

/// TEST-AUTH-JWT-003: JWT 篡改后验证应返回 InvalidToken
#[tokio::test]
async fn test_jwt_tampered() {
    let mgr = JwtManager::new(TEST_SECRET);
    let token = mgr
        .generate_token("user_3", "carol", "user", TokenType::Access)
        .expect("generate_token should succeed");

    // 篡改 payload 部分（替换最后一个 . 后的 base64 段）
    let tampered = {
        let parts: Vec<&str> = token.split('.').collect();
        assert_eq!(parts.len(), 3, "JWT should have 3 parts");
        // 修改 payload 部分的最后一个字符
        let mut payload = parts[1].to_string();
        let last_char = payload.pop().unwrap_or('A');
        payload.push(if last_char == 'A' { 'B' } else { 'A' });
        format!("{}.{}.{}", parts[0], payload, parts[2])
    };

    let result = mgr.verify_token(&tampered);
    assert!(result.is_err(), "tampered token should fail");
    assert!(
        matches!(result, Err(AuthError::InvalidToken)),
        "expected InvalidToken, got {:?}",
        result
    );
}

/// TEST-AUTH-JWT-004: 不同密钥签发的 JWT 验证应失败（无效签名）
#[tokio::test]
async fn test_jwt_invalid_signature() {
    let signer = JwtManager::new(TEST_SECRET);
    let verifier = JwtManager::new(ALT_SECRET);

    let token = signer
        .generate_token("user_4", "dave", "user", TokenType::Access)
        .expect("generate_token should succeed");

    let result = verifier.verify_token(&token);
    assert!(result.is_err(), "token signed with different key should fail");
    assert!(
        matches!(result, Err(AuthError::InvalidToken)),
        "expected InvalidToken for signature mismatch, got {:?}",
        result
    );
}

// ============================================================================
// T081: 密码哈希集成测试
// ============================================================================

/// TEST-AUTH-PWD-001: 密码哈希与验证
#[test]
fn test_password_hash_and_verify() {
    let hasher = PasswordHasher::new();
    let password = TEST_SECURE_PASSWORD;

    let hash = hasher.hash(password).expect("hash should succeed");
    assert!(hash.starts_with("$2b$"), "bcrypt hash should start with $2b$");
    assert_ne!(hash, password, "hash must differ from plaintext");

    hasher
        .verify(password, &hash)
        .expect("verify with correct password should succeed");
}

/// TEST-AUTH-PWD-002: 错误密码验证应失败
#[test]
fn test_password_wrong_password() {
    let hasher = PasswordHasher::new();
    let hash = hasher.hash(TEST_CORRECT_PASSWORD).expect("hash should succeed");

    let result = hasher.verify(TEST_WRONG_PASSWORD_456, &hash);
    assert!(result.is_err(), "wrong password should fail");
    assert!(
        matches!(result, Err(AuthError::InvalidCredentials)),
        "expected InvalidCredentials, got {:?}",
        result
    );
}

/// TEST-AUTH-PWD-003: 空密码应无法通过强度校验
#[test]
fn test_password_empty() {
    let hasher = PasswordHasher::new();

    let result = hasher.validate_strength("");
    assert!(result.is_err(), "empty password should fail strength check");
}

/// TEST-AUTH-PWD-004: 密码更新后旧哈希失效
#[test]
fn test_password_update() {
    let hasher = PasswordHasher::new();
    let old_password = TEST_OLD_PASSWORD;
    let new_password = TEST_NEW_PASSWORD;

    let old_hash = hasher.hash(old_password).expect("hash old should succeed");
    let new_hash = hasher.hash(new_password).expect("hash new should succeed");

    // 新密码验证通过
    hasher
        .verify(new_password, &new_hash)
        .expect("new password should verify");

    // 旧密码对新哈希验证失败
    let result = hasher.verify(old_password, &new_hash);
    assert!(result.is_err(), "old password should not verify against new hash");

    // 新密码对旧哈希验证失败
    let result = hasher.verify(new_password, &old_hash);
    assert!(result.is_err(), "new password should not verify against old hash");
}

// ============================================================================
// T082: AuthenticationManager 集成测试
// ============================================================================

/// TEST-AUTH-MGR-001: 认证成功
#[tokio::test]
async fn test_authenticate_success() {
    let mgr = make_manager_with_user().await;

    let token = mgr
        .authenticate(AuthCredentials {
            username: "alice".to_string(),
            password: TEST_PASSWORD.to_string(),
        })
        .await
        .expect("authenticate should succeed");

    assert!(!token.is_empty(), "token must be non-empty");

    // 验证 token 的 claims
    let claims = mgr.verify_token(&token).expect("verify_token should succeed");
    assert_eq!(claims.username, "alice");
    assert_eq!(claims.role, "admin");
}

/// TEST-AUTH-MGR-002: 错误密码认证失败
#[tokio::test]
async fn test_authenticate_wrong_password() {
    let mgr = make_manager_with_user().await;

    let result = mgr
        .authenticate(AuthCredentials {
            username: "alice".to_string(),
            password: TEST_WRONG_PASSWORD.to_string(),
        })
        .await;

    assert!(result.is_err(), "wrong password should fail");
    assert!(
        matches!(result, Err(AuthError::InvalidCredentials)),
        "expected InvalidCredentials, got {:?}",
        result
    );
}

/// TEST-AUTH-MGR-003: 未知用户认证失败
#[tokio::test]
async fn test_authenticate_unknown_user() {
    let mgr = make_manager_with_user().await;

    let result = mgr
        .authenticate(AuthCredentials {
            username: "nonexistent_user".to_string(),
            password: TEST_PASSWORD.to_string(),
        })
        .await;

    assert!(result.is_err(), "unknown user should fail");
    // 安全设计：用户不存在也返回 InvalidCredentials（避免用户名枚举）
    assert!(
        matches!(result, Err(AuthError::InvalidCredentials)),
        "expected InvalidCredentials for unknown user, got {:?}",
        result
    );
}
