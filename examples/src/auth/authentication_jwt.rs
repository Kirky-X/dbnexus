// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! JWT 认证示例
//!
//! 演示 `JwtManager` 的完整使用流程：
//! - 生成 access / refresh 两类 JWT token
//! - 验证和解析 token，展示 `JwtClaims`
//! - 展示 `TokenType` 区分
//! - 演示 token 过期处理与刷新流程
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example authentication_jwt --features "authentication"
//! ```

use dbnexus::{AuthError, JwtClaims, JwtManager, TokenType};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🔐 DBNexus JWT authentication example");
    println!("========================================\n");

    // ============================================
    // 创建 JwtManager
    // ============================================
    // ⚠️ 生产环境请从环境变量或密钥管理服务读取 secret，不要硬编码
    let secret = b"dbnexus-demo-secret-key-please-change-in-production";
    let manager = JwtManager::new(secret)?;
    println!("✓ JwtManager created (default expiration: access=1h, refresh=7d)\n");

    // ============================================
    // 生成 Access Token 与 Refresh Token
    // ============================================
    println!("--- Generate Token ---");
    let user_id = "u_10086";
    let username = "alice";
    let role = "admin";

    let access_token = manager.generate_token(user_id, username, role, TokenType::Access)?;
    let refresh_token = manager.generate_token(user_id, username, role, TokenType::Refresh)?;
    println!(
        "  ✓ Access token  (first 40 chars): {}...",
        &access_token[..40.min(access_token.len())]
    );
    println!(
        "  ✓ Refresh token (first 40 chars): {}...",
        &refresh_token[..40.min(refresh_token.len())]
    );

    // ============================================
    // 验证并解析 Token → JwtClaims
    // ============================================
    println!("\n--- Verify Token ---");
    let access_claims: JwtClaims = manager.verify_token(&access_token)?;
    println!("  Access token claims:");
    println!("    sub        = {}", access_claims.sub);
    println!("    username   = {}", access_claims.username);
    println!("    role       = {}", access_claims.role);
    println!(
        "    iat        = {} (issued-at timestamp)",
        access_claims.iat
    );
    println!(
        "    exp        = {} (expiration timestamp)",
        access_claims.exp
    );
    println!("    token_type = {:?}", access_claims.token_type);

    let refresh_claims = manager.verify_token(&refresh_token)?;
    println!("\n  Refresh token claims:");
    println!("    token_type = {:?}", refresh_claims.token_type);
    println!(
        "    validity = {} sec ({} → {})",
        refresh_claims.exp - refresh_claims.iat,
        refresh_claims.iat,
        refresh_claims.exp
    );

    // ============================================
    // TokenType 区分
    // ============================================
    println!("\n--- TokenType type distinction ---");
    println!("  Access  token type = {:?}", access_claims.token_type);
    println!("  Refresh token type = {:?}", refresh_claims.token_type);
    assert_eq!(access_claims.token_type, TokenType::Access);
    assert_eq!(refresh_claims.token_type, TokenType::Refresh);

    // ============================================
    // 使用 Refresh Token 刷新 Access Token
    // ============================================
    println!("\n--- Refresh Access Token ---");
    let new_access_token = manager.refresh_access_token(&refresh_token).await?;
    println!("  ✓ new access token generated with refresh token");
    println!(
        "    new token (first 40 chars): {}...",
        &new_access_token[..40.min(new_access_token.len())]
    );

    let new_claims = manager.verify_token(&new_access_token)?;
    println!(
        "  ✓ new token verified: sub={}, role={}, type={:?}",
        new_claims.sub, new_claims.role, new_claims.token_type
    );

    // ============================================
    // 自定义过期时间
    // ============================================
    println!("\n--- Custom expiration ---");
    let short_manager = JwtManager::with_expiration(secret, 60, 3600)?;
    let short_token = short_manager.generate_token(user_id, username, role, TokenType::Access)?;
    let short_claims = short_manager.verify_token(&short_token)?;
    println!("  ✓ access=60s, refresh=3600s");
    println!(
        "    validity = {} sec (exp - iat = {} - {} = {})",
        short_claims.exp - short_claims.iat,
        short_claims.exp,
        short_claims.iat,
        short_claims.exp - short_claims.iat
    );

    // ============================================
    // 错误场景：无效 token 与过期 token
    // ============================================
    println!("\n--- Error scenarios ---");

    // 无效 token
    let invalid_result = manager.verify_token("invalid.token.here");
    match &invalid_result {
        Err(AuthError::InvalidToken) => {
            println!("  ✓ invalid token correctly rejected: InvalidToken");
        }
        other => {
            println!("  ✗ expected InvalidToken, got: {:?}", other);
        }
    }

    // 模拟过期 token：使用 1 秒过期时间生成后立即等待 2 秒
    // verify_token 使用 leeway = 0 严格过期检查，2s 后 token 必定失效
    println!("\n  testing expired token (expires in 1s, waiting 2s)...");
    let expiring_manager = JwtManager::with_expiration(secret, 1, 1)?;
    let expiring_token =
        expiring_manager.generate_token(user_id, username, role, TokenType::Access)?;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let expired_result = expiring_manager.verify_token(&expiring_token);
    match &expired_result {
        Err(AuthError::TokenExpired) => {
            println!("  ✓ expired token correctly identified: TokenExpired");
        }
        other => {
            println!("  ✗ expected TokenExpired, got: {:?}", other);
        }
    }

    // 使用 access token 尝试刷新（应失败，因为不是 refresh token）
    let wrong_refresh = manager.refresh_access_token(&access_token).await;
    match &wrong_refresh {
        Err(AuthError::InvalidToken) => {
            println!("  ✓ refreshing with access token correctly rejected: InvalidToken");
        }
        other => {
            println!("  ✗ expected InvalidToken, got: {:?}", other);
        }
    }

    println!("\n========================================");
    println!("✨ JWT authentication example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - JwtManager::new(secret)                       - create JWT manager");
    println!("  - JwtManager::with_expiration(secret, acc, ref) - custom expiration");
    println!("  - manager.generate_token(uid, name, role, type) - generate token");
    println!("  - manager.verify_token(token) -> JwtClaims      - verify and decode");
    println!("  - manager.refresh_access_token(refresh_token)   - refresh access token");
    println!("  - TokenType::Access / TokenType::Refresh        - two token types");
    println!("  - AuthError::InvalidToken / TokenExpired         - error types");

    Ok(())
}
