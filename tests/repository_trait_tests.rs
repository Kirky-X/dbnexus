// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 泛型仓储 `Repository<T>` CRUD 端到端测试
//!
//! 覆盖：`JsonRepository` 参考实现的完整 CRUD 循环、分页、转义注入安全，
//! 以及 `impl_json_repository!` 宏生成的具体实现。

#![cfg(all(
    feature = "repository",
    feature = "sqlite",
    feature = "sql-parser",
    feature = "runtime-tokio-rustls"
))]

use dbnexus::{DbPool, JsonRepository, Repository};
use serde::{Deserialize, Serialize};

/// 测试实体
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct User {
    id: i64,
    name: String,
    email: String,
    age: i64,
}

/// 宏生成的仓储（示例口径）
#[derive(Default)]
struct UserRepo;

dbnexus::impl_json_repository!(UserRepo, User, table = "t418_users");

async fn setup_pool(tag: &str) -> (DbPool, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("dbnexus_t418_{}_{}.db", tag, std::process::id()));
    let _ = std::fs::remove_file(&path);
    let url = format!("sqlite:{}?mode=rwc", path.display());
    let pool = DbPool::new(&url).await.expect("pool");
    let admin = pool.get_session("admin").await.expect("admin session");
    admin
        .execute_raw_ddl(
            "CREATE TABLE t418_users (id INTEGER PRIMARY KEY, name TEXT, email TEXT, age INTEGER)",
        )
        .await
        .expect("create table");
    (pool, path)
}

/// JsonRepository：完整 CRUD 循环
#[tokio::test]
async fn test_json_repository_crud_roundtrip() {
    let (pool, path) = setup_pool("crud").await;
    let repo = JsonRepository::new("t418_users").expect("repo");

    // insert（实体携带显式主键；insert 返回 last_insert_id）
    let user = User {
        id: 7,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
        age: 30,
    };
    repo.insert(&pool, &user).await.expect("insert");

    // find_by_id
    let found = repo.find_by_id(&pool, 7).await.expect("find_by_id");
    // JsonRepository 的 T 仅出现在返回值类型，调用侧需标注（宏实现无需标注）
    let found: User = found.expect("row should exist");
    assert_eq!(found.name, "Alice");
    assert_eq!(found.age, 30);

    // update（主键列不参与 SET）
    let updated = User {
        name: "Alice II".to_string(),
        email: found.email.clone(),
        age: 31,
        id: 7,
    };
    let affected = repo.update(&pool, 7, &updated).await.expect("update");
    assert_eq!(affected, 1);
    let reread: User = repo.find_by_id(&pool, 7).await.unwrap().unwrap();
    assert_eq!(reread.name, "Alice II");

    // delete
    let affected = Repository::<User>::delete(&repo, &pool, 7)
        .await
        .expect("delete");
    assert_eq!(affected, 1);
    let gone: Option<User> = repo.find_by_id(&pool, 7).await.expect("find after delete");
    assert!(gone.is_none());

    let _ = std::fs::remove_file(&path);
}

/// find_all 分页 + count
#[tokio::test]
async fn test_json_repository_pagination_and_count() {
    let (pool, path) = setup_pool("page").await;
    let repo = JsonRepository::new("t418_users").expect("repo");

    for i in 1..=5i64 {
        let user = User {
            id: i,
            name: format!("u{i}"),
            email: format!("u{i}@x.com"),
            age: i,
        };
        repo.insert(&pool, &user).await.expect("insert");
    }

    assert_eq!(
        Repository::<User>::count(&repo, &pool)
            .await
            .expect("count"),
        5
    );

    let page0: Vec<User> = repo.find_all(&pool, 2, 0).await.expect("page 0");
    let page1: Vec<User> = repo.find_all(&pool, 2, 2).await.expect("page 1");
    assert_eq!(page0.len(), 2);
    assert_eq!(page1.len(), 2);
    // 按主键升序稳定分页
    assert_eq!(page0[0].age, 1);
    assert_eq!(page1[0].age, 3);

    // find_by_id 不存在的行 → None
    let missing: Option<User> = repo.find_by_id(&pool, 99_999).await.expect("find missing");
    assert!(missing.is_none());

    let _ = std::fs::remove_file(&path);
}

/// 含单引号的字符串值经标准转义安全写入并读回（注入安全口径）
#[tokio::test]
async fn test_string_values_with_quotes_roundtrip() {
    let (pool, path) = setup_pool("quotes").await;
    let repo = JsonRepository::new("t418_users").expect("repo");

    let tricky = User {
        id: 11,
        name: "O'Brien".to_string(),
        email: "x'y@z.com".to_string(),
        age: 1,
    };
    repo.insert(&pool, &tricky)
        .await
        .expect("insert with quotes");
    let read: User = repo.find_by_id(&pool, 11).await.unwrap().unwrap();
    assert_eq!(read.name, "O'Brien");
    assert_eq!(read.email, "x'y@z.com");
    // 表仍然健在（未被注入破坏）
    assert_eq!(
        Repository::<User>::count(&repo, &pool)
            .await
            .expect("count"),
        1
    );

    let _ = std::fs::remove_file(&path);
}

/// impl_json_repository! 宏生成的实现与 JsonRepository 行为一致
#[tokio::test]
async fn test_macro_generated_repository() {
    let (pool, path) = setup_pool("macro").await;
    let repo = UserRepo;

    assert_eq!(repo.table(), "t418_users");

    let user = User {
        id: 21,
        name: "Bob".to_string(),
        email: "bob@x.com".to_string(),
        age: 22,
    };
    repo.insert(&pool, &user).await.expect("insert");
    let found = repo.find_by_id(&pool, 21).await.unwrap().unwrap();
    assert_eq!(
        found,
        User {
            id: 21,
            name: "Bob".to_string(),
            email: "bob@x.com".to_string(),
            age: 22
        }
    );
    // 宏生成的实现：T 已由 impl 固定，方法调用可直接推断
    assert_eq!(repo.count(&pool).await.expect("count"), 1);
    assert_eq!(repo.delete(&pool, 21).await.expect("delete"), 1);

    let _ = std::fs::remove_file(&path);
}
