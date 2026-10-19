# dbnexus-macros

[dbnexus](https://github.com/Kirky-X/dbnexus) 仓库的过程宏子 crate（目录 `dbnexus/macros`）。

## 与父 crate 的关系

| 项 | 内容 |
|---|---|
| 所属仓库 | [dbnexus](https://github.com/Kirky-X/dbnexus)（[crates.io](https://crates.io/crates/dbnexus) · [docs.rs](https://docs.rs/dbnexus)） |
| 依赖方向 | `dbnexus` → `dbnexus-macros`（父 crate 以 `path + version` 双写依赖本 crate） |
| 是否 re-export | 是：父 crate 在 crate 根部导出 `db_entity`、`db_repository` 两个属性宏；`allowed_roles` / `allowed_operations` / `check_permission` / `cache_key` / `table_name` / `schema` 等为其内部辅助属性，不单独导出 |
| 适用场景 | 通常无需直接依赖；父 crate 的 `macros` 特性开启后即可使用导出的属性宏 |

## 提供的属性宏

| 宏 | 作用 |
|---|---|
| `#[db_entity]` | 由结构体生成表映射、主键/列描述、CRUD 与审计字段 |
| `#[db_repository]` | 由实体生成仓储层方法（查询、分页、软删除等） |

## 使用

经父 crate 使用（推荐）：

```rust
use dbnexus::{db_entity, db_repository};

#[db_entity(table = "users")]
struct User {
    id: i64,
    name: String,
}
```

## 版本与支持

- 版本经 `version.workspace = true` 随仓库统一，与 `dbnexus` 同号发布。
- 仅维护当前 minor 版本线，详见 [dbnexus 安全文档 · 支持的版本](https://github.com/Kirky-X/dbnexus/blob/main/docs/SECURITY.md#-支持的版本与维护策略)。

## 许可证

MIT
