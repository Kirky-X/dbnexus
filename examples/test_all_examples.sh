#\!/bin/bash
set -e

mkdir -p target/tmp
export TMPDIR=$PWD/target/tmp

echo "========================================="
echo "测试所有18个示例"
echo "========================================="

examples=(
  "audit"
  "cache"
  "config"
  "entity_basic"
  "global_index"
  "health"
  "macro_usage"
  "demo"
  "metrics"
  "migration"
  "permission_engine"
  "permissions"
  "quickstart"
  "security"
  "sharding"
  "sql_parser"
  "transactions"
  "authenticated_usage"
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
      ((warnings++))
    else
      echo "✅ $example 通过"
      ((passed++))
    fi
  else
    echo "❌ $example 失败"
    ((failed++))
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
