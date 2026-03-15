// DbEntity 宏缺少 primary_key 属性时的错误测试
use dbnexus::DbEntity;

#[derive(DbEntity, Clone, Debug)]
#[table_name = "users"]
struct User {
    id: i64,
    name: String,
}

fn main() {}
