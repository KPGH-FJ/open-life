#!/bin/bash
# =============================================================================
# OpenLife 启动脚本 (macOS / Linux)
# =============================================================================
# 使用方法:
#   chmod +x startup.sh
#   ./startup.sh [dev|a2a|check]
#
# 命令说明:
#   ./startup.sh dev    - 启动 Tauri 桌面应用开发模式（默认）
#   ./startup.sh a2a    - 启动独立 A2A 服务器
#   ./startup.sh check  - 仅检查环境依赖，不启动应用
#
# 前提条件:
#   - Rust >= 1.75    (https://rustup.rs/)
#   - Node.js >= 18   (https://nodejs.org/)
#   - pnpm >= 8（推荐）或 npm（已内置 fallback）
#   - Tauri CLI       (pnpm add -g @tauri-apps/cli)
#   - Ollama (可选)   (https://ollama.com/)
#
# 常见问题:
#   Q: 提示 "command not found: pnpm"
#   A: 可以直接使用 npm fallback 继续启动；如需 pnpm 可运行 "npm install -g pnpm"
#
#   Q: 提示 "Rust compiler not found"
#   A: 访问 https://rustup.rs/ 安装 Rust
#
#   Q: Tauri 构建失败/报错
#   A: 确保已安装系统依赖:
#      macOS: Xcode Command Line Tools (xcode-select --install)
#      Linux: libwebkit2gtk-4.0-dev, libssl-dev, etc.
#      详见 https://tauri.app/start/prerequisites/
#
#   Q: 对话功能不可用
#   A: 1) 启动 Ollama 并拉取模型: ollama pull qwen2.5:7b
#      2) 或在 .env 中配置 OPENROUTER_API_KEY
#
# =============================================================================

set -euo pipefail

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# 配置
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FRONTEND_DIR="$SCRIPT_DIR/frontend"
TAURI_DIR="$SCRIPT_DIR/src-tauri"
ENV_FILE="$SCRIPT_DIR/.env"
A2A_PORT="${A2A_PORT:-8765}"
VITE_PORT="${PORT:-5173}"

# =============================================================================
# 工具函数
# =============================================================================

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[OK]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_step() {
    echo -e "\n${CYAN}▶ $1${NC}"
}

check_command() {
    if command -v "$1" &>/dev/null; then
        log_success "$1 已安装 ($(command -v "$1"))"
        return 0
    else
        log_error "$1 未安装"
        return 1
    fi
}

check_version() {
    local cmd="$1"
    local min_version="$2"
    local current_version

    current_version=$($cmd --version 2>/dev/null | head -n1 | grep -oE '[0-9]+\.[0-9]+(\.[0-9]+)?' | head -n1)

    if [ -z "$current_version" ]; then
        log_warn "无法检测 $cmd 版本"
        return 1
    fi

    # 简单版本比较 (major.minor)
    local current_major current_minor min_major min_minor
    current_major=$(echo "$current_version" | cut -d. -f1)
    current_minor=$(echo "$current_version" | cut -d. -f2)
    min_major=$(echo "$min_version" | cut -d. -f1)
    min_minor=$(echo "$min_version" | cut -d. -f2)

    if [ "$current_major" -gt "$min_major" ] || \
       ([ "$current_major" -eq "$min_major" ] && [ "$current_minor" -ge "$min_minor" ]); then
        log_success "$cmd 版本 $current_version >= $min_version"
        return 0
    else
        log_error "$cmd 版本 $current_version < $min_version，需要升级"
        return 1
    fi
}

check_port() {
    local port="$1"
    if lsof -Pi ":$port" -sTCP:LISTEN -t >/dev/null 2>&1 || \
       netstat -tuln 2>/dev/null | grep -q ":$port " || \
       ss -tuln 2>/dev/null | grep -q ":$port "; then
        log_warn "端口 $port 已被占用"
        return 1
    else
        log_success "端口 $port 可用"
        return 0
    fi
}

wait_for_port() {
    local port="$1"
    local timeout="${2:-30}"
    local start_time=$(date +%s)

    log_info "等待端口 $port 就绪..."
    while ! curl -s "http://localhost:$port" >/dev/null 2>&1; do
        local current_time=$(date +%s)
        if [ $((current_time - start_time)) -ge "$timeout" ]; then
            log_error "端口 $port 在 ${timeout} 秒内未就绪"
            return 1
        fi
        sleep 1
    done
    log_success "端口 $port 已就绪"
}

# =============================================================================
# 环境检查
# =============================================================================

check_environment() {
    log_step "检查开发环境"
    local has_errors=0

    # 检查核心依赖
    check_command "node" || has_errors=1
    if check_command "pnpm"; then
        :
    elif check_command "npm"; then
        log_warn "pnpm 未安装，将使用 npm fallback 启动前端 Tauri CLI"
    else
        has_errors=1
    fi
    check_command "rustc" || has_errors=1
    check_command "cargo" || has_errors=1

    # 检查版本
    log_info "检查版本要求..."
    check_version "node" "18.0" || has_errors=1
    check_version "rustc" "1.75" || has_errors=1

    # 检查 Tauri CLI
    if ! check_command "tauri"; then
        if [ -f "$FRONTEND_DIR/node_modules/.bin/tauri" ]; then
            log_success "Tauri CLI 存在于 node_modules (可使用 pnpm 或 npm exec tauri)"
        else
            log_warn "Tauri CLI 未全局安装，将在首次运行时通过 pnpm 安装"
        fi
    fi

    # 检查 Ollama（可选）
    if check_command "ollama"; then
        log_info "Ollama 已安装"
        if curl -s "http://localhost:11434/api/tags" >/dev/null 2>&1; then
            log_success "Ollama 服务正在运行 (http://localhost:11434)"
        else
            log_warn "Ollama 已安装但未运行，启动后可用本地模型"
            log_info "  提示: ollama serve &"
        fi
    else
        log_warn "Ollama 未安装（可选，用于本地模型）"
        log_info "  安装: https://ollama.com/"
    fi

    # 检查可选工具
    check_command "python3" || log_warn "python3 未安装（仅影响量化脚本）"

    if [ $has_errors -ne 0 ]; then
        echo ""
        log_error "环境检查未通过，请安装缺失的依赖"
        echo ""
        echo "快速安装指南:"
        echo "  Rust:    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        echo "  Node.js: https://nodejs.org/"
        echo "  pnpm:    npm install -g pnpm   (推荐，可选)"
        echo "  Tauri:   pnpm add -g @tauri-apps/cli  或使用项目内 CLI"
        exit 1
    fi

    log_success "环境检查通过"
}

# =============================================================================
# 环境变量设置
# =============================================================================

setup_env() {
    log_step "配置环境变量"

    # 创建 .env 文件（如果不存在）
    if [ ! -f "$ENV_FILE" ]; then
        if [ -f "$SCRIPT_DIR/.env.template" ]; then
            cp "$SCRIPT_DIR/.env.template" "$ENV_FILE"
            log_success "从模板创建 .env 文件"
            log_info "请编辑 .env 文件配置你的 API Key"
        else
            log_warn ".env.template 不存在，创建空 .env"
            touch "$ENV_FILE"
        fi
    else
        log_success ".env 文件已存在"
    fi

    # 加载 .env
    if [ -f "$ENV_FILE" ]; then
        # 安全地加载环境变量（忽略注释和空行）
        set -a
        while IFS='=' read -r key value; do
            [[ -z "$key" || "$key" =~ ^# ]] && continue
            # 去除空格和引号
            key=$(echo "$key" | xargs)
            value=$(echo "$value" | sed -e 's/^["\x27]//' -e 's/["\x27]$//' | xargs)
            export "$key=$value"
        done < "$ENV_FILE"
        set +a
        log_success "已加载 .env 环境变量"
    fi

    # 检查 API Key 配置
    if [ -z "${OPENROUTER_API_KEY:-}" ] && [ -z "${OPENAI_API_KEY:-}" ]; then
        log_warn "未配置 LLM API Key，云端模型不可用"
        log_info "  请在 .env 中设置 OPENROUTER_API_KEY 或 OPENAI_API_KEY"
        log_info "  获取 OpenRouter Key: https://openrouter.ai/keys"
    else
        log_success "API Key 已配置"
    fi
}

# =============================================================================
# 依赖安装
# =============================================================================

install_dependencies() {
    log_step "安装前端依赖"

    if [ ! -d "$FRONTEND_DIR/node_modules" ]; then
        log_info "首次安装，运行 pnpm install..."
        (cd "$FRONTEND_DIR" && pnpm install)
        log_success "前端依赖安装完成"
    else
        log_success "前端依赖已安装"
    fi

    log_step "检查 Rust 依赖"
    if [ ! -d "$SCRIPT_DIR/target" ]; then
        log_info "首次构建，Rust 依赖将在启动时自动编译..."
    else
        log_success "Rust 构建缓存已存在"
    fi
}

# =============================================================================
# 数据库初始化（SQLite 自动建表，无需手动迁移）
# =============================================================================

init_database() {
    log_step "初始化数据存储"

    local data_dir
    if [ "$(uname)" = "Darwin" ]; then
        data_dir="${HOME}/Library/Application Support/com.openlife.app"
    elif [ -n "${XDG_DATA_HOME:-}" ]; then
        data_dir="${XDG_DATA_HOME}/com.openlife.app"
    else
        data_dir="${HOME}/.local/share/com.openlife.app"
    fi

    if [ ! -d "$data_dir" ]; then
        mkdir -p "$data_dir"
        log_success "创建数据目录: $data_dir"
    else
        log_success "数据目录已存在: $data_dir"
    fi

    log_info "SQLite 数据库将在首次启动时自动建表"
}

# =============================================================================
# 启动应用
# =============================================================================

start_dev() {
    log_step "启动 OpenLife 开发模式"

    # 检查端口
    check_port "$VITE_PORT" || {
        log_error "Vite 端口 $VITE_PORT 被占用，请修改 PORT 环境变量或关闭占用进程"
        exit 1
    }

    echo ""
    echo -e "${GREEN}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║           OpenLife 正在启动...                               ║${NC}"
    echo -e "${GREEN}║                                                              ║${NC}"
    echo -e "${GREEN}║  首次启动可能需要 1-3 分钟编译 Rust 代码                     ║${NC}"
    echo -e "${GREEN}║  请耐心等待...                                               ║${NC}"
    echo -e "${GREEN}╚══════════════════════════════════════════════════════════════╝${NC}"
    echo ""

    cd "$SCRIPT_DIR"

    # 检查使用哪种方式启动 Tauri
    if [ -f "$FRONTEND_DIR/node_modules/.bin/tauri" ]; then
        log_info "使用本地 Tauri CLI 启动..."
        pnpm --dir "$FRONTEND_DIR" tauri dev
    elif command -v tauri &>/dev/null; then
        log_info "使用全局 Tauri CLI 启动..."
        tauri dev
    else
        log_info "使用 npx 启动 Tauri..."
        cd "$FRONTEND_DIR" && npx tauri dev
    fi
}

start_a2a() {
    log_step "启动 A2A 独立服务器"

    # 检查端口
    check_port "$A2A_PORT" || {
        log_error "A2A 端口 $A2A_PORT 被占用"
        log_info "可设置环境变量: A2A_PORT=9999 ./startup.sh a2a"
        exit 1
    }

    log_info "A2A 服务器将监听: http://127.0.0.1:$A2A_PORT"
    log_info "API 端点:"
    log_info "  GET  http://127.0.0.1:$A2A_PORT/agent.json"
    log_info "  POST http://127.0.0.1:$A2A_PORT/tasks/send"

    cd "$TAURI_DIR"
    cargo run --bin openlife-a2a-server
}

# =============================================================================
# 主逻辑
# =============================================================================

main() {
    local command="${1:-dev}"

    echo -e "${CYAN}"
    echo "   ____                 __   _       __"
    echo "  / __ \____  ___  ____/ /  | |     / /___  _________ _____"
    echo " / / / / __ \/ _ \/ __  /   | | /| / / __ \/ ___/ __ \ / __ \\"
    echo "/ /_/ / /_/ /  __/ /_/ /    | |/ |/ / /_/ / /  / / / // /_/ /"
    echo "\____/ .___/\___/\__,_/     |__/|__/\____/_/  /_/ /_/ \____/"
    echo "    /_/"
    echo -e "${NC}"
    echo -e "${BLUE}OpenLife - 你的终身成长合伙人${NC}"
    echo ""

    case "$command" in
        check)
            check_environment
            setup_env
            echo ""
            log_success "环境检查完成！可以运行 ./startup.sh dev 启动应用"
            ;;
        dev)
            check_environment
            setup_env
            install_dependencies
            init_database
            start_dev
            ;;
        a2a)
            check_environment
            setup_env
            start_a2a
            ;;
        *)
            echo "用法: $0 [dev|a2a|check]"
            echo ""
            echo "  dev    - 启动 Tauri 桌面应用开发模式（默认）"
            echo "  a2a    - 启动独立 A2A 服务器"
            echo "  check  - 仅检查环境依赖"
            echo ""
            exit 1
            ;;
    esac
}

# 执行主函数
main "$@"
