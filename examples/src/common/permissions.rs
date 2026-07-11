// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 权限上下文辅助函数
//!
//! 为使用 `#[db_permission]` 宏的示例提供统一的 `PermissionContext` 构造方式。
//! 针对 `dbnexus::access::permission::PermissionContext`（基础 RBAC 上下文，
//! 配合 `new_default_with_rate_limit` 构造）。

use dbnexus::access::permission::PermissionContext;

/// 创建 admin 角色的权限上下文（带默认速率限制）
pub async fn setup_admin_context() -> Result<PermissionContext, Box<dyn std::error::Error>> {
    let ctx = PermissionContext::new_default_with_rate_limit("admin".to_string()).await?;
    Ok(ctx)
}

/// 创建 manager 角色的权限上下文（带默认速率限制）
pub async fn setup_manager_context() -> Result<PermissionContext, Box<dyn std::error::Error>> {
    let ctx = PermissionContext::new_default_with_rate_limit("manager".to_string()).await?;
    Ok(ctx)
}

/// 创建 guest 角色的权限上下文（带默认速率限制）
pub async fn setup_guest_context() -> Result<PermissionContext, Box<dyn std::error::Error>> {
    let ctx = PermissionContext::new_default_with_rate_limit("guest".to_string()).await?;
    Ok(ctx)
}

/// 创建指定角色的权限上下文（带默认速率限制）
pub async fn setup_role_context(role: &str) -> Result<PermissionContext, Box<dyn std::error::Error>> {
    let ctx = PermissionContext::new_default_with_rate_limit(role.to_string()).await?;
    Ok(ctx)
}
