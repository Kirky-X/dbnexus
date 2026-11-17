# 🤝 Dbnexus 贡献指南

感谢您对 Dbnexus 项目的关注！本文档描述参与开发所需的环境、流程与规范。

## 📋 目录

<details open>
<summary>📑 目录</summary>

- [🧰 开发环境](#-开发环境)
- [🔄 TDD 工作流](#-tdd-工作流)
- [🪝 Pre-commit Hooks](#-pre-commit-hooks)
- [🧪 代码质量](#-代码质量)
- [🔀 Pull Request 流程](#-pull-request-流程)
- [✍️ 代码风格](#️-代码风格)
- [📏 测试规范](#-测试规范)
- [📄 许可证](#-许可证)

</details>

---

## 🧰 开发环境

| 项 | 要求 |
|----|------|
| Rust | **1.97.1+**（`rust-toolchain.toml` 锁定，edition 2024） |
| Git | 配置了 pre-commit hooks |
| Docker | 可选，用于 PostgreSQL / MySQL 集成测试 |

### 初始化

```bash
git clone https://github.com/Kirky-X/dbnexus.git
cd dbnexus

# 安装 Git 钩子（二选一）
lefthook install
# 或：pip install pre-commit && pre-commit install

# 验证环境（嵌入式/服务器驱动互斥，禁用 --all-features：src/lib.rs compile_error! 守卫）
cargo build --no-default-features --features sqlite,default-no-db,all-optional
cargo test --no-default-features --features sqlite,default-no-db,all-optional --lib
```

---

## 🔄 TDD 工作流

本项目采用测试驱动开发（TDD），每个开发任务组必须遵循以下循环：

1. **定接口**：先定义 trait / API 签名（`trait Xxx { ... }`），不写实现
2. **写测试**：基于接口编写单元测试（`#[cfg(test)] mod tests { ... }`），此时测试应失败（red）
3. **写代码**：实现接口，使测试通过（green）
4. **跑测试**：`cargo test --features <对应特性> --lib`，确保所有测试通过
5. **commit**：通过后显式暂存改动文件并提交 `git commit -m "feat(<模块>): <描述>"`
6. **影响分析**：分析本次改动对其他模块的影响，识别需联动修改的代码
7. **继续下一个**：基于分析结果调整后续任务，再开始下一轮循环

---

## 🪝 Pre-commit Hooks

项目使用 pre-commit hooks（lefthook / pre-commit，二者功能等价）保证代码质量：

```bash
# 安装（二选一）
lefthook install
# 或：pip install pre-commit && pre-commit install

# 手动运行
pre-commit run --all-files
```

### 禁止事项

- **禁止使用 `--no-verify` 跳过** pre-commit hooks
- **禁止使用 `git commit -n`** 绕过检查
- 违反此规定视为质量红线事件

---

## 🧪 代码质量

贡献前请确保通过以下检查（与 CI 门禁一致）：

| 门禁 | 命令 |
|------|------|
| 格式 | `cargo fmt --all -- --check` |
| Lint | `cargo clippy --all-targets -- -D warnings` |
| 供应链 | `cargo deny check`、`cargo audit` |
| 覆盖率 | 行覆盖 ≥ 95%（CI 中 llvm-cov 统计） |
| 文档 | `cargo doc` 零告警 |

### 自动化审查工具

项目在开发流程中集成自动化审查工具：

| 工具 | 用途 | 典型场景 |
|------|------|----------|
| 代码质量审查 | 简化、架构优化、性能审查 | review / audit / tech debt / over-engineering / simplify；任意 write / add / refactor / fix 任务 |
| SAST 安全扫描 | 漏洞与注入检测 | 安全审查 / 漏洞扫描；hardcoded secrets / SQL injection / unsafe eval；发布前安全检查 |
| kueiku | 方法论框架：根因分析与结构化思考 | 分析 / 策略 / 决策 / 根因分析 |

---

## 🔀 Pull Request 流程

1. **创建 feature 分支**

   ```bash
   git checkout -b feat/<your-feature>
   # 或
   git checkout -b fix/<bug-description>
   ```

2. **确保 pre-commit hooks 通过**

   ```bash
   pre-commit run --all-files
   ```

3. **确保测试通过**

   ```bash
   cargo test --no-default-features --features sqlite,default-no-db,all-optional
   cargo clippy --no-default-features --features sqlite,default-no-db,all-optional -- -D warnings
   cargo fmt --all -- --check
   ```

   > 嵌入式与服务器端驱动互斥，禁用 `--all-features`（`src/lib.rs` 的 `compile_error!` 守卫会强制编译失败）。

4. **提交 PR**
   - 标题格式：Conventional Commits（`feat(<模块>): <描述>` 或 `fix(<模块>): <描述>`，commit-msg 钩子校验）
   - 描述变更内容和原因
   - 关联相关 issue

---

## ✍️ 代码风格

### 通用规范

- **遵循现有代码库命名和架构惯例**（即使你认为自己的写法更好）
- **依赖必须通过 feature 门控**，不引入未门控的依赖
- **edition 2024**，Rust 1.97.1
- **MIT License**

### Rust 规范

- 命名：`snake_case`（函数/变量/模块），`CamelCase`（类型/Trait）
- 错误处理：使用 `Result<T, E>` 返回错误，**严禁吞掉错误**
- **禁止 `unsafe` 代码**（`#![forbid(unsafe_code)]`）
- **必须有文档注释**（`#![deny(missing_docs)]`）

### Feature 门控规范

```toml
# Cargo.toml 中正确门控示例
[features]
my-feature = ["dep:some-crate"]

[dependencies]
some-crate = { version = "1.0", optional = true }
```

```rust
// src/lib.rs 中正确门控示例
#[cfg(feature = "my-feature")]
pub mod my_module;
```

---

## 📏 测试规范

- 测试要验证**正确行为的有意义属性**（值、结构、副作用、错误类型）
- 不仅验证"函数有返回值"或"不报错"
- "所有测试通过"是必要条件但非充分条件
- 测试太弱时要明确指出

场景定义、驱动组矩阵与静态门槛详见 [🧪 测试场景固化](TEST_SCENARIOS.md)。

### 运行测试

```bash
# 单元测试
cargo test --features sqlite --lib

# 集成测试
cargo test --features sqlite

# 全部测试（CI 标准特性组合）
cargo test --no-default-features --features sqlite,default-no-db,all-optional

# 特定数据库后端
cargo test --features postgres
cargo test --features mysql
```

> 嵌入式与服务器端驱动在编译期严格互斥，多驱动验证请按分组特性组合运行（见 [测试场景固化](TEST_SCENARIOS.md#驱动组组合矩阵)）。

---

## 📄 许可证

提交的代码将采用 MIT 许可证，与项目保持一致。

---

## 🗺️ 开发路线图

### 短期（0.6.0 正式发布）

- [x] 发布 0.6.0 正式版 — 已发布（tag v0.6.0，2026-11-14；trait-kit 0.5 / oxcache 0.5 依赖要求已按稳定版传导更新），后续 0.6.1 已于 2026-11-15 发布
- [x] 对齐 MSRV 声明与依赖实际要求 — 已按工作区 CONFIG_BASELINE 统一为 1.97.1（2026-09-06），覆盖传递依赖的 1.94 要求
- [x] 恢复 MySQL 集成测试的常规运行 — CI `test` job 以 matrix `db: [sqlite, postgres, mysql]` 常规运行：MySQL 8.0 service 容器（mysqladmin 健康检查就绪后注入 `DATABASE_URL`），`--features mysql,default-no-db,all-optional` 全量测试 + doc tests（2026-09-30）

### 中期

- [x] 解决 `duckdb` 与 `ladybug` 同时启用时的 mbedtls 重复符号链接冲突 — 冲突源为两驱动 bundled 各自 vendor 的 mbedtls 静态库（依赖层面无解），已建编译期互斥守卫：同启用即编译失败并给出分组指引，`ladybug` + `sqlite` 组合不受影响（2026-09-30）
- [x] 跟进代码质量审查留档的 Medium 项 — 查询缓存策略换装脏读已修：key 派生纳入数据保护策略世代，`set_data_protection` 换装即失效旧条目（2026-09-30）

> 条目整理自 [CHANGELOG.md](CHANGELOG.md) 与仓库验收记录。
