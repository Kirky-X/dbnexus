// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! SQL 注入检测测试套件
//!
//! 专门测试小写变体的注入模式，验证 `contains_sql_injection` 函数的大小写不敏感行为。

use dbnexus::contains_sql_injection;

// 小写变体测试

/// 测试堆叠查询注入的小写变体（; DROP TABLE 等）
#[test]
fn test_stacked_queries_lowercase() {
    // ; drop table (小写)
    assert!(
        contains_sql_injection("select * from users; drop table users"),
        "the lowercase '; drop table' should be detected"
    );
    // ; delete from (小写)
    assert!(
        contains_sql_injection("select * from users; delete from users"),
        "the lowercase '; delete from' should be detected"
    );
    // ; update (小写)
    assert!(
        contains_sql_injection("select * from users; update users set admin = 1"),
        "the lowercase '; update' should be detected"
    );
    // ; insert (小写)
    assert!(
        contains_sql_injection("select * from users; insert into users values (1, 'hacker')"),
        "the lowercase '; insert' should be detected"
    );
    // ; truncate (小写)
    assert!(
        contains_sql_injection("select * from users; truncate table users"),
        "the lowercase '; truncate' should be detected"
    );
    // ; alter (小写)
    assert!(
        contains_sql_injection("select * from users; alter table users add column hacked int"),
        "the lowercase '; alter' should be detected"
    );
    // ; create (小写)
    assert!(
        contains_sql_injection("select * from users; create table hacked (id int)"),
        "the lowercase '; create' should be detected"
    );
}

/// 测试 UNION 注入的小写变体
#[test]
fn test_union_injection_lowercase() {
    // union select (小写)
    assert!(
        contains_sql_injection("select * from users union select * from admin"),
        "the lowercase 'union select' should be detected"
    );
    // union all select (小写)
    assert!(
        contains_sql_injection("select * from users union all select * from admin"),
        "the lowercase 'union all select' should be detected"
    );
    // union distinct select (小写)
    assert!(
        contains_sql_injection("select * from users union distinct select password from admin"),
        "the lowercase 'union distinct select' should be detected"
    );
}

/// 测试布尔盲注的小写变体
#[test]
fn test_boolean_blind_lowercase() {
    // or 1=1 (小写)
    assert!(
        contains_sql_injection("select * from users where id = 1 or 1=1"),
        "the lowercase 'or 1=1' should be detected"
    );
    // or true (小写)
    assert!(
        contains_sql_injection("select * from users where id = 1 or true"),
        "the lowercase 'or true' should be detected"
    );
    // or false (小写)
    assert!(
        contains_sql_injection("select * from users where id = 1 or false"),
        "the lowercase 'or false' should be detected"
    );
    // and 1=1 (小写)
    assert!(
        contains_sql_injection("select * from users where id = 1 and 1=1"),
        "the lowercase 'and 1=1' should be detected"
    );
    // and true (小写)
    assert!(
        contains_sql_injection("select * from users where id = 1 and true"),
        "the lowercase 'and true' should be detected"
    );
}

/// 测试时间盲注的小写变体（跨数据库）
#[test]
fn test_time_blind_lowercase() {
    // MySQL: sleep( (小写)
    assert!(
        contains_sql_injection("select * from users where id = 1 and sleep(5)"),
        "the lowercase 'sleep(' should be detected"
    );
    // MySQL: benchmark( (小写)
    assert!(
        contains_sql_injection(
            "select * from users where id = 1 and benchmark(10000000,sha1('test'))"
        ),
        "the lowercase 'benchmark(' should be detected"
    );
    // PostgreSQL: pg_sleep( (小写)
    assert!(
        contains_sql_injection("select * from users where id = 1 and pg_sleep(5)"),
        "the lowercase 'pg_sleep(' should be detected"
    );
    // SQL Server: waitfor delay (小写)
    assert!(
        contains_sql_injection("waitfor delay '0:0:5'"),
        "the lowercase 'waitfor delay' should be detected"
    );
    // SQL Server: waitfor time (小写)
    assert!(
        contains_sql_injection("waitfor time '12:00:00'"),
        "the lowercase 'waitfor time' should be detected"
    );
}

/// 测试动态 SQL 执行的小写变体
#[test]
fn test_dynamic_sql_lowercase() {
    // exec( (小写)
    assert!(
        contains_sql_injection("exec('drop table users')"),
        "the lowercase 'exec(' should be detected"
    );
    // execute( (小写)
    assert!(
        contains_sql_injection("execute('select * from users')"),
        "the lowercase 'execute(' should be detected"
    );
    // sp_executesql (小写)
    assert!(
        contains_sql_injection("sp_executesql n'select * from users'"),
        "the lowercase 'sp_executesql' should be detected"
    );
    // xp_cmdshell (小写)
    assert!(
        contains_sql_injection("xp_cmdshell 'dir'"),
        "the lowercase 'xp_cmdshell' should be detected"
    );
}

/// 测试文件操作的小写变体
#[test]
fn test_file_operations_lowercase() {
    // load_file( (小写)
    assert!(
        contains_sql_injection("select load_file('/etc/passwd')"),
        "the lowercase 'load_file(' should be detected"
    );
    // into outfile (小写)
    assert!(
        contains_sql_injection("select * from users into outfile '/tmp/users.txt'"),
        "the lowercase 'into outfile' should be detected"
    );
    // into dumpfile (小写)
    assert!(
        contains_sql_injection("select * from users into dumpfile '/tmp/users.txt'"),
        "the lowercase 'into dumpfile' should be detected"
    );
}

/// 测试信息泄露的小写变体
#[test]
fn test_info_disclosure_lowercase() {
    // information_schema (小写)
    assert!(
        contains_sql_injection("select * from information_schema.tables"),
        "the lowercase 'information_schema' should be detected"
    );
    // sysobjects (小写)
    assert!(
        contains_sql_injection("select * from sysobjects"),
        "the lowercase 'sysobjects' should be detected"
    );
    // sys.tables (小写)
    assert!(
        contains_sql_injection("select * from sys.tables"),
        "the lowercase 'sys.tables' should be detected"
    );
    // mysql.user (小写)
    assert!(
        contains_sql_injection("select * from mysql.user"),
        "the lowercase 'mysql.user' should be detected"
    );
    // pg_user (小写)
    assert!(
        contains_sql_injection("select * from pg_user"),
        "the lowercase 'pg_user' should be detected"
    );
    // pg_shadow (小写)
    assert!(
        contains_sql_injection("select * from pg_shadow"),
        "the lowercase 'pg_shadow' should be detected"
    );
}

/// 测试编码绕过的小写变体
#[test]
fn test_encoding_bypass_lowercase() {
    // char( (小写)
    assert!(
        contains_sql_injection("select * from users where name = char(97,100,109,105,110)"),
        "the lowercase 'char(' should be detected"
    );
    // chr( (小写)
    assert!(
        contains_sql_injection("select * from users where name = chr(65)"),
        "the lowercase 'chr(' should be detected"
    );
    // concat( (小写)
    assert!(
        contains_sql_injection("select * from users where name = concat('ad','min')"),
        "the lowercase 'concat(' should be detected"
    );
    // concat_ws( (小写)
    assert!(
        contains_sql_injection("select concat_ws(',', 'a', 'b', 'c')"),
        "the lowercase 'concat_ws(' should be detected"
    );
}

/// 测试注释注入
#[test]
fn test_comment_injection() {
    // -- 注释（移除后面内容）
    assert!(
        contains_sql_injection("select * from users where id = 1 -- "),
        "the '-- ' comment injection should be detected"
    );
    assert!(
        contains_sql_injection("select * from users where id = 1 --+"),
        "the '--+' comment injection should be detected"
    );
    // # 注释 (MySQL)
    assert!(
        contains_sql_injection("select * from users where id = 1 #"),
        "the '#' comment injection should be detected"
    );
    // /* */ 块注释
    assert!(
        contains_sql_injection("select * /* comment */ from users"),
        "the '/* ' block comment injection should be detected"
    );
}

/// 测试混合大小写的注入模式
#[test]
fn test_mixed_case_injection() {
    // UnIoN SeLeCt (混合大小写)
    assert!(
        contains_sql_injection("select * from users UnIoN SeLeCt * from admin"),
        "the mixed-case 'UnIoN SeLeCt' should be detected"
    );
    // ; DrOp TaBlE (混合大小写)
    assert!(
        contains_sql_injection("select * from users; DrOp TaBlE users"),
        "the mixed-case '; DrOp TaBlE' should be detected"
    );
    // InFoRmAtIoN_ScHeMa (混合大小写)
    assert!(
        contains_sql_injection("select * from InFoRmAtIoN_ScHeMa.TaBlEs"),
        "the mixed-case 'InFoRmAtIoN_ScHeMa' should be detected"
    );
    // SlEeP( (混合大小写)
    assert!(
        contains_sql_injection("select * from users where id = 1 and SlEeP(5)"),
        "the mixed-case 'SlEeP(' should be detected"
    );
}

/// 测试正常 SQL 的小写形式不被误报
#[test]
fn test_false_positives_lowercase() {
    // 正常的 select 语句（小写）
    assert!(
        !contains_sql_injection("select id, name from users where id = 1"),
        "a normal SELECT should not be detected as injection"
    );
    // 正常的 insert 语句（小写）
    assert!(
        !contains_sql_injection(
            "insert into users (name, email) values ('test', 'test@example.com')"
        ),
        "a normal INSERT should not be detected as injection"
    );
    // 正常的 update 语句（小写）
    assert!(
        !contains_sql_injection("update users set name = 'new_name' where id = 1"),
        "a normal UPDATE should not be detected as injection"
    );
    // 正常的 delete 语句（小写）
    assert!(
        !contains_sql_injection("delete from users where id = 1"),
        "a normal DELETE should not be detected as injection"
    );
    // 正常的 join 操作（小写）
    assert!(
        !contains_sql_injection(
            "select u.name, o.order_id from users u join orders o on u.id = o.user_id"
        ),
        "a normal JOIN should not be detected as injection"
    );
}

// DDL Guard 集成测试 - 验证 DdlGuard 与 SQL 注入检测的协同工作

#[cfg(feature = "sql-parser")]
use dbnexus::{DdlGuard, DdlValidationResult};

/// 测试 DDL Guard 与 SQL 注入检测的边界
#[cfg(feature = "sql-parser")]
#[test]
fn test_ddl_guard_vs_sql_injection() {
    let guard = DdlGuard::new();

    // 合法 DDL 应该通过 DDL Guard
    let result = guard.validate("CREATE TABLE users (id INT PRIMARY KEY)");
    assert!(
        matches!(result, Ok(DdlValidationResult::Allowed)),
        "a legal CREATE TABLE should pass the DDL Guard"
    );

    // DDL Guard 边界（真实行为核正）：自 0540954 起白名单放行
    // DropTable（迁移场景，见 ddl_guard.rs ALLOWED_DDL_STATEMENTS），原断言
    // “DROP TABLE 应被拒”已过时；未列入白名单且命中 FORBIDDEN_PATTERNS 的
    // DROP DATABASE 仍被拒
    let result = guard.validate("DROP TABLE users");
    assert!(
        matches!(result, Ok(DdlValidationResult::Allowed)),
        "DROP TABLE should be allowed by the admin allowlist (migration scenarios)"
    );
    let result = guard.validate("DROP DATABASE users");
    assert!(
        matches!(result, Ok(DdlValidationResult::Forbidden(_))),
        "DROP DATABASE should be rejected by the DDL Guard"
    );

    // 注入检测独立工作：contains_sql_injection 仅针对 DML，DDL 经 DdlGuard 验证
}

/// 测试完整的 SQL 安全验证流程
#[cfg(feature = "sql-parser")]
#[test]
fn test_complete_sql_security_flow() {
    let safe_sql = "SELECT * FROM users WHERE id = 1";
    assert!(
        !contains_sql_injection(safe_sql),
        "safe SQL should not be flagged as injection"
    );

    let ddl_sql = "CREATE TABLE test_table (id INT)";
    let guard = DdlGuard::new();
    let result = guard.validate(ddl_sql);
    assert!(
        matches!(result, Ok(DdlValidationResult::Allowed)),
        "legal DDL should pass the DdlGuard"
    );

    let dangerous_ddl = "DROP DATABASE production";
    let result = guard.validate(dangerous_ddl);
    assert!(
        matches!(result, Ok(DdlValidationResult::Forbidden(_))),
        "dangerous DDL should be blocked"
    );

    let injection_sql = "SELECT * FROM users WHERE id = 1 OR 1=1";
    assert!(
        contains_sql_injection(injection_sql),
        "SQL containing injection patterns should be detected"
    );
}
