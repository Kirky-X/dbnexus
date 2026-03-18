// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Permission 单元测试模块
//!
//! 提供权限系统各组件的单元测试，包括：
//! - RBAC 权限提供者测试
//! - 高级 RBAC (角色继承) 测试
//! - PolicyDecisionPoint 决策测试
//! - 权限冲突解决测试

#[cfg(feature = "permission")]
pub mod advanced_rbac_unit_tests;

#[cfg(feature = "permission")]
pub mod pdp_unit_tests;

#[cfg(feature = "permission")]
pub mod rate_limiter_unit_tests;

#[cfg(feature = "permission")]
pub mod rbac_unit_tests;
