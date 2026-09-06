# 🔒 Dbnexus 安全文档

本文档描述 Dbnexus 的安全支持策略、漏洞报告流程、安全设计概览与使用时的安全最佳实践。安全架构的完整设计细节（含纵深防御图与权限模型图）见[架构文档](ARCHITECTURE.md)。

## 📋 目录

<details open>
<summary>📑 目录（点击展开）</summary>

- [支持版本](#支持版本)
- [漏洞报告流程](#漏洞报告流程)
- [安全设计概览](#安全设计概览)
- [安全最佳实践](#安全最佳实践)

</details>

---

## 支持版本

| 版本线 | 状态 | 说明 |
|--------|------|------|
| 0.6.x（当前 `0.6.0-rc.2`） | ✅ 支持中 | 安全修复随补丁版本发布 |
| 0.5.x 及更早 | ❌ 不再维护 | 建议升级到最新版本线 |

- 最低支持 Rust 版本（MSRV）为 **1.97.1**（见 `Cargo.toml` 的 `rust-version`）。
- 本项目仅维护最新版本线；收到安全公告后会评估影响并尽可能以补丁版本发布，请始终使用最新版本。

---

## 漏洞报告流程

**请勿通过公开 Issue / Discussion 报告安全漏洞。**

1. 通过 GitHub 的 **"Report a vulnerability"**（私有漏洞报告入口，位于仓库 Security 页签）私下提交报告。
2. 报告中请尽量包含：
   - 受影响的版本与启用的特性组合（如 `sqlite` + `permission`）；
   - 漏洞类型与影响（如权限绕过、注入、信息泄露）；
   - 复现步骤或最小复现代码；
   - 可能的缓解措施（如有）。
3. 维护者会尽快确认报告并与你跟进评估与修复计划；修复发布前，请勿公开披露细节（负责任的协同披露）。
4. 修复发布后，如果你愿意，我们会在更新日志中致谢报告者。

对非敏感的一般性问题（文档错误、普通 Bug），欢迎正常使用 [Issues](https://github.com/Kirky-X/dbnexus/issues)。

---

## 安全设计概览

Dbnexus 采用纵深防御（defense-in-depth）设计，自下而上分为五层。以下机制均真实存在于代码中：

### 第 1 层：编译时保证

- **禁止 unsafe 代码**：全库 `#![forbid(unsafe_code)]`（`src/lib.rs`）。
- **数据库驱动编译期互斥**：嵌入式（`sqlite`/`duckdb`）与服务器端（`postgres`/`mysql`）驱动混用时直接 `compile_error!` 失败，无逃生门，杜绝误配导致的多后端不一致。
- **特性依赖编译期校验**：`permission`、`sql-parser`、`permission-engine` 未启用 `cache` 等必需依赖时编译失败（fail-loud，无静默降级）。

### 第 2 层：运行时权限检查

- **基于角色的表级访问控制（RBAC）**：角色 → 表 → 操作（SELECT/INSERT/UPDATE/DELETE 等）的策略模型，支持通配符表（`*`）与操作级控制。
- **跨表权限检查**：JOIN / 子查询路径上的目标表同样经过权限检查（0.6.0-rc.2 补全）。
- **权限缓存**：带 TTL 的权限缓存 + singleflight 请求合并，防止缓存击穿；支持后台热加载。
- **速率限制**：权限检查内置令牌桶限流（`RateLimiter`），防止滥用与暴力尝试。
- **权限健康检查**：`health_check` 真实校验 memory 策略表容量与 YAML 策略文件可读性，空表/不可读即上报不健康。

### 第 3 层：SQL 注入防护

- **参数化查询**：默认全部使用参数化查询。
- **SQL 解析器校验**：表名提取基于 `SqlParser`（0.4.2 起替代朴素字符串匹配，消除解析层注入隐患）；`contains_sql_injection` 提供注入模式检测（含 Unicode 归一化防护）。
- **DDL 防护**：裸 DDL 执行（含 DuckDB 的 `execute_duckdb_raw`）需 admin 角色并通过 `DdlGuard` AST 校验；默认阻止 DDL 与多语句。
- **图查询防护**：图数据库执行裸 Cypher 已废弃，统一使用 `execute_cypher_with_params`（带注入防护，默认实现拒绝静默回退）。

### 第 4 层：认证与配置安全

- **JWT 认证**（`authentication` 特性）：访问/刷新令牌区分校验（`verify_access_token` / `verify_refresh_token` 额外校验 `token_type`，防止刷新令牌冒用为访问令牌）；JWT 密钥有最小长度要求。
- **密码安全**：bcrypt 哈希存储；`PasswordPolicy` 支持密码黑名单 + 复杂度要求；`register_user` 提供校验强度 → 哈希 → 入库的完整注册流程；`add_user` 会校验传入哈希格式，拒绝畸形哈希。
- **路径与凭据**：连接 URL / 策略文件路径做遍历校验（拒绝含 `..` 的路径）；URL 解析失败时不回显原始 URL，避免凭据泄露。

### 第 5 层：审计与脱敏

- **审计日志**（`audit` 特性）：完整操作日志与用户上下文跟踪；admin 绕过权限的操作也会被记录审计（0.4.2 起）。
- **敏感数据脱敏**：内置 `SensitiveMasker`，支持邮箱、电话、身份证号等多种脱敏类型（含 Unicode 安全处理）。

### 纵深防御补充

- **图事务安全**：`execute_cypher` 经 `graph_op_mutex` 串行化避免并发绕过事务隔离；事务句柄 panic 时经 RAII `PoisonGuard` 标记 poisoned；`Session` 析构时未提交的图事务自动回滚。
- **依赖与供应链安全**：CI 集成 `cargo deny`（licenses/advisories/bans）与 `cargo audit` 检查，已登记并豁免的公告记录在 `deny.toml` / `audit.toml`；启用 CodeQL 语义扫描与 Dependabot 依赖自动更新；`h2` 等传递依赖的安全升级随版本发布。

---

## 安全最佳实践

使用 Dbnexus 时建议遵循以下实践：

1. **显式启用安全特性组合**：生产环境启用 `permission`（强制依赖 `sql-parser`）与 `audit`；不要在禁用 SQL 解析的情况下绕过权限检查。
2. **最小权限原则**：为业务角色按表/操作粒度分配权限，避免在生产策略中使用通配符 `*`；不要让日常业务代码使用 admin 角色（admin 绕过操作会被审计记录）。
3. **使用参数化查询**：始终使用实体宏生成的 CRUD 与参数化接口；避免拼接 SQL 字符串，必要时才使用受 `DdlGuard` 保护的受限通道。
4. **图查询走参数化接口**：使用 `execute_cypher_with_params`，不要使用已废弃的裸 Cypher 执行。
5. **正确使用认证系统**：
   - JWT 密钥使用足够长度的高熵随机值（满足最小长度要求，避免硬编码）；
   - 用户注册使用 `register_user`（完整强度校验 + 哈希），不要绕过密码策略直接写入哈希；
   - 访问令牌与刷新令牌分开校验。
6. **保护策略与配置**：权限策略文件（YAML）与连接配置纳入最小权限的部署管理；环境变量中的数据库凭据不要写入版本控制。
7. **保持依赖更新**：定期运行 `cargo audit` / `cargo deny`；关注 crates.io 上的新版本发布并及时升级。
8. **开启审计与监控**：在生产环境启用 `audit` 特性并接入日志收集；结合 `metrics` 特性监控连接池与权限检查指标，及时发现异常访问模式。
