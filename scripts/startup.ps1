# =============================================================================
# OpenLife 启动脚本 (Windows PowerShell)
# =============================================================================
# 使用方法:
#   1. 打开 PowerShell（推荐 PowerShell 7+）
#   2. 进入项目目录: cd C:\path\to\openlife
#   3. 执行: .\startup.ps1 [dev|a2a|check]
#
# 命令说明:
#   .\startup.ps1 dev    - 启动 Tauri 桌面应用开发模式（默认）
#   .\startup.ps1 a2a    - 启动独立 A2A 服务器
#   .\startup.ps1 check  - 仅检查环境依赖，不启动应用
#
# 前提条件:
#   - Rust >= 1.75    (https://rustup.rs/)
#   - Node.js >= 18   (https://nodejs.org/)
#   - pnpm >= 9       (recommended via Corepack)
#   - Tauri CLI       (pnpm add -g @tauri-apps/cli)
#   - Ollama (可选)   (https://ollama.com/)
#
# 常见问题:
#   Q: 执行策略阻止运行脚本
#   A: 以管理员身份运行: Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
#
#   Q: 提示 "pnpm : 无法将""识别为 cmdlet"
#   A: 运行 "corepack enable" 和 "corepack prepare pnpm@9.1.0 --activate"
#
#   Q: Tauri 构建失败
#   A: 确保安装 Visual Studio Build Tools + WebView2 Runtime
#      详见 https://tauri.app/start/prerequisites/
#
#   Q: 对话功能不可用
#   A: 1) 启动 Ollama: ollama serve
#      2) 拉取模型: ollama pull qwen2.5:7b
#      3) 或在 .env 中配置 OPENROUTER_API_KEY
# =============================================================================

param(
    [Parameter(Position = 0)]
    [ValidateSet("dev", "a2a", "check")]
    [string]$Command = "dev"
)

# 错误处理
$ErrorActionPreference = "Stop"

# 配置
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..")
$FrontendDir = Join-Path $RepoRoot "frontend"
$TauriDir = Join-Path $RepoRoot "src-tauri"
$EnvFile = Join-Path $RepoRoot ".env"
$A2aPort = if ($env:A2A_PORT) { $env:A2A_PORT } else { "" }
$VitePort = if ($env:PORT) { $env:PORT } else { "" }

# =============================================================================
# 工具函数
# =============================================================================

function Write-Info($message) {
    Write-Host "[INFO] $message" -ForegroundColor Blue
}

function Write-Success($message) {
    Write-Host "[OK] $message" -ForegroundColor Green
}

function Write-Warn($message) {
    Write-Host "[WARN] $message" -ForegroundColor Yellow
}

function Write-Error($message) {
    Write-Host "[ERROR] $message" -ForegroundColor Red
}

function Write-Step($message) {
    Write-Host ""
    Write-Host "▶ $message" -ForegroundColor Cyan
}

function Test-Command($command) {
    $cmd = Get-Command $command -ErrorAction SilentlyContinue
    if ($cmd) {
        Write-Success "$command 已安装 ($($cmd.Source))"
        return $true
    }
    else {
        Write-Error "$command 未安装"
        return $false
    }
}

function Test-Version($command, $minVersion) {
    try {
        $output = & $command --version 2>$null
        $versionStr = ($output | Select-String -Pattern '(\d+\.\d+(\.\d+)?)').Matches[0].Value
        $version = [Version]$versionStr
        $minVer = [Version]$minVersion

        if ($version -ge $minVer) {
            Write-Success "$command 版本 $version >= $minVersion"
            return $true
        }
        else {
            Write-Error "$command 版本 $version < $minVersion，需要升级"
            return $false
        }
    }
    catch {
        Write-Warn "无法检测 $command 版本"
        return $false
    }
}

function Test-Port($port) {
    $listener = $null
    try {
        $listener = New-Object System.Net.Sockets.TcpListener ([System.Net.IPAddress]::Loopback, $port)
        $listener.Start()
        $listener.Stop()
        Write-Success "端口 $port 可用"
        return $true
    }
    catch {
        Write-Warn "端口 $port 已被占用"
        return $false
    }
    finally {
        if ($listener) { $listener.Stop() }
    }
}

function Wait-ForPort($port, $timeoutSeconds = 30) {
    Write-Info "等待端口 $port 就绪..."
    $startTime = Get-Date
    while ($true) {
        try {
            $client = New-Object System.Net.Sockets.TcpClient
            $client.Connect("127.0.0.1", $port)
            $client.Close()
            Write-Success "端口 $port 已就绪"
            return
        }
        catch {
            $elapsed = (Get-Date) - $startTime
            if ($elapsed.TotalSeconds -ge $timeoutSeconds) {
                throw "端口 $port 在 ${timeoutSeconds} 秒内未就绪"
            }
            Start-Sleep -Milliseconds 500
        }
    }
}

function Get-OpenLifeAppDirName {
    switch ($env:OPENLIFE_PROFILE) {
        "dev" { return "ai.openlife.app.dev" }
        "qa" { return "ai.openlife.app.qa" }
        default { return "ai.openlife.app" }
    }
}

function Set-RuntimeProfile($commandName) {
    if ($commandName -eq "dev" -or $commandName -eq "a2a") {
        if ($env:OPENLIFE_DATA_DIR -and $env:OPENLIFE_ALLOW_DEV_EXTENSIONS_WITH_CUSTOM_DATA_DIR -ne "1") {
            Write-Error "dev-extensions 拒绝使用 OPENLIFE_DATA_DIR；请使用隔离 dev profile"
            Write-Info "如确需隔离的自定义目录，请显式设置 OPENLIFE_ALLOW_DEV_EXTENSIONS_WITH_CUSTOM_DATA_DIR=1"
            exit 1
        }
        $env:OPENLIFE_PROFILE = "dev"
    }
    elseif (-not $env:OPENLIFE_PROFILE) {
        $env:OPENLIFE_PROFILE = "release"
    }

    if (-not $script:VitePort) {
        $script:VitePort = if ($env:PORT) { $env:PORT } else { "5173" }
    }
    if (-not $script:A2aPort) {
        $script:A2aPort = if ($env:OPENLIFE_PROFILE -eq "dev") { "8766" } else { "8765" }
    }
    $env:A2A_PORT = $script:A2aPort
}

function New-TauriConfigOverride {
    $env:OPENLIFE_DEV_URL = "http://127.0.0.1:$script:VitePort"
    $env:OPENLIFE_FRONTEND_DIST = Join-Path $RepoRoot "target/openlife-dev/frontend-dist-placeholder"
    $env:OPENLIFE_FRONTEND_MODE = "dev_server"
    New-Item -ItemType Directory -Force -Path $env:OPENLIFE_FRONTEND_DIST | Out-Null
    @"
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
"@ | Set-Content -Encoding UTF8 -Path (Join-Path $env:OPENLIFE_FRONTEND_DIST "index.html")
    @{
        build = @{
            beforeDevCommand = "cd `"$FrontendDir`" && corepack pnpm dev --host 127.0.0.1 --port $script:VitePort"
            devUrl = $env:OPENLIFE_DEV_URL
            frontendDist = $env:OPENLIFE_FRONTEND_DIST
        }
        app = @{
            security = @{
                capabilities = @("default", "dev-extensions")
            }
        }
    } | ConvertTo-Json -Compress -Depth 4
}

# =============================================================================
# 环境检查
# =============================================================================

function Test-Environment {
    Write-Step "检查开发环境"
    $hasErrors = $false

    # 检查核心依赖
    if (-not (Test-Command "node")) { $hasErrors = $true }
    if (-not (Test-Command "corepack")) {
        $hasErrors = $true
    }
    else {
        corepack pnpm --version *> $null
        if ($LASTEXITCODE -ne 0) {
            Write-Warn "pnpm 不可用"
            Write-Info "请运行: corepack prepare pnpm@9.1.0 --activate"
            $hasErrors = $true
        }
    }
    if (-not (Test-Command "rustc")) { $hasErrors = $true }
    if (-not (Test-Command "cargo")) { $hasErrors = $true }

    # 检查版本
    Write-Info "检查版本要求..."
    if (-not (Test-Version "node" "18.0")) { $hasErrors = $true }
    if (-not (Test-Version "rustc" "1.75")) { $hasErrors = $true }

    # 检查 Tauri CLI
    if (-not (Test-Command "tauri")) {
        $localTauri = Join-Path $FrontendDir "node_modules\.bin\tauri.cmd"
        if (Test-Path $localTauri) {
            Write-Success "Tauri CLI 存在于 node_modules"
        }
        else {
            Write-Warn "Tauri CLI 未全局安装，将在首次运行时通过 pnpm 安装"
        }
    }

    # 检查 Ollama（可选）
    if (Test-Command "ollama") {
        Write-Info "Ollama 已安装"
        try {
            $response = Invoke-WebRequest -Uri "http://localhost:11434/api/tags" -UseBasicParsing -TimeoutSec 2 -ErrorAction Stop
            Write-Success "Ollama 服务正在运行 (http://localhost:11434)"
        }
        catch {
            Write-Warn "Ollama 已安装但未运行，启动后可用本地模型"
            Write-Info "  提示: 在另一个终端运行: ollama serve"
        }
    }
    else {
        Write-Warn "Ollama 未安装（可选，用于本地模型）"
        Write-Info "  安装: https://ollama.com/"
    }

    # 检查可选工具
    if (-not (Test-Command "python")) {
        Write-Warn "python 未安装（仅影响量化脚本）"
    }

    if ($hasErrors) {
        Write-Host ""
        Write-Error "环境检查未通过，请安装缺失的依赖"
        Write-Host ""
        Write-Host "快速安装指南:" -ForegroundColor Yellow
        Write-Host "  Rust:    https://rustup.rs/"
        Write-Host "  Node.js: https://nodejs.org/"
        Write-Host "  pnpm:    corepack enable; corepack prepare pnpm@9.1.0 --activate"
        Write-Host "  Tauri:   pnpm add -g @tauri-apps/cli"
        Write-Host ""
        exit 1
    }

    Write-Success "环境检查通过"
}

# =============================================================================
# 环境变量设置
# =============================================================================

function Set-Environment {
    Write-Step "配置环境变量"

    # 创建 .env 文件（如果不存在）
    if (-not (Test-Path $EnvFile)) {
        $templateFile = Join-Path $RepoRoot ".env.example"
        if (Test-Path $templateFile) {
            Copy-Item $templateFile $EnvFile
            Write-Success "从模板创建 .env 文件"
            Write-Info "请编辑 .env 文件配置你的 API Key"
        }
        else {
            Write-Warn ".env.example 不存在，创建空 .env"
            New-Item -ItemType File -Path $EnvFile | Out-Null
        }
    }
    else {
        Write-Success ".env 文件已存在"
    }

    # 加载 .env 文件
    if (Test-Path $EnvFile) {
        Get-Content $EnvFile | ForEach-Object {
            $line = $_.Trim()
            if ($line -and -not $line.StartsWith("#")) {
                $parts = $line -split "=", 2
                if ($parts.Count -eq 2) {
                    $key = $parts[0].Trim()
                    $value = $parts[1].Trim() -replace '^["\x27]' -replace '["\x27]$'
                    [Environment]::SetEnvironmentVariable($key, $value, "Process")
                }
            }
        }
        Write-Success "已加载 .env 环境变量"
    }

    # 检查 API Key 配置
    if (-not $env:OPENROUTER_API_KEY -and -not $env:OPENAI_API_KEY) {
        Write-Warn "未配置 LLM API Key，云端模型不可用"
        Write-Info "  请在 .env 中设置 OPENROUTER_API_KEY 或 OPENAI_API_KEY"
        Write-Info "  获取 OpenRouter Key: https://openrouter.ai/keys"
    }
    else {
        Write-Success "API Key 已配置"
    }
}

# =============================================================================
# 依赖安装
# =============================================================================

function Install-Dependencies {
    Write-Step "安装前端依赖"

    $nodeModules = Join-Path $FrontendDir "node_modules"
    if (-not (Test-Path $nodeModules)) {
        Write-Info "首次安装，运行 pnpm install..."
        Push-Location $FrontendDir
        try {
            corepack pnpm install
        }
        finally {
            Pop-Location
        }
        Write-Success "前端依赖安装完成"
    }
    else {
        Write-Success "前端依赖已安装"
    }

    Write-Step "检查 Rust 依赖"
    $targetDir = Join-Path $RepoRoot "target"
    if (-not (Test-Path $targetDir)) {
        Write-Info "首次构建，Rust 依赖将在启动时自动编译..."
    }
    else {
        Write-Success "Rust 构建缓存已存在"
    }
}

# =============================================================================
# 数据库初始化
# =============================================================================

function Initialize-Database {
    Write-Step "初始化数据存储"

    $dataDir = if ($env:OPENLIFE_DATA_DIR) {
        $env:OPENLIFE_DATA_DIR
    } else {
        Join-Path $env:LOCALAPPDATA (Get-OpenLifeAppDirName)
    }
    if (-not (Test-Path $dataDir)) {
        New-Item -ItemType Directory -Path $dataDir -Force | Out-Null
        Write-Success "创建数据目录: $dataDir"
    }
    else {
        Write-Success "数据目录已存在: $dataDir"
    }

    Write-Info "SQLite 数据库将在首次启动时自动建表"
}

# =============================================================================
# 启动应用
# =============================================================================

function Start-Dev {
    Write-Step "启动 OpenLife 开发模式"
    $tauriConfigOverride = New-TauriConfigOverride

    # 检查端口
    if (-not (Test-Port $VitePort)) {
        Write-Error "Vite 端口 $VitePort 被占用，请修改 PORT 环境变量或关闭占用进程"
        exit 1
    }

    Write-Host ""
    Write-Host "╔══════════════════════════════════════════════════════════════╗" -ForegroundColor Green
    Write-Host "║           OpenLife 正在启动...                               ║" -ForegroundColor Green
    Write-Host "║                                                              ║" -ForegroundColor Green
    Write-Host "║  首次启动可能需要 1-3 分钟编译 Rust 代码                     ║" -ForegroundColor Green
    Write-Host "║  请耐心等待...                                               ║" -ForegroundColor Green
    Write-Host "╚══════════════════════════════════════════════════════════════╝" -ForegroundColor Green
    Write-Host ""
    Write-Info "Profile: $($env:OPENLIFE_PROFILE)"
    Write-Info "Vite: $($env:OPENLIFE_DEV_URL)"
    Write-Info "A2A: 127.0.0.1:$A2aPort"

    # 检查 pnpm
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
    if (-not (Get-Command "cargo" -ErrorAction SilentlyContinue)) {
        Write-Error "cargo 不可用"
        exit 1
    }
    if ($env:OPENLIFE_DEV_AUTOSTART_A2A -eq "1") {
        if ($env:OPENLIFE_ENABLE_DEV_A2A -ne "1" -or -not $env:OPENLIFE_A2A_PAIRED_TOKEN -or $env:OPENLIFE_A2A_PAIRED_TOKEN.Length -lt 32) {
            Write-Error "A2A autostart requires OPENLIFE_ENABLE_DEV_A2A=1 and a 32+ character OPENLIFE_A2A_PAIRED_TOKEN"
            exit 1
        }
        Write-Info "构建显式启用的开发 A2A sidecar..."
        cargo build --manifest-path (Join-Path $RepoRoot "Cargo.toml") -p openlife-a2a-server --bin openlife-a2a-server --features dev-extensions
    }

    # 检查使用哪种方式启动 Tauri
    $localTauri = Join-Path $FrontendDir "node_modules\.bin\tauri.cmd"
    $globalTauri = Get-Command "tauri" -ErrorAction SilentlyContinue

    Push-Location $RepoRoot
    try {
        if (Test-Path $localTauri) {
            Write-Info "使用本地 Tauri CLI 启动..."
            & $localTauri dev --features dev-extensions --config (Join-Path $TauriDir "tauri.dev.conf.json") --config $tauriConfigOverride
        }
        elseif ($globalTauri) {
            Write-Info "使用全局 Tauri CLI 启动..."
            tauri dev --features dev-extensions --config (Join-Path $TauriDir "tauri.dev.conf.json") --config $tauriConfigOverride
        }
        else {
            Write-Error "Tauri CLI 不可用"
            Write-Info "请先运行: corepack pnpm --dir `"$FrontendDir`" install"
            exit 1
        }
    }
    finally {
        Pop-Location
    }
}

function Start-A2A {
    Write-Step "启动 A2A 独立服务器"

    if ($env:OPENLIFE_ENABLE_DEV_A2A -ne "1" -or -not $env:OPENLIFE_A2A_PAIRED_TOKEN -or $env:OPENLIFE_A2A_PAIRED_TOKEN.Length -lt 32) {
        Write-Error "A2A 默认关闭；启动需要显式启用并配置强配对凭据"
        Write-Info "设置 OPENLIFE_ENABLE_DEV_A2A=1 和 32+ 字符 OPENLIFE_A2A_PAIRED_TOKEN"
        exit 1
    }
    $env:OPENLIFE_PROFILE = "dev"

    # 检查端口
    if (-not (Test-Port $A2aPort)) {
        Write-Error "A2A 端口 $A2aPort 被占用"
        Write-Info "可设置环境变量: `$env:A2A_PORT = '9999'; .\startup.ps1 a2a"
        exit 1
    }

    Write-Info "A2A 服务器将监听: http://127.0.0.1:$A2aPort"
    Write-Info "API 端点:"
    Write-Info "  GET  http://127.0.0.1:$A2aPort/agent.json"
    Write-Info "  POST http://127.0.0.1:$A2aPort/tasks/send"

    Push-Location $TauriDir
    try {
        cargo run -p openlife-a2a-server --bin openlife-a2a-server --features dev-extensions
    }
    finally {
        Pop-Location
    }
}

# =============================================================================
# 主逻辑
# =============================================================================

Write-Host ""
Write-Host "   ____                 __   _       __" -ForegroundColor Cyan
Write-Host "  / __ \____  ___  ____/ /  | |     / /___  _________ _____" -ForegroundColor Cyan
Write-Host " / / / / __ \/ _ \/ __  /   | | /| / / __ \/ ___/ __ \ / __ \\" -ForegroundColor Cyan
Write-Host "/ /_/ / /_/ /  __/ /_/ /    | |/ |/ / /_/ / /  / / / // /_/ /" -ForegroundColor Cyan
Write-Host "\____/ .___/\___/\__,_/     |__/|__/\____/_/  /_/ /_/ \____/" -ForegroundColor Cyan
Write-Host "    /_/" -ForegroundColor Cyan
Write-Host ""
Write-Host "OpenLife - 你的终身成长合伙人" -ForegroundColor Blue
Write-Host ""

switch ($Command) {
    "check" {
        Test-Environment
        Set-Environment
        Set-RuntimeProfile $Command
        Write-Host ""
        Write-Success "环境检查完成！可以运行 .\startup.ps1 dev 启动应用"
    }
    "dev" {
        Test-Environment
        Set-Environment
        Set-RuntimeProfile $Command
        Install-Dependencies
        Initialize-Database
        Start-Dev
    }
    "a2a" {
        Test-Environment
        Set-Environment
        Set-RuntimeProfile $Command
        Start-A2A
    }
}
