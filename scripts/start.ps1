# =============================================================================
# OpenLife 生产构建脚本 (Windows PowerShell)
# =============================================================================
# 用途：
#   构建 OpenLife 桌面应用的生产版本，生成分发安装包。
#
# 使用方法：
#   .\start.ps1
#
# 构建产物：
#   target\release\bundle\
#
# 预期时间：
#   首次构建约 5-15 分钟（取决于机器性能）
#
# 常见问题：
#   Q: 构建失败提示缺少系统依赖
#   A: 确保安装 Visual Studio Build Tools + WebView2 Runtime
#      详见 https://tauri.app/start/prerequisites/
#
#   Q: 构建产物在哪里？
#   A: 查看 src-tauri\target\release\bundle\ 目录
# =============================================================================

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..")
$FrontendDir = Join-Path $RepoRoot "frontend"
$TauriDir = Join-Path $RepoRoot "src-tauri"

function Write-Info($msg)    { Write-Host "[INFO]  $msg" -ForegroundColor Blue }
function Write-Success($msg) { Write-Host "[OK]    $msg" -ForegroundColor Green }
function Write-Warn($msg)    { Write-Host "[WARN]  $msg" -ForegroundColor Yellow }
function Write-Error($msg)   { Write-Host "[ERROR] $msg" -ForegroundColor Red }
function Write-Step($msg)    { Write-Host "`n▶ $msg" -ForegroundColor Cyan }

# 检查环境
Write-Step "检查构建环境"

if (-not (Get-Command "node" -ErrorAction SilentlyContinue)) {
    Write-Error "Node.js 未安装"
    exit 1
}

if (-not (Get-Command "cargo" -ErrorAction SilentlyContinue)) {
    Write-Error "Rust/Cargo 未安装"
    exit 1
}

$nodeModules = Join-Path $FrontendDir "node_modules"
if (-not (Test-Path $nodeModules)) {
    Write-Warn "前端依赖未安装"
    Write-Info "请先运行 .\scripts\setup.ps1 安装依赖"
    exit 1
}

Write-Success "环境检查通过"

# 构建
Write-Step "开始构建 OpenLife (Windows x86_64)"

Write-Host ""
Write-Host "   ____                 __   _       __" -ForegroundColor Cyan
Write-Host "  / __ \____  ___  ____/ /  ^| ^|     / /___  _________ _____" -ForegroundColor Cyan
Write-Host " / / / / __ \/ _ \/ __  /   ^| ^| /^| / / __ \/ ___/ __ \ / __ \\ " -ForegroundColor Cyan
Write-Host "/ /_/ / /_/ /  __/ /_/ /    ^| ^|/ ^|/ / /_/ / /  / / / // /_/ /" -ForegroundColor Cyan
Write-Host "\____/ .___/\___/\__,_/     ^|__/^|__/\____/_/  /_/ /_/ \____/" -ForegroundColor Cyan
Write-Host "    /_/" -ForegroundColor Cyan
Write-Host ""
Write-Host "OpenLife - 生产构建" -ForegroundColor Blue
Write-Host ""
Write-Host "╔══════════════════════════════════════════════════════════════╗" -ForegroundColor Green
Write-Host "║  📦 正在构建 Windows x86_64 版本...                           ║" -ForegroundColor Green
Write-Host "║                                                              ║" -ForegroundColor Green
Write-Host "║  首次构建可能需要 5-15 分钟，请耐心等待                     ║" -ForegroundColor Green
Write-Host "║  产物将输出到 target\release\bundle\                         ║" -ForegroundColor Green
Write-Host "╚══════════════════════════════════════════════════════════════╝" -ForegroundColor Green
Write-Host ""

# 检查 pnpm（通过 Corepack 调用，避免依赖全局 pnpm shim）
$corepack = Get-Command "corepack" -ErrorAction SilentlyContinue
if (-not $corepack) {
    Write-Error "corepack 未安装"
    Write-Info "请安装 Node.js 18+ 并启用 Corepack"
    exit 1
}
corepack pnpm --version *> $null
if ($LASTEXITCODE -ne 0) {
    Write-Error "pnpm 不可用"
    Write-Info "请运行: corepack prepare pnpm@9.1.0 --activate"
    exit 1
}

Push-Location $RepoRoot
$localTauri = Join-Path $FrontendDir "node_modules\.bin\tauri.cmd"
$globalTauri = Get-Command "tauri" -ErrorAction SilentlyContinue

try {
    if (Test-Path $localTauri) {
        Write-Info "使用本地 Tauri CLI 构建..."
        & $localTauri build
    } elseif ($globalTauri) {
        Write-Info "使用全局 Tauri CLI 构建..."
        tauri build
    } else {
        Write-Error "Tauri CLI 不可用"
        Write-Info "请先运行: corepack pnpm --dir `"$FrontendDir`" install"
        exit 1
    }
} finally {
    Pop-Location
}

# 检查构建结果
$bundleDir = Join-Path $RepoRoot "target\release\bundle"
if (Test-Path $bundleDir) {
    Write-Step "构建完成！"
    Write-Host ""
    Write-Success "构建产物位于: $bundleDir"
    Write-Host ""
    Write-Host "文件列表:" -ForegroundColor Cyan
    Get-ChildItem -Path $bundleDir -Recurse -File | ForEach-Object {
        $size = "{0:N2} MB" -f ($_.Length / 1MB)
        Write-Host "  $($_.FullName) ($size)"
    }
    Write-Host ""
} else {
    Write-Error "构建产物目录未找到，可能构建失败"
    exit 1
}
