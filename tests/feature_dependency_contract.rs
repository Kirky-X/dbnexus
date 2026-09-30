// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! feature 依赖闭包契约测试。
//!
//! 固化「sql-parser 与 cache/oxcache 解耦」的结构不变量：`sql-parser`
//! 作为独立解析能力不得传递拉入 `cache`（进而拉入 oxcache/moka），
//! 解析结果缓存由库内同步 LRU（prepare_cache 端口）承接。经
//! `cargo metadata` 读取真实 feature 依赖图断言，Cargo.toml 手改破坏
//! 解耦时此处红灯。

use serde_json::Value;

/// 读取 dbnexus 包的 feature 依赖表（`cargo metadata --no-deps --offline`，
/// 不触发编译也不访问网络）。
fn dbnexus_features() -> serde_json::Map<String, Value> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let output = std::process::Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--no-deps",
            "--offline",
            "--format-version",
            "1",
        ])
        .current_dir(manifest_dir)
        .output()
        .expect("cargo metadata 执行失败");
    assert!(
        output.status.success(),
        "cargo metadata 失败：{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let meta: Value =
        serde_json::from_slice(&output.stdout).expect("cargo metadata 输出非合法 JSON");
    let package = meta["packages"]
        .as_array()
        .expect("packages 数组")
        .iter()
        .find(|p| p["name"] == "dbnexus")
        .expect("workspace 内必有 dbnexus 包");
    package["features"]
        .as_object()
        .expect("features 表")
        .clone()
}

/// `sql-parser` 不得隐含 `cache`：解析能力是 permission/retry 等的公共底座，
/// 强拉 cache 会把 oxcache（及其 moka 后端）拖进所有上层闭包。
#[test]
fn sql_parser_feature_does_not_imply_cache() {
    let features = dbnexus_features();
    let sql_parser = features["sql-parser"]
        .as_array()
        .expect("sql-parser feature 依赖列表");
    let implies_cache = sql_parser.iter().any(|dep| {
        dep.as_str() == Some("cache") || dep.as_str().is_some_and(|d| d.starts_with("cache/"))
    });
    assert!(
        !implies_cache,
        "sql-parser 不得隐含 cache（解析结果缓存改由库内同步 LRU 承接），实际依赖：{sql_parser:?}"
    );
}

/// `permission` 显式携带 `cache`：权限路径的 DbPool 策略缓存直接使用
/// oxcache `Cache`（permission 门控代码），解耦后该依赖不再经 sql-parser
/// 传递获得，必须在 permission 闭包中显式成立。
#[test]
fn permission_feature_carries_cache_explicitly() {
    let features = dbnexus_features();
    let permission = features["permission"]
        .as_array()
        .expect("permission feature 依赖列表");
    assert!(
        permission.iter().any(|dep| dep.as_str() == Some("cache")),
        "permission 须显式携带 cache（DbPool 权限策略缓存依赖），实际依赖：{permission:?}"
    );
}

/// `global-index` 必须蕴含 `entity-macros`（sea-orm 派生宏转发）：实体
/// derive（`DeriveEntityModel`）依赖 sea-orm/macros，无驱动组合
/// （global-index / data-management / all-optional）不含任何驱动 feature，
/// 缺该蕴含即编译失败。
#[test]
fn global_index_implies_entity_macros() {
    let features = dbnexus_features();
    let global_index = features["global-index"]
        .as_array()
        .expect("global-index feature 依赖列表");
    assert!(
        global_index
            .iter()
            .any(|dep| dep.as_str() == Some("entity-macros")),
        "global-index 须蕴含 entity-macros（无驱动组合的实体 derive 依赖），实际依赖：{global_index:?}"
    );
}
