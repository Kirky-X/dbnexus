# 🔒 Dbnexus 安全文档

本文档描述 Dbnexus 的安全支持策略、漏洞报告流程、安全设计概览与使用时的安全最佳实践。安全架构的完整设计细节（含纵深防御图与权限模型）见[架构文档](ARCHITECTURE.md)。

## 📋 目录

<details open>
<summary>📑 目录</summary>

- [📌 支持的版本与维护策略](#-支持的版本与维护策略)
- [📮 漏洞报告流程](#-漏洞报告流程)
- [🏰 安全设计概览](#-安全设计概览)
- [✅ 安全最佳实践](#-安全最佳实践)

</details>

---

## 📌 支持的版本与维护策略

本项目处于 0.x 预发布阶段，**仅维护当前 minor 版本线**（`0.6.x`，含该线内的 rc 预发布版本）。

- **当前线**：接收缺陷修复与安全修复，修复随该线的最新发布版本提供。
- **旧 minor 线**：在下一个 minor 线的首个版本发布时即进入 EOL，不再提供任何修复，安全补丁也不回移。
- **不设 LTS**：1.0 之前不承诺跨 minor 的 API 兼容性，minor 位升级可能包含破坏性变更，升级前请阅读 [CHANGELOG](CHANGELOG.md)。
- **crates.io 可见性 ≠ 受支持**：旧版本保持可下载（除非因严重缺陷执行 yank），但不属于受支持范围。

| 版本线 | 状态 | 说明 |
|--------|------|------|
| `0.6.x`（当前） | ✅ 支持 | 唯一接收修复的版本线 |
| `≤ 0.5.x` | ❌ EOL | 已被当前线取代，不再维护，请升级 |

- 最低支持 Rust 版本（MSRV）为 **1.97.1**（见 `Cargo.toml` 的 `rust-version`）。

---

## 📮 漏洞报告流程

**请勿通过公开 Issue / Discussion 报告安全漏洞。**

1. **首选渠道**：通过 GitHub 的 **"Report a vulnerability"**（GitHub Security Advisories，私有漏洞报告入口，位于仓库 Security 页签）私下提交报告。
2. **备选渠道**：首选渠道不可用时，发送邮件至 **Kirky-X@outlook.com**。
3. 报告中请尽量包含：
   - 受影响的版本与启用的特性组合（如 `sqlite` + `permission`）；
   - 漏洞类型与影响（如权限绕过、注入、信息泄露）；
   - 复现步骤或最小复现代码；
   - 可能的缓解措施（如有）。
4. Kirky.X 会尽快确认报告并与你跟进评估与修复计划；修复发布前，请勿公开披露细节（负责任的协同披露）。
5. 修复发布后，如果你愿意，我们会在更新日志中致谢报告者。

对非敏感的一般性问题（文档错误、普通 Bug），欢迎正常使用 [Issues](https://github.com/Kirky-X/dbnexus/issues)。

### ⏱️ 响应承诺

- **48 小时内**确认收到
- **7 天内**给出初步评估（接受/拒绝/需补充信息）
- 修复发布前，报告者将获得补丁的预览验证机会
- 修复发布后，在 CHANGELOG 与 advisory 中致谢报告者（可匿名）

---

## 🏰 安全设计概览

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
- **统一注入检测引擎**：关系型 / DDL / 图三处规则表合并为单一注册表（`InjectionEngine`，0.6.0-rc.4）。
- **DDL 防护**：裸 DDL 执行（含 DuckDB 的安全通道）需 admin 角色并通过 `DdlGuard` AST 校验；危险模式（如 `DROP DATABASE`）由注入检测引擎硬拦截。
- **图查询防护**：图数据库裸 Cypher 执行已废弃，统一使用 `execute_cypher_with_params`（带注入防护，默认实现拒绝静默回退）。

### 第 4 层：认证与配置安全

- **JWT 认证**（`authentication` 特性）：访问/刷新令牌区分校验（`verify_access_token` / `verify_refresh_token` 额外校验 `token_type`，防止刷新令牌冒用为访问令牌）；JWT 密钥有最小长度要求；0.6.0-rc.4 起撤销缓存 TTL 化（等于令牌剩余有效期）。
- **密码安全**：bcrypt 哈希存储；`PasswordPolicy` 支持密码黑名单 + 复杂度要求（最大长度按**字节**计：bcrypt 只消费前 72 字节，超限口令直接拒绝，不静默截断以免口令碰撞）；配置的 `max_len` 超过 72 字节不生效，实际生效上限为 `min(max_len, 72)`（`PasswordPolicy::effective_max_len`，创建哈希器时显式告警）；旧策略按字符计数放行的超长存量口令可用 `PasswordHasher::verify_legacy` 按前 72 字节口径迁移（验证成功后应立即重设口令）；`register_user` 提供校验强度 → 哈希 → 入库的完整注册流程；`add_user` 会校验传入哈希格式，拒绝畸形哈希。
- **路径与凭据**：连接 URL / 策略文件路径做遍历校验（拒绝含 `..` 的路径）；URL 解析失败时不回显原始 URL，避免凭据泄露。

### 第 5 层：审计与脱敏

- **审计日志**（`audit` 特性）：完整操作日志与用户上下文跟踪；admin 绕过权限的操作也会被记录审计（0.4.2 起）。权限变更另有 HMAC-SHA256 链式签名的审计链与篡改检测（0.6.0-rc.4）。
- **敏感数据脱敏**：内置 `SensitiveMasker`，支持邮箱、电话、身份证号等多种脱敏类型（含 Unicode 安全处理）；`data-protection` 特性提供查询出口字段级脱敏与行级安全谓词注入（admin bypass 通道保留）。行级谓词注入**单表**语句：读出口（`query_rows`）与 raw 写出口（`execute_raw` / `execute_with_params` / `execute` 的 UPDATE/DELETE）均受行级隔离——表级权限不替代行级谓词；多表语句与集合操作（UNION/INTERSECT/EXCEPT）无法用单表谓词覆盖全部行，RLS 生效时按权限错误拒绝（fail-closed），语句无法解析时同样拒绝；谓词列名经标识符白名单校验，谓词值经后端转义后写为字符串字面量（字面量在 PostgreSQL 中由列类型决定，text/uuid/int 等列类型通用）；INSERT 的租户列由调用方赋值，不在行级谓词覆盖范围；PostgreSQL 与 SQLite 后端均消费注入后的语句。

### 纵深防御补充

- **图事务安全**：`execute_cypher` 经 `graph_op_mutex` 串行化避免并发绕过事务隔离；事务句柄 panic 时经 RAII `PoisonGuard` 标记 poisoned；`Session` 析构时未提交的图事务自动回滚。
- **依赖与供应链安全**：CI 集成 `cargo deny`（licenses/advisories/bans）与 `cargo audit` 检查，已登记并豁免的公告记录在 `deny.toml` / `audit.toml`；启用 CodeQL 语义扫描与 Dependabot 依赖自动更新；`h2` 等传递依赖的安全升级随版本发布。

---

## ✅ 安全最佳实践

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

---

相关文档：[📖 用户指南](USER_GUIDE.md) ｜ [📘 API 参考](API_REFERENCE.md) ｜ [🏗️ 架构文档](ARCHITECTURE.md)
