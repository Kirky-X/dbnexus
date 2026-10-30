# dbnexus 错误目录(zh)。与 en/errors.ftl 键一一对应,中文原文保留。
# 运维日志行见 locales/zh/logs.ftl。

config-missing-field = 缺少必填配置项: { $field }
config-missing-url = 缺少必填配置项: dbnexus.url
config-invalid-cache-capacity = 无效的缓存容量: { $reason }
config-invalid-value = 配置项 '{ $key }' 的值无效: { $message }
config-invalid-format = 配置格式无效: { $reason }
config-file-not-found = 配置文件未找到: { $path }
config-io-error = IO 错误: { $reason }
config-invalid-url = 无效的 URL: { $url }
config-unsupported-protocol = 不支持的数据库协议: { $protocol }
config-parse-error = 解析错误: { $reason }
config-validation-error = 验证错误: { $reason }

db-connection = 数据库连接错误: { $error }
db-config = 配置错误: { $message }
db-permission = 权限被拒绝: { $message }
db-transaction = 事务错误: { $message }
db-migration = 迁移错误: { $message }
db-cache = 缓存错误: { $message }
db-query = 查询错误: { $message }
db-validation = 验证错误: { $message }
db-unsupported = 不支持的操作: { $message }
db-version-conflict = ID 为 { $id } 的记录版本冲突
db-rate-limited = 请求过于频繁，已被速率限制，请稍后重试
db-rate-limited-retry = 请求过于频繁，已被速率限制，{ $retry_after_secs } 秒后重试

pool-acquire-timeout = 无法在超时时间内获取连接
pool-exhausted = 连接池已耗尽
pool-connection-failed = 创建连接失败: { $reason }
pool-health-check-failed = 健康检查失败: { $reason }

migration-file-not-found = 迁移文件未找到: { $path }
migration-parse-error = 迁移文件解析失败: { $reason }
migration-execution-error = 迁移执行失败: { $reason }
migration-version-conflict = 迁移版本冲突: { $reason }
migration-rollback-error = 迁移回滚失败: { $reason }
migration-rollback-no-down-section = 迁移 v{ $version } ({ $description }) 无可回滚的 DOWN 部分
migration-rollback-down-not-executable = DOWN 段存在但不含可执行语句，拒绝回滚以避免假回滚
migration-identifier-empty = { $identifier_type }不能为空
migration-identifier-too-long = { $identifier_type }长度不能超过 64 个字符
migration-identifier-invalid-chars = { $identifier_type } '{ $identifier }' 包含无效字符，只允许字母、数字和下划线，且不能以数字开头
migration-identifier-reserved-keyword = { $identifier_type } '{ $identifier }' 是 SQL 保留关键字，不允许使用
migration-identifier-type-table = 表名
migration-identifier-type-primary-key-column = 主键列名
migration-identifier-type-column = 列名
migration-identifier-type-index = 索引名
migration-identifier-type-index-column = 索引列名
migration-identifier-type-new-column = 新列名
migration-identifier-type-fk-table = 外键表名
migration-identifier-type-fk-constraint = 外键约束名
migration-identifier-type-fk-column = 外键列名
migration-identifier-type-fk-referenced-table = 外键引用表名
migration-identifier-type-fk-referenced-column = 外键引用列名
migration-sql-comment-create-table = -- 创建表: { $name }
migration-sql-comment-drop-table = -- 删除表: { $name }
migration-sql-comment-alter-table = -- 修改表: { $name }
migration-sql-comment-add-column = -- 添加列: { $name }
migration-sql-comment-drop-column = -- 删除列: { $name }
migration-sql-comment-add-index = -- 添加索引: { $name }
migration-sql-comment-drop-index = -- 删除索引: { $name }
migration-sql-comment-add-foreign-key = -- 添加外键: { $name }
migration-sql-comment-drop-foreign-key = -- 删除外键: { $name }
migration-sqlite-drop-column-note = -- SQLite 不支持直接删除列，请手动重建表 { $table }
migration-sqlite-modify-column-note = -- SQLite 不支持修改列定义，请手动重建表 { $table } 以调整列 { $column }
migration-sqlite-modify-type-note = -- SQLite 不支持修改列类型，请手动重建表 { $table } 以调整列 { $column } 的类型
migration-sqlite-modify-nullability-note = -- { $database } 不支持直接修改列可空性，请手动重建表 { $table } 以调整列 { $column } 的 NOT NULL 约束
migration-sqlite-modify-default-note = -- SQLite 不支持修改列默认值，请手动重建表 { $table } 以调整列 { $column } 的默认值
migration-scan-read-dir-failed = 读取迁移目录失败: { $error }
migration-scan-read-entry-failed = 读取迁移目录条目失败: { $error }
migration-scan-read-file-failed = 读取迁移文件失败: { $error }

audit-write-error = 审计日志写入失败: { $reason }
audit-serialization-error = 审计数据序列化失败: { $reason }
audit-config-error = 审计配置无效: { $reason }

perm-config-missing-field = 缺少必填字段: { $field }
perm-config-invalid-value = 字段 '{ $field }' 的值无效: { $reason }
perm-config-policy-not-found = 策略文件未找到: { $path }
perm-config-path-empty = 配置路径不能为空
perm-config-path-parent-reference = 配置路径包含无效的父目录引用
perm-config-path-safety-validation = 配置路径未通过安全性校验
perm-config-yaml-feature-not-enabled = 无法解析权限配置：未启用 'yaml' 特性

perm-denied = 对 { $resource } 的 { $operation } 操作权限被拒绝
perm-role-not-found = 角色未找到: { $role }
perm-invalid-policy = 策略配置无效: { $reason }
perm-rate-limited = 速率限制已超出
perm-parse-error = 策略解析错误: { $reason }

perm-provider-role-not-found = 角色 '{ $role }' 未找到
perm-provider-load-error = 加载配置失败: { $reason }
perm-provider-check-error = 权限检查失败: { $reason }
perm-provider-unknown = 未知错误: { $reason }
perm-provider-memory-policy-table-uninitialized = MemoryPermissionProvider 策略表未初始化：无任何角色策略（容量 0）
perm-provider-yaml-file-unreadable = YamlPermissionProvider 策略文件不可读（{ $path }）: { $error }
perm-provider-config-read-error = 权限配置文件读取错误（path='{ $path }'）: { $error }
perm-provider-yaml-parse-error = YAML 解析错误（path='{ $path }'）: { $error }
perm-provider-lock-error = 锁错误: 权限提供器内部状态已中毒
perm-rate-limiter-error = 限流后端故障: { $error }
perm-rate-limit-audit-dropped = 限流审计事件已丢弃: { $error }
perm-cache-log-insert-failed = PermissionCache 插入失败（错误不含键原文）: { $error }
perm-cache-log-invalidate-failed = PermissionCache invalidate 失败（错误不含键原文）: { $error }
perm-cache-log-clear-failed = PermissionCache clear 失败: { $error }
perm-cache-provider-no-policy = 权限提供器没有角色 '{ $role }' 的策略
perm-cache-provider-not-configured = 未为角色 '{ $role }' 配置权限提供器

# --- 图操作权限门（canonical mirror，与 src/access/permission/types.rs 内嵌的
# 规范串逐字一致） ---
graph-op-denied-admin-required = 角色 '{ $role }' 的图操作被拒绝: 需要 admin 角色

sql-parse-error = SQL 解析失败: { $reason }
sql-unsupported-statement = 不支持的 SQL 语句类型: { $stmt_type }
sql-empty-statement = 空的 SQL 语句
sql-multiple-statements = 不允许多条语句
sql-contains-variables = SQL 语句包含变量: { $details }

sensitive-masking-failed = 脱敏失败: { $reason }
sensitive-encryption-failed = 加密失败: { $reason }
sensitive-decryption-failed = 解密失败: { $reason }
sensitive-invalid-key = 无效的密钥: { $reason }
sensitive-invalid-input = 无效的输入: { $reason }

auth-invalid-credentials = 无效的凭据
auth-token-generation = 令牌生成失败: { $reason }
auth-invalid-token = 无效的令牌
auth-token-expired = 令牌已过期
auth-user-not-found = 用户未找到: { $user }
auth-password-hash = 密码哈希失败: { $reason }
auth-user-limit-reached = 用户存储已达上限: { $reason }
auth-password-too-long-bcrypt = 密码长度不得超过 { $max_len } 字节（bcrypt 截断上限）

circuit-breaker = 断路器处于 { $state } 状态
circuit-breaker-closed = 断路器闭合: successes={ $successes } success_threshold={ $threshold }
circuit-breaker-opened = 断路器打开: failures={ $failures } failure_threshold={ $threshold }
circuit-breaker-reopened = 断路器重新打开: half-open 探测失败
circuit-breaker-half-open = 断路器半开: timeout_ms={ $timeout_ms }

audit-builder-operation-required = 操作类型为必填项
audit-builder-entity-type-required = 实体类型为必填项
audit-builder-entity-id-required = 实体 ID 为必填项
audit-builder-id-empty = 事件 ID 不能为空

retry-exhausted = 重试 { $attempts } 次后仍失败: { $last_error }
retry-non-retryable = 不可重试的操作: { $error }
retry-timeout = 重试在 { $timeout_ms }ms 后超时: { $last_error }

saga-execution-failed = Saga 执行失败: { $reason }
saga-compensation-failed = Saga 补偿失败: { $reason }
saga-timeout = Saga 超时: { $reason }
saga-compensation-no-pool = 补偿会话不可用: 分片 { $shard_id } 无可用连接池
saga-compensation-session-unavailable = 补偿会话不可用: { $error }

snowflake-clock-backtrack = 时钟回拨: 等待后的时间戳 { $waited_ts } 仍落后于上次使用的 { $last_ts }
snowflake-timestamp-overflow = 时间戳溢出: { $timestamp } 超出 41 位容量

nexus-unsupported-database = 无效的数据库 URL: { $scheme }
nexus-url-parse-failed = URL 解析失败（scheme={ $scheme }）: { $error }

error-category-permission = 权限
error-category-injection-risk = 注入风险
error-category-syntax-error = 语法错误
error-category-shard-conflict = 分片冲突

query-error-report = [{ $category }] { $message } — 建议: { $suggestion }
query-error-suggestion-code-table = 参考错误码表定位处理策略

error-code = 错误码: { $code }（{ $name }）

migration = 已应用 { $count } 个迁移

pool-invalid-config = 无效的配置: { $error }
pool-read-config-failed = 读取权限配置文件 '{ $path }' 失败: { $error }
pool-parse-config-failed = 解析权限配置文件 '{ $path }' 失败: { $error }
pool-yaml-parse-error = '{ $source }' 中的 YAML 解析错误: { $error }
pool-invalid-db-url = 无效的数据库 URL: { $error }
pool-recreate-failed = 重新创建连接失败: { $error }
pool-sqlite-pragma-invalid = 无效的 SQLite PRAGMA 值 '{ $name }': '{ $value }'
pool-sqlite-pragma-failed = SQLite PRAGMA '{ $statement }' 执行失败: { $error }

health-check-timeout = 健康检查超时
health-check-timeout-recommendation = 健康检查超时，建议优化检查逻辑
health-recommendation-check-connection-config = 检查数据库连接配置是否正确
health-recommendation-check-network = 验证网络连接是否稳定
health-recommendation-ensure-pool-initialized = 请确保数据库连接池已正确初始化
health-recommendation-wait-circuit-breaker = 等待熔断器恢复
health-recommendation-increase-pool-size = 考虑增加连接池大小
health-recommendation-check-connection-leak = 检查是否有连接泄露
health-recommendation-increase-min-connections = 考虑增加最小连接数
health-status-failure-rate-too-high = 连接失败率过高: { $failed }/{ $created } ({ $percent }%)
health-status-no-connections = 无可用连接
health-status-circuit-breaker-open = 熔断器已打开，请求被拒绝
health-status-pool-full = 连接池已满: { $active }/{ $total } 活跃, { $waiting } 等待
health-status-no-idle-connections = 无空闲连接
health-details-pool = 连接池: total={ $total }, active={ $active }, idle={ $idle }, waiting={ $waiting }
health-details-stats = 统计: created={ $created }, failed={ $failed }, closed={ $closed }
health-details-circuit-breaker = 熔断器: { $state } ({ $failures } 次失败, { $successes } 次成功)
session-txn-begin-failed = 开始事务失败: { $error }
session-txn-begin-graph-failed = 开始图事务失败: { $error }
session-txn-commit-failed = 提交图事务失败: { $error }
session-txn-rollback-failed = 回滚事务失败: { $error }
session-txn-rollback-graph-failed = 回滚图事务失败: { $error }
session-ddl-not-allowed = DDL 操作不允许: { $reason }
session-ddl-parse-failed = DDL SQL 解析失败: { $error }
session-ddl-validation-error = DDL 验证错误: { $error }
session-unknown-operation = 未知操作: { $operation }
session-permission-denied = 对 { $table } 的 { $action } 权限被拒绝
session-for-update-txn-required = query_rows_for_update 需要活动事务（请先调用 begin_transaction）
session-for-update-relational-required = query_rows_for_update 需要关系型（SeaORM）后端
session-isolation-relational-required = begin_transaction_with_isolation 需要关系型（SeaORM）后端

session-already-in-transaction = 已处于事务中
session-already-in-transaction-concurrent = 已处于事务中（检测到并发开启）
session-already-in-graph-transaction = 已处于图事务中
session-already-in-graph-transaction-concurrent = 已处于图事务中（检测到并发开启）
session-connection-not-available = 连接不可用 - 会话可能已失效
session-not-in-transaction = 不在事务中
session-no-active-transaction-commit = 没有活动事务可提交
session-no-active-transaction-rollback = 没有活动事务可回滚
session-commit-transaction-in-use = 无法提交：事务正被并发查询使用
session-rollback-transaction-in-use = 无法回滚：事务正被并发查询使用
session-duckdb-ddl-admin-only = DuckDB 上下文中 DDL 操作仅允许 admin 角色。当前角色: '{ $role }'，admin 角色: '{ $admin_role }'
session-ddl-admin-only = DDL 操作仅允许 admin 角色。当前角色: '{ $role }'，admin 角色: '{ $admin_role }'

copy-path-empty = COPY 文件路径不能为空
copy-insert-column-required = INSERT 至少需要一列
copy-insert-row-required = 多行 INSERT 至少需要一行（空 VALUES 无意义）
copy-insert-table-invalid = INSERT 目标表标识符无效: '{ $table }'（允许字母/数字/下划线/点）
copy-insert-column-invalid = INSERT 列标识符无效: '{ $column }'（允许字母/数字/下划线/点）
copy-in-incompatible-connection = （连接池返回了不兼容的连接: { $error }）
copy-payload-path-utf8 = COPY 载荷临时路径不是有效的 UTF-8
copy-payload-cleanup-failed = DuckDB COPY 载荷临时文件清理失败: { $error }
copy-driver-feature-missing = ——启用 postgres 或 duckdb 驱动特性后方可使用 COPY
copy-payload-create-failed = 无法创建 COPY 载荷临时文件
copy-payload-create-io-failed = DuckDB COPY 载荷临时文件创建失败: { $error }
copy-payload-write-join-failed = DuckDB COPY 载荷写入任务 join 失败: { $error }

duckdb-execute-batch-failed = DuckDB execute_batch 失败: { $error }
duckdb-copy-prepare-failed = DuckDB COPY 预处理失败: { $error }
duckdb-copy-execute-failed = DuckDB COPY 执行失败: { $error }
duckdb-copy-fetch-failed = DuckDB COPY 结果读取失败: { $error }
duckdb-copy-count-read-failed = DuckDB COPY 计数列读取失败: { $error }

duckdb-pool-exhausted = DuckDB 连接池已耗尽: 无可用连接
duckdb-txn-begin-failed = DuckDB 开始事务失败: { $error }
duckdb-txn-commit-failed = DuckDB 提交事务失败: { $error }
duckdb-spawn-blocking-join-failed = spawn_blocking 任务 join 失败: { $error }

neo4j-connect-failed = neo4j 连接失败: { $error }
neo4j-url-no-credentials-env = neo4j URL 无凭据，且未设置 { $variable } 环境变量
neo4j-url-invalid-and-env-missing = neo4j URL 不是合法 URL，且未设置 { $variable } 环境变量
neo4j-not-connected = Neo4jConnection 未连接（占位连接）
neo4j-execute-failed = neo4j 执行失败: { $error }
neo4j-execute-with-params-failed = neo4j 参数化执行失败: { $error }
neo4j-row-fetch-failed = neo4j 行读取失败: { $error }
neo4j-row-deserialize-failed = neo4j 行反序列化失败: { $error }
neo4j-health-check-no-rows = neo4j 健康检查未返回任何行
neo4j-start-txn-failed = neo4j 开启事务失败: { $error }
neo4j-transaction-consumed = neo4j 事务已被消耗
neo4j-commit-failed = neo4j 提交失败: { $error }
neo4j-rollback-failed = neo4j 回滚失败: { $error }
neo4j-txn-execute-failed = neo4j 事务内执行失败: { $error }
neo4j-txn-execute-with-params-failed = neo4j 事务内参数化执行失败: { $error }
neo4j-txn-row-fetch-failed = neo4j 事务内行读取失败: { $error }
neo4j-txn-row-deserialize-failed = neo4j 事务内行反序列化失败: { $error }

repository-version-column-unsafe = repository 版本列必须是安全标识符: '{ $column }'
repository-version-field-missing = update_if_version 要求实体包含字段 '{ $column }'
repository-version-column-integer = 版本列 '{ $column }' 必须是整数
repository-column-unsafe = repository 列必须是安全标识符: '{ $column }'
repository-entity-deserialize-failed = 实体反序列化失败: { $error }
repository-query-role-not-configured = repository 查询角色未配置；查询前请先调用 with_role()

shard-route-unregistered = 分片键 '{ $key }' 路由到未注册的分片 { $shard_id }
shard-migration-orchestration-not-implemented = 分片迁移编排尚未实现：未接入 MigrationExecutor，该分片未执行任何迁移

migration-history-table-invalid = 非法的迁移历史表名: { $name }

query-cache-get-failed = 查询缓存读取失败: { $error }
query-cache-decode-failed = 查询缓存解码失败: { $error }
query-cache-encode-failed = 查询缓存编码失败: { $error }
query-cache-fill-failed = 查询缓存回填失败: { $error }
query-cache-clock-before-epoch = 系统时钟早于 Unix 纪元: { $error }
query-cache-invalidate-failed = 查询缓存失效操作失败: { $error }
query-cache-tables-required = 查询缓存要求 tables 中至少声明一个表；未声明表的查询永远无法失效（fail-closed）
query-cache-version-read-failed = 查询缓存表版本读取失败: { $error }
query-cache-params-encode-failed = 查询缓存参数编码失败: { $error }
query-cache-table-name-invalid = 查询缓存表名无效: '{ $table }'（允许字母/数字/下划线/点，不得以数字开头）

kit-health-no-connections = 尚未建立任何连接
kit-health-waiting = { $waiting } 个等待，{ $active }/{ $total } 活跃/总数
kit-health-no-idle = 无空闲连接，{ $active }/{ $total } 活跃/总数

# --- Scatter-gather（canonical mirror：英文模式与 src/database/scatter.rs 构造点
# 内嵌的规范串逐字一致） ---
scatter-query-timed-out = Scatter-gather 查询超时
scatter-partial-failure = Scatter-gather 失败：{ $shards } 个分片失败

# --- 审计日志清理（canonical mirror，与 src/domain/audit/audit_impl.rs 内嵌的
# 规范串逐字一致） ---
audit-cleanup-invalid-date = 日期计算无效

# --- 迁移差异（canonical mirror，与 src/domain/migration/differ.rs 内嵌的规范串
# 逐字一致） ---
migration-graph-alter-unsupported = 图数据库不支持关系型 ALTER TABLE 操作

# --- 权限配置（canonical mirror，与 src/access/permission/context.rs 内嵌的
# 规范串逐字一致） ---
perm-config-role-not-found = 权限配置中未找到该角色

# --- 连接池访问角色校验（canonical mirror，与 src/database/pool/db_pool/access.rs
# 内嵌的规范串逐字一致） ---
perm-role-not-allowed-without-config = 缺少显式权限配置，角色 '{ $role }' 不被允许

# --- 会话层（canonical mirror：英文模式与 src/database/pool/session/
# {execute,duckdb,mod}.rs 构造点内嵌的规范串逐字一致） ---
session-ddl-not-allowed-context = 当前上下文不允许 DDL 操作
session-ddl-not-allowed-duckdb = DuckDB 查询上下文不允许 DDL 操作
session-permission-table-name-required = SQL 语句缺少用于权限检查的有效表名
session-permission-table-extract-failed = 提取权限检查所需表名失败
session-permission-parse-failed = 解析权限检查所需 SQL 语句失败
session-sql-parser-feature-required = { $function } 需要启用 sql-parser 特性
session-sql-parser-feature-required-secure = { $function } 需要启用 sql-parser 特性以执行安全检查
session-rls-parse-failed = 解析行级安全所需的 SQL 语句失败
session-rls-not-applicable = 行级安全策略无法应用于该语句
session-rls-multi-table-unsafe = 行级安全不支持多次引用受保护表的语句（自连接或子查询）
session-rls-single-table-required = 行级安全要求单表语句
session-query-rows-select-only = query_rows 只允许 SELECT 语句
session-query-rows-sqlite-single-table = sqlite 上的 query_rows（MVP）要求单表 SELECT 语句
session-query-rows-sqlite-postgres-only = query_rows 的 MVP 仅支持 postgres/sqlite 后端
session-batch-rollback-failed = 批量执行失败: { $error }; 回滚也失败: { $rollback_error }
session-cypher-length-exceeded = Cypher 查询超过最大长度（{ $max } 字节，实际 { $actual } 字节）- 疑似 DoS 载荷
session-cypher-multiple-statements = Cypher 查询包含多条语句（查询内含 ';'）- 疑似注入
session-cypher-line-comment = Cypher 查询包含行注释 '//' - 疑似注入
session-cypher-block-comment = Cypher 查询包含块注释 '/* */' - 疑似注入
session-cypher-dangerous-procedure = Cypher 查询调用了危险过程（'{ $procedure }'）- 疑似提权

# --- 行级安全策略错误（canonical mirror，与 src/access/data_protection.rs 内嵌的
# 规范串逐字一致） ---
rls-unsafe-predicate-column = 表 '{ $table }' 的行级安全策略包含不安全的谓词列 '{ $column }'
rls-set-operations-unsupported = 行级安全策略不支持集合操作（UNION/INTERSECT/EXCEPT）

# --- 数据 API 网关（canonical mirror，与 src/database/data_api.rs 内嵌的规范串
# 逐字一致） ---
data-api-owner-column-unsafe = data-api 归属列必须是安全标识符: '{ $column }'
data-api-query-role-not-configured = data-api 查询角色未配置；查询前请先调用 with_role()

# --- 查询片段 DSL（canonical mirror，与 src/database/query_dsl.rs 内嵌的规范串
# 逐字一致） ---
query-fragment-column-unsafe = 查询片段列名必须是安全标识符: '{ $column }'
query-fragment-table-unsafe = 查询片段表名必须是安全标识符: '{ $table }'
query-fragment-condition-column-unsafe = 查询片段条件列必须是安全标识符: '{ $column }'
query-fragment-order-column-unsafe = 查询片段排序列必须是安全标识符: '{ $column }'
