#!/bin/bash
# =============================================================================
# OpenLife 生产构建脚本 (macOS / Linux)
# =============================================================================
# 用途：
#   构建 OpenLife 桌面应用的生产版本，生成分发安装包。
#   支持 macOS、Linux 平台构建。
#
# 使用方法：
#   chmod +x scripts/start.sh && ./scripts/start.sh [target]
#
# 可选参数 target：
#   ./scripts/start.sh       - 自动检测当前平台并构建
#   ./scripts/start.sh macos - 构建 macOS universal 应用
#   ./scripts/start.sh linux - 构建 Linux 应用
#
# 构建产物：
#   src-tauri/target/release/bundle/
#
# 预期时间：
#   首次构建约 5-15 分钟（取决于机器性能）
#
# 常见问题：
#   Q: 构建失败提示缺少系统依赖
#   A: macOS: xcode-select --install
#      Linux: sudo apt-get install libwebkit2gtk-4.0-dev libssl-dev
#
#   Q: 构建产物在哪里？
#   A: 查看 src-tauri/target/release/bundle/ 目录
# =============================================================================

set -euo pipefail

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FRONTEND_DIR="$SCRIPT_DIR/frontend"
TAURI_DIR="$SCRIPT_DIR/src-tauri"
TARGET="${1:-auto}"

# 颜色输出
log_info()    { echo -e "${BLUE}[INFO]${NC}  $1"; }
log_success() { echo -e "${GREEN}[OK]${NC}   $1"; }
log_warn()    { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error()   { echo -e "${RED}[ERROR]${NC} $1"; }
log_step()    { echo -e "\n${CYAN}▶ $1${NC}"; }

# 检测平台
detect_platform() {
    case "$(uname -s)" in
        Darwin*)
            echo "macos"
            ;;
        Linux*)
            echo "linux"
            ;;
        *)
            log_error "不支持的平台: $(uname -s)"
            exit 1
            ;;
    esac
}

# 获取构建目标
case "$TARGET" in
    auto)
        TARGET=$(detect_platform)
        log_info "自动检测平台: $TARGET"
        ;;
    macos|darwin)
        TARGET="macos"
        ;;
    linux)
        TARGET="linux"
        ;;
    *)
        log_error "未知目标: $TARGET"
        echo "用法: $0 [macos|linux|auto]"
        exit 1
        ;;
esac

# 检查环境
log_step "检查构建环境"

if ! command -v node &>/dev/null; then
    log_error "Node.js 未安装"
    exit 1
fi

if ! command -v cargo &>/dev/null; then
    log_error "Rust/Cargo 未安装"
    exit 1
fi

if [ ! -d "$FRONTEND_DIR/node_modules" ]; then
    log_warn "前端依赖未安装，尝试安装..."
    (cd "$FRONTEND_DIR" && pnpm install)
fi

log_success "环境检查通过"

# 确定构建参数
case "$TARGET" in
    macos)
        BUILD_TARGET="universal-apple-darwin"
        BUILD_NAME="macOS Universal"
        ;;
    linux)
        BUILD_TARGET="x86_64-unknown-linux-gnu"
        BUILD_NAME="Linux x86_64"
        ;;
esac

# 构建
log_step "开始构建 OpenLife ($BUILD_NAME)"

echo -e "${CYAN}"
echo "   ____                 __   _       __"
echo "  / __ \____  ___  ____/ /  | |     / /___  _________ _____"
echo " / / / / __ \/ _ \/ __  /   | | /| / / __ \/ ___/ __ \ / __ \\"
echo "/ /_/ / /_/ /  __/ /_/ /    | |/ |/ / /_/ / /  / / / // /_/ /"
echo "\____/ .___/\___/\__,_/     |__/|__/\____/_/  /_/ /_/ \____/"
echo "    /_/"
echo -e "${NC}"
echo -e "${BLUE}OpenLife - 生产构建${NC}"
echo ""
echo -e "${GREEN}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║  📦 正在构建 $BUILD_NAME 版本...${NC}"
echo -e "${GREEN}║                                                              ║${NC}"
echo -e "${GREEN}║  首次构建可能需要 5-15 分钟，请耐心等待                     ║${NC}"
echo -e "${GREEN}║  产物将输出到 src-tauri/target/release/bundle/              ║${NC}"
echo -e "${GREEN}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

cd "$FRONTEND_DIR"

# 检查使用哪种方式调用 Tauri
if [ -f "$FRONTEND_DIR/node_modules/.bin/tauri" ]; then
    log_info "使用本地 Tauri CLI 构建..."
    pnpm tauri build --target "$BUILD_TARGET"
elif command -v tauri &>/dev/null; then
    log_info "使用全局 Tauri CLI 构建..."
    tauri build --target "$BUILD_TARGET"
else
    log_info "使用 npx 构建 Tauri..."
    npx tauri build --target "$BUILD_TARGET"
fi

# 检查构建结果
BUNDLE_DIR="$TAURI_DIR/target/release/bundle"
if [ -d "$BUNDLE_DIR" ]; then
    log_step "构建完成！"
    echo ""
    log_success "构建产物位于: $BUNDLE_DIR"
    echo ""
    echo -e "${CYAN}文件列表:${NC}"
    find "$BUNDLE_DIR" -maxdepth 2 -type f -exec ls -lh {} \; 2>/dev/null || true
    echo ""
else
    log_error "构建产物目录未找到，可能构建失败"
    exit 1
fi
