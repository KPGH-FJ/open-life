#!/bin/bash
# OpenLife Smoke Test
# 验证核心链路完整性

set -e

echo "🧪 OpenLife Smoke Test"
echo "======================"

# 1. 编译检查
echo ""
echo "1️⃣  Rust 编译检查..."
cargo check -p openlife-core -q
cargo check -p openlife-tauri -q
echo "   ✅ Rust 编译通过"

# 2. 运行 Rust 测试
echo ""
echo "2️⃣  Rust 单元测试..."
cargo test -p openlife-core --lib -q
cargo test -p openlife-tauri --lib -q
echo "   ✅ Rust 测试通过"

# 3. 前端测试
echo ""
echo "3️⃣  前端测试..."
cd frontend
corepack pnpm test -- --run
echo "   ✅ 前端测试通过"
cd ..

# 4. 数据库文件检查
echo ""
echo "4️⃣  数据库文件检查..."
DATA_DIR="$HOME/Library/Application Support/ai.openlife.desktop"
if [ -d "$DATA_DIR" ]; then
    echo "   ✅ 数据目录存在: $DATA_DIR"
    for db in messages.db vectors.db agent_runs.db proposals.db; do
        if [ -f "$DATA_DIR/$db" ]; then
            echo "   ✅ $db 存在"
        else
            echo "   ⚠️  $db 不存在（首次运行正常）"
        fi
    done
else
    echo "   ⚠️  数据目录不存在（首次运行正常）"
fi

# 5. 核心模块检查
echo ""
echo "5️⃣  核心模块检查..."
MODULES=(
    "openlife-core/src/agent/types/mod.rs"
    "openlife-core/src/agent/store.rs"
    "openlife-core/src/agent/proposal_store.rs"
    "openlife-core/src/scheduler.rs"
)
for mod in "${MODULES[@]}"; do
    if [ -f "$mod" ]; then
        echo "   ✅ $mod"
    else
        echo "   ❌ $mod 缺失"
        exit 1
    fi
done

# 6. Tauri 命令检查
echo ""
echo "6️⃣  Tauri 命令检查..."
COMMANDS=(
    "get_agent_run"
    "list_agent_runs"
    "get_pending_proposals"
    "accept_proposal"
    "reject_proposal"
    "edit_proposal"
    "postpone_proposal"
    "list_proposals"
    "batch_accept_low_risk_proposals"
    "builder_create_proposals"
    "calibration_create_proposals"
)
for cmd in "${COMMANDS[@]}"; do
    if grep -q "pub async fn $cmd" src-tauri/src/commands/*.rs; then
        echo "   ✅ $cmd"
    else
        echo "   ❌ $cmd 缺失"
        exit 1
    fi
done

echo ""
echo "======================"
echo "✅ Smoke Test 全部通过"
echo ""
echo "接下来可以进行的试用验证："
echo "1. 启动应用: ./scripts/dev.sh"
echo "2. Settings 测试 DeepSeek API"
echo "3. Builder 完成快速构建 → 发送到 Review Center"
echo "4. Calibration 生成建议 → 发送到 Review Center"
echo "5. Review Center 查看/编辑/确认 Proposal"
echo "6. Chat 发一条消息（观察 AgentRun trace）"
echo "7. 查看 Workspace Dashboard 待确认提醒"
echo "8. Safe Mode 验证：degraded 状态下 accept/edit 应被阻止"
