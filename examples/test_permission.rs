use dbnexus::{DbConfig, DbPool};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = DbConfig {
        url: "sqlite:file::memory:?cache=shared".to_string(),
        permissions_path: Some("src/permissions.yaml".to_string()),
        admin_role: "admin".to_string(),
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await?;
    
    let session = pool.get_session("admin").await?;
    
    // 测试 users 表
    session.execute_raw_ddl("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)").await?;
    let _ = session.execute_raw("INSERT INTO users (id, name) VALUES (1, 'Alice')").await?;
    println!("users table permission OK");
    
    // 测试 products 表
    session.execute_raw_ddl("CREATE TABLE products (id INTEGER PRIMARY KEY, name TEXT)").await?;
    let _ = session.execute_raw("INSERT INTO products (id, name) VALUES (1, 'Product1')").await?;
    println!("products table permission OK");
    
    Ok(())
}
