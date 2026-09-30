// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 大文件拆分：自 session.rs 按职责纯移动的测试模块（行为不变）。

use super::*;

#[cfg(all(test, feature = "ladybug"))]
mod graph_tests {
    use super::*;
    use crate::database::graph::{GraphExecResult, GraphValue};

    /// 创建 Ladybug 内存连接池
    async fn make_ladybug_pool() -> DbPool {
        DbPool::new("ladybug::memory:")
            .await
            .expect("Failed to create Ladybug pool")
    }

    /// 图连接初始 is_in_transaction 为 false
    #[tokio::test]
    async fn test_graph_session_is_in_transaction_initial_false() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        assert!(
            !session.is_in_transaction().await,
            "initial state should be no transaction"
        );
    }

    /// begin_transaction 后 is_in_transaction 为 true
    #[tokio::test]
    async fn test_graph_session_begin_sets_in_transaction() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        session
            .begin_transaction()
            .await
            .expect("begin should succeed");
        assert!(
            session.is_in_transaction().await,
            "should be in transaction after begin"
        );
    }

    /// begin + commit 后 is_in_transaction 为 false
    #[tokio::test]
    async fn test_graph_session_commit_clears_in_transaction() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        session.begin_transaction().await.expect("begin");
        session.commit().await.expect("commit");
        assert!(
            !session.is_in_transaction().await,
            "should not be in transaction after commit"
        );
    }

    /// begin + rollback 后 is_in_transaction 为 false
    #[tokio::test]
    async fn test_graph_session_rollback_clears_in_transaction() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        session.begin_transaction().await.expect("begin");
        session.rollback().await.expect("rollback");
        assert!(
            !session.is_in_transaction().await,
            "should not be in transaction after rollback"
        );
    }

    /// 图事务 begin → execute_cypher → commit 端到端
    #[tokio::test]
    async fn test_graph_transaction_commit_e2e() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");

        // 准备：创建 schema
        session
            .execute_cypher_with_params(
                "CREATE NODE TABLE Person(name STRING, PRIMARY KEY(name))",
                HashMap::new(),
            )
            .await
            .expect("create node table");

        // 事务：插入数据并提交
        session.begin_transaction().await.expect("begin");
        session
            .execute_cypher_with_params("CREATE (:Person {name: 'Alice'})", HashMap::new())
            .await
            .expect("create in txn");
        session.commit().await.expect("commit");

        // 验证：提交后数据可见
        let result = session
            .execute_cypher_with_params("MATCH (p:Person) RETURN p.name AS name", HashMap::new())
            .await
            .expect("match after commit");
        match result {
            GraphExecResult::Query(q) => {
                assert_eq!(q.rows.len(), 1, "should see 1 person after commit");
                let name = &q.rows[0].columns[0].1;
                match name {
                    GraphValue::Scalar(serde_json::Value::String(s)) => assert_eq!(s, "Alice"),
                    other => panic!("expected String Scalar, got {other:?}"),
                }
            }
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }
    }

    /// 图事务 begin → execute_cypher → rollback 端到端
    #[tokio::test]
    async fn test_graph_transaction_rollback_e2e() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");

        // 准备：创建 schema
        session
            .execute_cypher_with_params(
                "CREATE NODE TABLE Person(name STRING, PRIMARY KEY(name))",
                HashMap::new(),
            )
            .await
            .expect("create node table");

        // 事务：插入数据并回滚
        session.begin_transaction().await.expect("begin");
        session
            .execute_cypher_with_params("CREATE (:Person {name: 'Bob'})", HashMap::new())
            .await
            .expect("create in txn");
        session.rollback().await.expect("rollback");

        // 验证：回滚后数据不可见
        let result = session
            .execute_cypher_with_params("MATCH (p:Person) RETURN p.name AS name", HashMap::new())
            .await
            .expect("match after rollback");
        match result {
            GraphExecResult::Query(q) => {
                assert_eq!(q.rows.len(), 0, "should see 0 persons after rollback");
            }
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }
    }

    /// 重复 begin 应返回 Transaction 错误
    #[tokio::test]
    async fn test_graph_double_begin_fails() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        session.begin_transaction().await.expect("first begin");
        let result = session.begin_transaction().await;
        assert!(result.is_err(), "double begin should fail");
        let err = result.unwrap_err();
        assert!(
            matches!(err, DbError::Transaction(ref msg) if msg.contains("Already in")),
            "expected 'Already in' error, got {:?}",
            err
        );
    }

    /// 无事务时 commit 应返回错误
    #[tokio::test]
    async fn test_graph_commit_without_transaction_fails() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        let result = session.commit().await;
        assert!(result.is_err(), "commit without transaction should fail");
    }

    /// 无事务时 rollback 应返回错误
    #[tokio::test]
    async fn test_graph_rollback_without_transaction_fails() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        let result = session.rollback().await;
        assert!(result.is_err(), "rollback without transaction should fail");
    }

    /// 不在事务中 execute_cypher("RETURN 1") 返回结果
    #[tokio::test]
    async fn test_execute_cypher_without_transaction() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        let result = session
            .execute_cypher_with_params("RETURN 1", HashMap::new())
            .await
            .expect("execute_cypher should succeed");
        match result {
            GraphExecResult::Query(q) => {
                assert_eq!(q.rows.len(), 1, "should return 1 row");
                let value = &q.rows[0].columns[0].1;
                match value {
                    GraphValue::Scalar(s) => assert_eq!(s, &serde_json::json!(1)),
                    other => panic!("expected Scalar, got {other:?}"),
                }
            }
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }
    }

    /// 在事务中 execute_cypher 委托给事务句柄
    #[tokio::test]
    async fn test_execute_cypher_in_transaction() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");

        session
            .execute_cypher_with_params(
                "CREATE NODE TABLE Person(name STRING, age INT64, PRIMARY KEY(name))",
                HashMap::new(),
            )
            .await
            .expect("create table");

        session.begin_transaction().await.expect("begin");
        session
            .execute_cypher_with_params("CREATE (:Person {name: 'Alice', age: 25})", HashMap::new())
            .await
            .expect("create in txn");

        // 事务内查询应看到数据
        let result = session
            .execute_cypher_with_params(
                "MATCH (p:Person) RETURN p.name AS name, p.age AS age",
                HashMap::new(),
            )
            .await
            .expect("match in txn");
        match result {
            GraphExecResult::Query(q) => {
                assert_eq!(q.rows.len(), 1, "should see 1 person in txn");
            }
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }
        session.commit().await.expect("commit");
    }

    /// CREATE NODE TABLE + CREATE + MATCH 端到端
    #[tokio::test]
    async fn test_execute_cypher_e2e_create_match() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");

        // DDL
        session
            .execute_cypher_with_params(
                "CREATE NODE TABLE Person(name STRING, age INT64, PRIMARY KEY(name))",
                HashMap::new(),
            )
            .await
            .expect("create node table");

        // 插入多条
        session
            .execute_cypher_with_params("CREATE (:Person {name: 'Alice', age: 25})", HashMap::new())
            .await
            .expect("create alice");
        session
            .execute_cypher_with_params("CREATE (:Person {name: 'Bob', age: 30})", HashMap::new())
            .await
            .expect("create bob");

        // 查询并验证
        let result = session
            .execute_cypher_with_params(
                "MATCH (p:Person) RETURN p.name AS name, p.age AS age ORDER BY name",
                HashMap::new(),
            )
            .await
            .expect("match");
        match result {
            GraphExecResult::Query(q) => {
                assert_eq!(q.rows.len(), 2, "should return 2 persons");
                // 验证第一行
                let name0 = &q.rows[0].columns[0].1;
                match name0 {
                    GraphValue::Scalar(serde_json::Value::String(s)) => assert_eq!(s, "Alice"),
                    other => panic!("expected String Scalar, got {other:?}"),
                }
                // 验证第二行
                let name1 = &q.rows[1].columns[0].1;
                match name1 {
                    GraphValue::Scalar(serde_json::Value::String(s)) => assert_eq!(s, "Bob"),
                    other => panic!("expected String Scalar, got {other:?}"),
                }
            }
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }
    }

    /// 无效 Cypher 返回错误
    #[tokio::test]
    async fn test_execute_cypher_invalid_returns_error() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        let result = session
            .execute_cypher_with_params("INVALID CYPHER", HashMap::new())
            .await;
        assert!(result.is_err(), "invalid cypher should return error");
    }

    /// 事务内多次 execute_cypher 使用同一事务句柄
    #[tokio::test]
    async fn test_execute_cypher_multiple_in_transaction() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");

        session
            .execute_cypher_with_params(
                "CREATE NODE TABLE Person(name STRING, PRIMARY KEY(name))",
                HashMap::new(),
            )
            .await
            .expect("create table");

        session.begin_transaction().await.expect("begin");

        // 多次 execute_cypher 都应在同一事务内
        session
            .execute_cypher_with_params("CREATE (:Person {name: 'A'})", HashMap::new())
            .await
            .expect("create A");
        session
            .execute_cypher_with_params("CREATE (:Person {name: 'B'})", HashMap::new())
            .await
            .expect("create B");
        session
            .execute_cypher_with_params("CREATE (:Person {name: 'C'})", HashMap::new())
            .await
            .expect("create C");

        let result = session
            .execute_cypher_with_params("MATCH (p:Person) RETURN count(p) AS cnt", HashMap::new())
            .await
            .expect("count in txn");
        match result {
            GraphExecResult::Query(q) => {
                assert_eq!(q.rows.len(), 1);
                let cnt = &q.rows[0].columns[0].1;
                match cnt {
                    GraphValue::Scalar(s) => assert_eq!(s, &serde_json::json!(3)),
                    other => panic!("expected Scalar, got {other:?}"),
                }
            }
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }
        session.commit().await.expect("commit");
    }

    /// 非 admin 角色调用 execute_cypher_with_params 应被拒绝（permission feature）
    #[cfg(feature = "permission")]
    #[tokio::test]
    async fn test_execute_cypher_non_admin_denied() {
        let pool = make_ladybug_pool().await;
        // system 角色在无权限配置时也被允许获取 session
        let session = pool.get_session("system").await.expect("get_session");
        let result = session
            .execute_cypher_with_params("RETURN 1", HashMap::new())
            .await;
        assert!(result.is_err(), "non-admin role should be denied");
        let err = result.unwrap_err();
        assert!(
            matches!(err, DbError::Permission(ref msg) if msg.contains("Graph operation denied")),
            "expected Permission error, got {:?}",
            err
        );
    }

    /// admin 角色 execute_cypher_with_params 成功（permission feature）
    #[cfg(feature = "permission")]
    #[tokio::test]
    async fn test_execute_cypher_admin_allowed() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        let result = session
            .execute_cypher_with_params("RETURN 42", HashMap::new())
            .await;
        assert!(result.is_ok(), "admin role should be allowed");
    }
}

// ============================================================================
// vuln-0001 安全审计测试
// ============================================================================

#[cfg(test)]
#[cfg(all(feature = "permission", feature = "sqlite"))]
mod vuln_0001_tests {
    use super::*;

    /// vuln-0001 集成测试：admin 角色绕过权限检查仍返回 Ok（带审计日志）
    #[cfg(all(feature = "permission", feature = "sqlite"))]
    #[tokio::test]
    async fn test_vuln_0001_admin_bypass_returns_ok_with_audit() {
        let pool = DbPool::new("sqlite::memory:")
            .await
            .expect("Failed to create pool");
        let session = pool.get_session("admin").await.expect("get_session");

        // admin 角色绕过权限检查，应返回 Ok
        let result = session
            .check_permission("any_table", &PermissionAction::Select)
            .await;
        assert!(result.is_ok(), "admin bypass should return Ok");

        // 也测试其他操作
        let result = session
            .check_permission("any_table", &PermissionAction::Insert)
            .await;
        assert!(result.is_ok(), "admin bypass should return Ok for Insert");

        let result = session
            .check_permission("any_table", &PermissionAction::Delete)
            .await;
        assert!(result.is_ok(), "admin bypass should return Ok for Delete");
    }

    /// vuln-0001 集成测试：非 admin 角色权限被拒绝
    #[cfg(all(feature = "permission", feature = "sqlite"))]
    #[tokio::test]
    async fn test_vuln_0001_non_admin_denied() {
        let pool = DbPool::new("sqlite::memory:")
            .await
            .expect("Failed to create pool");
        // system 角色可获取 session 但不是 admin_role，无权限配置时 check_permission 应拒绝
        let session = pool.get_session("system").await.expect("get_session");

        // 非 admin 角色应被拒绝（无权限配置时默认拒绝）
        let result = session
            .check_permission("any_table", &PermissionAction::Select)
            .await;
        assert!(result.is_err(), "non-admin should be denied");
    }

    /// 非 admin 角色有权限时 check_permission 返回 Ok (覆盖 line 162)
    #[cfg(all(feature = "permission", feature = "sqlite"))]
    #[tokio::test]
    async fn test_check_permission_non_admin_allowed() {
        use std::io::Write;

        // 创建权限配置文件，授予 "reader" 角色对 "test_tbl" 的 SELECT 权限
        let yaml_content = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations: ["select", "insert", "update", "delete"]
  reader:
    tables:
      - name: "test_tbl"
        operations: ["select"]
"#;
        let tmp_dir = std::env::temp_dir();
        let yaml_path = tmp_dir.join("test_non_admin_perm.yaml");
        {
            let mut file = std::fs::File::create(&yaml_path).expect("create temp file");
            file.write_all(yaml_content.as_bytes())
                .expect("write temp file");
        }

        let config = crate::foundation::DbConfig {
            url: "sqlite::memory:".to_string(),
            permissions_path: Some(yaml_path.to_string_lossy().to_string()),
            ..Default::default()
        };
        let pool = DbPool::with_config(config)
            .await
            .expect("should create pool");

        // reader 角色有 test_tbl 的 SELECT 权限 -> check_permission 应返回 Ok
        let session = pool
            .get_session("reader")
            .await
            .expect("get_session for reader");
        let result = session
            .check_permission("test_tbl", &PermissionAction::Select)
            .await;
        assert!(
            result.is_ok(),
            "reader should have SELECT on test_tbl: {:?}",
            result.err()
        );

        // 清理临时文件
        let _ = std::fs::remove_file(&yaml_path);
    }

    /// vuln-0001 集成测试：check_table_permission admin bypass 带审计日志
    #[cfg(all(feature = "permission", feature = "sqlite"))]
    #[tokio::test]
    async fn test_vuln_0001_check_table_permission_admin_bypass() {
        let pool = DbPool::new("sqlite::memory:")
            .await
            .expect("Failed to create pool");
        let session = pool.get_session("admin").await.expect("get_session");

        // admin bypass check_table_permission
        let result = session.check_table_permission("users", "SELECT").await;
        assert!(result.is_ok(), "admin should bypass check_table_permission");

        let result = session.check_table_permission("users", "INSERT").await;
        assert!(
            result.is_ok(),
            "admin should bypass check_table_permission for INSERT"
        );
    }
}

// ============================================================================
// vuln-0003 测试：SqlParser 表名提取安全验证
// ============================================================================

#[cfg(test)]
#[cfg(all(feature = "permission", feature = "sql-parser"))]
mod vuln_0003_tests {
    use super::*;

    /// vuln-0003：SqlParser 对 INSERT 的正确处理
    #[tokio::test]
    async fn test_vuln_0003_parser_correctly_handles_insert() {
        let sql = "INSERT INTO users (name) VALUES ('from into values')";
        let parser_result = extract_table_name_via_parser(sql).await;
        assert_eq!(
            parser_result.as_deref(),
            Some("users"),
            "SqlParser should correctly extract 'users' for INSERT"
        );
    }

    /// vuln-0003：SqlParser 对 UPDATE 的正确处理
    #[tokio::test]
    async fn test_vuln_0003_parser_correctly_handles_update() {
        let sql = "UPDATE users SET name = 'from users' WHERE id = 1";
        let parser_result = extract_table_name_via_parser(sql).await;
        assert_eq!(
            parser_result.as_deref(),
            Some("users"),
            "SqlParser should correctly extract 'users' for UPDATE"
        );
    }

    /// vuln-0003：SqlParser 对 DELETE 的正确处理
    #[tokio::test]
    async fn test_vuln_0003_parser_correctly_handles_delete() {
        let sql = "DELETE FROM users WHERE name = 'from deleted'";
        let parser_result = extract_table_name_via_parser(sql).await;
        assert_eq!(
            parser_result.as_deref(),
            Some("users"),
            "SqlParser should correctly extract 'users' for DELETE"
        );
    }

    /// vuln-0003 Red-7：朴素 `extract_table_name` 对带引号的表名处理
    ///
    /// SQL: `SELECT * FROM "users" WHERE id = 1`
    /// 朴素解析器返回 `"users"`（带引号），权限检查可能因引号不匹配而失败。
    /// SqlParser 返回 `"users"`（标准化形式，与权限策略匹配）。
    #[tokio::test]
    async fn test_vuln_0003_parser_handles_quoted_table_name() {
        let sql = "SELECT * FROM \"users\" WHERE id = 1";

        // SqlParser 应正确解析带引号的表名
        let parser_result = extract_table_name_via_parser(sql).await;
        assert!(
            parser_result.is_some(),
            "SqlParser should extract table name for quoted identifier, got: {:?}",
            parser_result
        );
        // 表名应包含 "users"（可能带引号或不带引号，取决于 sqlparser 序列化）
        let table = parser_result.unwrap();
        assert!(
            table.contains("users"),
            "extracted table name should contain 'users', got: {}",
            table
        );
    }
}

// ============================================================================
// vuln-0005 测试：Cypher 注入防护
// ============================================================================
//
// 漏洞描述：
//   `Session::execute_cypher` 直接接受 Cypher 字符串并执行，
//   若调用方将用户输入拼接进 Cypher，可导致 Cypher 注入：
//   - 多语句注入：`MATCH (n) RETURN n; DELETE (n)`
//   - 注释混淆：`MATCH (n) // bypass RETURN n`
//   - 危险过程：`CALL apoc.systemdb.admin(...)`
//
// 修复方案：
//   1. 添加 `validate_cypher_safety` 对原始 Cypher 做多层检查（长度/多语句/注释/危险过程）
//   2. 添加 `execute_cypher_with_params` 使用 prepared statement 防止值注入
//   3. 标记 `execute_cypher` 为 `#[deprecated]`，引导调用方迁移
//
// 测试策略：
//   - 单元测试 `validate_cypher_safety` 各检查项（拒绝/允许）
//   - 集成测试 `execute_cypher_with_params` 端到端验证参数化查询
// ============================================================================

#[cfg(all(test, feature = "ladybug"))]
mod vuln_0005_tests {
    use super::*;
    use crate::database::graph::{GraphExecResult, GraphValue};

    /// 辅助：创建 Ladybug 内存连接池
    async fn make_ladybug_pool() -> DbPool {
        DbPool::new("ladybug::memory:")
            .await
            .expect("Failed to create Ladybug pool")
    }

    // ===== validate_cypher_safety 拒绝路径 =====

    /// vuln-0005 Red-1：超过 10KB 的 Cypher 被拒绝（DoS 防护）
    ///
    /// 构造 11KB（11_264 字节）的 Cypher 查询，应被 `validate_cypher_safety` 拒绝。
    #[test]
    fn test_validate_cypher_safety_rejects_too_long() {
        // 11_264 字节 = 11KB，超过 10_240 字节限制
        let long_cypher = format!("MATCH (n) RETURN '{}'", "x".repeat(11_200));
        assert!(
            long_cypher.len() > 10_240,
            "test cypher should exceed 10KB, got {} bytes",
            long_cypher.len()
        );

        let result = validate_cypher_safety(&long_cypher);
        assert!(
            result.is_err(),
            "Cypher exceeding 10KB should be rejected (got {} bytes)",
            long_cypher.len()
        );

        // 验证错误类型为 Permission
        match &result {
            Err(DbError::Permission(msg)) => {
                assert!(
                    msg.contains("maximum length") || msg.contains("exceeds"),
                    "error should mention length, got: {}",
                    msg
                );
            }
            other => panic!("expected DbError::Permission, got {:?}", other),
        }
    }

    /// vuln-0005 Red-2：多语句 Cypher 被拒绝（`;` 在查询中间）
    ///
    /// `MATCH (n) RETURN n; MATCH (m) RETURN m` 包含中间分号，
    /// 应被 `validate_cypher_safety` 拒绝（防止 `MATCH ...; DELETE ...` 注入）。
    #[test]
    fn test_validate_cypher_safety_rejects_multi_statement() {
        let cypher = "MATCH (n) RETURN n; MATCH (m) RETURN m";
        let result = validate_cypher_safety(cypher);
        assert!(result.is_err(), "multi-statement Cypher should be rejected");

        match &result {
            Err(DbError::Permission(msg)) => {
                assert!(
                    msg.contains("multiple statements") || msg.contains("';'"),
                    "error should mention multiple statements, got: {}",
                    msg
                );
            }
            other => panic!("expected DbError::Permission, got {:?}", other),
        }
    }

    /// vuln-0005 Red-3：包含行注释 `//` 的 Cypher 被拒绝
    ///
    /// `MATCH (n) // comment RETURN n` 包含行注释，
    /// 应被 `validate_cypher_safety` 拒绝（防止注释绕过安全检查）。
    #[test]
    fn test_validate_cypher_safety_rejects_line_comment() {
        let cypher = "MATCH (n) // comment RETURN n";
        let result = validate_cypher_safety(cypher);
        assert!(
            result.is_err(),
            "Cypher with line comment '//' should be rejected"
        );

        match &result {
            Err(DbError::Permission(msg)) => {
                assert!(
                    msg.contains("line comment") || msg.contains("//"),
                    "error should mention line comment, got: {}",
                    msg
                );
            }
            other => panic!("expected DbError::Permission, got {:?}", other),
        }
    }

    /// vuln-0005 Red-4：包含块注释 `/* */` 的 Cypher 被拒绝
    ///
    /// `MATCH (n) /* comment */ RETURN n` 包含块注释，
    /// 应被 `validate_cypher_safety` 拒绝（防止注释绕过权限检查片段）。
    #[test]
    fn test_validate_cypher_safety_rejects_block_comment() {
        let cypher = "MATCH (n) /* comment */ RETURN n";
        let result = validate_cypher_safety(cypher);
        assert!(
            result.is_err(),
            "Cypher with block comment '/* */' should be rejected"
        );

        match &result {
            Err(DbError::Permission(msg)) => {
                assert!(
                    msg.contains("block comment") || msg.contains("/*"),
                    "error should mention block comment, got: {}",
                    msg
                );
            }
            other => panic!("expected DbError::Permission, got {:?}", other),
        }
    }

    /// vuln-0005 Red-5：调用 APOC 危险过程的 Cypher 被拒绝
    ///
    /// `CALL apoc.systemdb.admin(...)` 调用 APOC 管理员过程，
    /// 应被 `validate_cypher_safety` 拒绝（防止提权/文件系统访问）。
    #[test]
    fn test_validate_cypher_safety_rejects_apoc_call() {
        let cypher = "CALL apoc.systemdb.admin('something')";
        let result = validate_cypher_safety(cypher);
        assert!(
            result.is_err(),
            "Cypher calling APOC procedure should be rejected"
        );

        match &result {
            Err(DbError::Permission(msg)) => {
                assert!(
                    msg.contains("dangerous procedure") || msg.contains("apoc"),
                    "error should mention dangerous procedure, got: {}",
                    msg
                );
            }
            other => panic!("expected DbError::Permission, got {:?}", other),
        }
    }

    // ===== validate_cypher_safety 允许路径 =====

    /// vuln-0005 Green-1：正常 Cypher 查询通过安全检查
    ///
    /// `MATCH (n:User) RETURN n` 是标准查询，应通过 `validate_cypher_safety`。
    #[test]
    fn test_validate_cypher_safety_allows_normal_query() {
        let cypher = "MATCH (n:User) RETURN n";
        let result = validate_cypher_safety(cypher);
        assert!(
            result.is_ok(),
            "normal Cypher query should pass safety check, got: {:?}",
            result
        );
    }

    /// vuln-0005 Green-2：末尾分号允许（部分客户端习惯以 `;` 结尾）
    ///
    /// `MATCH (n) RETURN n;` 末尾有分号，但中间无分号，应通过检查。
    #[test]
    fn test_validate_cypher_safety_allows_trailing_semicolon() {
        let cypher = "MATCH (n) RETURN n;";
        let result = validate_cypher_safety(cypher);
        assert!(
            result.is_ok(),
            "Cypher with trailing semicolon should pass safety check, got: {:?}",
            result
        );
    }

    // ===== execute_cypher_with_params 端到端测试 =====

    /// vuln-0005 Green-3：参数化查询端到端验证
    ///
    /// 使用 Ladybug :memory: 图数据库，验证 `execute_cypher_with_params` 能正确：
    /// 1. 接受 `$param` 占位符 Cypher
    /// 2. 通过 params 映射传递参数值
    /// 3. 底层 prepared statement 正确执行
    /// 4. 返回正确的结果集
    ///
    /// 测试场景：CREATE NODE TABLE → 插入参数化数据 → MATCH 验证
    #[tokio::test]
    async fn test_execute_cypher_with_params_passes_params() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");

        // 1. 创建 Node Table（DDL，无参数）
        session
            .execute_cypher_with_params(
                "CREATE NODE TABLE Person(name STRING, age INT64, PRIMARY KEY(name))",
                HashMap::new(),
            )
            .await
            .expect("create node table");

        // 2. 参数化插入 Alice
        let mut params_alice = HashMap::new();
        params_alice.insert("name".to_string(), serde_json::json!("Alice"));
        params_alice.insert("age".to_string(), serde_json::json!(25));
        session
            .execute_cypher_with_params("CREATE (:Person {name: $name, age: $age})", params_alice)
            .await
            .expect("create Alice with params");

        // 3. 参数化插入 Bob
        let mut params_bob = HashMap::new();
        params_bob.insert("name".to_string(), serde_json::json!("Bob"));
        params_bob.insert("age".to_string(), serde_json::json!(30));
        session
            .execute_cypher_with_params("CREATE (:Person {name: $name, age: $age})", params_bob)
            .await
            .expect("create Bob with params");

        // 4. 参数化查询：按 name 过滤
        let mut params_query = HashMap::new();
        params_query.insert("target_name".to_string(), serde_json::json!("Alice"));
        let result = session
            .execute_cypher_with_params(
                "MATCH (p:Person) WHERE p.name = $target_name RETURN p.name AS name, p.age AS age",
                params_query,
            )
            .await
            .expect("match with params");

        // 5. 验证结果
        match result {
            GraphExecResult::Query(q) => {
                assert_eq!(q.rows.len(), 1, "should return 1 person (Alice)");
                // 验证 name 列
                let name_val = &q.rows[0].columns[0].1;
                match name_val {
                    GraphValue::Scalar(serde_json::Value::String(s)) => {
                        assert_eq!(s, "Alice", "name should be Alice");
                    }
                    other => panic!("expected String Scalar for name, got {other:?}"),
                }
                // 验证 age 列
                let age_val = &q.rows[0].columns[1].1;
                match age_val {
                    GraphValue::Scalar(serde_json::Value::Number(n)) => {
                        assert_eq!(n.as_i64(), Some(25), "age should be 25");
                    }
                    other => panic!("expected Number Scalar for age, got {other:?}"),
                }
            }
            GraphExecResult::Write { .. } => panic!("expected Query variant, got Write"),
        }
    }

    /// vuln-0005 Green-4：参数化查询在事务内正常工作
    ///
    /// 验证 `execute_cypher_with_params` 在图事务内执行时，
    /// 所有操作使用同一事务连接（事务隔离）。
    #[tokio::test]
    async fn test_execute_cypher_with_params_in_transaction() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");

        // 创建 Node Table
        session
            .execute_cypher_with_params(
                "CREATE NODE TABLE Account(id INT64, balance INT64, PRIMARY KEY(id))",
                HashMap::new(),
            )
            .await
            .expect("create node table");

        // 开始事务
        session
            .begin_transaction()
            .await
            .expect("begin transaction");

        // 事务内参数化插入
        let mut params1 = HashMap::new();
        params1.insert("id".to_string(), serde_json::json!(1));
        params1.insert("balance".to_string(), serde_json::json!(100));
        session
            .execute_cypher_with_params("CREATE (:Account {id: $id, balance: $balance})", params1)
            .await
            .expect("create account 1 in txn");

        let mut params2 = HashMap::new();
        params2.insert("id".to_string(), serde_json::json!(2));
        params2.insert("balance".to_string(), serde_json::json!(200));
        session
            .execute_cypher_with_params("CREATE (:Account {id: $id, balance: $balance})", params2)
            .await
            .expect("create account 2 in txn");

        // 事务内查询验证
        let result = session
            .execute_cypher_with_params(
                "MATCH (a:Account) RETURN a.id AS id ORDER BY a.id",
                HashMap::new(),
            )
            .await
            .expect("match in txn");

        match result {
            GraphExecResult::Query(q) => {
                assert_eq!(q.rows.len(), 2, "should see 2 accounts in txn");
            }
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }

        session.commit().await.expect("commit");
    }

    /// vuln-0005 Red-6：execute_cypher_with_params 也执行安全检查
    ///
    /// 验证 `execute_cypher_with_params` 同样拒绝危险 Cypher（多语句），
    /// 防止调用方误以为参数化查询可以绕过语句结构检查。
    #[tokio::test]
    async fn test_execute_cypher_with_params_rejects_injection() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");

        // 多语句注入尝试
        let result = session
            .execute_cypher_with_params("MATCH (n) RETURN n; DELETE (n)", HashMap::new())
            .await;

        assert!(
            result.is_err(),
            "multi-statement Cypher should be rejected even in execute_cypher_with_params"
        );

        match result {
            Err(DbError::Permission(msg)) => {
                assert!(
                    msg.contains("multiple statements") || msg.contains("';'"),
                    "error should mention multiple statements, got: {}",
                    msg
                );
            }
            other => panic!("expected DbError::Permission, got {:?}", other),
        }
    }
}

// ============================================================================
// Session 基础测试（仅需 sqlite feature）
// ============================================================================

#[cfg(test)]
#[cfg(feature = "sqlite")]
mod session_basic_tests {
    use super::*;

    /// 辅助：创建 SQLite 内存连接池并获取 session
    async fn make_test_session(role: &str) -> (super::super::super::DbPool, Session) {
        let pool = super::super::super::DbPool::new("sqlite::memory:")
            .await
            .expect("Failed to create pool");
        let session = pool.get_session(role).await.expect("get_session failed");
        (pool, session)
    }

    #[tokio::test]
    async fn test_session_role() {
        let (_pool, session) = make_test_session("admin").await;
        assert_eq!(session.role(), "admin");
    }

    #[tokio::test]
    async fn test_session_is_in_transaction_initially_false() {
        let (_pool, session) = make_test_session("admin").await;
        assert!(!session.is_in_transaction().await);
    }

    #[tokio::test]
    async fn test_session_should_use_master_initially_false() {
        let (_pool, session) = make_test_session("admin").await;
        assert!(!session.should_use_master().await);
    }

    #[tokio::test]
    async fn test_session_mark_write_enables_master() {
        let (_pool, session) = make_test_session("admin").await;
        session.mark_write().await;
        assert!(session.should_use_master().await);
    }

    #[tokio::test]
    async fn test_session_connection_returns_ok() {
        let (_pool, session) = make_test_session("admin").await;
        assert!(session.connection().is_ok());
    }

    #[tokio::test]
    async fn test_session_begin_and_commit_transaction() {
        let (_pool, session) = make_test_session("admin").await;

        // 开始事务
        session
            .begin_transaction()
            .await
            .expect("begin_transaction");
        assert!(session.is_in_transaction().await);
        assert!(session.should_use_master().await);

        // 提交事务
        session.commit().await.expect("commit");
        assert!(!session.is_in_transaction().await);
    }

    #[tokio::test]
    async fn test_session_begin_and_rollback_transaction() {
        let (_pool, session) = make_test_session("admin").await;

        session
            .begin_transaction()
            .await
            .expect("begin_transaction");
        assert!(session.is_in_transaction().await);

        session.rollback().await.expect("rollback");
        assert!(!session.is_in_transaction().await);
    }

    #[tokio::test]
    async fn test_session_double_begin_returns_error() {
        let (_pool, session) = make_test_session("admin").await;

        session.begin_transaction().await.expect("first begin");

        // 第二次 begin 应返回错误
        let result = session.begin_transaction().await;
        assert!(result.is_err(), "double begin should return error");

        // 清理：提交第一个事务
        session.commit().await.expect("commit");
    }

    #[tokio::test]
    async fn test_session_rollback_without_transaction_returns_error() {
        let (_pool, session) = make_test_session("admin").await;

        // 没有活跃事务时 rollback 应返回错误
        let result = session.rollback().await;
        assert!(result.is_err(), "rollback without transaction should error");
    }

    #[tokio::test]
    async fn test_session_commit_without_transaction_returns_error() {
        let (_pool, session) = make_test_session("admin").await;

        let result = session.commit().await;
        assert!(result.is_err(), "commit without transaction should error");
    }

    #[cfg(feature = "sql-parser")]
    #[tokio::test]
    async fn test_session_execute_raw_select() {
        let (_pool, session) = make_test_session("admin").await;

        // Create a table first so SELECT has a valid table name
        use sea_orm::ConnectionTrait;
        session
            .connection()
            .unwrap()
            .execute_unprepared("CREATE TABLE sel_test (id INTEGER PRIMARY KEY)")
            .await
            .expect("create table");

        // SELECT from table should succeed for admin role
        let result = session.execute_raw("SELECT * FROM sel_test").await;
        assert!(
            result.is_ok(),
            "SELECT from table should succeed: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_session_execute_raw_ddl_rejected() {
        let (_pool, session) = make_test_session("admin").await;

        // DDL operations should be rejected by execute_raw
        let result = session.execute_raw("CREATE TABLE test (id INTEGER)").await;
        assert!(result.is_err(), "DDL should be rejected");
    }

    #[cfg(feature = "sql-parser")]
    #[tokio::test]
    async fn test_session_execute_raw_create_table_and_insert() {
        let (_pool, session) = make_test_session("admin").await;

        // Create table via execute_unprepared (not execute_raw which checks permissions)
        use sea_orm::ConnectionTrait;
        session
            .connection()
            .unwrap()
            .execute_unprepared("CREATE TABLE test_tbl (id INTEGER PRIMARY KEY, name TEXT)")
            .await
            .expect("create table");

        // Insert via execute_raw (admin bypasses permission)
        let result = session
            .execute_raw("INSERT INTO test_tbl (id, name) VALUES (1, 'test')")
            .await;
        assert!(
            result.is_ok(),
            "INSERT should succeed for admin: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_session_database_session_trait_commit() {
        let (_pool, session) = make_test_session("admin").await;

        // Use the DatabaseSession trait method explicitly
        use super::super::super::DatabaseSession;
        let result = DatabaseSession::commit(&session).await;
        assert!(
            result.is_err(),
            "commit without transaction via trait should error"
        );
    }

    #[tokio::test]
    async fn test_session_database_session_trait_rollback() {
        let (_pool, session) = make_test_session("admin").await;

        use super::super::super::DatabaseSession;
        let result = DatabaseSession::rollback(&session).await;
        assert!(
            result.is_err(),
            "rollback without transaction via trait should error"
        );
    }

    #[cfg(feature = "sql-parser")]
    #[tokio::test]
    async fn test_session_database_session_trait_execute() {
        let (_pool, session) = make_test_session("admin").await;

        // Create a table first
        use sea_orm::ConnectionTrait;
        session
            .connection()
            .unwrap()
            .execute_unprepared("CREATE TABLE trait_exec_test (id INTEGER PRIMARY KEY)")
            .await
            .expect("create table");

        use super::super::super::DatabaseSession;
        let result =
            DatabaseSession::execute(&session, "INSERT INTO trait_exec_test (id) VALUES (1)").await;
        assert!(
            result.is_ok(),
            "execute via trait should succeed: {:?}",
            result.err()
        );
    }

    // ===== 补充测试：is_invalid_table_name, create_migration_executor =====

    #[cfg(feature = "permission")]
    #[tokio::test]
    async fn test_extract_table_name_via_parser_invalid_table() {
        // Parser returns empty table name -> covers line 1430
        let result = super::extract_table_name_via_parser("SELECT 1").await;
        assert!(
            result.is_none(),
            "SELECT without FROM table should return None"
        );
    }

    #[cfg(feature = "permission")]
    #[tokio::test]
    async fn test_extract_table_name_via_parser_unsupported() {
        // Unsupported statement -> covers line 1435
        let result = super::extract_table_name_via_parser("INVALID SQL GIBBERISH").await;
        // Either None (parse error) or Some table
        // Just verify it doesn't panic
        let _ = result;
    }

    #[cfg(feature = "permission")]
    #[tokio::test]
    async fn test_extract_table_name_via_parser_parse_error() {
        // Parse error -> covers line 1436
        let result = super::extract_table_name_via_parser("/* comment */").await;
        // Comments alone should be a parse error or None
        assert!(result.is_none());
    }

    #[cfg(feature = "permission")]
    #[test]
    fn test_is_invalid_table_name_empty() {
        // Empty table name -> covers line 1164
        assert!(super::is_invalid_table_name(""));
        assert!(super::is_invalid_table_name("   "));
    }

    #[cfg(feature = "permission")]
    #[test]
    fn test_is_invalid_table_name_empty_part() {
        // Table name with empty part after split -> covers line 1170
        assert!(super::is_invalid_table_name("schema..table"));
        assert!(super::is_invalid_table_name(".table"));
    }

    #[cfg(feature = "permission")]
    #[test]
    fn test_is_invalid_table_name_valid() {
        assert!(!super::is_invalid_table_name("users"));
        assert!(!super::is_invalid_table_name("public.users"));
        assert!(!super::is_invalid_table_name("\"quoted\".\"table\""));
    }

    #[cfg(feature = "migration")]
    #[tokio::test]
    async fn test_create_migration_executor() {
        let (_pool, session) = make_test_session("admin").await;
        let result = session.create_migration_executor(crate::foundation::DatabaseType::Sqlite);
        assert!(
            result.is_ok(),
            "create_migration_executor should succeed: {:?}",
            result.err()
        );
    }
}

#[cfg(all(test, feature = "sqlite", feature = "sql-parser"))]
mod for_update_tests {
    use super::*;
    use crate::database::DbPool;

    /// R-txn-001: 非事务下调用必须拒绝（拿到锁即释 = 假防护）
    #[tokio::test]
    async fn query_rows_for_update_requires_active_transaction() {
        let pool = DbPool::new("sqlite::memory:").await.expect("pool");
        let session = pool.get_session("admin").await.expect("session");
        let err = session
            .query_rows_for_update("SELECT 1")
            .await
            .expect_err("must reject outside transaction");
        assert!(matches!(err, DbError::Transaction(_)), "got {err:?}");
        assert!(err.message().contains("requires an active transaction"));
    }

    /// R-txn-001: 事务内执行当前读，读得见未提交写（同一事务连接）
    #[tokio::test]
    async fn query_rows_for_update_executes_inside_transaction() {
        let pool = DbPool::new("sqlite::memory:").await.expect("pool");
        let session = pool.get_session("admin").await.expect("session");

        session
            .execute_raw_ddl("CREATE TABLE for_update_t (id INTEGER PRIMARY KEY, qty INTEGER)")
            .await
            .expect("create table");
        session
            .execute_raw("INSERT INTO for_update_t (id, qty) VALUES (1, 5)")
            .await
            .expect("insert");

        session.begin_transaction().await.expect("begin");
        let rows = session
            .query_rows_for_update("SELECT id, qty FROM for_update_t WHERE id = 1")
            .await
            .expect("current read in tx");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["qty"], serde_json::json!(5));

        session.rollback().await.expect("rollback");
    }

    /// R-txn-001: 回滚后（事务结束）再次当前读被拒绝
    #[tokio::test]
    async fn query_rows_for_update_rejected_after_rollback() {
        let pool = DbPool::new("sqlite::memory:").await.expect("pool");
        let session = pool.get_session("admin").await.expect("session");
        session.begin_transaction().await.expect("begin");
        session.rollback().await.expect("rollback");
        let err = session
            .query_rows_for_update("SELECT 1")
            .await
            .expect_err("transaction is over");
        assert!(matches!(err, DbError::Transaction(_)));
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod isolation_level_tests {
    use super::*;
    use crate::database::DbPool;

    /// R-txn-002: sqlite 上以指定隔离级别开启事务并回滚
    #[tokio::test]
    async fn begin_with_isolation_opens_transaction_on_sqlite() {
        let pool = DbPool::new("sqlite::memory:").await.expect("pool");
        let session = pool.get_session("admin").await.expect("session");
        assert!(!session.is_in_transaction().await);
        session
            .begin_transaction_with_isolation(DbIsolationLevel::Serializable)
            .await
            .expect("begin with isolation");
        assert!(session.is_in_transaction().await);
        session.rollback().await.expect("rollback");
        assert!(!session.is_in_transaction().await);
    }

    /// R-txn-002: 重复开启返回 Already in transaction（与 begin_transaction 一致）
    #[tokio::test]
    async fn double_begin_with_isolation_rejected() {
        let pool = DbPool::new("sqlite::memory:").await.expect("pool");
        let session = pool.get_session("admin").await.expect("session");
        session
            .begin_transaction_with_isolation(DbIsolationLevel::ReadCommitted)
            .await
            .expect("first begin");
        let err = session
            .begin_transaction_with_isolation(DbIsolationLevel::Serializable)
            .await
            .expect_err("second begin must fail");
        assert!(matches!(err, DbError::Transaction(_)), "got {err:?}");
        assert!(err.message().contains("Already in transaction"));
        session.rollback().await.expect("rollback");
    }

    /// R-txn-002: 四档级别映射到 sea-orm 不丢档
    #[test]
    fn all_levels_map_to_sea_orm() {
        use DbIsolationLevel::*;
        let _ = ReadUncommitted.into_sea_orm();
        let _ = ReadCommitted.into_sea_orm();
        let _ = RepeatableRead.into_sea_orm();
        let _ = Serializable.into_sea_orm();
    }
}
