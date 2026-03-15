// db_permission 宏路径遍历攻击检测测试
use dbnexus::DbEntity;
use dbnexus::db_permission;

#[derive(DbEntity, Clone, Debug)]
#[table_name = "users"]
#[db_permission(config = "../../../etc/shadow")]
struct User {
    #[primary_key]
    id: i64,
    name: String,
}

fn main() {}
