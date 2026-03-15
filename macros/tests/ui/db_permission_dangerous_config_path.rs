// db_permission 宏危险配置路径错误测试
use dbnexus::DbEntity;
use dbnexus::db_permission;

#[derive(DbEntity, Clone, Debug)]
#[table_name = "users"]
#[db_permission(config = "/etc/passwd")]
struct User {
    #[primary_key]
    id: i64,
    name: String,
}

fn main() {}
