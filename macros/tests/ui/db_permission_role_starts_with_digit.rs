// db_permission 宏角色名以数字开头的错误测试
use dbnexus::DbEntity;
use dbnexus::db_permission;

#[derive(DbEntity, Clone, Debug)]
#[table_name = "users"]
#[db_permission(roles = "123admin")]
struct User {
    #[primary_key]
    id: i64,
    name: String,
}

fn main() {}
