#!/bin/bash
# =============================================================================
# DB Nexus Pre-commit 安装脚本
# =============================================================================
# 此脚本安装 pre-commit hooks 和必要的依赖
#
# 用法:
#   ./scripts/install-pre-commit.sh [选项]
#
# 选项:
#   --force            强制重新安装
#   --update           更新到最新版本的 hooks
#   --uninstall        卸载 pre-commit hooks
#   --check-only       只检查先决条件，不安装
#   --help, -h         显示帮助信息
#
# =============================================================================

set -euo pipefail

# 颜色定义
readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly BLUE='\033[0;34m'
readonly CYAN='\033[0;36m'
readonly BOLD='\033[1m'
readonly NC='\033[0m' # No Color

# 项目配置
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
PRE_COMMIT_CONFIG="$PROJECT_ROOT/.pre-commit-config.yaml"
PRE_COMMIT_HOOKS_DIR="$PROJECT_ROOT/.git/hooks"
PRE_COMMIT_YAML="$PROJECT_ROOT/.pre-commit-hooks.yaml"

# 选项
FORCE_INSTALL=false
UPDATE_ONLY=false
UNINSTALL_ONLY=false
CHECK_ONLY=false

# =============================================================================
# 辅助函数
# =============================================================================

log_info() {
    echo -e "${CYAN}[INFO] $1${NC}"
}

log_success() {
    echo -e "${GREEN}[SUCCESS] $1${NC}"
}

log_error() {
    echo -e "${RED}[ERROR] $1${NC}" >&2
}

log_warning() {
    echo -e "${YELLOW}[WARNING] $1${NC}"
}

log_header() {
    echo ""
    echo -e "${BOLD}${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BOLD}${BLUE}  $1${NC}"
    echo -e "${BOLD}${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo ""
}

check_prerequisites() {
    log_header "检查先决条件"

    local missing_deps=()

    # 检查 Rust
    if command -v rustc &> /dev/null; then
        log_success "Rust 已安装: $(rustc --version | head -n1)"
    else
        log_warning "未找到 Rust"
        log_info "安装方法: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        missing_deps+=("rust")
    fi

    # 检查 Git
    if command -v git &> /dev/null; then
        log_success "Git 已安装: $(git --version | head -n1)"
    else
        log_error "未找到 Git，请先安装 Git"
        missing_deps+=("git")
    fi

    # 检查 Python (用于 pre-commit)
    if command -v python3 &> /dev/null; then
        log_success "Python 3 已安装: $(python3 --version)"
    elif command -v python &> /dev/null; then
        log_warning "找到 Python ($(python --version | head -n1))，但建议使用 Python 3"
    else
        log_warning "未找到 Python，pre-commit 可能无法正常工作"
        log_info "安装方法: https://www.python.org/downloads/"
    fi

    # 检查 cargo (用于 Rust 检查)
    if command -v cargo &> /dev/null; then
        log_success "Cargo 已安装: $(cargo --version)"
    else
        log_warning "未找到 Cargo，Rust 相关检查将不可用"
    fi

    if [ ${#missing_deps[@]} -ne 0 ]; then
        log_warning "缺少以下依赖: ${missing_deps[*]}"
        echo ""
        echo "建议先安装缺失的依赖再继续。"
    fi

    echo ""
    return 0
}

install_python_deps() {
    log_info "安装 Python 依赖..."

    # 确保 pip 可用
    if ! command -v pip3 &> /dev/null && ! command -v pip &> /dev/null; then
        log_info "尝试安装 pip..."
        if command -v python3 &> /dev/null; then
            python3 -m ensurepip --upgrade 2>/dev/null || {
                curl https://bootstrap.pypa.io/get-pip.py -o /tmp/get-pip.py
                python3 /tmp/get-pip.py
                rm /tmp/get-pip.py
            }
        fi
    fi

    # 安装 pre-commit
    log_info "安装 pre-commit..."
    if command -v pip3 &> /dev/null; then
        pip3 install --upgrade pre-commit
    elif command -v pip &> /dev/null; then
        pip install --upgrade pre-commit
    elif command -v python3 &> /dev/null; then
        python3 -m pip install --upgrade pre-commit
    else
        log_error "无法安装 pre-commit: 未找到 pip 或 python3"
        return 1
    fi

    log_success "pre-commit 安装完成"
}

install_pre_commit_hooks() {
    log_header "安装 Pre-commit Hooks"

    # 确保在项目根目录
    cd "$PROJECT_ROOT"

    # 检查配置文件
    if [ ! -f "$PRE_COMMIT_CONFIG" ]; then
        log_error "未找到配置文件: $PRE_COMMIT_CONFIG"
        return 1
    fi

    # 验证配置文件语法
    log_info "验证配置文件语法..."
    if command -v python3 &> /dev/null; then
        if ! python3 -c "import yaml; yaml.safe_load(open('$PRE_COMMIT_CONFIG'))" 2>/dev/null; then
            log_error "配置文件语法错误: $PRE_COMMIT_CONFIG"
            return 1
        fi
    fi
    log_success "配置文件语法正确"

    # 安装/更新 hooks
    log_info "安装 pre-commit hooks..."

    if [ "$FORCE_INSTALL" = true ]; then
        log_info "强制重新安装 (--force)"
        pre-commit clean
    fi

    if ! pre-commit install; then
        log_error "安装 pre-commit hooks 失败"
        return 1
    fi

    # 设置 hook 脚本权限
    chmod +x "$PRE_COMMIT_HOOKS_DIR/pre-commit" 2>/dev/null || true

    log_success "pre-commit hooks 安装完成"

    # 安装其他 hook 类型
    log_info "安装 commit-msg hook..."
    pre-commit install --hook-type commit-msg 2>/dev/null || true

    log_info "安装 pre-push hook..."
    pre-commit install --hook-type pre-push 2>/dev/null || true
}

update_pre_commit() {
    log_header "更新 Pre-commit"

    # 更新 pre-commit 本身
    log_info "更新 pre-commit 包..."
    if command -v pip3 &> /dev/null; then
        pip3 install --upgrade pre-commit
    elif command -v pip &> /dev/null; then
        pip install --upgrade pre-commit
    fi

    # 更新 hooks 到最新版本
    log_info "更新 hooks 配置..."
    pre-commit autoupdate

    # 重新安装 hooks
    log_info "重新安装 hooks..."
    pre-commit clean
    pre-commit install

    log_success "pre-commit 更新完成"
}

uninstall_pre_commit() {
    log_header "卸载 Pre-commit"

    cd "$PROJECT_ROOT"

    # 移除 hooks
    log_info "移除 pre-commit hooks..."
    pre-commit uninstall 2>/dev/null || true

    # 移除 hook 文件
    rm -f "$PRE_COMMIT_HOOKS_DIR/pre-commit" 2>/dev/null || true
    rm -f "$PRE_COMMIT_HOOKS_DIR/commit-msg" 2>/dev/null || true
    rm -f "$PRE_COMMIT_HOOKS_DIR/pre-push" 2>/dev/null || true

    log_success "pre-commit hooks 已卸载"
    log_info "注意: pre-commit 包本身仍保留，如需卸载请运行: pip uninstall pre-commit"
}

verify_installation() {
    log_header "验证安装"

    cd "$PROJECT_ROOT"

    # 检查 pre-commit 是否可用
    if ! command -v pre-commit &> /dev/null; then
        log_error "pre-commit 命令不可用"
        return 1
    fi

    log_success "pre-commit 命令可用: $(pre-commit --version)"

    # 检查 hooks 是否安装
    if [ -f "$PRE_COMMIT_HOOKS_DIR/pre-commit" ]; then
        log_success "pre-commit hook 已安装"
    else
        log_error "pre-commit hook 未安装"
        return 1
    fi

    # 运行快速验证
    log_info "运行快速验证..."
    if pre-commit run --all-files --show-diff-on-failure 2>&1 | head -50; then
        log_success "验证完成"
    else
        log_warning "验证过程中发现问题，请检查输出"
    fi

    return 0
}

create_custom_hook_script() {
    log_info "创建自定义 hook 脚本..."

    # 确保脚本存在且可执行
    if [ -f "$SCRIPT_DIR/pre-commit-check.sh" ]; then
        chmod +x "$SCRIPT_DIR/pre-commit-check.sh"
        log_success "自定义脚本已准备就绪: $SCRIPT_DIR/pre-commit-check.sh"
    else
        log_error "未找到自定义脚本: $SCRIPT_DIR/pre-commit-check.sh"
    fi
}

print_summary() {
    log_header "安装完成!"

    echo ""
    echo -e "${BOLD}使用说明:${NC}"
    echo ""
    echo "1. 提交代码时，pre-commit hooks 会自动运行"
    echo ""
    echo "2. 运行所有检查:"
    echo "   ${CYAN}./scripts/pre-commit-check.sh all${NC}"
    echo ""
    echo "3. 运行特定检查:"
    echo "   ${CYAN}./scripts/pre-commit-check.sh cargo_clippy${NC}"
    echo "   ${CYAN}./scripts/pre-commit-check.sh cargo_fmt${NC}"
    echo ""
    echo "4. 跳过 pre-commit hooks:"
    echo "   ${CYAN}git commit --no-verify -m \"message\"${NC}"
    echo ""
    echo "5. 更新 hooks:"
    echo "   ${CYAN}pre-commit autoupdate${NC}"
    echo "   ${CYAN}pre-commit run --all-files${NC}"
    echo ""
    echo "6. 查看日志:"
    echo "   ${CYAN}ls -la .git/hooks/pre-commit-*.log${NC}"
    echo ""

    echo -e "${BOLD}${GREEN}Happy coding! 🚀${NC}"
    echo ""
}

show_help() {
    cat << EOF
DB Nexus Pre-commit 安装脚本

用法: $(basename "$0") [选项]

选项:
  --force, -f         强制重新安装（清除缓存）
  --update, -u        更新到最新版本的 hooks
  --uninstall         卸载 pre-commit hooks
  --check-only, -c    只检查先决条件，不安装
  --help, -h          显示帮助信息

示例:
  $(basename "$0")                    # 安装 pre-commit hooks
  $(basename "$0") --force            # 强制重新安装
  $(basename "$0") --update           # 更新 hooks
  $(basename "$0") --uninstall        # 卸载 hooks
  $(basename "$0") --check-only       # 检查先决条件

功能:
  - Rust 代码格式化检查 (cargo fmt)
  - Rust 代码质量检查 (cargo clippy)
  - Rust 编译检查 (cargo check)
  - YAML/TOML 格式检查
  - 行尾空白字符检查
  - 合并冲突标记检测
  - 大文件检测
  - 敏感信息检测 (私钥、AWS 密钥)

EOF
}

# =============================================================================
# 主程序
# =============================================================================

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --force|-f)
                FORCE_INSTALL=true
                shift
                ;;
            --update|-u)
                UPDATE_ONLY=true
                shift
                ;;
            --uninstall)
                UNINSTALL_ONLY=true
                shift
                ;;
            --check-only|-c)
                CHECK_ONLY=true
                shift
                ;;
            --help|-h)
                show_help
                exit 0
                ;;
            *)
                log_error "未知选项: $1"
                show_help
                exit 1
                ;;
        esac
    done
}

main() {
    parse_args "$@"

    log_header "DB Nexus Pre-commit 安装程序"

    echo "项目根目录: $PROJECT_ROOT"
    echo "配置文件: $PRE_COMMIT_CONFIG"
    echo ""

    # 检查先决条件
    check_prerequisites

    if [ "$CHECK_ONLY" = true ]; then
        log_info "只检查先决条件，跳过安装"
        exit 0
    fi

    # 卸载
    if [ "$UNINSTALL_ONLY" = true ]; then
        uninstall_pre_commit
        exit 0
    fi

    # 更新
    if [ "$UPDATE_ONLY" = true ]; then
        update_pre_commit
        exit 0
    fi

    # 安装 Python 依赖
    install_python_deps || {
        log_warning "Python 依赖安装失败，尝试继续..."
    }

    # 创建自定义 hook 脚本
    create_custom_hook_script

    # 安装 hooks
    install_pre_commit_hooks || {
        log_error "安装 hooks 失败"
        exit 1
    }

    # 验证安装
    verify_installation || {
        log_warning "验证过程中发现问题"
    }

    # 打印汇总
    print_summary
}

# 运行主程序
main "$@"
