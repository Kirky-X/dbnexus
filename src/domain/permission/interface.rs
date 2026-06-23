// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 权限模块接口定义

use super::error::PermissionError;
use super::types::{PermissionAction, RolePolicy};
use async_trait::async_trait;

/// 权限检查能力
#[async_trait]
pub trait PermissionChecker: Send + Sync {
    /// 检查权限
    async fn check(&self, role: &str, table: &str, action: PermissionAction) -> Result<bool, PermissionError>;
}

/// 策略管理能力
#[async_trait]
pub trait PolicyManager: Send + Sync {
    /// 获取角色策略
    async fn get_policy(&self, role: &str) -> Result<Option<RolePolicy>, PermissionError>;

    /// 刷新策略缓存
    async fn refresh(&self) -> Result<(), PermissionError>;
}

/// 生命周期管理
#[async_trait]
pub trait PermissionLifecycle: Send + Sync {
    /// 健康检查
    async fn health_check(&self) -> anyhow::Result<()>;

    /// 关闭
    async fn shutdown(&self);
}

/// 权限提供者组合 trait
pub trait PermissionProvider: PermissionChecker + PolicyManager + PermissionLifecycle {}
