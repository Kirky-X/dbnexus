#\!/bin/bash
set -e

mkdir -p target/tmp
export TMPDIR=$PWD/target/tmp

echo "========================================="
echo "测试所有51个示例"
echo "========================================="

# 清单与 examples/Cargo.toml 的 [[bin]] 一一对应（共 51 个，按 Cargo.toml 中的注册顺序排列）。
# 说明：graph_ladybug 未注册为 [[bin]]（需 ladybug feature，与 duckdb 存在 mbedtls 链接冲突），
# 不纳入本脚本，需按 Cargo.toml 注释单独编译。
# database_postgres / database_mysql / graph_neo4j 在无数据库服务时优雅降级退出，无需跳过。
examples=(
  # 基础模块 (basic/)
  "basic_connection"
  "basic_crud"
  "basic_transaction"
  # 权限模块 (permission/)
  "permission_rbac"
  "permission_yaml"
  "permission_macro"
  "permission_engine"
  # 安全模块 (security/)
  "sql_parser"
  "sql_injection_detection"
  "ddl_guard"
  "sensitive_masker"
  "rate_limiter"
  # 配置模块 (config/)
  "config_env"
  "config_yaml"
  "config_toml"
  "config_presets"
  # 数据库模块 (database/)
  "database_sqlite"
  "database_postgres"
  "database_mysql"
  "migration"
  "sharding"
  "global_index"
  "pool_management"
  # 可观测性模块 (observability/)
  "metrics_prometheus"
  "health_check"
  "latency_histogram"
  # 认证与审计模块 (auth/)
  "authentication_jwt"
  "authentication_password"
  "audit_logging"
  # 宏模块 (macros/)
  "macros_db_entity"
  "macros_db_crud"
  "macros_db_audit"
  "macros_db_cache"
  "macros_soft_delete_unique"
  "macros_db_entity_v2"
  "macros_advanced_query"
  # 图数据库模块 (graph/)
  "graph_neo4j"
  # 国际化模块 (common/i18n_formatting.rs)
  "i18n_formatting"
  # 集成适配器模块 (integrations/)
  "oxcache_adapter"
  # 独立缓存功能 (common/cache_standalone.rs)
  "cache_standalone"
  # Kit 统一能力管理 (kit/)
  "kit_usage"
  "kit_advanced"
  # 分布式能力 (distributed/)
  "distributed_id"
  "saga"
  "scatter_gather"
  "replica_routing"
  "shard_migration"
  # 可靠性模块 (reliability/)
  "retry"
  "failover"
  # 通用模块 (common/)
  "error_handling"
  # DuckDB 示例 (database/duckdb_query.rs)
  "duckdb_query"
)

total=${#examples[@]}
passed=0
failed=0
warnings=0

for example in "${examples[@]}"; do
  echo ""
  echo ">>> 测试: $example"
  echo "----------------------------------------"

  if cargo run --bin "$example" 2>&1 | tee /tmp/${example}.log | tail -5; then
    if grep -q "warning:" /tmp/${example}.log; then
      echo "⚠️  $example 通过但有警告"
      warnings=$((warnings + 1))
    else
      echo "✅ $example 通过"
      passed=$((passed + 1))
    fi
  else
    echo "❌ $example 失败"
    failed=$((failed + 1))
  fi
done

echo ""
echo "========================================="
echo "测试总结"
echo "========================================="
echo "总计: $total"
echo "通过: $passed"
echo "有警告: $warnings"
echo "失败: $failed"
echo "========================================="
