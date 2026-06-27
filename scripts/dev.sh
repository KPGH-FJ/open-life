#!/bin/bash
# =============================================================================
# OpenLife 开发模式启动脚本 (macOS / Linux)
# =============================================================================
# 用途：
#   以开发模式启动 OpenLife 桌面应用，包含热重载、调试输出。
#   自动选择可用的 Tauri CLI 启动方式。
#
# 使用方法：
#   chmod +x scripts/dev.sh && ./scripts/dev.sh
#   或: ./scripts/startup.sh dev
#
# 前提条件：
#   - 已完成环境初始化 (./scripts/setup.sh)
#   - 已配置 API Key（可选但推荐）
#
# 常见问题：
#   Q: 首次启动很慢
#   A: 首次需要编译 Rust 代码，耗时 1-3 分钟，请耐心等待
#
#   Q: 端口 5173 被占用
#   A: 设置环境变量 PORT=5174 ./scripts/dev.sh
#
#   Q: 白屏或前端报错
#   A: 检查 frontend/node_modules 是否存在，运行 ./scripts/setup.sh 重新安装
# =============================================================================

set -euo pipefail

# 颜色
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FRONTEND_DIR="$REPO_ROOT/frontend"
DEV_FRONTEND_DIST="$REPO_ROOT/target/openlife-dev/frontend-dist-placeholder"

# 加载 .env
ENV_FILE="$REPO_ROOT/.env"
if [ -f "$ENV_FILE" ]; then
    set -a
    while IFS='=' read -r key value; do
        [[ -z "$key" || "$key" =~ ^# ]] && continue
        key=$(echo "$key" | xargs)
        value=$(echo "$value" | sed -e 's/^["\x27]//' -e 's/["\x27]$//' | xargs)
        export "$key=$value"
    done < "$ENV_FILE"
    set +a
fi

OPENLIFE_PROFILE="${OPENLIFE_PROFILE:-dev}"
export OPENLIFE_PROFILE
if [ -z "${A2A_PORT:-}" ]; then
    if [ "$OPENLIFE_PROFILE" = "dev" ]; then
        A2A_PORT="8766"
    else
        A2A_PORT="8765"
    fi
    export A2A_PORT
fi
VITE_PORT="${PORT:-5173}"
OPENLIFE_DEV_URL="http://127.0.0.1:$VITE_PORT"
OPENLIFE_FRONTEND_DIST="$DEV_FRONTEND_DIST"
export OPENLIFE_DEV_URL OPENLIFE_FRONTEND_DIST OPENLIFE_FRONTEND_MODE="dev_server"

mkdir -p "$DEV_FRONTEND_DIST"
cat > "$DEV_FRONTEND_DIST/index.html" <<'HTML'
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <title>OpenLife Dev Server Placeholder</title>
  </head>
  <body>
    OpenLife dev server placeholder. If you see this, Vite did not load.
  </body>
</html>
HTML

json_escape() {
    printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

FRONTEND_DIR_JSON="$(json_escape "$FRONTEND_DIR")"
DEV_URL_JSON="$(json_escape "$OPENLIFE_DEV_URL")"
FRONTEND_DIST_JSON="$(json_escape "$OPENLIFE_FRONTEND_DIST")"
TAURI_CONFIG_OVERRIDE=$(cat <<JSON
{
  "build": {
    "beforeDevCommand": "cd \"$FRONTEND_DIR_JSON\" && corepack pnpm dev --host 127.0.0.1 --port $VITE_PORT",
    "devUrl": "$DEV_URL_JSON",
    "frontendDist": "$FRONTEND_DIST_JSON"
  }
}
JSON
)

# 检查端口
if lsof -Pi ":$VITE_PORT" -sTCP:LISTEN -t >/dev/null 2>&1 || \
   ss -tuln 2>/dev/null | grep -q ":$VITE_PORT "; then
    echo -e "${YELLOW}[WARN]${NC} 端口 $VITE_PORT 已被占用"
    echo "       可设置环境变量: PORT=5174 ./scripts/dev.sh"
    exit 1
fi

# 检查 node_modules
if [ ! -d "$FRONTEND_DIR/node_modules" ]; then
    echo -e "${YELLOW}[WARN]${NC} 前端依赖未安装，请先运行 ./scripts/setup.sh"
    exit 1
fi

echo -e "${CYAN}"
echo "   ____                 __   _       __"
echo "  / __ \____  ___  ____/ /  | |     / /___  _________ _____"
echo " / / / / __ \/ _ \/ __  /   | | /| / / __ \/ ___/ __ \ / __ \\"
echo "/ /_/ / /_/ /  __/ /_/ /    | |/ |/ / /_/ / /  / / / // /_/ /"
echo "\____/ .___/\___/\__,_/     |__/|__/\____/_/  /_/ /_/ \____/"
echo "    /_/"
echo -e "${NC}"
echo -e "${BLUE}OpenLife - 开发模式启动${NC}"
echo -e "${BLUE}[INFO]${NC} Profile: $OPENLIFE_PROFILE"
echo -e "${BLUE}[INFO]${NC} Vite: $OPENLIFE_DEV_URL"
echo -e "${BLUE}[INFO]${NC} Dev frontendDist placeholder: $OPENLIFE_FRONTEND_DIST"
echo -e "${BLUE}[INFO]${NC} A2A: 127.0.0.1:$A2A_PORT"
echo ""
echo -e "${GREEN}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║  🚀 正在启动 OpenLife 开发服务器...                           ║${NC}"
echo -e "${GREEN}║                                                              ║${NC}"
echo -e "${GREEN}║  首次启动可能需要 1-3 分钟编译 Rust 代码                     ║${NC}"
echo -e "${GREEN}║  请耐心等待...                                               ║${NC}"
echo -e "${GREEN}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

cd "$REPO_ROOT"

# 检查 pnpm（通过 Corepack 调用，避免依赖全局 pnpm symlink）
if ! command -v corepack &>/dev/null || ! corepack pnpm --version &>/dev/null; then
    echo -e "${YELLOW}[ERROR]${NC} pnpm 不可用"
    echo "       请准备 pnpm:"
    echo "       corepack prepare pnpm@9.1.0 --activate"
    exit 1
fi

if ! command -v cargo &>/dev/null; then
    echo -e "${YELLOW}[ERROR]${NC} cargo 不可用"
    exit 1
fi

echo -e "${BLUE}[INFO]${NC} 构建 A2A sidecar..."
cargo build --bin openlife-a2a-server

# 自动检测 Tauri CLI 启动方式
if [ -f "$FRONTEND_DIR/node_modules/.bin/tauri" ]; then
    echo -e "${BLUE}[INFO]${NC} 使用本地 Tauri CLI 启动..."
    "$FRONTEND_DIR/node_modules/.bin/tauri" dev --config "$TAURI_CONFIG_OVERRIDE"
elif command -v tauri &>/dev/null; then
    echo -e "${BLUE}[INFO]${NC} 使用全局 Tauri CLI 启动..."
    tauri dev --config "$TAURI_CONFIG_OVERRIDE"
else
    echo -e "${YELLOW}[ERROR]${NC} Tauri CLI 不可用"
    echo "       请先运行: corepack pnpm --dir \"$FRONTEND_DIR\" install"
    exit 1
fi
