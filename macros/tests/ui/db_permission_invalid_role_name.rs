// db_permission 宏无效角色名错误测试
use dbnexus::DbEntity;
use dbnexus::db_permission;

#[derive(DbEntity, Clone, Debug)]
#[table_name = "users"]
#[db_permission(roles = "admin,invalid-role!")]
struct User {
    #[primary_key]
    id: i64,
    name: String,
}

fn main() {}
