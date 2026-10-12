# dbnexus operational log catalog (en).
# Keys emitted through log::*/tracing::* or diagnostic status output.

config-confers-subscribe-failed = confers subscribe failed: { $error }
config-confers-hot-reload-failed = permission hot reload failed: { $error }
config-confers-skip-invalid-event = skip invalid permission event: { $error }
config-confers-log-hot-reload = permission hot reload: { $error }
config-confers-log-ack-failed = confers ack failed: { $error }
prepare-cache-log-read-miss = PreparedStatementCache { $op } read failed, treated as a cache miss: { $error }
prepare-cache-log-write-failed = PreparedStatementCache { $op } write failed: { $error }
prepare-cache-log-clear-failed = PreparedStatementCache clear failed: { $error }

metrics-export-error = Export failed: { $reason }
metrics-not-initialized = Collector not initialized
metrics-unknown = Unknown metrics error: { $reason }
metrics-collector-not-configured = metrics collector not configured (use with_metrics_collector)
metrics-export-join-failed = metrics export task join failed: { $error }
metrics-slow-query = slow query: query_type={ $query_type } duration_ms={ $duration_ms } threshold_ms={ $threshold_ms }
metrics-feature-not-enabled = metrics feature not enabled (rebuild with the metrics feature to expose the collector endpoint)
pool-log-acquire-timeout = pool acquire timeout: waited_ms={ $waited_ms } timeout_ms={ $timeout_ms } waiters={ $waiters }
pool-log-idle-check-wait-timeout = pool acquire waited for idle-connection check: waited_ms={ $waited_ms } in_progress={ $in_progress }
session-log-sql-parse-denied = permission denied: role={ $role } reason=sql-parse-failure (fail-closed)
session-log-permission-denied = permission denied: action={ $action } table={ $table }
session-log-rate-limited = rate limited: table access throttled (retry_after={ $retry_after })

otel-serialize-failed = serialize failed: { $error }
otel-http-task-failed = otlp http task failed: { $error }
otel-endpoint-not-http = otlp endpoint must be http:// (got: { $endpoint })
otel-connect-failed = otlp connect failed: { $error }
otel-write-failed = otlp write failed: { $error }
otel-read-failed = otlp read failed: { $error }
otel-collector-non-2xx = otlp collector responded non-2xx: { $status }
otel-export-failed = otlp export failed: { $error }
