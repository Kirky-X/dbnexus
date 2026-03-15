// DbEntity 宏与泛型结构体测试
use dbnexus::DbEntity;

#[derive(DbEntity, Clone, Debug)]
#[table_name = "items"]
struct Item<T> {
    #[primary_key]
    id: i64,
    value: T,
}

fn main() {
    // 验证泛型结构体的宏展开
    assert_eq!(Item::<String>::table_name(), "items");
    assert_eq!(Item::<String>::primary_key_column(), "id");

    println!("DbEntity generic test passed!");
}
