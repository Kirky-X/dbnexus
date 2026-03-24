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
        AuthenticationManager, AuthCredentials, User, TokenType
    }
};

// ============================================
// 示例1：基本认证流程
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔐 DBNexus 认证使用示例\n");

    // ============================================================
    // 步骤1：初始化认证管理器
    // ============================================================
    println!("📝 步骤1：初始化认证管理器");

    // 在实际应用中，JWT 密钥应该从环境变量读取
    let jwt_secret = b"your-super-secret-jwt-key-min-32-bytes";
    let auth_manager = AuthenticationManager::new(jwt_secret);

    // 配置 Token 过期时间（可选）
    let auth_manager = AuthenticationManager::with_config(
        jwt_secret,
        3600,  // access_token: 1小时
        86400 // refresh_token: 24小时
    );

    println!("  ✓ 认证管理器初始化成功\n");

    // ============================================================
    // 步骤2：添加测试用户
    // ============================================================
    println!("📝 步骤2：添加测试用户");

    // 创建用户（密码会自动哈希）
    let admin_user = User {
        id: "user_001".to_string(),
        username: "admin".to_string(),
        password_hash: auth_manager.password_hasher().hash("Admin@123")?,
        role: "admin".to_string(),
        email: Some("admin@example.com".to_string()),
        created_at: Some chrono::Utc::now().to_rfc3339()),
    };

    let manager_user = User {
        id: "user_002".to_string(),
        username: "manager".to_string(),
        password_hash: auth_manager.password_hasher().hash("Manager@123")?,
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
        permissions_path: Some("examples/demo/permissions.yaml".to_string()),
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

    // 重新登录获取 access token 和 refresh token
    let credentials = AuthCredentials {
        username: "admin".to_string(),
        password: "Admin@123".to_string(),
    };

    if let Ok(access_token) = auth_manager.authenticate(credentials.clone()).await {
        // 生成 refresh token
        if let Ok(refresh_token) = auth_manager.generate_token(
            "admin",
            TokenType::Refresh
        ) {
            println!("  Access Token: {}", access_token);
            println!("  Refresh Token: {}", refresh_token);

            // 使用 refresh token 获取新的 access token
            match auth_manager.refresh_access_token(&refresh_token) {
                Ok(new_access_token) => {
                    println!("  ✓ Token 刷新成功");
                    println!("  新 Access Token: {}\n", new_access_token);
                }
                Err(e) => println!("  ✗ Token 刷新失败: {}\n", e),
            }
        }
    }

    println!("✅ 示例运行完成！");
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
    println!("      过期时间: {}", claims.expiration_time);

    // 步骤2：从 Token 中提取角色
    let role = &claims.role;

    // 步骤3：使用角色获取 Session
    let _session = pool.get_session(role).await?;

    // Session 创建成功，可以使用它执行数据库操作
    // session.execute("SELECT * FROM users").await?;

    Ok(())
}

// ============================================
// 反面示例：不安全的用法
// ============================================

/// ⚠️ 警告：不安全的用法示例
///
/// 这些代码展示了**不应该**做的事情：
#[allow(dead_code)]
async fn insecure_examples() {
    let pool: DbPool = unsafe { std::mem::zeroed() };

    // ❌ 不安全1：直接使用用户输入
    // async fn bad_handler_1(user_input_role: &str) {
    //     let session = pool.get_session(user_input_role).await.unwrap(); // 危险！
    // }

    // ❌ 不安全2：硬编码角色
    // async fn bad_handler_2() {
    //     let session = pool.get_session("admin").await.unwrap(); // 生产环境危险！
    // }

    // ❌ 不安全3：绕过认证
    // async fn bad_handler_3(user_claims_admin: bool) {
    //     let role = if user_claims_admin { "admin" } else { "guest" };
    //     let session = pool.get_session(role).await.unwrap(); // 用户可以伪装！
    // }
}

// ============================================
// 正确示例：安全的用法
// ============================================

/// ✅ 正确的安全用法示例
#[allow(dead_code)]
async fn secure_examples() {
    let pool: DbPool = unsafe { std::mem::zeroed() };
    let auth_manager: AuthenticationManager = unsafe { std::mem::zeroed() };

    // ✅ 安全1：从 Token 中提取角色
    async fn good_handler_1(
        pool: &DbPool,
        auth_manager: &AuthenticationManager,
        token: &str
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 先验证 Token
        let claims = auth_manager.verify_token(token)?;
        // 从 Token 中提取角色
        let session = pool.get_session(&claims.role).await?;
        // 使用 session
        drop(session);
        Ok(())
    }

    // ✅ 安全2：多层验证
    async fn good_handler_2(
        pool: &DbPool,
        auth_manager: &AuthenticationManager,
        token: &str
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 验证 Token
        let claims = auth_manager.verify_token(token)?;

        // 可选：额外的业务逻辑验证
        if claims.role != "admin" {
            return Err("权限不足".into());
        }

        // 获取 Session
        let _session = pool.get_session(&claims.role).await?;
        Ok(())
    }

    // 使用示例
    let _ = good_handler_1(&pool, &auth_manager, "dummy_token");
    let _ = good_handler_2(&pool, &auth_manager, "dummy_token");
}
