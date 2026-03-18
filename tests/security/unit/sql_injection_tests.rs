// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! SQL 注入检测测试套件
//!
//! 专门测试小写变体的注入模式，验证 `contains_sql_injection` 函数的大小写不敏感行为。

use dbnexus::sql_parser::contains_sql_injection;

// ============================================================================
// 小写变体测试 - 任务 1.4 要求
// ============================================================================

/// 测试堆叠查询注入的小写变体（; DROP TABLE 等）
#[test]
fn test_stacked_queries_lowercase() {
    // ; drop table (小写)
    assert!(
        contains_sql_injection("select * from users; drop table users"),
        "应该检测到小写的 '; drop table'"
    );
    // ; delete from (小写)
    assert!(
        contains_sql_injection("select * from users; delete from users"),
        "应该检测到小写的 '; delete from'"
    );
    // ; update (小写)
    assert!(
        contains_sql_injection("select * from users; update users set admin = 1"),
        "应该检测到小写的 '; update'"
    );
    // ; insert (小写)
    assert!(
        contains_sql_injection("select * from users; insert into users values (1, 'hacker')"),
        "应该检测到小写的 '; insert'"
    );
    // ; truncate (小写)
    assert!(
        contains_sql_injection("select * from users; truncate table users"),
        "应该检测到小写的 '; truncate'"
    );
    // ; alter (小写)
    assert!(
        contains_sql_injection("select * from users; alter table users add column hacked int"),
        "应该检测到小写的 '; alter'"
    );
    // ; create (小写)
    assert!(
        contains_sql_injection("select * from users; create table hacked (id int)"),
        "应该检测到小写的 '; create'"
    );
}

/// 测试 UNION 注入的小写变体
#[test]
fn test_union_injection_lowercase() {
    // union select (小写)
    assert!(
        contains_sql_injection("select * from users union select * from admin"),
        "应该检测到小写的 'union select'"
    );
    // union all select (小写)
    assert!(
        contains_sql_injection("select * from users union all select * from admin"),
        "应该检测到小写的 'union all select'"
    );
    // union distinct select (小写)
    assert!(
        contains_sql_injection("select * from users union distinct select password from admin"),
        "应该检测到小写的 'union distinct select'"
    );
}

/// 测试布尔盲注的小写变体
#[test]
fn test_boolean_blind_lowercase() {
    // or 1=1 (小写)
    assert!(
        contains_sql_injection("select * from users where id = 1 or 1=1"),
        "应该检测到小写的 'or 1=1'"
    );
    // or true (小写)
    assert!(
        contains_sql_injection("select * from users where id = 1 or true"),
        "应该检测到小写的 'or true'"
    );
    // or false (小写)
    assert!(
        contains_sql_injection("select * from users where id = 1 or false"),
        "应该检测到小写的 'or false'"
    );
    // and 1=1 (小写)
    assert!(
        contains_sql_injection("select * from users where id = 1 and 1=1"),
        "应该检测到小写的 'and 1=1'"
    );
    // and true (小写)
    assert!(
        contains_sql_injection("select * from users where id = 1 and true"),
        "应该检测到小写的 'and true'"
    );
}

/// 测试时间盲注的小写变体（跨数据库）
#[test]
fn test_time_blind_lowercase() {
    // MySQL: sleep( (小写)
    assert!(
        contains_sql_injection("select * from users where id = 1 and sleep(5)"),
        "应该检测到小写的 'sleep('"
    );
    // MySQL: benchmark( (小写)
    assert!(
        contains_sql_injection("select * from users where id = 1 and benchmark(10000000,sha1('test'))"),
        "应该检测到小写的 'benchmark('"
    );
    // PostgreSQL: pg_sleep( (小写)
    assert!(
        contains_sql_injection("select * from users where id = 1 and pg_sleep(5)"),
        "应该检测到小写的 'pg_sleep('"
    );
    // SQL Server: waitfor delay (小写)
    assert!(
        contains_sql_injection("waitfor delay '0:0:5'"),
        "应该检测到小写的 'waitfor delay'"
    );
    // SQL Server: waitfor time (小写)
    assert!(
        contains_sql_injection("waitfor time '12:00:00'"),
        "应该检测到小写的 'waitfor time'"
    );
}

/// 测试动态 SQL 执行的小写变体
#[test]
fn test_dynamic_sql_lowercase() {
    // exec( (小写)
    assert!(
        contains_sql_injection("exec('drop table users')"),
        "应该检测到小写的 'exec('"
    );
    // execute( (小写)
    assert!(
        contains_sql_injection("execute('select * from users')"),
        "应该检测到小写的 'execute('"
    );
    // sp_executesql (小写)
    assert!(
        contains_sql_injection("sp_executesql n'select * from users'"),
        "应该检测到小写的 'sp_executesql'"
    );
    // xp_cmdshell (小写)
    assert!(
        contains_sql_injection("xp_cmdshell 'dir'"),
        "应该检测到小写的 'xp_cmdshell'"
    );
}

/// 测试文件操作的小写变体
#[test]
fn test_file_operations_lowercase() {
    // load_file( (小写)
    assert!(
        contains_sql_injection("select load_file('/etc/passwd')"),
        "应该检测到小写的 'load_file('"
    );
    // into outfile (小写)
    assert!(
        contains_sql_injection("select * from users into outfile '/tmp/users.txt'"),
        "应该检测到小写的 'into outfile'"
    );
    // into dumpfile (小写)
    assert!(
        contains_sql_injection("select * from users into dumpfile '/tmp/users.txt'"),
        "应该检测到小写的 'into dumpfile'"
    );
}

/// 测试信息泄露的小写变体
#[test]
fn test_info_disclosure_lowercase() {
    // information_schema (小写)
    assert!(
        contains_sql_injection("select * from information_schema.tables"),
        "应该检测到小写的 'information_schema'"
    );
    // sysobjects (小写)
    assert!(
        contains_sql_injection("select * from sysobjects"),
        "应该检测到小写的 'sysobjects'"
    );
    // sys.tables (小写)
    assert!(
        contains_sql_injection("select * from sys.tables"),
        "应该检测到小写的 'sys.tables'"
    );
    // mysql.user (小写)
    assert!(
        contains_sql_injection("select * from mysql.user"),
        "应该检测到小写的 'mysql.user'"
    );
    // pg_user (小写)
    assert!(
        contains_sql_injection("select * from pg_user"),
        "应该检测到小写的 'pg_user'"
    );
    // pg_shadow (小写)
    assert!(
        contains_sql_injection("select * from pg_shadow"),
        "应该检测到小写的 'pg_shadow'"
    );
}

/// 测试编码绕过的小写变体
#[test]
fn test_encoding_bypass_lowercase() {
    // char( (小写)
    assert!(
        contains_sql_injection("select * from users where name = char(97,100,109,105,110)"),
        "应该检测到小写的 'char('"
    );
    // chr( (小写)
    assert!(
        contains_sql_injection("select * from users where name = chr(65)"),
        "应该检测到小写的 'chr('"
    );
    // concat( (小写)
    assert!(
        contains_sql_injection("select * from users where name = concat('ad','min')"),
        "应该检测到小写的 'concat('"
    );
    // concat_ws( (小写)
    assert!(
        contains_sql_injection("select concat_ws(',', 'a', 'b', 'c')"),
        "应该检测到小写的 'concat_ws('"
    );
}

/// 测试注释注入
#[test]
fn test_comment_injection() {
    // -- 注释（移除后面内容）
    assert!(
        contains_sql_injection("select * from users where id = 1 -- "),
        "应该检测到 '-- ' 注释注入"
    );
    assert!(
        contains_sql_injection("select * from users where id = 1 --+"),
        "应该检测到 '--+' 注释注入"
    );
    // # 注释 (MySQL)
    assert!(
        contains_sql_injection("select * from users where id = 1 #"),
        "应该检测到 '#' 注释注入"
    );
    // /* */ 块注释
    assert!(
        contains_sql_injection("select * /* comment */ from users"),
        "应该检测到 '/* ' 块注释注入"
    );
}

/// 测试混合大小写的注入模式
#[test]
fn test_mixed_case_injection() {
    // UnIoN SeLeCt (混合大小写)
    assert!(
        contains_sql_injection("select * from users UnIoN SeLeCt * from admin"),
        "应该检测到混合大小写的 'UnIoN SeLeCt'"
    );
    // ; DrOp TaBlE (混合大小写)
    assert!(
        contains_sql_injection("select * from users; DrOp TaBlE users"),
        "应该检测到混合大小写的 '; DrOp TaBlE'"
    );
    // InFoRmAtIoN_ScHeMa (混合大小写)
    assert!(
        contains_sql_injection("select * from InFoRmAtIoN_ScHeMa.TaBlEs"),
        "应该检测到混合大小写的 'InFoRmAtIoN_ScHeMa'"
    );
    // SlEeP( (混合大小写)
    assert!(
        contains_sql_injection("select * from users where id = 1 and SlEeP(5)"),
        "应该检测到混合大小写的 'SlEeP('"
    );
}

/// 测试正常 SQL 的小写形式不被误报
#[test]
fn test_false_positives_lowercase() {
    // 正常的 select 语句（小写）
    assert!(
        !contains_sql_injection("select id, name from users where id = 1"),
        "正常的 SELECT 不应被检测为注入"
    );
    // 正常的 insert 语句（小写）
    assert!(
        !contains_sql_injection("insert into users (name, email) values ('test', 'test@example.com')"),
        "正常的 INSERT 不应被检测为注入"
    );
    // 正常的 update 语句（小写）
    assert!(
        !contains_sql_injection("update users set name = 'new_name' where id = 1"),
        "正常的 UPDATE 不应被检测为注入"
    );
    // 正常的 delete 语句（小写）
    assert!(
        !contains_sql_injection("delete from users where id = 1"),
        "正常的 DELETE 不应被检测为注入"
    );
    // 正常的 join 操作（小写）
    assert!(
        !contains_sql_injection("select u.name, o.order_id from users u join orders o on u.id = o.user_id"),
        "正常的 JOIN 不应被检测为注入"
    );
}

// ============================================================================
// DDL Guard 集成测试 - 验证 DdlGuard 与 SQL 注入检测的协同工作
// ============================================================================

#[cfg(feature = "sql-parser")]
use dbnexus::security::{DdlGuard, DdlValidationResult};

/// 测试 DDL Guard 与 SQL 注入检测的边界
#[cfg(feature = "sql-parser")]
#[test]
fn test_ddl_guard_vs_sql_injection() {
    let guard = DdlGuard::new();

    // 合法 DDL 应该通过 DDL Guard
    let result = guard.validate("CREATE TABLE users (id INT PRIMARY KEY)");
    assert!(
        matches!(result, Ok(DdlValidationResult::Allowed)),
        "合法的 CREATE TABLE 应该通过 DDL Guard"
    );

    // DDL Guard 禁止的操作
    let result = guard.validate("DROP TABLE users");
    assert!(
        matches!(result, Ok(DdlValidationResult::Forbidden(_))),
        "DROP TABLE 应该被 DDL Guard 拒绝"
    );

    // SQL 注入检测应该独立工作
    // 注意: contains_sql_injection 是针对 DML 的注入检测，
    // 对于 DDL 语句，应该通过 DdlGuard 进行验证
}

/// 测试完整的 SQL 安全验证流程
#[cfg(feature = "sql-parser")]
#[test]
fn test_complete_sql_security_flow() {
    // 1. 首先检查 SQL 注入（适用于 DML）
    let safe_sql = "SELECT * FROM users WHERE id = 1";
    assert!(!contains_sql_injection(safe_sql), "安全的 SQL 不应被标记为注入");

    // 2. DDL 操作应该通过 DdlGuard
    let ddl_sql = "CREATE TABLE test_table (id INT)";
    let guard = DdlGuard::new();
    let result = guard.validate(ddl_sql);
    assert!(
        matches!(result, Ok(DdlValidationResult::Allowed)),
        "合法 DDL 应该通过 DdlGuard"
    );

    // 3. 危险的 DDL 应该被拦截
    let dangerous_ddl = "DROP DATABASE production";
    let result = guard.validate(dangerous_ddl);
    assert!(
        matches!(result, Ok(DdlValidationResult::Forbidden(_))),
        "危险的 DDL 应该被拦截"
    );

    // 4. 包含注入模式的 DML 应该被检测
    let injection_sql = "SELECT * FROM users WHERE id = 1 OR 1=1";
    assert!(contains_sql_injection(injection_sql), "包含注入模式的 SQL 应该被检测");
}
