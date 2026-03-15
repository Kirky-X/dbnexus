// 简化测试 - 只测试 DbEntity 基本功能
use dbnexus::DbEntity;

#[derive(DbEntity, Clone, Debug)]
#[table_name = "test_table"]
struct TestEntity {
    #[primary_key]
    id: i64,
    name: String,
}

fn main() {
    assert_eq!(TestEntity::table_name(), "test_table");
    assert_eq!(TestEntity::primary_key_column(), "id");
    println!("Test passed!");
}
