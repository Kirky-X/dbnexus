# dbnexus 运维日志目录(zh)。与 en/logs.ftl 键一一对应。

config-confers-subscribe-failed = confers 订阅失败: { $error }
config-confers-hot-reload-failed = 权限热重载失败: { $error }
config-confers-skip-invalid-event = 跳过无效的权限变更事件: { $error }
config-confers-log-hot-reload = 权限热重载: { $error }
config-confers-log-ack-failed = confers 确认失败: { $error }
prepare-cache-log-read-miss = PreparedStatementCache { $op } 读取失败，按未命中处理: { $error }
prepare-cache-log-write-failed = PreparedStatementCache { $op } 写入失败: { $error }
prepare-cache-log-clear-failed = PreparedStatementCache clear 失败: { $error }

metrics-export-error = 导出失败: { $reason }
metrics-not-initialized = 收集器未初始化
metrics-unknown = 未知指标错误: { $reason }
metrics-collector-not-configured = 指标收集器未配置（请使用 with_metrics_collector）
metrics-export-join-failed = 指标导出任务 join 失败: { $error }
metrics-slow-query = 慢查询: query_type={ $query_type } duration_ms={ $duration_ms } threshold_ms={ $threshold_ms }
metrics-feature-not-enabled = metrics 特性未启用（重新编译时启用 metrics 特性以暴露收集器端点）
pool-log-acquire-timeout = 连接池获取超时: waited_ms={ $waited_ms } timeout_ms={ $timeout_ms } waiters={ $waiters }
pool-log-idle-check-wait-timeout = 连接池获取等待空闲连接校验回填超时: waited_ms={ $waited_ms } in_progress={ $in_progress }
session-log-sql-parse-denied = 权限被拒绝: role={ $role } reason=sql-parse-failure (fail-closed)
session-log-permission-denied = 权限被拒绝: action={ $action } table={ $table }
session-log-rate-limited = 速率受限: 表访问被限流 (retry_after={ $retry_after })
session-log-rls-rejected = 行级安全策略被拒绝: { $error }

# --- 认证口令策略（canonical mirror，与 src/access/authentication/password.rs
# 内嵌的规范串逐字一致） ---
auth-log-policy-max-len-exceeds-bcrypt = 密码策略 max_len { $max_len } 超过 bcrypt 上限 { $limit } 字节；实际生效上限为 { $limit } 字节
auth-log-legacy-verify-exceeds-limit = 存量口令校验: 口令超过 bcrypt 上限，按存量 72 字节前缀比对
auth-log-legacy-verify-split-multibyte = 存量口令校验失败，72 字节截断切开了多字节字符: 存量哈希覆盖的字节序列不是合法 UTF-8，无法重新校验 —— 该账号需要重设口令

otel-serialize-failed = 序列化失败: { $error }
otel-http-task-failed = otlp HTTP 任务失败: { $error }
otel-endpoint-not-http = otlp 端点必须为 http://（当前: { $endpoint }）
otel-connect-failed = otlp 连接失败: { $error }
otel-write-failed = otlp 写入失败: { $error }
otel-read-failed = otlp 读取失败: { $error }
otel-collector-non-2xx = otlp collector 返回非 2xx: { $status }
otel-export-failed = otlp 导出失败: { $error }
