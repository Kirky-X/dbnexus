// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
// db_entity 宏 permissions 子参数测试
#![allow(unexpected_cfgs)]

use dbnexus::db_entity;
use sea_orm::entity::prelude::*;

#[db_entity(
    table_name = "users",
    primary_key = "id",
    permissions(roles = ["admin", "manager"], operations = ["SELECT", "INSERT", "UPDATE"])
)]
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
    assert_eq!(Model::ALLOWED_ROLES, &["admin", "manager"]);
    assert_eq!(Model::ALLOWED_OPERATIONS, &["SELECT", "INSERT", "UPDATE"]);
    println!("db_entity permissions test passed!");
}
