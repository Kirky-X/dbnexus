// DbEntity 宏基本展开测试
use dbnexus::DbEntity;

#[derive(DbEntity, Clone, Debug)]
#[table_name = "users"]
struct User {
    #[primary_key]
    id: i64,
    name: String,
}

fn main() {
    // 验证生成的 table_name() 方法
    assert_eq!(User::table_name(), "users");

    // 验证生成的 primary_key_column() 方法
    assert_eq!(User::primary_key_column(), "id");

    println!("DbEntity basic test passed!");
}
