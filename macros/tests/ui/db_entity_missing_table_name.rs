// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
// db_entity 宏缺少 table_name 参数时的错误测试
use dbnexus::db_entity;
use sea_orm::entity::prelude::*;

#[db_entity(primary_key = "id")]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

fn main() {}
