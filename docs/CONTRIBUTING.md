# 贡献指南

感谢您对 dbnexus 项目的关注！本文档描述了参与开发所需的流程和规范。

## 开发环境

- **Rust 1.85+**（edition 2024）
- **Git**（配置了 pre-commit hooks）
- **Docker**（可选，用于 PostgreSQL/MySQL 集成测试）

### 初始化

```bash
git clone https://github.com/Kirky-X/dbnexus.git
cd dbnexus

# 安装 pre-commit hooks
./scripts/install-pre-commit.sh

# 验证环境
cargo build --all-features
cargo test --all-features --lib
```

## TDD 工作流

本项目采用测试驱动开发（TDD），每个开发任务组必须遵循以下循环：

1. **定接口**：先定义 trait / API 签名（`trait Xxx { ... }`），不写实现
2. **写测试**：基于接口编写单元测试（`#[cfg(test)] mod tests { ... }`），此时测试应失败（red）
3. **写代码**：实现接口，使测试通过（green）
4. **跑测试**：`cargo test --features <对应特性> --lib`，确保所有测试通过
5. **commit**：通过后执行 `git add . && git commit -m "feat(<模块>): <描述>"`
6. **gitnexus analyze**：用 gitnexus 工具分析本任务对其他模块的影响，识别需联动修改的代码
7. **继续下一个**：基于 analyze 结果调整后续任务，再开始下一轮循环

## Pre-commit Hooks

项目使用 pre-commit hooks 保证代码质量：

```bash
# 安装
./scripts/install-pre-commit.sh

# 手动运行
pre-commit run --all-files
```

### 禁止事项

- **禁止使用 `--no-verify` 跳过** pre-commit hooks
- **禁止使用 `git commit -n`** 绕过检查
- 违反此规定视为质量红线事件

## 代码质量

项目集成多个质量工具，贡献前请确保通过以下检查：

### diting — 代码质量审查

用于代码简化、架构优化、性能审查。触发场景：
- review / audit / tech debt / over-engineering / simplify
- 任意 write / add / refactor / fix 任务

### tiangang — SAST 安全扫描

用于安全审查、漏洞扫描。触发场景：
- 安全审查 / 漏洞扫描 / SAST
- hardcoded secrets / SQL injection / unsafe eval
- 发布前安全检查

### kueiku — 方法论框架

用于硬性 bug 分析、根因分析、结构化思考。触发场景：
- 分析 / 策略 / 决策 / 根因分析

## Pull Request 流程

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
   cargo test --all-features
   cargo clippy --all-features -- -D warnings
   cargo fmt --all -- --check
   ```

4. **提交 PR**
   - 标题格式：`feat(<模块>): <描述>` 或 `fix(<模块>): <描述>`
   - 描述变更内容和原因
   - 关联相关 issue

## 代码风格

### 通用规范

- **遵循现有代码库命名和架构惯例**（即使你认为自己的写法更好）
- **依赖必须通过 feature 门控**，不引入未门控的依赖
- **edition 2024**，rust 1.85+
- **MIT License**

### Rust 规范

- 命名：`snake_case`（函数/变量/模块），`CamelCase`（类型/Trait）
- 错误处理：使用 `Result<T, E>` 返回错误，**严禁吞掉错误**（规则 12）
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

## 测试规范

- 测试要验证**正确行为的有意义属性**（值、结构、副作用、错误类型）
- 不仅验证"函数有返回值"或"不报错"
- "所有测试通过"是必要条件但非充分条件
- 测试太弱时要明确指出

### 运行测试

```bash
# 单元测试
cargo test --features sqlite --lib

# 集成测试
cargo test --features sqlite

# 全部测试
cargo test --all-features

# 特定数据库后端
cargo test --features postgres
cargo test --features mysql
```

## 许可证

提交的代码将采用 MIT 许可证，与项目保持一致。
