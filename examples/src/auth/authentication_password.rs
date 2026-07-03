// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 密码哈希与用户认证示例
//!
//! 演示 `PasswordHasher` 与 `AuthenticationManager` 的完整使用流程：
//! - 使用 bcrypt 哈希密码并验证
//! - 展示 `AuthCredentials` 凭据结构
//! - 通过 `AuthenticationManager` 进行用户管理（增删查）
//! - 完整的认证流程：添加用户 → 凭据认证 → 获取 token → 验证 token
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example authentication_password --features "authentication"
//! ```

use dbnexus::{AuthCredentials, AuthError, AuthenticationManager, JwtManager, PasswordHasher, TokenType, User};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🔑 DBNexus 密码哈希与用户认证示例");
    println!("========================================\n");

    // ============================================
    // 1. PasswordHasher — 哈希与验证
    // ============================================
    println!("--- 1. PasswordHasher 哈希与验证 ---");
    let hasher = PasswordHasher::new();

    let password = "MySecurePass123";
    let password_hash = hasher.hash(password)?;
    println!("  原始密码: {}", password);
    println!("  哈希结果: {}", password_hash);
    assert!(password_hash.starts_with("$2b$"), "bcrypt 哈希应以 $2b$ 开头");

    // 验证正确密码
    let verify_ok = hasher.verify(password, &password_hash);
    println!(
        "\n  验证正确密码: {:?}",
        verify_ok.as_ref().map(|_| "✓ 通过").map_err(|e| format!("✗ {}", e))
    );
    assert!(verify_ok.is_ok());

    // 验证错误密码
    let verify_fail = hasher.verify("WrongPassword", &password_hash);
    println!(
        "  验证错误密码: {:?}",
        verify_fail.as_ref().map(|_| "✓ 通过").map_err(|e| format!("✗ {}", e))
    );
    assert!(matches!(verify_fail, Err(AuthError::InvalidCredentials)));

    // ============================================
    // 2. 密码强度校验
    // ============================================
    println!("\n--- 2. 密码强度校验 ---");
    let test_cases = [
        ("Short1", "太短（<8 字符）"),
        ("OnlyLetters", "缺少数字"),
        ("12345678", "缺少字母"),
        ("ValidPass123", "有效密码"),
    ];
    for (pwd, desc) in &test_cases {
        let result = hasher.validate_strength(pwd);
        let status = if result.is_ok() { "✓ 通过" } else { "✗ 拒绝" };
        println!("  {:>15} [{}] - {}", pwd, status, desc);
    }

    // ============================================
    // 3. AuthenticationManager — 用户管理
    // ============================================
    println!("\n--- 3. AuthenticationManager 用户管理 ---");
    let manager = AuthenticationManager::new(b"dbnexus-demo-secret");

    // 添加用户
    println!("\n  [添加用户]");
    let users_to_add = [
        ("u_001", "alice", "admin", "alice@example.com"),
        ("u_002", "bob", "user", "bob@example.com"),
        ("u_003", "carol", "manager", "carol@example.com"),
    ];
    for (uid, name, role, email) in &users_to_add {
        let hash = hasher.hash(&format!("{}_Pass123", name))?;
        let user = User {
            id: uid.to_string(),
            username: name.to_string(),
            password_hash: hash,
            role: role.to_string(),
            email: Some(email.to_string()),
            created_at: Some("2026-06-25T00:00:00Z".to_string()),
        };
        manager.add_user(user).await?;
        println!("  ✓ 添加用户: id={}, username={}, role={}", uid, name, role);
    }

    // 查询用户
    println!("\n  [查询用户]");
    let fetched = manager.get_user("alice").await?;
    println!(
        "  ✓ 查询 alice: id={}, role={}, email={:?}",
        fetched.id, fetched.role, fetched.email
    );

    // ============================================
    // 4. AuthCredentials — 凭据认证流程
    // ============================================
    println!("\n--- 4. AuthCredentials 凭据认证 ---");

    // 正确凭据
    let credentials = AuthCredentials {
        username: "alice".to_string(),
        password: "alice_Pass123".to_string(),
    };
    println!(
        "  凭据: username={}, password={}",
        credentials.username, credentials.password
    );

    let token = manager.authenticate(credentials).await?;
    println!(
        "  ✓ 认证成功，获得 access token (前 40 字符): {}...",
        &token[..40.min(token.len())]
    );

    // 验证 token
    let claims = manager.verify_token(&token)?;
    println!(
        "  ✓ Token 验证: sub={}, username={}, role={}",
        claims.sub, claims.username, claims.role
    );

    // 错误凭据
    println!("\n  [错误凭据测试]");
    let wrong_password = AuthCredentials {
        username: "alice".to_string(),
        password: "WrongPassword999".to_string(),
    };
    let wrong_result = manager.authenticate(wrong_password).await;
    println!(
        "  错误密码: {:?}",
        wrong_result.map(|_| "✓ 通过").map_err(|e| format!("✗ {}", e))
    );

    let nonexistent = AuthCredentials {
        username: "nonexistent".to_string(),
        password: "AnyPassword123".to_string(),
    };
    let nonexistent_result = manager.authenticate(nonexistent).await;
    println!(
        "  不存在用户: {:?}",
        nonexistent_result.map(|_| "✓ 通过").map_err(|e| format!("✗ {}", e))
    );

    // ============================================
    // 5. Token 刷新流程
    // ============================================
    println!("\n--- 5. Token 刷新流程 ---");
    // AuthenticationManager 内部的 jwt_manager 是私有的，
    // 这里使用相同 secret 创建独立 JwtManager 来签发 refresh token（生产中通常由登录接口返回）
    let jwt_for_refresh = JwtManager::new(b"dbnexus-demo-secret");
    let refresh_token = jwt_for_refresh.generate_token("u_001", "alice", "admin", TokenType::Refresh)?;
    let new_access = manager.refresh_token(&refresh_token)?;
    println!("  ✓ 用 refresh token 换取新 access token");
    let new_claims = manager.verify_token(&new_access)?;
    println!("  ✓ 新 token: sub={}, type={:?}", new_claims.sub, new_claims.token_type);

    // ============================================
    // 6. 用户删除
    // ============================================
    println!("\n--- 6. 用户删除 ---");
    manager.remove_user("carol").await?;
    println!("  ✓ 删除用户 carol");

    let removed_result = manager.get_user("carol").await;
    println!(
        "  再次查询 carol: {:?}",
        removed_result
            .map(|u| format!("✓ 仍存在: {}", u.username))
            .map_err(|e| format!("✗ 已删除: {}", e))
    );

    println!("\n========================================");
    println!("✨ 密码哈希与用户认证示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - PasswordHasher::new()                        - 创建密码哈希器");
    println!("  - hasher.hash(password) -> String              - bcrypt 哈希");
    println!("  - hasher.verify(password, hash)                - 验证密码");
    println!("  - hasher.validate_strength(password)           - 密码强度校验");
    println!("  - AuthenticationManager::new(secret)           - 创建认证管理器");
    println!("  - manager.add_user(user)                       - 添加用户");
    println!("  - manager.get_user(username) -> User           - 查询用户");
    println!("  - manager.authenticate(AuthCredentials) -> token - 凭据认证");
    println!("  - manager.verify_token(token) -> JwtClaims     - 验证 token");
    println!("  - manager.refresh_token(refresh_token)         - 刷新 token");
    println!("  - manager.remove_user(username)                - 删除用户");

    Ok(())
}
