// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 认证使用示例
//!
//! 展示如何在应用层正确使用 AuthenticationManager 和 DbPool：
//! - 初始化 AuthenticationManager
//! - 用户登录并获取 JWT Token
//! - 验证 Token 并从 Token 中提取角色
//! - 使用角色获取数据库 Session
//! - 安全的错误处理
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example authenticated_usage --features authentication,sqlite
//! ```

use dbnexus::{
    DbConfig, DbPool,
    authentication::{
        AuthenticationManager, AuthCredentials, PasswordHasher, User,
    }
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔐 DBNexus 认证使用示例\n");

    // ============================================================
    // 步骤1：初始化认证管理器
    // ============================================================
    println!("📝 步骤1：初始化认证管理器");

    // 在实际应用中，JWT 密钥应该从环境变量读取
    let jwt_secret = b"your-super-secret-jwt-key-min-32-bytes";

    // 配置 Token 过期时间
    let auth_manager = AuthenticationManager::with_config(
        jwt_secret,
        3600,  // access_token: 1小时
        86400  // refresh_token: 24小时
    );

    println!("  ✓ 认证管理器初始化成功\n");

    // ============================================================
    // 步骤2：添加测试用户
    // ============================================================
    println!("📝 步骤2：添加测试用户");

    // 创建密码哈希器
    let password_hasher = PasswordHasher::new();

    // 创建用户（密码会自动哈希）
    let admin_user = User {
        id: "user_001".to_string(),
        username: "admin".to_string(),
        password_hash: password_hasher.hash("Admin@123")?,
        role: "admin".to_string(),
        email: Some("admin@example.com".to_string()),
        created_at: Some(chrono::Utc::now().to_rfc3339()),
    };

    let manager_user = User {
        id: "user_002".to_string(),
        username: "manager".to_string(),
        password_hash: password_hasher.hash("Manager@123")?,
        role: "manager".to_string(),
        email: Some("manager@example.com".to_string()),
        created_at: Some(chrono::Utc::now().to_rfc3339()),
    };

    auth_manager.add_user(admin_user).await?;
    auth_manager.add_user(manager_user).await?;

    println!("  ✓ 测试用户添加成功\n");

    // ============================================================
    // 步骤3：初始化数据库连接池
    // ============================================================
    println!("📝 步骤3：初始化数据库连接池");

    let config = DbConfig {
        url: "sqlite:file::memory:?cache=shared".to_string(),
        permissions_path: Some("examples/src/permissions.yaml".to_string()),
        admin_role: "admin".to_string(),
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await?;

    println!("  ✓ 数据库连接池创建成功\n");

    // ============================================================
    // 步骤4：用户登录并获取 Token
    // ============================================================
    println!("📝 步骤4：用户登录\n");

    // 场景1：管理员登录
    println!("  场景1：管理员登录");
    let admin_credentials = AuthCredentials {
        username: "admin".to_string(),
        password: "Admin@123".to_string(),
    };

    match auth_manager.authenticate(admin_credentials).await {
        Ok(token) => {
            println!("    ✓ 登录成功！");
            println!("    Token: {}\n", token);

            // 使用 Token 获取 Session（安全方式）
            match authenticate_and_get_session(&pool, &auth_manager, &token).await {
                Ok(_) => println!("    ✓ Session 创建成功（管理员权限）\n"),
                Err(e) => println!("    ✗ Session 创建失败: {}\n", e),
            }
        }
        Err(e) => println!("    ✗ 登录失败: {}\n", e),
    }

    // 场景2：经理登录
    println!("  场景2：经理登录");
    let manager_credentials = AuthCredentials {
        username: "manager".to_string(),
        password: "Manager@123".to_string(),
    };

    match auth_manager.authenticate(manager_credentials).await {
        Ok(token) => {
            println!("    ✓ 登录成功！");
            println!("    Token: {}\n", token);

            // 使用 Token 获取 Session
            match authenticate_and_get_session(&pool, &auth_manager, &token).await {
                Ok(_) => println!("    ✓ Session 创建成功（经理权限）\n"),
                Err(e) => println!("    ✗ Session 创建失败: {}\n", e),
            }
        }
        Err(e) => println!("    ✗ 登录失败: {}\n", e),
    }

    // 场景3：错误密码
    println!("  场景3：错误密码（安全测试）");
    let wrong_credentials = AuthCredentials {
        username: "admin".to_string(),
        password: "WrongPassword".to_string(),
    };

    match auth_manager.authenticate(wrong_credentials).await {
        Ok(_) => println!("    ✗ 安全警告：应该拒绝错误密码\n"),
        Err(_) => println!("    ✓ 正确拒绝了错误密码\n"),
    }

    // ============================================================
    // 步骤5：Token 刷新
    // ============================================================
    println!("📝 步骤5：Token 刷新");

    // 重新登录获取 access token
    let credentials = AuthCredentials {
        username: "admin".to_string(),
        password: "Admin@123".to_string(),
    };

    if let Ok(access_token) = auth_manager.authenticate(credentials).await {
        println!("  Access Token 获取成功");
        println!("  Token: {}...\n", &access_token[..access_token.len().min(50)]);

        // 验证 Token
        match auth_manager.verify_token(&access_token) {
            Ok(claims) => {
                println!("  ✓ Token 验证成功");
                println!("    用户名: {}", claims.username);
                println!("    角色: {}", claims.role);
                println!("    过期时间 (Unix): {}", claims.exp);
            }
            Err(e) => println!("  ✗ Token 验证失败: {}", e),
        }
    }

    println!("\n✅ 示例运行完成！");
    println!("\n💡 安全提示：");
    println!("  1. 永远不要直接使用用户输入作为角色参数");
    println!("  2. 始终先验证 Token，再从 Token 中提取角色");
    println!("  3. 在生产环境中，JWT 密钥应从环境变量读取");
    println!("  4. 使用 HTTPS 传输 Token");
    println!("  5. 定期轮换 JWT 密钥");

    Ok(())
}

// ============================================
// 辅助函数：认证并获取 Session
// ============================================

/// 安全的 Session 获取流程
///
/// 展示最佳实践：
/// 1. 先验证 Token
/// 2. 从 Token 中提取角色
/// 3. 使用角色获取 Session
async fn authenticate_and_get_session(
    pool: &DbPool,
    auth_manager: &AuthenticationManager,
    token: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // 步骤1：验证 Token
    let claims = auth_manager.verify_token(token)?;

    println!("    → Token 验证成功");
    println!("      用户: {}", claims.username);
    println!("      角色: {}", claims.role);
    println!("      过期时间 (Unix): {}", claims.exp);

    // 步骤2：从 Token 中提取角色
    let role = &claims.role;

    // 步骤3：使用角色获取 Session
    let _session = pool.get_session(role).await?;

    // Session 创建成功，可以使用它执行数据库操作
    // session.execute("SELECT * FROM users").await?;

    Ok(())
}
