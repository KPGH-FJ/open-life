#!/bin/bash
# =============================================================================
# OpenLife 环境初始化脚本 (macOS / Linux)
# =============================================================================
# 用途：
#   一键初始化 OpenLife 开发环境，包括：
#   - 检查必要工具安装（Node.js、Rust、pnpm 等）
#   - 安装前端依赖（pnpm install）
#   - 安装/检查 Rust 工具链
#   - 创建 .env 配置文件
#   - 初始化数据存储目录
#   - 验证安装完整性
#
# 使用方法：
#   chmod +x scripts/setup.sh && ./scripts/setup.sh
#
# 预期时间：
#   首次运行约 2-5 分钟（主要耗时在前端依赖下载和 Rust 编译缓存生成）
#
# 常见问题：
#   Q: 提示 "pnpm not found"
#   A: corepack enable && corepack prepare pnpm@9.1.0 --activate
#
#   Q: 提示 "rustc not found"
#   A: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
#
#   Q: 提示 "xcode-select: error"
#   A: macOS 运行 xcode-select --install
#
#   Q: Tauri 构建报错缺少系统库
#   A: Linux 需安装 libwebkit2gtk-4.0-dev libssl-dev libappindicator3-dev
# =============================================================================

set -euo pipefail

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# 路径配置
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FRONTEND_DIR="$REPO_ROOT/frontend"
ENV_FILE="$REPO_ROOT/.env"
ENV_TEMPLATE="$REPO_ROOT/.env.example"

# =============================================================================
# 工具函数
# =============================================================================

log_info()    { echo -e "${BLUE}[INFO]${NC}  $1"; }
log_success() { echo -e "${GREEN}[OK]${NC}   $1"; }
log_warn()    { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error()   { echo -e "${RED}[FAIL]${NC} $1"; }
log_step()    { echo -e "\n${CYAN}▶ $1${NC}"; }

check_command() {
    if command -v "$1" &>/dev/null; then
        local version
        version=$($1 --version 2>/dev/null | head -n1 | grep -oE '[0-9]+\.[0-9]+(\.[0-9]+)?' | head -n1)
        log_success "$1 已安装${version:+ (v$version)}"
        return 0
    else
        log_error "$1 未安装"
        return 1
    fi
}

version_ge() {
    local v1="$1" v2="$2"
    if [ "$(printf '%s\n' "$v2" "$v1" | sort -V | head -n1)" = "$v2" ]; then
        return 0
    else
        return 1
    fi
}

check_version() {
    local cmd="$1" min="$2"
    local ver
    ver=$($cmd --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+(\.[0-9]+)?' | head -n1)
    if [ -z "$ver" ]; then
        log_warn "无法检测 $cmd 版本"
        return 1
    fi
    if version_ge "$ver" "$min"; then
        log_success "$cmd 版本 $ver >= $min"
        return 0
    else
        log_error "$cmd 版本 $ver < $min，请升级"
        return 1
    fi
}

# =============================================================================
# Step 1: 检查必要工具
# =============================================================================

step_check_tools() {
    log_step "Step 1/5: 检查必要工具"
    local failed=0

    check_command "node"   || failed=1
    check_command "corepack" || failed=1
    corepack pnpm --version &>/dev/null || {
        log_warn "pnpm 不可用"
        log_info "建议准备 pnpm:"
        log_info "  corepack prepare pnpm@9.1.0 --activate"
        failed=1
    }
    check_command "rustc"  || failed=1
    check_command "cargo"  || failed=1
    check_command "git"    || failed=1

    # 版本检查
    log_info "检查版本要求..."
    check_version "node"  "18.0"   || failed=1
    check_version "rustc" "1.75"   || failed=1

    if [ $failed -ne 0 ]; then
        echo ""
        log_error "环境检查未通过，请安装以下缺失依赖："
        echo ""
        echo "  Rust (>= 1.75):  https://rustup.rs/"
        echo "  Node.js (>= 18): https://nodejs.org/"
        echo "  pnpm:            corepack enable && corepack prepare pnpm@9.1.0 --activate"
        echo ""
        echo "macOS 额外依赖:"
        echo "  xcode-select --install"
        echo ""
        echo "Linux 额外依赖:"
        echo "  sudo apt-get install libwebkit2gtk-4.0-dev libssl-dev libappindicator3-dev"
        echo ""
        exit 1
    fi

    log_success "所有必要工具已就绪"
}

# =============================================================================
# Step 2: 安装前端依赖
# =============================================================================

step_install_frontend_deps() {
    log_step "Step 2/5: 安装前端依赖"

    if [ -d "$FRONTEND_DIR/node_modules" ]; then
        log_info "检测到已存在的 node_modules，跳过安装"
        log_info "如需重新安装，请删除 frontend/node_modules 后重试"
        return 0
    fi

    log_info "运行 pnpm install (位于 $FRONTEND_DIR)..."
    (cd "$FRONTEND_DIR" && corepack pnpm install)
    log_success "前端依赖安装完成"
}

# =============================================================================
# Step 3: 安装/验证 Rust 依赖
# =============================================================================

step_install_rust_deps() {
    log_step "Step 3/5: 验证 Rust 工具链"

    log_info "检查 Rust target..."
    local target
    case "$(uname -s)" in
        Darwin*) target="$(uname -m)-apple-darwin" ;;
        Linux*)  target="x86_64-unknown-linux-gnu" ;;
        *)       target="unknown" ;;
    esac

    log_info "当前平台: $target"

    # 检查 tauri-cli 是否可用
    if ! command -v tauri &>/dev/null; then
        local local_tauri="$FRONTEND_DIR/node_modules/.bin/tauri"
        if [ -f "$local_tauri" ]; then
            log_success "Tauri CLI 在 node_modules 中可用"
        else
            log_warn "Tauri CLI 未找到，将在首次启动时通过 pnpm 自动安装"
        fi
    else
        log_success "Tauri CLI 已全局安装"
    fi

    log_info "Rust 依赖将在首次构建时自动下载（由 cargo 管理）"
    log_success "Rust 工具链验证完成"
}

# =============================================================================
# Step 4: 创建 .env 配置文件
# =============================================================================

step_setup_env() {
    log_step "Step 4/5: 配置环境变量"

    if [ -f "$ENV_FILE" ]; then
        log_success ".env 文件已存在，跳过创建"
        log_info "如需重置配置，请删除 .env 后重新运行本脚本"
        return 0
    fi

    if [ -f "$ENV_TEMPLATE" ]; then
        cp "$ENV_TEMPLATE" "$ENV_FILE"
        log_success "从 .env.example 创建 .env"
    else
        log_warn ".env.example 不存在，创建空 .env"
        touch "$ENV_FILE"
    fi

    log_warn "⚠  请编辑 .env 文件，填入你的 API Key 以启用对话功能"
    log_info "  - OPENROUTER_API_KEY (推荐): https://openrouter.ai/keys"
    log_info "  - OPENAI_API_KEY (备选): https://platform.openai.com/api-keys"
}

# =============================================================================
# Step 5: 初始化数据目录
# =============================================================================

step_init_data_dir() {
    log_step "Step 5/5: 初始化数据存储"

    local data_dir
    case "$(uname -s)" in
        Darwin*)
            data_dir="${HOME}/Library/Application Support/ai.openlife.desktop"
            ;;
        Linux*)
            if [ -n "${XDG_DATA_HOME:-}" ]; then
                data_dir="${XDG_DATA_HOME}/ai.openlife.desktop"
            else
                data_dir="${HOME}/.local/share/ai.openlife.desktop"
            fi
            ;;
        *)
            data_dir="${REPO_ROOT}/.openlife"
            ;;
    esac

    if [ ! -d "$data_dir" ]; then
        mkdir -p "$data_dir"
        log_success "创建数据目录: $data_dir"
    else
        log_info "数据目录已存在: $data_dir"
    fi

    log_info "SQLite 数据库将在首次启动应用时自动建表"
    log_success "数据存储初始化完成"
}

# =============================================================================
# 验证安装
# =============================================================================

verify_installation() {
    log_step "验证安装完整性"
    local failed=0

    [ -d "$FRONTEND_DIR/node_modules" ] || { log_error "node_modules 缺失"; failed=1; }
    [ -f "$ENV_FILE" ]                   || { log_error ".env 文件缺失"; failed=1; }
    [ -f "$REPO_ROOT/Cargo.toml" ]      || { log_error "Cargo.toml 缺失"; failed=1; }
    [ -d "$REPO_ROOT/openlife-core" ]   || { log_error "openlife-core 目录缺失"; failed=1; }
    [ -d "$REPO_ROOT/src-tauri" ]       || { log_error "src-tauri 目录缺失"; failed=1; }

    if [ $failed -ne 0 ]; then
        log_error "验证未通过，请检查项目完整性"
        exit 1
    fi

    log_success "验证通过！环境初始化完成"
}

# =============================================================================
# 主逻辑
# =============================================================================

main() {
    echo -e "${CYAN}"
    echo "   ____                 __   _       __"
    echo "  / __ \____  ___  ____/ /  | |     / /___  _________ _____"
    echo " / / / / __ \/ _ \/ __  /   | | /| / / __ \/ ___/ __ \ / __ \\"
    echo "/ /_/ / /_/ /  __/ /_/ /    | |/ |/ / /_/ / /  / / / // /_/ /"
    echo "\____/ .___/\___/\__,_/     |__/|__/\____/_/  /_/ /_/ \____/"
    echo "    /_/"
    echo -e "${NC}"
    echo -e "${BLUE}OpenLife 环境初始化脚本${NC}"
    echo ""

    step_check_tools
    step_install_frontend_deps
    step_install_rust_deps
    step_setup_env
    step_init_data_dir
    verify_installation

    echo ""
    echo -e "${GREEN}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║                🎉 环境初始化完成！                           ║${NC}"
    echo -e "${GREEN}╚══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${CYAN}下一步操作：${NC}"
    echo ""
    echo "  1. 编辑 .env 文件，配置 API Key（可选但推荐）"
    echo "  2. 启动开发模式："
    echo -e "     ${YELLOW}./scripts/dev.sh${NC}     或  ${YELLOW}./scripts/startup.sh dev${NC}"
    echo ""
    echo "  或运行检查："
    echo -e "     ${YELLOW}./scripts/startup.sh check${NC}"
    echo ""
}

main "$@"
