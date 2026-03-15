// DbEntity 宏生成的方法签名测试
use dbnexus::DbEntity;

#[derive(DbEntity, Clone, Debug)]
#[table_name = "orders"]
struct Order {
    #[primary_key]
    id: i64,
    customer_id: i64,
    total: f64,
}

fn main() {
    // 验证 table_name() 方法签名
    let table_name: &'static str = Order::table_name();
    assert_eq!(table_name, "orders");

    // 验证 primary_key_column() 方法签名
    let pk: &'static str = Order::primary_key_column();
    assert_eq!(pk, "id");

    // 验证方法可以在编译时调用
    const TABLE_NAME: &str = "orders";
    const PK: &str = "id";
    assert_eq!(Order::table_name(), TABLE_NAME);
    assert_eq!(Order::primary_key_column(), PK);

    println!("DbEntity method signatures test passed!");
}
