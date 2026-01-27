// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Cross Cutting 集成测试

pub mod concurrency;

#[cfg(feature = "migration")]
pub mod multi_db;
