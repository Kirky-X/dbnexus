// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

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
    println!("🔐 DBNexus JWT 认证示例");
    println!("========================================\n");

    // ============================================
    // 1. 创建 JwtManager
    // ============================================
    // ⚠️ 生产环境请从环境变量或密钥管理服务读取 secret，不要硬编码
    let secret = b"dbnexus-demo-secret-key-please-change-in-production";
    let manager = JwtManager::new(secret);
    println!("✓ JwtManager 创建成功（使用默认过期时间：access=1h, refresh=7d）\n");

    // ============================================
    // 2. 生成 Access Token 与 Refresh Token
    // ============================================
    println!("--- 生成 Token ---");
    let user_id = "u_10086";
    let username = "alice";
    let role = "admin";

    let access_token = manager.generate_token(user_id, username, role, TokenType::Access)?;
    let refresh_token = manager.generate_token(user_id, username, role, TokenType::Refresh)?;
    println!(
        "  ✓ Access token  (前 40 字符): {}...",
        &access_token[..40.min(access_token.len())]
    );
    println!(
        "  ✓ Refresh token (前 40 字符): {}...",
        &refresh_token[..40.min(refresh_token.len())]
    );

    // ============================================
    // 3. 验证并解析 Token → JwtClaims
    // ============================================
    println!("\n--- 验证 Token ---");
    let access_claims: JwtClaims = manager.verify_token(&access_token)?;
    println!("  Access token claims:");
    println!("    sub        = {}", access_claims.sub);
    println!("    username   = {}", access_claims.username);
    println!("    role       = {}", access_claims.role);
    println!("    iat        = {} (签发时间戳)", access_claims.iat);
    println!("    exp        = {} (过期时间戳)", access_claims.exp);
    println!("    token_type = {:?}", access_claims.token_type);

    let refresh_claims = manager.verify_token(&refresh_token)?;
    println!("\n  Refresh token claims:");
    println!("    token_type = {:?}", refresh_claims.token_type);
    println!(
        "    有效期 = {} 秒 ({} → {})",
        refresh_claims.exp - refresh_claims.iat,
        refresh_claims.iat,
        refresh_claims.exp
    );

    // ============================================
    // 4. TokenType 区分
    // ============================================
    println!("\n--- TokenType 类型区分 ---");
    println!("  Access  token type = {:?}", access_claims.token_type);
    println!("  Refresh token type = {:?}", refresh_claims.token_type);
    assert_eq!(access_claims.token_type, TokenType::Access);
    assert_eq!(refresh_claims.token_type, TokenType::Refresh);

    // ============================================
    // 5. 使用 Refresh Token 刷新 Access Token
    // ============================================
    println!("\n--- 刷新 Access Token ---");
    let new_access_token = manager.refresh_access_token(&refresh_token)?;
    println!("  ✓ 使用 refresh token 生成新的 access token");
    println!(
        "    新 token (前 40 字符): {}...",
        &new_access_token[..40.min(new_access_token.len())]
    );

    let new_claims = manager.verify_token(&new_access_token)?;
    println!(
        "  ✓ 新 token 验证成功: sub={}, role={}, type={:?}",
        new_claims.sub, new_claims.role, new_claims.token_type
    );

    // ============================================
    // 6. 自定义过期时间
    // ============================================
    println!("\n--- 自定义过期时间 ---");
    let short_manager = JwtManager::with_expiration(secret, 60, 3600);
    let short_token = short_manager.generate_token(user_id, username, role, TokenType::Access)?;
    let short_claims = short_manager.verify_token(&short_token)?;
    println!("  ✓ access=60s, refresh=3600s");
    println!(
        "    有效期 = {} 秒 (exp - iat = {} - {} = {})",
        short_claims.exp - short_claims.iat,
        short_claims.exp,
        short_claims.iat,
        short_claims.exp - short_claims.iat
    );

    // ============================================
    // 7. 错误场景：无效 token 与过期 token
    // ============================================
    println!("\n--- 错误场景 ---");

    // 无效 token
    let invalid_result = manager.verify_token("invalid.token.here");
    match &invalid_result {
        Err(AuthError::InvalidToken) => {
            println!("  ✓ 无效 token 被正确拒绝: InvalidToken");
        }
        other => {
            println!("  ✗ 预期 InvalidToken，实际得到: {:?}", other);
        }
    }

    // 模拟过期 token：使用 1 秒过期时间生成后立即等待 2 秒
    // verify_token 使用 leeway = 0 严格过期检查，2s 后 token 必定失效
    println!("\n  测试过期 token (1s 过期，等待 2s)...");
    let expiring_manager = JwtManager::with_expiration(secret, 1, 1);
    let expiring_token = expiring_manager.generate_token(user_id, username, role, TokenType::Access)?;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let expired_result = expiring_manager.verify_token(&expiring_token);
    match &expired_result {
        Err(AuthError::TokenExpired) => {
            println!("  ✓ 过期 token 被正确识别: TokenExpired");
        }
        other => {
            println!("  ✗ 预期 TokenExpired，实际得到: {:?}", other);
        }
    }

    // 使用 access token 尝试刷新（应失败，因为不是 refresh token）
    let wrong_refresh = manager.refresh_access_token(&access_token);
    match &wrong_refresh {
        Err(AuthError::InvalidToken) => {
            println!("  ✓ 用 access token 刷新被正确拒绝: InvalidToken");
        }
        other => {
            println!("  ✗ 预期 InvalidToken，实际得到: {:?}", other);
        }
    }

    println!("\n========================================");
    println!("✨ JWT 认证示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - JwtManager::new(secret)                       - 创建 JWT 管理器");
    println!("  - JwtManager::with_expiration(secret, acc, ref) - 自定义过期时间");
    println!("  - manager.generate_token(uid, name, role, type) - 生成 token");
    println!("  - manager.verify_token(token) -> JwtClaims      - 验证并解析");
    println!("  - manager.refresh_access_token(refresh_token)   - 刷新 access token");
    println!("  - TokenType::Access / TokenType::Refresh        - 两种 token 类型");
    println!("  - AuthError::InvalidToken / TokenExpired         - 错误类型");

    Ok(())
}
