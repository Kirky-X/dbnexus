#!/usr/bin/env bash
# 检查是否直接提交到 master 分支（main 分支允许开发提交，与 limiteron/oxcache/inklog 一致）

BRANCH=$(git rev-parse --abbrev-ref HEAD)

if [ "$BRANCH" = "master" ]; then
    echo "错误: 禁止直接提交到 master 分支。"
    echo ""
    echo "请创建功能分支后再提交:"
    echo "  git checkout -b feature/your-feature-name"
    exit 1
fi

exit 0
