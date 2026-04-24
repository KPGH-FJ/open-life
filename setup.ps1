# =============================================================================
# OpenLife 环境初始化脚本 (Windows PowerShell)
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
#   .\setup.ps1
#
# 预期时间：
#   首次运行约 2-5 分钟（主要耗时在前端依赖下载和 Rust 编译缓存生成）
#
# 常见问题：
#   Q: 执行策略阻止运行脚本
#   A: 以管理员身份运行: Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
#
#   Q: 提示 "pnpm not found"
#   A: npm install -g pnpm
#
#   Q: Tauri 构建报错
#   A: 确保安装 Visual Studio Build Tools + WebView2 Runtime
#      详见 https://tauri.app/start/prerequisites/
# =============================================================================

$ErrorActionPreference = "Stop"

# 路径配置
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$FrontendDir = Join-Path $ScriptDir "frontend"
$EnvFile = Join-Path $ScriptDir ".env"
$EnvTemplate = Join-Path $ScriptDir ".env.template"

# 工具函数
function Write-Info($msg)    { Write-Host "[INFO]  $msg" -ForegroundColor Blue }
function Write-Success($msg) { Write-Host "[OK]    $msg" -ForegroundColor Green }
function Write-Warn($msg)    { Write-Host "[WARN]  $msg" -ForegroundColor Yellow }
function Write-Error($msg)   { Write-Host "[FAIL]  $msg" -ForegroundColor Red }
function Write-Step($msg)    { Write-Host "`n▶ $msg" -ForegroundColor Cyan }

function Test-Command($cmd) {
    $found = Get-Command $cmd -ErrorAction SilentlyContinue
    if ($found) {
        Write-Success "$cmd 已安装 ($($found.Source))"
        return $true
    } else {
        Write-Error "$cmd 未安装"
        return $false
    }
}

function Test-Version($cmd, $min) {
    try {
        $verStr = (& $cmd --version 2>$null | Select-String -Pattern '(\d+\.\d+(\.\d+)?)').Matches[0].Value
        $ver = [Version]$verStr
        $minVer = [Version]$min
        if ($ver -ge $minVer) {
            Write-Success "$cmd 版本 $ver >= $min"
            return $true
        } else {
            Write-Error "$cmd 版本 $ver < $min，请升级"
            return $false
        }
    } catch {
        Write-Warn "无法检测 $cmd 版本"
        return $false
    }
}

# Step 1: 检查必要工具
Write-Step "Step 1/5: 检查必要工具"
$failed = $false

Test-Command "node"   || $failed = $true
Test-Command "npm"    || $failed = $true

if (-not (Test-Command "pnpm")) {
    Write-Warn "pnpm 未安装，尝试通过 npm 安装..."
    npm install -g pnpm
    Test-Command "pnpm" || $failed = $true
}

Test-Command "rustc"  || $failed = $true
Test-Command "cargo"  || $failed = $true
Test-Command "git"    || $failed = $true

Write-Info "检查版本要求..."
Test-Version "node"  "18.0" || $failed = $true
Test-Version "rustc" "1.75" || $failed = $true

if ($failed) {
    Write-Host ""
    Write-Error "环境检查未通过，请安装以下缺失依赖："
    Write-Host ""
    Write-Host "  Rust (>= 1.75):  https://rustup.rs/"
    Write-Host "  Node.js (>= 18): https://nodejs.org/"
    Write-Host "  pnpm:            npm install -g pnpm"
    Write-Host ""
    Write-Host "Windows 额外依赖:"
    Write-Host "  Visual Studio Build Tools + WebView2 Runtime"
    Write-Host "  详见 https://tauri.app/start/prerequisites/"
    Write-Host ""
    exit 1
}

Write-Success "所有必要工具已就绪"

# Step 2: 安装前端依赖
Write-Step "Step 2/5: 安装前端依赖"
$nodeModules = Join-Path $FrontendDir "node_modules"
if (Test-Path $nodeModules) {
    Write-Info "检测到已存在的 node_modules，跳过安装"
} else {
    Write-Info "运行 pnpm install..."
    Push-Location $FrontendDir
    try { pnpm install } finally { Pop-Location }
    Write-Success "前端依赖安装完成"
}

# Step 3: 验证 Rust 工具链
Write-Step "Step 3/5: 验证 Rust 工具链"
Write-Info "当前平台: $([System.Environment]::OSVersion.Platform)"
$localTauri = Join-Path $FrontendDir "node_modules\.bin\tauri.cmd"
if (Test-Path $localTauri) {
    Write-Success "Tauri CLI 在 node_modules 中可用"
} elseif (Get-Command "tauri" -ErrorAction SilentlyContinue) {
    Write-Success "Tauri CLI 已全局安装"
} else {
    Write-Warn "Tauri CLI 未找到，将在首次启动时通过 npx/pnpm 自动安装"
}
Write-Info "Rust 依赖将在首次构建时自动下载（由 cargo 管理）"
Write-Success "Rust 工具链验证完成"

# Step 4: 创建 .env
Write-Step "Step 4/5: 配置环境变量"
if (Test-Path $EnvFile) {
    Write-Success ".env 文件已存在，跳过创建"
} elseif (Test-Path $EnvTemplate) {
    Copy-Item $EnvTemplate $EnvFile
    Write-Success "从 .env.template 创建 .env"
} else {
    New-Item -ItemType File -Path $EnvFile | Out-Null
    Write-Warn ".env.template 不存在，创建空 .env"
}
Write-Warn "⚠  请编辑 .env 文件，填入你的 API Key 以启用对话功能"
Write-Info "  - OPENROUTER_API_KEY (推荐): https://openrouter.ai/keys"
Write-Info "  - OPENAI_API_KEY (备选): https://platform.openai.com/api-keys"

# Step 5: 初始化数据目录
Write-Step "Step 5/5: 初始化数据存储"
$dataDir = Join-Path $env:LOCALAPPDATA "ai.openlife.app"
if (-not (Test-Path $dataDir)) {
    New-Item -ItemType Directory -Path $dataDir -Force | Out-Null
    Write-Success "创建数据目录: $dataDir"
} else {
    Write-Info "数据目录已存在: $dataDir"
}
Write-Info "SQLite 数据库将在首次启动应用时自动建表"
Write-Success "数据存储初始化完成"

# 验证
Write-Step "验证安装完整性"
$verifyFailed = $false
if (-not (Test-Path $nodeModules))     { Write-Error "node_modules 缺失"; $verifyFailed = $true }
if (-not (Test-Path $EnvFile))          { Write-Error ".env 文件缺失"; $verifyFailed = $true }
if (-not (Test-Path "$ScriptDir\Cargo.toml")) { Write-Error "Cargo.toml 缺失"; $verifyFailed = $true }

if ($verifyFailed) {
    Write-Error "验证未通过，请检查项目完整性"
    exit 1
}

Write-Success "验证通过！环境初始化完成"

Write-Host ""
Write-Host "╔══════════════════════════════════════════════════════════════╗" -ForegroundColor Green
Write-Host "║                🎉 环境初始化完成！                           ║" -ForegroundColor Green
Write-Host "╚══════════════════════════════════════════════════════════════╝" -ForegroundColor Green
Write-Host ""
Write-Host "下一步操作：" -ForegroundColor Cyan
Write-Host ""
Write-Host "  1. 编辑 .env 文件，配置 API Key（可选但推荐）"
Write-Host "  2. 启动开发模式："
Write-Host "     .\dev.ps1     或  .\startup.ps1 dev" -ForegroundColor Yellow
Write-Host ""
Write-Host "  或运行检查："
Write-Host "     .\startup.ps1 check" -ForegroundColor Yellow
Write-Host ""
