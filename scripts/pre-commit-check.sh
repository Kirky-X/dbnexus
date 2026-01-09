#!/bin/bash
# =============================================================================
# DB Nexus Pre-commit Check Script
# =============================================================================
# 此脚本在 git commit 时运行，进行代码质量和格式检查
#
# 用法:
#   scripts/pre-commit-check.sh <check_type>
#
# 检查类型:
#   - cargo_check: 编译检查
#   - cargo_build: 构建检查
#   - trailing_whitespace: 行尾空白字符检查
#   - merge_conflict: 合并冲突标记检查
#   - large_files: 大文件检查
#   - added_large_files: 新增大文件检查
#   - private_keys: 私钥检测
#   - aws_keys: AWS 密钥检测
#   - all: 运行所有检查
#
# 环境变量:
#   PRE_COMMIT_TIMEOUT: 检查超时时间（秒），默认 300
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

# 计时统计
declare -A START_TIMES
declare -A END_TIMES

# 检查计数器
CHECKS_PASSED=0
CHECKS_FAILED=0
WARNINGS_COUNT=0

# 超时设置 (秒)
TIMEOUT="${PRE_COMMIT_TIMEOUT:-300}"

# 项目根目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# 日志文件
LOG_FILE="$PROJECT_ROOT/.git/pre-commit-$(date +%Y%m%d-%H%M%S).log"

# =============================================================================
# 辅助函数
# =============================================================================

log_info() {
    local msg="[INFO] $1"
    echo -e "${CYAN}${msg}${NC}"
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $msg" >> "$LOG_FILE"
}

log_success() {
    local msg="[SUCCESS] $1"
    echo -e "${GREEN}${msg}${NC}"
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $msg" >> "$LOG_FILE"
    ((CHECKS_PASSED++))
}

log_error() {
    local msg="[ERROR] $1"
    echo -e "${RED}${msg}${NC}" >&2
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $msg" >> "$LOG_FILE"
    ((CHECKS_FAILED++))
}

log_warning() {
    local msg="[WARNING] $1"
    echo -e "${YELLOW}${msg}${NC}"
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $msg" >> "$LOG_FILE"
    ((WARNINGS_COUNT++))
}

log_header() {
    echo ""
    echo -e "${BOLD}${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BOLD}${BLUE}  $1${NC}"
    echo -e "${BOLD}${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo ""
}

start_timer() {
    START_TIMES["$1"]=$(date +%s%N)
}

end_timer() {
    local name="$1"
    END_TIMES["$name"]=$(date +%s%N)
    local duration=$(( (${END_TIMES["$name"]} - ${START_TIMES["$name"]}) / 1000000 ))
    echo "  耗时: ${duration}ms"
}

print_duration() {
    local duration=$1
    if [ $duration -lt 1000 ]; then
        echo "${duration}ms"
    elif [ $duration -lt 60000 ]; then
        echo "$(echo "scale=2; $duration/1000" | bc)s"
    else
        echo "$(echo "scale=2; $duration/60000" | bc)min $(echo "scale=2; ($duration % 60000)/1000" | bc)s"
    fi
}

check_prerequisites() {
    log_header "检查先决条件"

    local missing_deps=()

    # 检查 Rust
    if ! command -v cargo &> /dev/null; then
        log_error "未找到 Rust/Cargo，请先安装 Rust: https://rustup.rs/"
        missing_deps+=("rust")
    else
        log_success "Rust 已安装: $(cargo --version)"
    fi

    # 检查 Git
    if ! command -v git &> /dev/null; then
        log_error "未找到 Git"
        missing_deps+=("git")
    else
        log_success "Git 已安装: $(git --version | head -n1)"
    fi

    # 检查 Python (用于 YAML/JSON 验证)
    if ! command -v python3 &> /dev/null && ! command -v python &> /dev/null; then
        log_warning "未找到 Python，某些检查可能不可用"
    else
        log_success "Python 已安装"
    fi

    if [ ${#missing_deps[@]} -ne 0 ]; then
        log_error "缺少必要的依赖: ${missing_deps[*]}"
        return 1
    fi

    return 0
}

# =============================================================================
# Rust 检查
# =============================================================================

check_cargo_fmt() {
    log_header "检查 Rust 代码格式化 (cargo fmt)"

    start_timer "fmt"

    if ! cargo fmt --all -- --check >> "$LOG_FILE" 2>&1; then
        log_error "代码格式化检查失败"
        echo ""
        echo -e "${YELLOW}修复建议:${NC}"
        echo "  运行以下命令格式化代码:"
        echo "    ${CYAN}cargo fmt --all${NC}"
        echo ""
        return 1
    fi

    end_timer "fmt"
    log_success "代码格式化检查通过"
    return 0
}

check_cargo_clippy() {
    log_header "检查 Rust 代码质量 (cargo clippy)"

    start_timer "clippy"

    local clippy_output
    if ! clippy_output=$(cargo clippy --all-targets --all-features --workspace -- -D warnings 2>&1); then
        echo "$clippy_output" >> "$LOG_FILE"
        log_error "Clippy 检查发现警告/错误"

        # 统计警告数量
        local warnings=$(echo "$clippy_output" | grep -c "warning:" || echo "0")
        local errors=$(echo "$clippy_output" | grep -c "error:" || echo "0")

        echo ""
        echo -e "${YELLOW}统计:${NC}"
        echo "  警告: $warnings"
        echo "  错误: $errors"
        echo ""

        # 显示部分错误示例
        echo -e "${RED}部分问题示例:${NC}"
        echo "$clippy_output" | grep -E "(error:|warning:)" | head -10

        echo ""
        echo -e "${YELLOW}修复建议:${NC}"
        echo "  1. 运行以下命令查看完整报告:"
        echo "     ${CYAN}cargo clippy --all-targets --all-features --workspace${NC}"
        echo ""
        echo "  2. 自动修复部分问题:"
        echo "     ${CYAN}cargo clippy --fix --all-targets --all-features --workspace${NC}"
        echo ""
        echo "  3. 如果是第三方依赖的警告，可以临时允许:"
        echo "     在代码中添加: #[allow(clippy::warning)]"
        echo ""

        end_timer "clippy"
        return 1
    fi

    end_timer "clippy"
    log_success "Clippy 检查通过 (零警告)"
    return 0
}

check_cargo_check() {
    log_header "检查 Rust 代码编译 (cargo check)"

    start_timer "check"

    local check_output
    if ! check_output=$(cargo check --all-features --workspace 2>&1); then
        echo "$check_output" >> "$LOG_FILE"
        log_error "编译检查失败"

        echo ""
        echo -e "${RED}编译错误:${NC}"
        echo "$check_output" | grep -E "^error" | head -10

        echo ""
        echo -e "${YELLOW}修复建议:${NC}"
        echo "  1. 查看完整编译错误:"
        echo "     ${CYAN}cargo check --all-features --workspace${NC}"
        echo ""
        echo "  2. 检查 Cargo.toml 中的依赖配置"
        echo ""

        end_timer "check"
        return 1
    fi

    end_timer "check"
    log_success "编译检查通过"
    return 0
}

check_cargo_build() {
    log_header "检查 Rust 构建 (cargo build)"

    start_timer "build"

    if ! cargo build --all-features --workspace --all-targets >> "$LOG_FILE" 2>&1; then
        log_error "构建检查失败"
        echo ""
        echo -e "${YELLOW}修复建议:${NC}"
        echo "  运行以下命令查看详细错误:"
        echo "     ${CYAN}cargo build --all-features --workspace --all-targets${NC}"
        echo ""

        end_timer "build"
        return 1
    fi

    end_timer "build"
    log_success "构建检查通过"
    return 0
}

# =============================================================================
# 通用检查
# =============================================================================

check_trailing_whitespace() {
    log_header "检查行尾空白字符"

    start_timer "trailing"

    local files_with_ws=()

    # 检查所有暂存的文本文件
    while IFS= read -r file; do
        # 跳过二进制文件
        if file "$file" 2>/dev/null | grep -q "text"; then
            if grep -q '[[:space:]]$' "$file" 2>/dev/null; then
                files_with_ws+=("$file")
            fi
        fi
    done < <(git diff --cached --name-only | grep -E '\.(rs|toml|yaml|yml|json|md|txt|sh|py)$' || true)

    if [ ${#files_with_ws[@]} -ne 0 ]; then
        log_error "发现 ${#files_with_ws[@]} 个文件包含行尾空白字符"
        echo ""
        echo -e "${RED}受影响的文件:${NC}"
        for f in "${files_with_ws[@]:0:10}"; do
            echo "  - $f"
        done
        if [ ${#files_with_ws[@]} -gt 10 ]; then
            echo "  ... 还有 $(( ${#files_with_ws[@]} - 10)) 个文件"
        fi

        echo ""
        echo -e "${YELLOW}修复建议:${NC}"
        echo "  运行以下命令自动修复:"
        echo "    ${CYAN}scripts/pre-commit-check.sh fix_trailing_whitespace${NC}"
        echo ""
        echo "  或者手动修复:"
        echo "    ${CYAN}sed -i 's/[[:space:]]*$//' <file>${NC}"
        echo ""

        end_timer "trailing"
        return 1
    fi

    end_timer "trailing"
    log_success "行尾空白字符检查通过"
    return 0
}

check_merge_conflict() {
    log_header "检查合并冲突标记"

    start_timer "conflict"

    local files_with_conflicts=()

    # 检查暂存的冲突标记
    while IFS= read -r file; do
        if grep -qE '<<<<<<<|=======|>>>>>>>' "$file" 2>/dev/null; then
            files_with_conflicts+=("$file")
        fi
    done < <(git diff --cached --name-only || true)

    if [ ${#files_with_conflicts[@]} -ne 0 ]; then
        log_error "发现 ${#files_with_conflicts[@]} 个文件包含合并冲突标记"
        echo ""
        echo -e "${RED}受影响的文件:${NC}"
        for f in "${files_with_conflicts[@]}"; do
            echo "  - $f"
        done

        echo ""
        echo -e "${YELLOW}修复建议:${NC}"
        echo "  1. 打开文件，查找并解决冲突标记:"
        echo "     - <<<<<<< HEAD"
        echo "     - ======="
        echo "     - >>>>>>> branch-name"
        echo ""
        echo "  2. 使用合并工具解决冲突:"
        echo "     ${CYAN}git mergetool${NC}"
        echo ""
        echo "  3. 解决后重新暂存文件:"
        echo "     ${CYAN}git add <file>${NC}"
        echo ""

        end_timer "conflict"
        return 1
    fi

    end_timer "conflict"
    log_success "合并冲突标记检查通过"
    return 0
}

check_large_files() {
    log_header "检查大文件 (超过 1MB)"

    start_timer "large"

    local large_files=()
    local max_size=$((1024 * 1024)) # 1MB

    # 检查暂存的大文件
    while IFS= read -r file; do
        local size
        size=$(git ls-files -s "$file" | awk '{print $4}' || echo "0")
        if [ "$size" -gt "$max_size" ]; then
            large_files+=("$file ($((size / 1024))KB)")
        fi
    done < <(git diff --cached --name-only || true)

    if [ ${#large_files[@]} -ne 0 ]; then
        log_warning "发现 ${#large_files[@]} 个大文件"
        echo ""
        echo -e "${YELLOW}大文件列表:${NC}"
        for f in "${large_files[@]}"; do
            echo "  - $f"
        done

        echo ""
        echo -e "${YELLOW}建议:${NC}"
        echo "  1. 考虑使用 Git LFS 管理大文件:"
        echo "     ${CYAN}git lfs install${NC}"
        echo "     ${CYAN}git lfs track \"*.<ext>\"${NC}"
        echo "     ${CYAN}git add .gitattributes${NC}"
        echo ""
        echo "  2. 或者从提交中移除这些文件:"
        echo "     ${CYAN}git reset HEAD <file>${NC}"
        echo "     ${CYAN}rm <file>${NC}"
        echo ""

        end_timer "large"
        return 1
    fi

    end_timer "large"
    log_success "大文件检查通过"
    return 0
}

check_added_large_files() {
    log_header "检查新增的大文件"

    start_timer "added_large"

    local large_files=()
    local max_size=$((1024 * 1024)) # 1MB

    # 检查暂存的新增文件
    while IFS= read -r file; do
        if [ -f "$file" ]; then
            local size
            size=$(stat -c%s "$file" 2>/dev/null || stat -f%z "$file" 2>/dev/null || echo "0")
            if [ "$size" -gt "$max_size" ]; then
                large_files+=("$file ($(($size / 1024))KB)")
            fi
        fi
    done < <(git diff --cached --name-only --diff-filter=A || true)

    if [ ${#large_files[@]} -ne 0 ]; then
        log_warning "发现 ${#large_files[@]} 个新增的大文件"
        echo ""
        echo -e "${YELLOW}新增大文件列表:${NC}"
        for f in "${large_files[@]}"; do
            echo "  - $f"
        done

        echo ""
        echo -e "${YELLOW}建议:${NC}"
        echo "  1. 使用 Git LFS 管理这些文件"
        echo "  2. 或者移除这些文件"

        end_timer "added_large"
        return 1
    fi

    end_timer "added_large"
    log_success "新增大文件检查通过"
    return 0
}

check_private_keys() {
    log_header "检查私钥文件"

    start_timer "private"

    local private_files=()
    local patterns=(
        "-----BEGIN RSA PRIVATE KEY-----"
        "-----BEGIN DSA PRIVATE KEY-----"
        "-----BEGIN EC PRIVATE KEY-----"
        "-----BEGIN OPENSSH PRIVATE KEY-----"
        "-----BEGIN PGP PRIVATE KEY BLOCK-----"
        "\.pem$"
        "\.key$"
        "\.priv$"
        "\.ppk$"
    )

    # 检查暂存的敏感文件
    while IFS= read -r file; do
        for pattern in "${patterns[@]}"; do
            if grep -qE "$pattern" "$file" 2>/dev/null; then
                private_files+=("$file")
                break
            fi
        done
    done < <(git diff --cached --name-only || true)

    if [ ${#private_files[@]} -ne 0 ]; then
        log_error "发现 ${#private_files[@]} 个可能的私钥文件"
        echo ""
        echo -e "${RED}警告: 不要将私钥提交到版本控制!${NC}"
        echo ""
        echo -e "${RED}受影响的文件:${NC}"
        for f in "${private_files[@]}"; do
            echo "  - $f"
        done

        echo ""
        echo -e "${YELLOW}修复建议:${NC}"
        echo "  1. 立即从暂存区移除:"
        echo "     ${CYAN}git reset HEAD <file>${NC}"
        echo "     ${CYAN}rm <file>${NC}"
        echo ""
        echo "  2. 将文件添加到 .gitignore:"
        echo "     ${CYAN}echo \"<file>\" >> .gitignore${NC}"
        echo ""
        echo "  3. 使用环境变量或密钥管理服务"
        echo ""

        end_timer "private"
        return 1
    fi

    end_timer "private"
    log_success "私钥文件检查通过"
    return 0
}

check_aws_keys() {
    log_header "检查 AWS 密钥"

    start_timer "aws"

    local aws_files=()
    local patterns=(
        "AKIA[0-9A-Z]{16}"
        "aws_access_key_id"
        "aws_secret_access_key"
        "\.aws/credentials"
    )

    # 检查暂存的文件
    while IFS= read -r file; do
        for pattern in "${patterns[@]}"; do
            if grep -qE "$pattern" "$file" 2>/dev/null; then
                aws_files+=("$file")
                break
            fi
        done
    done < <(git diff --cached --name-only || true)

    if [ ${#aws_files[@]} -ne 0 ]; then
        log_error "发现 ${#aws_files[@]} 个可能的 AWS 密钥文件"
        echo ""
        echo -e "${RED}警告: 不要将 AWS 密钥提交到版本控制!${NC}"
        echo ""
        echo -e "${RED}受影响的文件:${NC}"
        for f in "${aws_files[@]}"; do
            echo "  - $f"
        done

        echo ""
        echo -e "${YELLOW}修复建议:${NC}"
        echo "  1. 立即从暂存区移除:"
        echo "     ${CYAN}git reset HEAD <file>${NC}"
        echo "     ${CYAN}rm <file>${NC}"
        echo ""
        echo "  2. 轮换泄露的 AWS 密钥:"
        echo "     https://console.aws.amazon.com/iam/"
        echo ""

        end_timer "aws"
        return 1
    fi

    end_timer "aws"
    log_success "AWS 密钥检查通过"
    return 0
}

# =============================================================================
# 修复函数
# =============================================================================

fix_trailing_whitespace() {
    log_header "修复行尾空白字符"

    local fixed_count=0

    # 检查所有暂存的文本文件
    while IFS= read -r file; do
        # 跳过二进制文件
        if file "$file" 2>/dev/null | grep -q "text"; then
            if grep -q '[[:space:]]$' "$file" 2>/dev/null; then
                sed -i 's/[[:space:]]*$//' "$file"
                git add "$file"
                ((fixed_count++))
                echo "  已修复: $file"
            fi
        fi
    done < <(git diff --cached --name-only | grep -E '\.(rs|toml|yaml|yml|json|md|txt|sh|py)$' || true)

    if [ $fixed_count -gt 0 ]; then
        log_success "已修复 $fixed_count 个文件的行尾空白字符"
        echo ""
        echo "请重新暂存修改后的文件:"
        echo "  ${CYAN}git add <files>${NC}"
    else
        log_info "没有需要修复的文件"
    fi
}

# =============================================================================
# 完整检查
# =============================================================================

run_all_checks() {
    log_header "开始完整的 Pre-commit 检查"

    echo "项目根目录: $PROJECT_ROOT"
    echo "超时设置: ${TIMEOUT}秒"
    echo "日志文件: $LOG_FILE"
    echo ""

    # 确保日志目录存在
    mkdir -p "$(dirname "$LOG_FILE")"

    # 检查先决条件
    if ! check_prerequisites; then
        log_error "先决条件检查失败"
        return 1
    fi

    echo ""
    local total_start=$(date +%s%N)

    # 运行 Rust 检查
    echo ""
    log_info "开始 Rust 代码检查..."

    local rust_checks=(
        "check_cargo_fmt"
        "check_cargo_clippy"
        "check_cargo_check"
        "check_cargo_build"
    )

    for check in "${rust_checks[@]}"; do
        $check || true
    done

    # 运行通用检查
    echo ""
    log_info "开始通用检查..."

    local common_checks=(
        "check_trailing_whitespace"
        "check_merge_conflict"
        "check_large_files"
        "check_added_large_files"
        "check_private_keys"
        "check_aws_keys"
    )

    for check in "${common_checks[@]}"; do
        $check || true
    done

    local total_end=$(date +%s%N)
    local total_duration=$(( (total_end - total_start) / 1000000 ))

    # 打印汇总
    echo ""
    echo -e "${BOLD}${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BOLD}${BLUE}  检查汇总${NC}"
    echo -e "${Bold}${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo ""
    echo -e "  ${GREEN}通过: $CHECKS_PASSED${NC}"
    echo -e "  ${RED}失败: $CHECKS_FAILED${NC}"
    echo -e "  ${YELLOW}警告: $WARNINGS_COUNT${NC}"
    echo "  总耗时: $(print_duration $total_duration)"
    echo "  日志文件: $LOG_FILE"
    echo ""

    if [ $CHECKS_FAILED -gt 0 ]; then
        echo -e "${RED}═══════════════════════════════════════════════════════════════${NC}"
        echo -e "${RED}  检查失败! 请修复上述问题后重新提交${NC}"
        echo -e "${RED}═══════════════════════════════════════════════════════════════${NC}"
        return 1
    fi

    if [ $WARNINGS_COUNT -gt 0 ]; then
        echo -e "${YELLOW}═══════════════════════════════════════════════════════════════${NC}"
        echo -e "${YELLOW}  检查完成，但有警告。建议查看日志了解更多详情${NC}"
        echo -e "${YELLOW}═══════════════════════════════════════════════════════════════${NC}"
        return 0
    fi

    echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}  所有检查通过! 代码可以提交${NC}"
    echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
    return 0
}

# =============================================================================
# 帮助信息
# =============================================================================

show_help() {
    cat << EOF
DB Nexus Pre-commit 检查脚本

用法: $(basename "$0") <check_type> [选项]

检查类型:
  cargo_check           运行 cargo check
  cargo_build           运行 cargo build
  cargo_fmt             运行 cargo fmt (检查模式)
  cargo_clippy          运行 cargo clippy
  trailing_whitespace   检查行尾空白字符
  merge_conflict        检查合并冲突标记
  large_files           检查大文件
  added_large_files     检查新增大文件
  private_keys          检查私钥文件
  aws_keys              检查 AWS 密钥
  fix_trailing_whitespace  修复行尾空白字符
  all                   运行所有检查

选项:
  --help, -h            显示帮助信息
  --verbose, -v         显示详细输出
  --timeout SECONDS     设置超时时间 (默认: 300)

环境变量:
  PRE_COMMIT_TIMEOUT    超时时间 (秒)

示例:
  $(basename "$0") all                    # 运行所有检查
  $(basename "$0") cargo_clippy           # 只运行 clippy
  $(basename "$0") fix_trailing_whitespace  # 修复行尾空白字符
  PRE_COMMIT_TIMEOUT=600 $(basename "$0") all  # 设置超时为 10 分钟

EOF
}

# =============================================================================
# 主程序
# =============================================================================

main() {
    local check_type="${1:-all}"

    # 切换到项目根目录
    cd "$PROJECT_ROOT"

    # 设置日志文件权限
    touch "$LOG_FILE"
    chmod 600 "$LOG_FILE" 2>/dev/null || true

    case "$check_type" in
        --help|-h)
            show_help
            exit 0
            ;;
        cargo_check)
            check_cargo_check
            ;;
        cargo_build)
            check_cargo_build
            ;;
        cargo_fmt)
            check_cargo_fmt
            ;;
        cargo_clippy)
            check_cargo_clippy
            ;;
        trailing_whitespace)
            check_trailing_whitespace
            ;;
        merge_conflict)
            check_merge_conflict
            ;;
        large_files)
            check_large_files
            ;;
        added_large_files)
            check_added_large_files
            ;;
        private_keys)
            check_private_keys
            ;;
        aws_keys)
            check_aws_keys
            ;;
        fix_trailing_whitespace)
            fix_trailing_whitespace
            ;;
        all)
            run_all_checks
            ;;
        *)
            log_error "未知的检查类型: $check_type"
            echo ""
            show_help
            exit 1
            ;;
    esac
}

# 运行主程序
main "$@"
