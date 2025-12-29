#!/bin/bash

# SQL 生成脚本
# 根据模板和变量文件生成数据库特定的初始化脚本

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEMPLATE_DIR="$SCRIPT_DIR/template"

# 检查参数
if [ $# -ne 1 ]; then
    echo "Usage: $0 <database_type>"
    echo "Supported types: postgres, mysql, sqlite"
    exit 1
fi

DB_TYPE=$1
VARS_FILE="$TEMPLATE_DIR/$DB_TYPE.vars"
TEMPLATE_FILE="$TEMPLATE_DIR/init.sql.template"
OUTPUT_FILE="$SCRIPT_DIR/init-$DB_TYPE.sql"

# 检查文件是否存在
if [ ! -f "$VARS_FILE" ]; then
    echo "Error: Variables file not found: $VARS_FILE"
    exit 1
fi

if [ ! -f "$TEMPLATE_FILE" ]; then
    echo "Error: Template file not found: $TEMPLATE_FILE"
    exit 1
fi

echo "Generating $DB_TYPE initialization script..."

# 读取变量文件
declare -A VARS
while IFS='=' read -r key value; do
    # 跳过空行和注释
    if [[ -n "$key" && ! "$key" =~ ^# ]]; then
        VARS["$key"]="$value"
    fi
done < "$VARS_FILE"

# 复制模板到输出文件
cp "$TEMPLATE_FILE" "$OUTPUT_FILE"

# 替换变量
for key in "${!VARS[@]}"; do
    value="${VARS[$key]}"
    # 使用 sed 进行替换，处理特殊字符
    sed -i "s/{{$key}}/$value/g" "$OUTPUT_FILE"
done

echo "Generated: $OUTPUT_FILE"

# 显示生成的文件的前几行作为预览
echo "Preview:"
head -20 "$OUTPUT_FILE"