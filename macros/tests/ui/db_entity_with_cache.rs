// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
// db_entity 宏 cache 子参数测试
#![allow(unexpected_cfgs)]

use dbnexus::db_entity;
use sea_orm::entity::prelude::*;

#[db_entity(
    table_name = "articles",
    primary_key = "id",
    cache(ttl = 60, strategy = "lru", max_capacity = 5000)
)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "articles")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub title: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

fn main() {
    assert_eq!(Model::table_name(), "articles");
    assert_eq!(Model::CACHE_TTL, 60u64);
    assert_eq!(Model::CACHE_STRATEGY, "lru");
    assert_eq!(Model::CACHE_MAX_CAPACITY, 5000usize);
    assert_eq!(Model::CACHE_ENABLED, true);
    assert_eq!(Model::cache_key(42), "articles:42");
    println!("db_entity cache test passed!");
}
