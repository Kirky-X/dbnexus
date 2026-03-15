// DbEntity 宏缺少 table_name 属性时的错误测试
use dbnexus::DbEntity;

#[derive(DbEntity, Clone, Debug)]
struct User {
    #[primary_key]
    id: i64,
    name: String,
}

fn main() {}
