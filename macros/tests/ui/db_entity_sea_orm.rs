// DbEntity 宏基本展开测试
use dbnexus::DbEntity;

#[derive(DbEntity, Clone, Debug)]
#[table_name = "products"]
struct Product {
    #[primary_key]
    id: i64,
    name: String,
    price: f64,
}

fn main() {
    // 验证从属性中提取表名
    assert_eq!(Product::table_name(), "products");
    assert_eq!(Product::primary_key_column(), "id");

    println!("DbEntity sea_orm attrs test passed!");
}
