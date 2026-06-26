// db_entity 宏缺少 primary_key 参数时的错误测试
use dbnexus::db_entity;
use sea_orm::entity::prelude::*;

#[db_entity(table_name = "users")]
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
