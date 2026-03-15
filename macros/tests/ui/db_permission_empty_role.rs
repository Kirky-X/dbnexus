// db_permission 宏空角色名错误测试
use dbnexus::DbEntity;
use dbnexus::db_permission;

#[derive(DbEntity, Clone, Debug)]
#[table_name = "users"]
#[db_permission(roles = "")]
struct User {
    #[primary_key]
    id: i64,
    name: String,
}

fn main() {}
