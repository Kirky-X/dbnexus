// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
// db_entity 宏 find_by_ids 方法生成测试
#![allow(unexpected_cfgs)]

use dbnexus::db_entity;
use sea_orm::entity::prelude::*;

#[db_entity(table_name = "users", primary_key = "id")]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

fn main() {
    // 验证 find_by_ids 方法存在（async fn 编译期检查）
    // 0.4.2: find_by_ids 泛型化后需指定类型参数才能取函数指针
    let _ = Model::find_by_ids::<i64>;
    assert_eq!(Model::table_name(), "users");
    println!("db_entity find_by_ids test passed!");
}
