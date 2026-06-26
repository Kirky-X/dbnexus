// db_entity 宏 audit 子参数测试
#![allow(unexpected_cfgs)]

use dbnexus::db_entity;
use sea_orm::entity::prelude::*;

#[db_entity(
    table_name = "products",
    primary_key = "id",
    audit(table_name = "product_audit_log", log_values = true)
)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "products")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

fn main() {
    assert_eq!(Model::table_name(), "products");
    assert_eq!(Model::AUDIT_TABLE_NAME, "product_audit_log");
    assert_eq!(Model::AUDIT_LOG_VALUES, true);
    assert_eq!(Model::AUDIT_ENABLED, true);
    println!("db_entity audit test passed!");
}
