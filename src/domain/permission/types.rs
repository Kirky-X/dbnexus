// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 权限模块类型定义

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 权限操作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PermissionAction {
    /// 查询
    Select,
    /// 插入
    Insert,
    /// 更新
    Update,
    /// 删除
    Delete,
}

impl std::fmt::Display for PermissionAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PermissionAction::Select => write!(f, "SELECT"),
            PermissionAction::Insert => write!(f, "INSERT"),
            PermissionAction::Update => write!(f, "UPDATE"),
            PermissionAction::Delete => write!(f, "DELETE"),
        }
    }
}

/// 表权限定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TablePermission {
    /// 表名（支持通配符 *）
    pub name: String,
    /// 允许的操作
    pub operations: Vec<PermissionAction>,
}

impl TablePermission {
    /// 检查是否允许指定操作
    pub fn allows(&self, action: &PermissionAction) -> bool {
        self.operations.contains(action)
    }
}

/// 角色策略
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RolePolicy {
    /// 表权限列表
    pub tables: Vec<TablePermission>,
}

impl RolePolicy {
    /// 检查是否允许对指定表执行指定操作
    pub fn allows(&self, table: &str, action: &PermissionAction) -> bool {
        for tp in &self.tables {
            if (tp.name == "*" || tp.name == table) && tp.allows(action) {
                return true;
            }
        }
        false
    }
}

/// 权限策略集合
#[derive(Debug, Clone, Default)]
pub struct PolicySet {
    /// 角色策略映射
    pub roles: HashMap<String, RolePolicy>,
}
