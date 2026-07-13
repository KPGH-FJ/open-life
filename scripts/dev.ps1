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
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..")
$FrontendDir = Join-Path $RepoRoot "frontend"

# 加载 .env
$EnvFile = Join-Path $RepoRoot ".env"
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

$OpenLifeProfile = "dev"
if ($env:OPENLIFE_DATA_DIR -and $env:OPENLIFE_ALLOW_DEV_EXTENSIONS_WITH_CUSTOM_DATA_DIR -ne "1") {
    Write-Host "[WARN] dev-extensions 拒绝使用 OPENLIFE_DATA_DIR；请使用隔离 dev profile" -ForegroundColor Yellow
    Write-Host "       如确需隔离的自定义目录，请显式设置 OPENLIFE_ALLOW_DEV_EXTENSIONS_WITH_CUSTOM_DATA_DIR=1"
    exit 1
}
$env:OPENLIFE_PROFILE = $OpenLifeProfile
if (-not $env:A2A_PORT) {
    if ($OpenLifeProfile -eq "dev") {
        $env:A2A_PORT = "8766"
    } else {
        $env:A2A_PORT = "8765"
    }
}
$VitePort = if ($env:PORT) { $env:PORT } else { "5173" }
$env:OPENLIFE_DEV_URL = "http://127.0.0.1:$VitePort"
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
$TauriConfigOverride = @{
    build = @{
        beforeDevCommand = "cd `"$FrontendDir`" && corepack pnpm dev --host 127.0.0.1 --port $VitePort"
        devUrl = $env:OPENLIFE_DEV_URL
        frontendDist = $env:OPENLIFE_FRONTEND_DIST
    }
    app = @{
        security = @{
            capabilities = @("default", "dev-extensions")
        }
    }
} | ConvertTo-Json -Compress -Depth 4

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
Write-Host "[INFO] Profile: $OpenLifeProfile" -ForegroundColor Blue
Write-Host "[INFO] Vite: $($env:OPENLIFE_DEV_URL)" -ForegroundColor Blue
Write-Host "[INFO] Dev frontendDist placeholder: $($env:OPENLIFE_FRONTEND_DIST)" -ForegroundColor Blue
Write-Host "[INFO] A2A: 127.0.0.1:$($env:A2A_PORT)" -ForegroundColor Blue
Write-Host ""
Write-Host "╔══════════════════════════════════════════════════════════════╗" -ForegroundColor Green
Write-Host "║  🚀 正在启动 OpenLife 开发服务器...                           ║" -ForegroundColor Green
Write-Host "║                                                              ║" -ForegroundColor Green
Write-Host "║  首次启动可能需要 1-3 分钟编译 Rust 代码                     ║" -ForegroundColor Green
Write-Host "║  请耐心等待...                                               ║" -ForegroundColor Green
Write-Host "╚══════════════════════════════════════════════════════════════╝" -ForegroundColor Green
Write-Host ""

# 检查 pnpm（通过 Corepack 调用，避免依赖全局 pnpm shim）
$corepack = Get-Command "corepack" -ErrorAction SilentlyContinue
if (-not $corepack) {
    Write-Host "[ERROR] corepack 未安装" -ForegroundColor Red
    Write-Host "       请安装 Node.js 18+ 并启用 Corepack"
    exit 1
}
corepack pnpm --version *> $null
if ($LASTEXITCODE -ne 0) {
    Write-Host "[ERROR] pnpm 不可用" -ForegroundColor Red
    Write-Host "       请运行: corepack prepare pnpm@9.1.0 --activate"
    exit 1
}
if (-not (Get-Command "cargo" -ErrorAction SilentlyContinue)) {
    Write-Host "[ERROR] cargo 不可用" -ForegroundColor Red
    exit 1
}
if ($env:OPENLIFE_DEV_AUTOSTART_A2A -eq "1") {
    if ($env:OPENLIFE_ENABLE_DEV_A2A -ne "1" -or -not $env:OPENLIFE_A2A_PAIRED_TOKEN -or $env:OPENLIFE_A2A_PAIRED_TOKEN.Length -lt 32) {
        Write-Host "[ERROR] A2A autostart requires OPENLIFE_ENABLE_DEV_A2A=1 and a 32+ character OPENLIFE_A2A_PAIRED_TOKEN" -ForegroundColor Red
        exit 1
    }
    Write-Host "[INFO] 构建显式启用的开发 A2A sidecar..." -ForegroundColor Blue
    cargo build --manifest-path (Join-Path $RepoRoot "Cargo.toml") --bin openlife-a2a-server --features dev-extensions
}

Push-Location $RepoRoot
$localTauri = Join-Path $FrontendDir "node_modules\.bin\tauri.cmd"
$globalTauri = Get-Command "tauri" -ErrorAction SilentlyContinue

try {
    if (Test-Path $localTauri) {
        Write-Host "[INFO] 使用本地 Tauri CLI 启动..." -ForegroundColor Blue
        & $localTauri dev --features dev-extensions --config (Join-Path $RepoRoot "src-tauri/tauri.dev.conf.json") --config $TauriConfigOverride
    } elseif ($globalTauri) {
        Write-Host "[INFO] 使用全局 Tauri CLI 启动..." -ForegroundColor Blue
        tauri dev --features dev-extensions --config (Join-Path $RepoRoot "src-tauri/tauri.dev.conf.json") --config $TauriConfigOverride
    } else {
        Write-Host "[ERROR] Tauri CLI 不可用" -ForegroundColor Red
        Write-Host "       请先运行: corepack pnpm --dir `"$FrontendDir`" install"
        exit 1
    }
} finally {
    Pop-Location
}
