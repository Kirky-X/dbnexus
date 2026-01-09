# DB Nexus 贡献指南

感谢您考虑为 DB Nexus 项目贡献力量！本指南将帮助您了解如何参与项目贡献。

## 目录

- [如何贡献](#如何贡献)
- [开发环境设置](#开发环境设置)
- [代码规范](#代码规范)
- [测试要求](#测试要求)
- [Pull Request 流程](#pull-request-流程)
- [代码审查标准](#代码审查标准)

---

##### 如何贡献

 贡献类型

我们欢迎各种形式的贡献，包括但不限于：

- 🐛 **Bug 修复** - 修复已知的 bug 和问题
- ✨ **新功能** - 添加新特性或改进现有功能
- 📚 **文档** - 改进或翻译文档
- 🎨 **代码优化** - 改进代码性能和可读性
- 🧪 **测试** - 添加或改进测试用例
- 📦 **依赖更新** - 更新依赖版本
- 💡 **想法和建议** - 提出新想法或改进建议

### 贡献流程

1. **Fork 本仓库**
2. **创建特性分支**
3. **提交更改**
4. **推送分支**
5. **创建 Pull Request**

---

## 开发环境设置

### 环境要求

- **Rust 版本**: 1.85+
- **Git**: 最新版本
- **数据库**: SQLite 3.35+ / PostgreSQL 12+ / MySQL 8.0+（用于测试）
- **Docker**: 可选，用于启动测试数据库

### 步骤 1：Fork 仓库

访问 [DB Nexus 仓库](https://github.com/Kirky-X/dbnexus)，点击右上角的 Fork 按钮。

### 步骤 2：克隆仓库

```bash
# 克隆您的 Fork
git clone https://github.com/YOUR_USERNAME/dbnexus.git
cd dbnexus

# 添加上游仓库
git remote add upstream https://github.com/Kirky-X/dbnexus.git
```

### 步骤 3：设置开发环境

```bash
# 安装 Rust（如果尚未安装）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 验证 Rust 版本
rustc --version
cargo --version

# 安装依赖
cargo fetch
cargo build --all-features
```

### 步骤 4：运行测试（验证环境）

```bash
# 运行基本测试
cargo test --features sqlite --lib

# 运行所有测试
cargo test --all-features --all
```

---

## 代码规范

### 编码规范

| 类型 | 约定 | 示例 |
|------|------|------|
| 结构体/枚举 | PascalCase | `DbPool`, `Session` |
| 函数/变量 | snake_case | `check_connection_health` |
| 常量 | SCREAMING_SNAKE_CASE | `ALLOWED_ROLES` |
| 宏属性 | snake_case | `#[db_crud]`, `#[primary_key]` |
| 字段 | snake_case | `max_connections` |

### 格式化配置

项目使用 `rustfmt` 进行代码格式化，配置在 `rustfmt.toml`：

```toml
edition = "2024"
max_width = 120
tab_spaces = 4
fn_single_line = true
```

### 运行格式化

```bash
# 格式化代码
cargo fmt --all

# 检查格式化
cargo fmt --check --all
```

### Clippy 检查

所有警告视为错误：

```bash
# 运行 Clippy
cargo clippy --all-features --all -- -D warnings
```

### 文档要求

所有公开 API 必须有文档注释：

```rust
/// 创建新的数据库连接池
///
/// # 参数
///
/// * `url` - 数据库连接字符串
///
/// # 示例
///
/// ```rust
/// use dbnexus::DbPool;
///
/// let pool = DbPool::new("sqlite::memory:").await?;
/// ```
pub async fn new(url: &str) -> Result<Self, DbError>;
```

### 禁止使用的代码

- `unsafe_code` - 项目强制禁止使用 unsafe 代码
- `unwrap()` 在生产代码中（使用 `?` 或 `expect()` 替代）
- 硬编码的敏感信息

---

## 测试要求

### 测试类型

| 测试类型 | 位置 | 描述 |
|----------|------|------|
| 单元测试 | 各模块 `#[cfg(test)]` 块 | 测试单个函数或模块 |
| 集成测试 | `tests/*_integration.rs` | 测试组件交互 |
| 文档测试 | `///` 代码块 | 验证文档示例 |

### 运行测试

```bash
# 运行所有测试
cargo test --all-features --all

# 运行特定测试
cargo test --features sqlite test_name

# 运行集成测试
cargo test --features sqlite --test pool_integration

# 运行文档测试
cargo test --doc
```

### 测试覆盖率

我们使用 `cargo tarpaulin` 监控测试覆盖率：

```bash
# 生成覆盖率报告
cargo tarpaulin --out Html

# 查看覆盖率
open tarpaulin-report.html
```

### 测试数据库

测试使用环境变量选择数据库：

```bash
# SQLite（默认）
export TEST_DB_TYPE=sqlite

# PostgreSQL
export TEST_DB_TYPE=postgres
export DATABASE_URL=postgres://dbnexus:dbnexus_password@localhost:15432/dbnexus_test

# MySQL
export TEST_DB_TYPE=mysql
export DATABASE_URL=mysql://dbnexus:dbnexus_password@localhost:13306/dbnexus_test
```

### 测试辅助函数

使用 `common/mod.rs` 中的辅助函数：

```rust
use dbnexus::tests::common;

// 获取测试配置
let config = common::get_test_config();

// 获取 SQLite 内存数据库
let config = common::get_sqlite_memory_config();

// 创建测试夹具
let (pool, _migrations_dir, _temp_dir) = common::create_test_fixture().await;
```

### 测试要求

- 新功能必须包含对应的测试
- Bug 修复必须包含回归测试
- 公共 API 必须包含文档测试
- 目标：测试覆盖率不低于 80%

---

## Pull Request 流程

### 创建 PR 前的准备

1. **保持代码同步**

```bash
# 同步上游更改
git fetch upstream
git rebase upstream/main

# 解决冲突后
git push --force-with-lease
```

2. **运行所有检查**

```bash
# 格式化检查
cargo fmt --check --all

# Clippy 检查
cargo clippy --all-features --all -- -D warnings

# 运行测试
cargo test --all-features --all
```

3. **更新相关文档**

- 如果添加新功能，更新用户指南
- 如果修改 API，更新 API 参考
- 如果添加示例，更新示例代码

### 创建 PR

1. 访问您的 Fork 仓库
2. 点击 **New Pull Request**
3. 选择您的特性分支
4. 填写 PR 模板

### PR 模板

```markdown
## 描述

简要描述您的更改。

## 变更类型

- [ ] Bug 修复
- [ ] 新功能
- [ ] 破坏性变更
- [ ] 文档更新
- [ ] 代码重构

## 测试

- [ ] 我添加了测试来验证我的更改
- [ ] 所有现有测试通过
- [ ] 我在本地运行了测试

## 清单

- [ ] 我的代码遵循项目的代码风格
- [ ] 我的更改需要更新文档
- [ ] 我已经更新了相关文档
- [ ] 我的更改不会导致任何警告
- [ ] 我的 PR 标题清晰描述了更改内容
```

### PR 描述要点

1. **清晰描述** - 解释您做了什么以及为什么
2. **关联 Issue** - 如果修复了某个 issue，关联它
3. **截图/演示** - 如果是 UI 更改，添加截图
4. **测试结果** - 列出测试通过情况

---

## 代码审查标准

### 代码审查清单

- [ ] **正确性** - 代码是否按预期工作？
- [ ] **完整性** - 是否包含所有必要的部分？
- [ ] **清晰性** - 代码是否易于理解？
- [ ] **安全性** - 是否有安全漏洞？
- [ ] **性能** - 是否有明显的性能问题？
- [ ] **测试** - 是否有足够的测试？
- [ ] **文档** - 是否有适当的文档？

### 常见审查意见

#### 代码风格

```rust
// 不推荐
fn do_something(a:i64,b:&str)->Result<bool,Error>{
    if a>0 { return Ok(true); }
    else { return Err(Error::new("invalid")); }
}

// 推荐
fn do_something(a: i64, b: &str) -> Result<bool, Error> {
    if a > 0 {
        Ok(true)
    } else {
        Err(Error::new("invalid"))
    }
}
```

#### 错误处理

```rust
// 不推荐
.unwrap()

// 推荐
.expect("描述错误原因")
// 或
?
```

#### 文档注释

```rust
// 不推荐
/// Get the user
pub fn get_user() -> User {}

// 推荐
/// 根据 ID 获取用户
///
/// # 参数
///
/// * `id` - 用户的唯一标识符
///
/// # 返回
///
/// 返回找到的用户，如果不存在返回 None
///
/// # 示例
///
/// ```
/// let user = get_user(1);
/// ```
pub fn get_user(id: i64) -> Option<User> {}
```

### 响应审查意见

1. **礼貌回应** - 感谢审查者的反馈
2. **解释思考** - 如果不同意，解释您的理由
3. **积极修改** - 按意见进行修改
4. **标记完成** - 修改后标记为已解决

---

## 行为准则

### 我们的承诺

为了营造一个开放和包容的社区，我们承诺让参与本项目的每个人都享有无骚扰的体验，无论其年龄、体型、可见或不可见的残疾、种族、性别特征、性别认同和表达、经验水平、教育程度、社会经济地位、国籍、个人外貌、种族、宗教或性取向如何。

### 我们的标准

**鼓励的行为**：
- 使用友好和包容的语言
- 尊重不同的观点和经历
- 优雅地接受建设性批评
- 关注对社区最有利的事情

**不可接受的行为**：
- 使用带有性意味的语言或图像
- 骚扰、侮辱或贬低的评论
- 公开或私下骚扰
- 未经许可发布他人的私人信息
- 其他不合理的行为

### 举报

如果遇到不可接受的行为，请通过以下方式报告：
- GitHub Issues
- 发送邮件给项目维护者

---

## 获取帮助

### 常见问题

**Q: 我不知道如何开始**
A: 查看 GitHub Issues 中标记为 `good first issue` 的问题

**Q: 我的 PR 被拒绝了**
A: 查看审查意见，根据反馈修改后重新提交

**Q: 有问题需要帮助**
A: 在 GitHub Discussions 中提问

### 资源

- [项目文档](https://docs.rs/dbnexus)
- [GitHub Issues](https://github.com/Kirky-X/dbnexus/issues)
- [GitHub Discussions](https://github.com/Kirky-X/dbnexus/discussions)
- [Sea-ORM 文档](https://www.sea-ql.org/SeaORM/)

---

感谢您对 DB Nexus 项目的贡献！ 🙏
