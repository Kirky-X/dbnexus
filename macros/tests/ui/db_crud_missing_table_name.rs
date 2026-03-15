// db_crud 宏缺少 table_name 参数时的错误测试
use dbnexus::db_crud;

#[db_crud]
struct User {
    id: i64,
    name: String,
}

fn main() {}
