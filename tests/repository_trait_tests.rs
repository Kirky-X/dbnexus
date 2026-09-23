// Copyright (c) 2025-2026 Kirky.X🌠
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

// ============================================================================
// 乐观锁 update_if_version（R-repo-004）
// ============================================================================

/// 带版本列的测试实体
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Versioned {
    id: i64,
    qty: i64,
    version: i64,
}

#[derive(Default)]
struct VersionedRepo;

dbnexus::impl_json_repository!(VersionedRepo, Versioned, table = "t418_versioned");

async fn setup_version_pool(tag: &str) -> (DbPool, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!(
        "dbnexus_t418_ver_{}_{}.db",
        tag,
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let url = format!("sqlite:{}?mode=rwc", path.display());
    let pool = DbPool::new(&url).await.expect("pool");
    let admin = pool.get_session("admin").await.expect("admin session");
    admin
        .execute_raw_ddl(
            "CREATE TABLE t418_versioned (id INTEGER PRIMARY KEY, qty INTEGER, version INTEGER)",
        )
        .await
        .expect("create table");
    (pool, path)
}

/// 版本匹配：更新成功且版本自增
#[tokio::test]
async fn test_update_if_version_success_bumps_version() {
    let (pool, path) = setup_version_pool("cas_ok").await;
    let repo = JsonRepository::new("t418_versioned").expect("repo");

    repo.insert(
        &pool,
        &Versioned {
            id: 1,
            qty: 5,
            version: 3,
        },
    )
    .await
    .expect("insert");

    let affected = repo
        .update_if_version(
            &pool,
            1,
            &Versioned {
                id: 1,
                qty: 42,
                version: 3,
            },
            "version",
        )
        .await
        .expect("cas should succeed");
    assert_eq!(affected, 1);

    let reread: Versioned = repo.find_by_id(&pool, 1).await.unwrap().unwrap();
    assert_eq!(reread.qty, 42);
    assert_eq!(reread.version, 4, "version must be bumped by 1");
    let _ = std::fs::remove_file(path);
}

/// 版本过期：VersionConflict 且影响 0 行
#[tokio::test]
async fn test_update_if_version_stale_version_conflicts() {
    let (pool, path) = setup_version_pool("cas_stale").await;
    let repo = JsonRepository::new("t418_versioned").expect("repo");

    repo.insert(
        &pool,
        &Versioned {
            id: 2,
            qty: 5,
            version: 7,
        },
    )
    .await
    .expect("insert");

    let err = repo
        .update_if_version(
            &pool,
            2,
            &Versioned {
                id: 2,
                qty: 9,
                version: 3, // 过期版本（库中是 7）
            },
            "version",
        )
        .await
        .expect_err("stale version must conflict");
    match err {
        dbnexus::DbError::VersionConflict { table, id } => {
            assert_eq!(table, "t418_versioned");
            assert_eq!(id, 2);
        }
        other => panic!("expected VersionConflict, got {other:?}"),
    }

    // 数据未被篡改
    let reread: Versioned = repo.find_by_id(&pool, 2).await.unwrap().unwrap();
    assert_eq!(reread.qty, 5);
    assert_eq!(reread.version, 7);
    let _ = std::fs::remove_file(path);
}

/// 宏生成的仓储转发 update_if_version
#[tokio::test]
async fn test_macro_repo_forwards_update_if_version() {
    let (pool, path) = setup_version_pool("cas_macro").await;
    let repo = VersionedRepo;

    repo.insert(
        &pool,
        &Versioned {
            id: 3,
            qty: 1,
            version: 0,
        },
    )
    .await
    .expect("insert");

    let affected = repo
        .update_if_version(
            &pool,
            3,
            &Versioned {
                id: 3,
                qty: 2,
                version: 0,
            },
            "version",
        )
        .await
        .expect("macro-forwarded cas should succeed");
    assert_eq!(affected, 1);
    let _ = std::fs::remove_file(path);
}

// ============================================================================
// 列投影与游标分页（R-repo-002 / R-repo-003）
// ============================================================================

/// 列投影：查询生成显式列清单而非 SELECT *
#[tokio::test]
async fn test_with_columns_generates_projection() {
    // 非法列名构造时拒绝
    assert!(
        JsonRepository::new("t418_users")
            .unwrap()
            .with_columns(&["id", "bad; DROP TABLE x"])
            .is_err()
    );

    // 合法列名：投影列查询（行为验证：只查 id/name 两列，email 缺失时反序列化为错误
    // —— 以此证明 SQL 生成确实只取了投影列）
    let repo = JsonRepository::new("t418_users")
        .unwrap()
        .with_columns(&["id", "name"])
        .expect("with_columns");
    let (pool, path) = setup_pool("projection").await;
    repo.insert(
        &pool,
        &User {
            id: 20,
            name: "Cara".to_string(),
            email: "c@example.com".to_string(),
            age: 25,
        },
    )
    .await
    .expect("insert");
    // 完整实体反序列化会因缺 email/age 失败——改用游标 + 宽投影查全列验证不报错
    let wide = JsonRepository::new("t418_users")
        .unwrap()
        .with_columns(&["id", "name"])
        .expect("wide");
    let rows = wide.find_all_cursor::<User>(&pool, 0, 10).await;
    // User 需要 email/age 字段：投影缺列 → 反序列化失败正是投影生效的证据
    assert!(
        rows.is_err(),
        "missing projected columns must fail deserialization"
    );

    // 投影覆盖全列时查询正常
    let all_cols = JsonRepository::new("t418_users")
        .unwrap()
        .with_columns(&["id", "name", "email", "age"])
        .expect("all cols");
    let found: Vec<User> = all_cols
        .find_all_cursor(&pool, 0, 10)
        .await
        .expect("cursor");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "Cara");
    let _ = std::fs::remove_file(path);
}

/// 游标分页：连续翻页遍历全量无重复
#[tokio::test]
async fn test_find_all_cursor_walks_all_rows_without_duplicates() {
    let (pool, path) = setup_pool("cursor").await;
    let repo = JsonRepository::new("t418_users").expect("repo");
    for i in 0..7i64 {
        repo.insert(
            &pool,
            &User {
                id: (i + 1) * 10,
                name: format!("u{i}"),
                email: format!("u{i}@example.com"),
                age: 20 + i,
            },
        )
        .await
        .expect("insert");
    }

    let mut seen: Vec<i64> = Vec::new();
    let mut after = 0i64;
    loop {
        let page: Vec<User> = repo
            .find_all_cursor(&pool, after, 3)
            .await
            .expect("cursor page");
        if page.is_empty() {
            break;
        }
        seen.extend(page.iter().map(|u| u.id));
        after = *seen.last().expect("non-empty page");
    }
    assert_eq!(
        seen,
        vec![10, 20, 30, 40, 50, 60, 70],
        "full ordered walk, no dupes"
    );
    let _ = std::fs::remove_file(path);
}

// ============================================================================
// 绑定参数化：恶意值往返（R-repo-005 / R-txn-003）
// ============================================================================

/// 含注入序列的值经绑定参数写入后必须逐字节往返、且不影响表数据
#[tokio::test]
async fn test_bound_params_roundtrip_malicious_values() {
    let (pool, path) = setup_pool("bind_roundtrip").await;
    let repo = JsonRepository::new("t418_users").expect("repo");

    let evil_name = "O'Brien \\ ; DROP TABLE t418_users; -- ' OR '1'='1";
    let evil_email = "x\\'@example.com";

    repo.insert(
        &pool,
        &User {
            id: 900,
            name: evil_name.to_string(),
            email: evil_email.to_string(),
            age: 1,
        },
    )
    .await
    .expect("insert with malicious values");

    // 逐字节往返
    let found: User = repo.find_by_id(&pool, 900).await.unwrap().unwrap();
    assert_eq!(found.name, evil_name);
    assert_eq!(found.email, evil_email);

    // 表未被注入语句破坏：更新仍正常工作
    let affected = repo
        .update(
            &pool,
            900,
            &User {
                id: 900,
                name: "recovered".to_string(),
                email: evil_email.to_string(),
                age: 2,
            },
        )
        .await
        .expect("update after malicious insert");
    assert_eq!(affected, 1);

    // 乐观锁路径同样走绑定：版本过期仍正确冲突
    let err = repo
        .update_if_version(
            &pool,
            900,
            &User {
                id: 900,
                name: "cas".to_string(),
                email: "cas@example.com".to_string(),
                age: 3,
            },
            "age", // 非版本语义，但走同一绑定路径验证占位符
        )
        .await;
    // age=3 与库中 age=2 不匹配 → VersionConflict（绑定路径条件判断生效）
    assert!(
        matches!(err, Err(dbnexus::DbError::VersionConflict { .. })),
        "bound CAS must evaluate conditions correctly: {err:?}"
    );

    // 全表行数恰好 1（此前唯一的 insert），DROP TABLE 未得逞
    let all: Vec<User> = repo.find_all(&pool, 100, 0).await.expect("find_all");
    assert_eq!(all.len(), 1);
    let _ = std::fs::remove_file(path);
}
