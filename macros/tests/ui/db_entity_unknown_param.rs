// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
// db_entity 宏未知参数错误测试
use dbnexus::db_entity;
use sea_orm::entity::prelude::*;

#[db_entity(table_name = "users", primary_key = "id", unknown_param = "value")]
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
