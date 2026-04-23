# =============================================================================
# OpenLife Makefile - 常用开发命令快捷方式
# =============================================================================
# 使用方法：
#   make <命令>
#
# 可用命令：
#   make setup      - 初始化开发环境（首次使用）
#   make dev        - 启动开发模式
#   make build      - 生产构建
#   make check      - 检查环境依赖
#   make test       - 运行所有测试
#   make test-front - 运行前端测试
#   make test-rust  - 运行 Rust 测试
#   make clean      - 清理构建缓存
#   make a2a        - 启动 A2A 独立服务器
#
# 平台支持：
#   自动检测 macOS/Linux/Windows，调用对应的脚本
# =============================================================================

# 检测操作系统
ifeq ($(OS),Windows_NT)
    SHELL_EXT = ps1
    SHELL_RUN = powershell -ExecutionPolicy Bypass -File
else
    UNAME_S := $(shell uname -s)
    SHELL_EXT = sh
    SHELL_RUN = bash
endif

# 默认目标
.DEFAULT_GOAL := help

# =============================================================================
# 主要命令
# =============================================================================

.PHONY: help setup dev build check test test-front test-rust clean a2a lint

## 显示帮助信息
help:
	@echo "OpenLife Makefile - 可用命令"
	@echo ""
	@echo "  make setup       - 初始化开发环境（首次使用）"
	@echo "  make dev         - 启动开发模式（桌面应用）"
	@echo "  make build       - 生产构建"
	@echo "  make check       - 检查环境依赖"
	@echo "  make test        - 运行所有测试"
	@echo "  make test-front  - 运行前端测试"
	@echo "  make test-rust   - 运行 Rust 测试"
	@echo "  make clean       - 清理构建缓存"
	@echo "  make a2a         - 启动 A2A 独立服务器"
	@echo ""

## 初始化开发环境
setup:
	@echo "🚀 正在初始化 OpenLife 开发环境..."
	$(SHELL_RUN) setup.$(SHELL_EXT)

## 启动开发模式
dev:
	@echo "🔧 启动开发模式..."
	$(SHELL_RUN) dev.$(SHELL_EXT)

## 生产构建
build:
	@echo "📦 开始生产构建..."
	$(SHELL_RUN) start.$(SHELL_EXT)

## 检查环境依赖
check:
	@echo "🔍 检查环境依赖..."
	$(SHELL_RUN) startup.$(SHELL_EXT) check

## 启动 A2A 独立服务器
a2a:
	@echo "🌐 启动 A2A 独立服务器..."
	$(SHELL_RUN) startup.$(SHELL_EXT) a2a

# =============================================================================
# 测试
# =============================================================================

## 运行所有测试
test: test-front test-rust
	@echo "✅ 所有测试完成"

## 运行前端测试
test-front:
	@echo "🧪 运行前端测试..."
	cd frontend && pnpm test

## 运行 Rust 测试
test-rust:
	@echo "🧪 运行 Rust 测试..."
	cargo test -p openlife-core
	cargo test -p openlife-tauri

# =============================================================================
# 清理
# =============================================================================

## 清理构建缓存和产物
clean:
	@echo "🧹 清理构建缓存..."
	cd frontend && rm -rf dist node_modules/.vite
	rm -rf src-tauri/target
	@echo "✅ 清理完成"

## 深度清理（包含 node_modules）
clean-all: clean
	@echo "🧹 深度清理..."
	cd frontend && rm -rf node_modules
	cargo clean
	@echo "✅ 深度清理完成"

# =============================================================================
# 前端独立命令
# =============================================================================

## 安装前端依赖
install-front:
	@echo "📦 安装前端依赖..."
	cd frontend && pnpm install

## 前端生产构建（不启动 Tauri）
build-front:
	@echo "🔨 前端生产构建..."
	cd frontend && pnpm build

## 前端开发服务器（不启动桌面窗口）
dev-front:
	@echo "🌐 启动前端开发服务器..."
	cd frontend && pnpm dev

# =============================================================================
# Rust 独立命令
# =============================================================================

## 运行 Rust Clippy（代码检查）
lint-rust:
	@echo "🔍 运行 Rust 代码检查..."
	cargo clippy -p openlife-core -- -D warnings
	cargo clippy -p openlife-tauri -- -D warnings

## 格式化 Rust 代码
fmt-rust:
	@echo "✨ 格式化 Rust 代码..."
	cargo fmt

## 格式化前端代码
fmt-front:
	@echo "✨ 格式化前端代码..."
	cd frontend && npx prettier --write "src/**/*.{ts,tsx,css}"
