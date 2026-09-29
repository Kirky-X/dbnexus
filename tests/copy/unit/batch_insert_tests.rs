// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! batch_insert 多值 INSERT 构建契约测试
//!
//! - 构建层（纯函数）：占位符两种风格（`?` / `$N`）、500 行分块、参数扁平化
//!   顺序、空集/行列不匹配/注入标识符的显性错误。
//! - 执行层（sqlite）：构建产物对接 `Session::execute_with_params` 参数化
//!   落库——值按字面量绑定（注入向量不逃逸），类型 round-trip 可查。
//!
//! 消费契约（供 inklog 等批量落库方）：`chunk_rows` 产出的
//! `(sql, params)` 序列逐条送入 `execute_with_params` 即为完整批量写入。

#![cfg(all(
    feature = "runtime-tokio-rustls",
    feature = "sqlite",
    feature = "copy",
    feature = "sql-parser"
))]

use dbnexus::database::copy::{
    BATCH_INSERT_CHUNK_SIZE, BatchInsertStatement, PlaceholderStyle, bind_param_limit,
};

fn temp_db_url(tag: &str) -> (String, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!(
        "dbnexus_batch_insert_{tag}_{}.db",
        std::process::id()
    ));
    (format!("sqlite:{}?mode=rwc", path.display()), path)
}

// ============================================================================
// 分块常量（对齐 global_index 的 500 分块范式）
// ============================================================================

#[test]
fn test_batch_insert_chunk_size_matches_global_index_paradigm() {
    assert_eq!(
        BATCH_INSERT_CHUNK_SIZE, 500,
        "多值 INSERT 分块应与 global_index 的 BATCH_SYNC_CHUNK_SIZE 同为 500"
    );
}

// ============================================================================
// build：多值 VALUES 子句构建
// ============================================================================

#[test]
fn test_build_qmark_single_row() {
    let stmt = BatchInsertStatement::new("t_logs", &["id".to_string(), "msg".to_string()])
        .expect("合法标识符应通过");
    let sql = stmt
        .build(1, PlaceholderStyle::QMark)
        .expect("单行构建应成功");
    assert_eq!(
        sql, r#"INSERT INTO "t_logs" ("id", "msg") VALUES (?, ?)"#,
        "单行多值 INSERT 应为 (?, ?)，实际: {sql}"
    );
}

#[test]
fn test_build_qmark_multi_rows() {
    let stmt = BatchInsertStatement::new("t", &["a".to_string(), "b".to_string()])
        .expect("合法标识符应通过");
    let sql = stmt
        .build(3, PlaceholderStyle::QMark)
        .expect("多行构建应成功");
    assert_eq!(
        sql, r#"INSERT INTO "t" ("a", "b") VALUES (?, ?), (?, ?), (?, ?)"#,
        "三行 VALUES 子句应逗号分隔，实际: {sql}"
    );
}

#[test]
fn test_build_dollar_placeholders_number_continuously_across_rows() {
    let stmt = BatchInsertStatement::new("t", &["a".to_string(), "b".to_string()])
        .expect("合法标识符应通过");
    let sql = stmt
        .build(2, PlaceholderStyle::Dollar)
        .expect("Dollar 构建应成功");
    assert_eq!(
        sql, r#"INSERT INTO "t" ("a", "b") VALUES ($1, $2), ($3, $4)"#,
        "$N 编号应跨行连续，实际: {sql}"
    );
}

#[test]
fn test_build_zero_rows_is_error() {
    let stmt = BatchInsertStatement::new("t", &["a".to_string()]).expect("合法标识符应通过");
    let err = stmt
        .build(0, PlaceholderStyle::QMark)
        .expect_err("空 VALUES 子句无意义，应显性报错");
    assert!(
        format!("{err}").contains("row"),
        "错误应说明行数非法，实际: {err}"
    );
}

#[test]
fn test_build_rejects_injection_identifiers() {
    // 表名/列名与 CopyStatement 同一白名单口径，注入在构建期关闭
    assert!(BatchInsertStatement::new("t; DROP TABLE x", &["a".to_string()]).is_err());
    assert!(BatchInsertStatement::new("t", &["a; DROP TABLE x".to_string()]).is_err());
    assert!(BatchInsertStatement::new("", &["a".to_string()]).is_err());
    assert!(BatchInsertStatement::new("t", &[]).is_err(), "空列应拒绝");
}

// ============================================================================
// chunk_rows：500 分块 + 参数扁平化
// ============================================================================

#[test]
fn test_chunk_rows_empty_input_yields_no_statements() {
    let stmt = BatchInsertStatement::new("t", &["a".to_string()]).expect("合法标识符应通过");
    let chunks = stmt
        .chunk_rows(&[], PlaceholderStyle::QMark)
        .expect("空集是合法输入（调用方无事可做）");
    assert!(chunks.is_empty(), "空集应产出零条语句");
}

#[test]
fn test_chunk_rows_splits_at_chunk_size() {
    let stmt = BatchInsertStatement::new("t", &["n".to_string()]).expect("合法标识符应通过");
    // 501 行 → 2 条语句（500 + 1），正好覆盖分块边界
    let rows: Vec<Vec<serde_json::Value>> = (0..BATCH_INSERT_CHUNK_SIZE + 1)
        .map(|i| vec![serde_json::json!(i)])
        .collect();
    let chunks = stmt
        .chunk_rows(&rows, PlaceholderStyle::QMark)
        .expect("分块应成功");
    assert_eq!(chunks.len(), 2, "501 行应分为 500+1 两块");
    // 第一块一条语句承载 500 行（500 个占位符）；尾块单行
    let (sql0, params0) = &chunks[0];
    assert_eq!(sql0.matches('?').count(), 500, "首块语句应含 500 个占位符");
    assert_eq!(
        sql0,
        &stmt
            .build(BATCH_INSERT_CHUNK_SIZE, PlaceholderStyle::QMark)
            .expect("同参构建应一致"),
        "chunk_rows 产出的语句应与 build(500) 逐字节一致"
    );
    assert_eq!(params0.len(), 500, "第一块应含 500 个参数");
    assert_eq!(
        chunks[1].0, r#"INSERT INTO "t" ("n") VALUES (?)"#,
        "尾块应为单行语句"
    );
    let params1 = &chunks[1].1;
    assert_eq!(params1.len(), 1, "第二块应含 1 个参数");
    // 参数扁平化必须保持行序
    assert_eq!(params0[0], serde_json::json!(0));
    assert_eq!(params0[499], serde_json::json!(499));
    assert_eq!(params1[0], serde_json::json!(500));
}

#[test]
fn test_chunk_rows_flattens_row_major_order() {
    let stmt = BatchInsertStatement::new("t", &["a".to_string(), "b".to_string(), "c".to_string()])
        .expect("合法标识符应通过");
    let rows = vec![vec![
        serde_json::json!(1),
        serde_json::json!("x"),
        serde_json::json!(null),
    ]];
    let params = &stmt
        .chunk_rows(&rows, PlaceholderStyle::QMark)
        .expect("分块应成功")[0]
        .1;
    assert_eq!(
        params,
        &vec![
            serde_json::json!(1),
            serde_json::json!("x"),
            serde_json::Value::Null
        ],
        "参数应按行内列序展开（行优先）"
    );
}

#[test]
fn test_chunk_rows_rejects_ragged_rows() {
    let stmt = BatchInsertStatement::new("t", &["a".to_string(), "b".to_string()])
        .expect("合法标识符应通过");
    let rows = vec![
        vec![serde_json::json!(1), serde_json::json!(2)],
        vec![serde_json::json!(3)], // 行宽 1 ≠ 列数 2
    ];
    let err = stmt
        .chunk_rows(&rows, PlaceholderStyle::QMark)
        .expect_err("行列数不匹配必须显性报错而非静默截断");
    let msg = format!("{err}");
    assert!(
        msg.contains("2") && msg.contains("1"),
        "错误应含期望列数与实际列数，实际: {msg}"
    );
    // 过宽同样拒绝
    let rows = vec![vec![
        serde_json::json!(1),
        serde_json::json!(2),
        serde_json::json!(3),
    ]];
    assert!(
        stmt.chunk_rows(&rows, PlaceholderStyle::QMark).is_err(),
        "行宽超出列数同样应拒绝"
    );
}

#[test]
fn test_chunk_rows_shrinks_chunk_size_for_wide_rows_qmark() {
    // 70 列宽表：QMark 上限 32766，32766/70 = 468 → 有效块行数收缩为 468（<500）
    let cols: Vec<String> = (0..70).map(|i| format!("c{i}")).collect();
    let width = cols.len();
    let stmt = BatchInsertStatement::new("t_wide", &cols).expect("合法标识符应通过");
    let rows: Vec<Vec<serde_json::Value>> = (0..1000)
        .map(|i| (0..width).map(|_| serde_json::json!(i)).collect())
        .collect();
    let chunks = stmt
        .chunk_rows(&rows, PlaceholderStyle::QMark)
        .expect("宽表分块应成功");

    let limit = bind_param_limit(PlaceholderStyle::QMark);
    let expected_chunk = BATCH_INSERT_CHUNK_SIZE.min(limit / width);
    assert_eq!(expected_chunk, 468, "32766/70 应收缩为 468");
    assert_eq!(
        chunks.len(),
        1000_usize.div_ceil(expected_chunk),
        "块数按收缩后大小计算"
    );
    for (sql, params) in &chunks {
        let placeholders = sql.matches('?').count();
        assert!(
            placeholders <= limit,
            "单语句占位符数不得超过后端绑定上限 {limit}，实际: {placeholders}"
        );
        assert_eq!(placeholders, params.len(), "占位符数与参数数一致");
    }
    let total: usize = chunks.iter().map(|(_, p)| p.len()).sum();
    assert_eq!(total, 1000 * width, "分块不得丢数据");
    assert_eq!(
        chunks[0].1.len(),
        expected_chunk * width,
        "首块应按收缩后大小满载"
    );
}

#[test]
fn test_chunk_rows_shrinks_chunk_size_for_wide_rows_dollar() {
    // 200 列宽表：Dollar 上限 65535，65535/200 = 327 → 有效块行数收缩为 327
    let cols: Vec<String> = (0..200).map(|i| format!("c{i}")).collect();
    let width = cols.len();
    let stmt = BatchInsertStatement::new("t_wide_d", &cols).expect("合法标识符应通过");
    let rows: Vec<Vec<serde_json::Value>> = (0..3000)
        .map(|i| (0..width).map(|_| serde_json::json!(i)).collect())
        .collect();
    let chunks = stmt
        .chunk_rows(&rows, PlaceholderStyle::Dollar)
        .expect("宽表分块应成功");

    let limit = bind_param_limit(PlaceholderStyle::Dollar);
    let expected_chunk = BATCH_INSERT_CHUNK_SIZE.min(limit / width);
    assert_eq!(expected_chunk, 327, "65535/200 应收缩为 327");
    for (sql, _) in &chunks {
        // $N 编号跨行连续，语句内最大编号即占位符总数，必须不超后端上限
        let max_no: usize = sql
            .split('$')
            .skip(1)
            .filter_map(|seg| seg.split(|c: char| !c.is_ascii_digit()).next())
            .filter_map(|d| d.parse::<usize>().ok())
            .max()
            .unwrap_or(0);
        assert!(
            max_no <= limit,
            "Dollar 编号不得超过 {limit}，实际 {max_no}"
        );
    }
    let total: usize = chunks.iter().map(|(_, p)| p.len()).sum();
    assert_eq!(total, 3000 * width, "分块不得丢数据");
}

#[test]
fn test_chunk_rows_rejects_width_over_bind_param_limit() {
    // 列数超过后端单语句绑定上限时，任何行数的语句都无法安全执行：
    // 构建期显性报错而非产出必败语句
    let width = bind_param_limit(PlaceholderStyle::QMark) + 1;
    let cols: Vec<String> = (0..width).map(|i| format!("c{i}")).collect();
    let stmt = BatchInsertStatement::new("t_hyper_wide", &cols).expect("合法标识符应通过");
    let row: Vec<serde_json::Value> = (0..width).map(|_| serde_json::json!(1)).collect();
    let err = stmt
        .chunk_rows(&[row], PlaceholderStyle::QMark)
        .expect_err("列数超上限必须构建期报错而非产出必败语句");
    let msg = format!("{err}");
    assert!(
        msg.contains("column") && msg.contains("placeholder"),
        "错误应说明列数与绑定上限矛盾，实际: {msg}"
    );
}

// ============================================================================
// chunk_rows_owned：消费所有权变体（零深拷贝扁平化）
// ============================================================================

#[test]
fn test_chunk_rows_owned_matches_borrowed_output() {
    let stmt =
        BatchInsertStatement::new("t", &["a".to_string(), "b".to_string()]).expect("合法标识符");
    // 1207 行跨块：owned 版产出必须与借用版逐字节/逐值一致
    let rows: Vec<Vec<serde_json::Value>> = (0..1207)
        .map(|i| vec![serde_json::json!(i), serde_json::json!(format!("v{i}"))])
        .collect();
    let borrowed = stmt
        .chunk_rows(&rows, PlaceholderStyle::QMark)
        .expect("借用版分块应成功");
    let owned = stmt
        .chunk_rows_owned(rows, PlaceholderStyle::QMark)
        .expect("所有权版分块应成功");
    assert_eq!(borrowed, owned, "所有权版产出应与借用版完全一致");
    assert_eq!(owned.len(), 3, "1207 行应分为 500+500+207 三块");
}

#[test]
fn test_chunk_rows_owned_moves_json_trees_without_clone() {
    // 值相等性保证 move 语义不改变数据；大 JSON 树（对象/数组列）是
    // 深克隆成本的主体，此处验证其经 owned 路径后内容无损
    let stmt =
        BatchInsertStatement::new("t", &["id".to_string(), "doc".to_string()]).expect("合法标识符");
    let docs: Vec<serde_json::Value> = (0..4)
        .map(|i| serde_json::json!({"id": i, "nested": {"arr": [i, i + 1, i + 2]}}))
        .collect();
    let rows: Vec<Vec<serde_json::Value>> = docs
        .iter()
        .enumerate()
        .map(|(i, d)| vec![serde_json::json!(i), d.clone()])
        .collect();
    let owned = stmt
        .chunk_rows_owned(rows, PlaceholderStyle::QMark)
        .expect("所有权版分块应成功");
    let flat: Vec<&serde_json::Value> = owned[0].1.iter().collect();
    assert_eq!(flat[0], &serde_json::json!(0));
    assert_eq!(flat[1], &docs[0], "JSON 树应按 move 原样搬运");
    assert_eq!(flat[3], &docs[1]);
}

#[test]
fn test_chunk_rows_owned_rejects_ragged_rows_before_consuming() {
    let stmt =
        BatchInsertStatement::new("t", &["a".to_string(), "b".to_string()]).expect("合法标识符");
    let rows = vec![
        vec![serde_json::json!(1), serde_json::json!(2)],
        vec![serde_json::json!(3)],
    ];
    let err = stmt
        .chunk_rows_owned(rows, PlaceholderStyle::QMark)
        .expect_err("行列数不匹配必须整体报错");
    assert!(
        format!("{err}").contains("row"),
        "错误应说明行宽非法，实际: {err}"
    );
    // 宽表收缩与超限拒绝对 owned 版同契约
    let cols: Vec<String> = (0..70).map(|i| format!("c{i}")).collect();
    let wide = BatchInsertStatement::new("t_w", &cols).expect("合法标识符");
    let rows: Vec<Vec<serde_json::Value>> = (0..1000)
        .map(|i| (0..70).map(|_| serde_json::json!(i)).collect())
        .collect();
    let owned = wide
        .chunk_rows_owned(rows, PlaceholderStyle::QMark)
        .expect("宽表 owned 分块应成功");
    assert_eq!(owned[0].1.len(), 468 * 70, "首块应按收缩后大小满载");
}

// ============================================================================
// 执行层（sqlite）：构建产物经 execute_with_params 参数化落库
// ============================================================================

#[tokio::test]
async fn test_batch_insert_roundtrip_types_via_execute_with_params() {
    let (url, path) = temp_db_url("roundtrip");
    let pool = dbnexus::DbPool::new(&url).await.expect("pool");
    let session = pool.get_session("admin").await.expect("admin session");

    session
        .execute_raw_ddl("CREATE TABLE t_evt (id INTEGER PRIMARY KEY, score REAL, label TEXT, flag BOOLEAN, extra TEXT)")
        .await
        .expect("create table");

    let stmt = BatchInsertStatement::new(
        "t_evt",
        &[
            "id".to_string(),
            "score".to_string(),
            "label".to_string(),
            "flag".to_string(),
            "extra".to_string(),
        ],
    )
    .expect("合法标识符应通过");
    let rows = vec![
        vec![
            serde_json::json!(1),
            serde_json::json!(2.5),
            serde_json::json!("hello"),
            serde_json::json!(true),
            serde_json::json!({"k": 1}),
        ],
        vec![
            serde_json::json!(2),
            serde_json::Value::Null,
            serde_json::Value::Null,
            serde_json::json!(false),
            serde_json::json!([1, 2]),
        ],
    ];
    let chunks = stmt
        .chunk_rows(&rows, PlaceholderStyle::QMark)
        .expect("分块应成功");
    assert_eq!(chunks.len(), 1);
    for (sql, params) in &chunks {
        session
            .execute_with_params(sql, params)
            .await
            .expect("参数化执行应成功");
    }

    let result = session
        .query_rows("SELECT id, score, label, flag, extra FROM t_evt ORDER BY id")
        .await
        .expect("查询应成功");
    assert_eq!(result.len(), 2, "两行应全部落库");
    assert_eq!(result[0]["id"], serde_json::json!(1));
    assert_eq!(result[0]["score"], serde_json::json!(2.5));
    assert_eq!(result[0]["label"], serde_json::json!("hello"));
    assert_eq!(
        result[0]["flag"],
        serde_json::json!(1),
        "sqlite 布尔以 0/1 存储"
    );
    assert_eq!(result[0]["extra"], serde_json::json!(r#"{"k":1}"#));
    assert_eq!(
        result[1]["score"],
        serde_json::Value::Null,
        "NULL 应落为 NULL"
    );
    assert_eq!(result[1]["flag"], serde_json::json!(0));

    drop(session);
    drop(pool);
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn test_batch_insert_binds_injection_payload_as_literal() {
    let (url, path) = temp_db_url("injection");
    let pool = dbnexus::DbPool::new(&url).await.expect("pool");
    let session = pool.get_session("admin").await.expect("admin session");

    session
        .execute_raw_ddl("CREATE TABLE t_msg (id INTEGER PRIMARY KEY, msg TEXT)")
        .await
        .expect("create table");

    let stmt =
        BatchInsertStatement::new("t_msg", &["id".to_string(), "msg".to_string()]).expect("ok");
    let injection = "x'); DROP TABLE t_msg;--";
    let rows = vec![vec![serde_json::json!(1), serde_json::json!(injection)]];
    for (sql, params) in stmt
        .chunk_rows(&rows, PlaceholderStyle::QMark)
        .expect("分块应成功")
    {
        session
            .execute_with_params(&sql, &params)
            .await
            .expect("参数化执行应成功");
    }

    // 注入载荷按字面量落库，表未被 drop
    let result = session
        .query_rows("SELECT id, msg FROM t_msg")
        .await
        .expect("表应仍存在且可查");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["msg"], serde_json::json!(injection));

    drop(session);
    drop(pool);
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn test_batch_insert_large_batch_crossing_chunk_boundary() {
    let (url, path) = temp_db_url("large");
    let pool = dbnexus::DbPool::new(&url).await.expect("pool");
    let session = pool.get_session("admin").await.expect("admin session");

    session
        .execute_raw_ddl("CREATE TABLE t_bulk (id INTEGER PRIMARY KEY, val TEXT)")
        .await
        .expect("create table");

    let stmt =
        BatchInsertStatement::new("t_bulk", &["id".to_string(), "val".to_string()]).expect("ok");
    // 1207 行 = 2×500 + 207：覆盖整块与尾块
    let rows: Vec<Vec<serde_json::Value>> = (0..1207)
        .map(|i| vec![serde_json::json!(i), serde_json::json!(format!("v{i}"))])
        .collect();
    let chunks = stmt
        .chunk_rows(&rows, PlaceholderStyle::QMark)
        .expect("分块应成功");
    assert_eq!(chunks.len(), 3, "1207 行应分为 500+500+207 三块");
    for (sql, params) in &chunks {
        session
            .execute_with_params(sql, params)
            .await
            .expect("参数化执行应成功");
    }

    // query_rows 的 sqlite 臂做主表列内省（聚合列不在内省列表），
    // 计数断言改用全行主键查询：行数 + 首尾行同时验证
    let all = session
        .query_rows("SELECT id FROM t_bulk ORDER BY id")
        .await
        .expect("全行查询应成功");
    assert_eq!(all.len(), 1207, "全部行应落库");
    assert_eq!(all[0]["id"], serde_json::json!(0));
    assert_eq!(all[1206]["id"], serde_json::json!(1206), "尾块边界行应落库");
    let tail = session
        .query_rows("SELECT val FROM t_bulk WHERE id = 1206")
        .await
        .expect("尾行查询应成功");
    assert_eq!(tail[0]["val"], serde_json::json!("v1206"));

    drop(session);
    drop(pool);
    let _ = std::fs::remove_file(&path);
}
