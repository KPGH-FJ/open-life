# OpenLife UI Beta Shell Contract

> Historical UI shell contract. Do not use this file as current Beta status,
> navigation, Tool Taxonomy, or implementation authority.
>
> Current Agent development order and product/runtime boundary are governed by
> `AGENTS.md`, `plans/README.md`, and
> `plans/openlife_lifemodel_governed_agent_runtime.md`.
>
> This file is retained only as a scoped UX reference for older Beta-shell
> thinking. Re-check current code and docs before reviving any item.
>
> 版本：Beta v1.0
> 日期：2026-04-30

---

## 能力分级体系

| 级别 | 定义 | 用户预期 |
|-----|------|---------|
| **Stable** | 核心功能，经过验证，可放心使用 | 默认展示，无特殊标识 |
| **Beta** | 可用但可能有边界问题，持续改进中 | 展示 Beta 角标 |
| **Experimental** | 灰度测试功能，可能不稳定 | 需手动开启，有明显警告 |
| **Declarative-only** | 仅声明展示，不可执行 | 灰色显示，明确标注 |
| **Hidden** | Beta 阶段不展示 | 入口隐藏 |

---

## 功能矩阵

| 模块 | 用户目标 | 级别 | 离线 | 需授权 | 产生 Proposal | 写数据 | 一级导航 | 入口位置 |
|-----|---------|------|------|--------|--------------|--------|---------|---------|
| **Workspace** | 操作台 | Stable | ✅ | ❌ | ❌ | ❌ | ✅ | 默认页 |
| **Agent Chat** | 对话 | Beta | 视配置 | ❌ | ✅ | ❌ | ✅ | Agent 页 |
| **Runs** | 追踪执行 | Beta | ✅ | ❌ | ❌ | ❌ | ✅ | Agent 页 |
| **Review Center** | 授权变更 | Stable | ✅ | ❌ | ✅ | ✅ | ✅ | 独立页 |
| **LifeModel** | 人生模型 | Stable | ✅ | ✅ | ✅ | ✅ | ✅ | Life 页 |
| **Memory** | 记忆管理 | Beta | ✅ | ❌ | ✅ | ✅ | ✅ | Memory 页 |
| **Settings** | 系统配置 | Stable | ✅ | ❌ | ❌ | ❌ | ✅ | Settings 页 |
| **Built-in Skills** | 运行技能 | Beta | 视配置 | ❌ | ✅ | ✅ | ❌ | Workspace 卡片 |
| **MCP** | 外部工具 | Experimental | ❌ | ✅ | ✅ | ❌ | ❌ | Settings 子页 |
| **A2A** | 外部代理 | Experimental | 视配置 | ✅ | ✅ | ❌ | ❌ | Settings 子页 |
| **Plugin** | 插件声明 | Declarative-only | ✅ | ❌ | ❌ | ❌ | ❌ | Settings 子页 |
| **ModelRouter Health** | 模型健康 | Beta | ❌ | ❌ | ❌ | ❌ | ❌ | Settings 子页 |

---

## 导航结构（一级 6 项）

```
OpenLife
├── Workspace （默认页）
│   ├── 最近 Runs 摘要
│   ├── 待处理 Proposals 提醒
│   ├── Built-in Skills 卡片
│   │   ├── Weekly Review [Beta]
│   │   ├── Goal Breakdown [Beta]
│   │   └── Memory Consolidation [Beta]
│   └── 快速 Chat 入口
│
├── Agent
│   ├── Chat [Beta]
│   └── Runs [Beta]
│
├── Life
│   └── LifeModel [Stable]
│
├── Memory
│   └── Memory [Beta]
│
├── Review Center
│   └── Proposal Review [Stable]
│
└── Settings
    ├── 模型配置 [Stable]
    ├── MCP [Experimental]
    ├── A2A [Experimental]
    ├── Plugin [Declarative-only]
    ├── ModelRouter Health [Beta]
    └── 实验性功能开关
```

---

## 隐藏/降级清单

| 功能 | 原位置 | 处理方式 | 说明 |
|-----|--------|---------|------|
| Plugin tool execution | 工具列表 | Hidden | 声明-only，不注册到可执行列表 |
| Runs restore | Trash 页面 | Hidden | 保留 soft delete，隐藏恢复入口 |
| Legacy builder direct apply | Builder | Removed | 已全部走 Proposal 链路 |
| ContextAssembler V2 | Settings | Experimental | 需手动开启 |
| ModelRouter advanced routing | Settings | Experimental | 需手动开启 |

---

## Workspace 设计规范

### 首屏布局

```
┌─────────────────────────────────────┐
│  欢迎语 + 今日状态摘要               │
├─────────────────────────────────────┤
│  Skills 卡片行                       │
│  [Weekly Review] [Goal Breakdown]   │
│  [Memory Consolidation]             │
├─────────────────────────────────────┤
│  待处理 Proposals（最多 3 个）       │
│  → 跳转 Review Center               │
├─────────────────────────────────────┤
│  最近 Runs（最近 5 条）              │
│  → 跳转 Runs 页面                   │
├─────────────────────────────────────┤
│  快速 Chat 入口                      │
│  [输入框] → 跳转 Chat               │
└─────────────────────────────────────┘
```

### Skills 卡片规范

- 每个卡片：图标 + 名称 + 简短描述
- 角标：[Beta]
- 点击：执行 Skill → 跳转 Run Detail
- 如果生成 proposals：跳转 Review Center

---

## 标识规范

| 级别 | UI 标识 | 颜色 |
|-----|---------|------|
| Stable | 无 | - |
| Beta | [Beta] 角标 | 蓝色/灰色 |
| Experimental | [实验] 角标 + 警告图标 | 橙色 |
| Declarative-only | [声明] 标签 | 灰色 |

---

## 数据流保证

所有 Stable/Beta 功能必须满足：

1. **AgentRun 追踪**：每次执行生成可查询的 Run 记录
2. **Proposal 治理**：涉及 LifeModel/Memory/Tool 的变更必须经过 Review Center
3. **本地优先**：核心数据（LifeModel、Memory、Runs）本地存储
4. **可解释**：用户能查看 AI 使用了什么上下文、哪个模型

---

## 验收标准

- [ ] 一级导航不超过 6 项
- [ ] Workspace 作为有意义的默认页
- [ ] Beta/experimental 功能有明确标识
- [ ] Plugin tools 不在可执行列表中
- [ ] 所有 Phase 1-7 功能在重构后正常工作

---

*历史说明：本文档对应旧 Sprint 1-5 完成后的 UI Beta shell 设想，不代表当前 W123 后的产品/工具状态。*
