@echo off
REM =============================================================================
REM OpenLife 启动脚本 (Windows CMD / 批处理包装器)
REM =============================================================================
REM 本脚本会自动调用 PowerShell 版本的启动脚本
REM 如需更多功能，请直接使用 PowerShell 运行: .\startup.ps1
REM =============================================================================

echo.
echo   ____                 __   _       __
echo  / __ \____  ___  ____/ /  ^| ^|     / /___  _________ _____
echo / / / / __ \/ _ \/ __  /   ^| ^| /^| / / __ \/ ___/ __ \ / __ \\ 
echo/ /_/ / /_/ /  __/ /_/ /    ^| ^|/ ^|/ / /_/ / /  / / / // /_/ /
echo\____/ .___/\___/\__,_/     ^|__/^|__/\____/_/  /_/ /_/ \____/
echo     /_/ 
echo.
echo OpenLife - 你的终身成长合伙人
echo.

REM 检查 PowerShell 是否可用
where powershell >nul 2>&1
if %errorlevel% neq 0 (
    echo [ERROR] PowerShell 未安装，请安装后重试
    exit /b 1
)

REM 获取脚本所在目录
set "SCRIPT_DIR=%~dp0"
set "PS1_SCRIPT=%SCRIPT_DIR%startup.ps1"

if not exist "%PS1_SCRIPT%" (
    echo [ERROR] 未找到 startup.ps1，请确保文件完整
    exit /b 1
)

REM 检查是否需要设置执行策略
echo [INFO] 正在启动 PowerShell 脚本...
echo.

REM 使用 Bypass 执行策略运行脚本（仅当前会话）
powershell -ExecutionPolicy Bypass -File "%PS1_SCRIPT%" %*

if %errorlevel% neq 0 (
    echo.
    echo [ERROR] 启动失败，错误代码: %errorlevel%
    echo.
    echo 常见问题:
    echo   1. 如果提示 "无法加载脚本"，请以管理员身份运行:
    echo      Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
    echo.
    echo   2. 如果提示缺少依赖，请安装:
    echo      - Rust: https://rustup.rs/
    echo      - Node.js: https://nodejs.org/
    echo      - pnpm: corepack enable ^&^& corepack prepare pnpm@9.1.0 --activate
    echo.
    pause
    exit /b %errorlevel%
)
