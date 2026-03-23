#!/usr/bin/env bash
# 检查是否直接提交到 main 分支

BRANCH=$(git rev-parse --abbrev-ref HEAD)

if [ "$BRANCH" = "main" ]; then
    echo "错误: 禁止直接提交到 main 分支。"
    echo ""
    echo "请创建功能分支后再提交:"
    echo "  git checkout -b feature/your-feature-name"
    exit 1
fi

exit 0
