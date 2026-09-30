// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 编译期 feature 互斥守卫契约测试。
//!
//! `src/lib.rs` 的互斥守卫依赖 rustc 的 `cfg` + `compile_error!` 语义，
//! 真正触发守卫需要实际启用冲突 feature 组合（其中 embedded 驱动为
//! bundled C++ 全量编译，代价极高），故以源码契约测试固化守卫的
//! 存在性与作用域：守卫被删改或作用域漂移时此处红灯。

/// 被 include 的 `src/lib.rs` 源码（编译期内嵌，随 lib 一起受 git 与评审约束）。
const LIB_RS: &str = include_str!("../src/lib.rs");

/// 提取 lib.rs 中全部 `compile_error!` 守卫，返回 `(cfg 条件文本, 报错消息)` 对。
fn compile_error_guards(src: &str) -> Vec<(String, String)> {
    fn matching_paren(bytes: &[u8], open: usize) -> usize {
        let mut depth = 0usize;
        for (i, &b) in bytes[open..].iter().enumerate() {
            match b {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return open + i;
                    }
                }
                _ => {}
            }
        }
        panic!("cfg 条件括号不配对");
    }

    let bytes = src.as_bytes();
    let mut guards = Vec::new();
    let mut search_from = 0usize;
    while let Some(rel) = src[search_from..].find("compile_error!(") {
        let abs = search_from + rel;
        let quote_open = src[abs..]
            .find('"')
            .unwrap_or_else(|| panic!("compile_error! 缺少消息字符串（偏移 {abs}）"))
            + abs;
        let quote_close = src[quote_open + 1..]
            .find('"')
            .unwrap_or_else(|| panic!("compile_error! 消息字符串未闭合（偏移 {quote_open}）"))
            + quote_open
            + 1;
        let message = src[quote_open + 1..quote_close].to_string();
        let cfg_open = src[..abs]
            .rfind("#[cfg(")
            .unwrap_or_else(|| panic!("compile_error!({message:?}) 前缺少 #[cfg(...)] 属性"));
        let cond_start = cfg_open + "#[cfg(".len();
        let cond_end = matching_paren(bytes, cfg_open + "#[cfg".len());
        guards.push((src[cond_start..cond_end].to_string(), message));
        search_from = quote_close;
    }
    guards
}

/// duckdb 与 ladybug 必须编译期互斥：两者均以 bundled 方式各自 vendor 一份
/// mbedtls 静态库（libduckdb-sys 的 `duckdb/third_party/mbedtls` 与 lbug 的
/// `lbug-src/third_party/mbedtls`，均以同名 `mbedtls` 静态库整档链接），
/// 同一二进制内链接必然重复符号，依赖层面无解，只能互斥。
#[test]
fn duckdb_and_ladybug_are_compile_time_mutex() {
    let guards = compile_error_guards(LIB_RS);
    let (cond, _) = guards
        .iter()
        .find(|(cond, msg)| {
            cond.contains("feature = \"duckdb\"")
                && cond.contains("feature = \"ladybug\"")
                && !msg.is_empty()
        })
        .expect("lib.rs 必须声明 duckdb 与 ladybug 的互斥 compile_error 守卫");

    assert!(
        cond.contains("all(") && cond.contains("not(clippy)"),
        "duckdb/ladybug 守卫须为 all(...) 组合条件且带 not(clippy) 门控，实际条件：{cond}"
    );
}

/// 守卫不得误伤合法组合：ladybug + sqlite（graph_ladybug 示例的编译组合）
/// 不 bundle 冲突的 mbedtls（libsqlite3-sys 无此依赖），必须保持可用。
#[test]
fn ladybug_sqlite_combo_stays_allowed() {
    let guards = compile_error_guards(LIB_RS);
    let mutex = guards
        .iter()
        .find(|(cond, _)| {
            cond.contains("feature = \"duckdb\"") && cond.contains("feature = \"ladybug\"")
        })
        .expect("duckdb/ladybug 互斥守卫必须存在");
    let (cond, _) = mutex;
    assert!(
        !cond.contains("feature = \"sqlite\""),
        "duckdb/ladybug 守卫条件不得波及 sqlite 组合，实际条件：{cond}"
    );
}

/// 全部互斥守卫统一带 `not(clippy)` 门控（与既有守卫约定一致）：
/// clippy/rustdoc 的全 feature 检查路径不受互斥守卫硬失败拖累。
#[test]
fn all_feature_mutex_guards_are_clippy_gated() {
    for (cond, message) in compile_error_guards(LIB_RS) {
        assert!(
            cond.contains("not(clippy)"),
            "compile_error!({message:?}) 的 cfg 条件缺少 not(clippy) 门控：{cond}"
        );
    }
}
