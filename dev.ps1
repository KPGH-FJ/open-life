# =============================================================================
# OpenLife 开发模式启动脚本 (Windows PowerShell)
# =============================================================================
# 用途：
#   以开发模式启动 OpenLife 桌面应用，包含热重载、调试输出。
#
# 使用方法：
#   .\dev.ps1
#   或: .\startup.ps1 dev
#
# 前提条件：
#   - 已完成环境初始化 (.\setup.ps1)
#   - 已配置 API Key（可选但推荐）
#
# 常见问题：
#   Q: 首次启动很慢
#   A: 首次需要编译 Rust 代码，耗时 1-3 分钟，请耐心等待
#
#   Q: 端口 5173 被占用
#   A: $env:PORT = "5174"; .\dev.ps1
# =============================================================================

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$FrontendDir = Join-Path $ScriptDir "frontend"
$VitePort = if ($env:PORT) { $env:PORT } else { "5173" }

# 加载 .env
$EnvFile = Join-Path $ScriptDir ".env"
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
}

# 检查端口
$listener = $null
try {
    $listener = New-Object System.Net.Sockets.TcpListener ([System.Net.IPAddress]::Loopback, $VitePort)
    $listener.Start()
} catch {
    Write-Host "[WARN] 端口 $VitePort 已被占用" -ForegroundColor Yellow
    Write-Host "       可设置环境变量: `$env:PORT = '5174'; .\dev.ps1" -ForegroundColor Yellow
    exit 1
} finally {
    if ($listener) { $listener.Stop() }
}

# 检查 node_modules
if (-not (Test-Path (Join-Path $FrontendDir "node_modules"))) {
    Write-Host "[WARN] 前端依赖未安装，请先运行 .\setup.ps1" -ForegroundColor Yellow
    exit 1
}

Write-Host ""
Write-Host "   ____                 __   _       __" -ForegroundColor Cyan
Write-Host "  / __ \____  ___  ____/ /  ^| ^|     / /___  _________ _____" -ForegroundColor Cyan
Write-Host " / / / / __ \/ _ \/ __  /   ^| ^| /^| / / __ \/ ___/ __ \ / __ \\ " -ForegroundColor Cyan
Write-Host "/ /_/ / /_/ /  __/ /_/ /    ^| ^|/ ^|/ / /_/ / /  / / / // /_/ /" -ForegroundColor Cyan
Write-Host "\____/ .___/\___/\__,_/     ^|__/^|__/\____/_/  /_/ /_/ \____/" -ForegroundColor Cyan
Write-Host "    /_/" -ForegroundColor Cyan
Write-Host ""
Write-Host "OpenLife - 开发模式启动" -ForegroundColor Blue
Write-Host ""
Write-Host "╔══════════════════════════════════════════════════════════════╗" -ForegroundColor Green
Write-Host "║  🚀 正在启动 OpenLife 开发服务器...                           ║" -ForegroundColor Green
Write-Host "║                                                              ║" -ForegroundColor Green
Write-Host "║  首次启动可能需要 1-3 分钟编译 Rust 代码                     ║" -ForegroundColor Green
Write-Host "║  请耐心等待...                                               ║" -ForegroundColor Green
Write-Host "╚══════════════════════════════════════════════════════════════╝" -ForegroundColor Green
Write-Host ""

Push-Location $ScriptDir
$localTauri = Join-Path $FrontendDir "node_modules\.bin\tauri.cmd"
$globalTauri = Get-Command "tauri" -ErrorAction SilentlyContinue

try {
    if (Test-Path $localTauri) {
        Write-Host "[INFO] 使用本地 Tauri CLI 启动..." -ForegroundColor Blue
        Push-Location $FrontendDir
        try { pnpm tauri dev } finally { Pop-Location }
    } elseif ($globalTauri) {
        Write-Host "[INFO] 使用全局 Tauri CLI 启动..." -ForegroundColor Blue
        tauri dev
    } else {
        Write-Host "[INFO] 使用 npx 启动 Tauri..." -ForegroundColor Blue
        Push-Location $FrontendDir
        try { npx tauri dev } finally { Pop-Location }
    }
} finally {
    Pop-Location
}
