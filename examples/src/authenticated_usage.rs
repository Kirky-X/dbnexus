// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 认证使用示例
//!
//! 此示例需要启用 `authentication` feature。

#[cfg(feature = "authentication")]
mod inner {
    use dbnexus::{
        DbConfig, DbPool,
        access::authentication::{
            AuthenticationManager, AuthCredentials, PasswordHasher, User,
        }
    };

    #[tokio::main]
    pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
        println!("🔐 DBNexus 认证使用示例\n");

        // 初始化数据库连接池
        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            ..Default::default()
        };
        let pool = DbPool::with_config(config).await?;
        println!("✓ 数据库连接池已初始化");

        // 初始化认证管理器
        let auth_manager = AuthenticationManager::new();
        println!("✓ 认证管理器已初始化");

        // 创建测试用户
        let admin_user = User::new("admin", "admin@example.com", "admin_role");
        let manager_user = User::new("manager", "manager@example.com", "manager_role");
        auth_manager.add_user(admin_user.clone()).await?;
        auth_manager.add_user(manager_user.clone()).await?;
        println!("✓ 测试用户已创建: admin, manager");

        // 测试管理员登录
        println!("\n--- 测试管理员登录 ---");
        let admin_credentials = AuthCredentials::new("admin", "password123");
        match auth_manager.authenticate(admin_credentials).await {
            Ok(token) => {
                println!("    ✓ 管理员登录成功");
                println!("    Token: {}", token);
            }
            Err(e) => println!("    ✗ 管理员登录失败: {}", e),
        }

        // 测试无效凭据
        println!("\n--- 测试无效凭据 ---");
        let invalid_credentials = AuthCredentials::new("admin", "wrongpassword");
        match auth_manager.authenticate(invalid_credentials).await {
            Ok(token) => println!("    ✓ 登录成功 (不应该发生): {}", token),
            Err(e) => println!("    ✗ 登录失败 (预期): {}", e),
        }

        // 测试用户登录并获取 token
        println!("\n--- 测试用户登录 ---");
        let manager_credentials = AuthCredentials::new("manager", "password123");
        match auth_manager.authenticate(manager_credentials).await {
            Ok(token) => {
                println!("    ✓ 用户登录成功");
                println!("    Token: {}", &token[..token.len().min(50)]);

                // 验证 token 并获取用户信息
                match auth_manager.verify_token(&token).await {
                    Ok(claims) => {
                        println!("    ✓ Token 验证成功");
                        println!("    用户: {}, 角色: {}", claims.sub, claims.role);
                    }
                    Err(e) => println!("    ✗ Token 验证失败: {}", e),
                }
            }
            Err(e) => println!("    ✗ 用户登录失败: {}", e),
        }

        // 获取数据库 session（使用角色）
        println!("\n--- 使用角色获取数据库 Session ---");
        let session = pool.get_session("admin_role").await?;
        println!("    ✓ 已通过角色 'admin_role' 获取数据库 Session");
        drop(session);

        let session = pool.get_session("manager_role").await?;
        println!("    ✓ 已通过角色 'manager_role' 获取数据库 Session");

        println!("\n=== 认证示例完成 ===\n");
        Ok(())
    }
}

#[cfg(not(feature = "authentication"))]
fn main() {
    println!("此示例需要启用 'authentication' feature。");
    println!("请使用以下命令运行:");
    println!("  cargo run --example authenticated_usage --features authentication,sqlite");
}

#[cfg(feature = "authentication")]
fn main() {
    inner::main();
}
