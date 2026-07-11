// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
// db_entity 宏基本展开测试
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
    assert_eq!(Model::table_name(), "users");
    assert_eq!(Model::primary_key_column(), "id");
    println!("db_entity basic test passed!");
}
