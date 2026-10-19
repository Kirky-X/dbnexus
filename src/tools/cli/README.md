# dbnexus-cli

[dbnexus](https://github.com/Kirky-X/dbnexus) 仓库的命令行子 crate（目录 `dbnexus/src/tools/cli`），产出二进制 `dbnexus-cli`。

## 与父 crate 的关系

| 项 | 内容 |
|---|---|
| 所属仓库 | [dbnexus](https://github.com/Kirky-X/dbnexus)（[crates.io](https://crates.io/crates/dbnexus)） |
| 依赖方向 | 本 crate → `dbnexus`（作为库的消费方，不含库内部实现对 CLI 的反向依赖） |
| 是否 re-export | 否：独立二进制，不参与库的 re-export 面 |
| 适用场景 | 迁移管理、连接与池状态诊断、健康检查与权限自检等运维动作；应用代码不应依赖本 crate |

## 子命令

| 子命令 | 作用 | 特性门控 |
|---|---|---|
| `create <描述>` | 创建新的迁移文件 | `migration` |
| `up [--version N]` | 应用迁移 | `migration` |
| `down [--version N] [--all]` | 回滚迁移 | `migration` |
| `status` | 查看迁移状态 | `migration` |
| `generate` | 基于 schema 差异生成迁移文件 | `migration` |
| `list` | 列出所有迁移文件 | `migration` |
| `migrate [--version N]` | 应用全部待应用迁移（机器可读输出，退出码 0/1/2） | `migration` |
| `test-connection` | 测试数据库连接 | — |
| `health` | 健康检查（JSON 输出，退出码 0 健康 / 1 不健康 / 2 用法错误） | — |
| `user` | 管理员用户增删（`dbnexus_users` 表，JSON 输出） | — |
| `pool-status` | 连接池状态 | `health-check` |
| `audit-query` | 审计日志查询 | `audit` |
| `shard-info` | 分片信息 | `sharding` |
| `permission-check` | 权限判定自检 | `permission-engine` |

## 驱动特性互斥

数据库驱动必须且只能选一个，不可同时启用：

```bash
# SQLite（默认）
cargo run -p dbnexus-cli --features sqlite -- health

# PostgreSQL
cargo run -p dbnexus-cli --no-default-features --features postgres -- health

# MySQL
cargo run -p dbnexus-cli --no-default-features --features mysql -- health
```

## 全局参数

- `-m, --migrations-dir <DIR>`：迁移文件目录，默认 `./migrations`（可置于子命令前后任意位置）
- `--lang <en|zh>`：手动指定输出语言

## 版本与支持

- 版本经 `version.workspace = true` 随仓库统一，与 `dbnexus` 同号发布。
- 仅维护当前 minor 版本线，详见 [dbnexus 安全文档 · 支持的版本](https://github.com/Kirky-X/dbnexus/blob/main/docs/SECURITY.md#-支持的版本与维护策略)。

## 许可证

MIT
