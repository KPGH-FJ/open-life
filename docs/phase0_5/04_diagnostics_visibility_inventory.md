# Diagnostics Visibility Inventory

## Inventory

| Diagnostic Surface | Location | Current visibility | User value | Risk / cognitive load | Recommended V2 visibility |
| --- | --- | --- | --- | --- | --- |
| Safe Mode banner | `ProductShell`, `TodayPage`, `MemorySearch`, Settings tabs | Default product when active | Prevents unsafe writes and points to recovery. | Low; high trust value. | `DEFAULT_PRODUCT` |
| Usage readiness banner | `ProductShell` | Default product when diagnostics says not ready | Helps first-use setup. | Medium if it overclaims readiness. | `DEFAULT_PRODUCT`, projection-backed only |
| Chat execution evidence | `MainChatExecutionEvidence` | Visible during/after Chat evidence | Shows task status, blockers, observations, route. | Medium; technical labels. | `COLLAPSED_DETAILS` with concise default timeline |
| Agent Control Plane | `AgentControlPlane` | Visible in Chat when typed snapshot exists | Rich controls, proposals, final delivery, plan review. | High; many internal IDs and sections. | `COLLAPSED_DETAILS` / `ADVANCED_INSPECTOR` |
| Reasoning trace | `ReasoningTracePanel` | Toggle in Chat | Useful for debugging route/tool/model behavior. | High; internal strategy/generation terms. | `ADVANCED_INSPECTOR` |
| Run trace | `RunTracePanel` | Run detail and trace contexts | Metadata-safe trace for ReAct, skill runtime, plan-execute. | High for normal users. | `ADVANCED_INSPECTOR` |
| Tool call details | `ToolCallCard` | Chat/tool evidence | Shows tool status, permission, risk, sanitized arguments. | Medium; can be default when action is blocked. | `DEFAULT_PRODUCT` summary, `COLLAPSED_DETAILS` internals |
| Runtime disclosure strip | `RuntimeDisclosureStrip`, `runtimeDisclosure.ts` | Runs/Run detail/Chat | Explains route, boundary, tools, proposals, blockers. | Medium; mixed English technical labels. | `DEFAULT_PRODUCT` if renamed and simplified |
| Run/task history | `RunsPage` | Top-level product route | Lets user inspect, resume, cancel, retry, delete. | Medium; mixes history and diagnostics. | `DEFAULT_PRODUCT` as `任务`, with advanced details collapsed |
| Run detail timeline/transcript | `AgentRunDetail` | Product subroute | Auditable task/run evidence and replay/detail. | High if raw transcript dominates. | `COLLAPSED_DETAILS` / `ADVANCED_INSPECTOR` |
| Readiness checklist | Settings Overview | Settings default tab | Setup and recovery guidance. | Medium; currently mixes diagnostics and product readiness. | `DEFAULT_PRODUCT` if projection-backed |
| Runtime build info | Settings Overview | Visible when runtime info exists | Useful support/debug context. | Medium; not daily-use content. | `ADVANCED_INSPECTOR` |
| Provider readiness | Settings Provider/Advanced | Settings visible | Explains local/cloud route and validation. | Medium; English statuses and provider internals. | `DEFAULT_PRODUCT` summary, `ADVANCED_INSPECTOR` detail |
| Tool permissions and network policy | Settings Tools & Permissions | Settings visible | User control over web/file/tool authority. | Medium; should remain understandable. | `DEFAULT_PRODUCT` for permissions, advanced for manifests |
| Privacy/provider transmission history | Settings Privacy | Settings visible | Important external-transmission trust evidence. | Medium/high; IDs and route types are technical. | `COLLAPSED_DETAILS` with clear trust summary |
| MCP/A2A pages | `/mcp`, `/a2a` | Advanced menu | External connection management and audit. | High. | `DEVELOPER_ONLY` or `ADVANCED_INSPECTOR`, depending product strategy |
| Metrics | `/metrics` | Advanced menu | Operational metrics. | High. | `DEVELOPER_ONLY` |
| Calibration | `/calibration` | Advanced menu | Product learning/calibration decisions. | Medium. | `NEEDS_HUMAN_DECISION` |
| Versions | `/versions` | Advanced menu | Snapshots, diff, restore. | Medium; trust/safety value. | `NEEDS_HUMAN_DECISION` |
| PolicyRouter authority | Settings Advanced | Settings advanced tab | Confirms active routing authority and old-router state. | High. | `DEVELOPER_ONLY` |
| ModelRouter provider health | Settings Advanced | Settings advanced tab | Provider availability/latency/probed status. | High. | `ADVANCED_INSPECTOR` |
| Internal debug toggles | Settings Advanced | Gated by `isInternalDebugSurfaceEnabled` | Development controls. | Very high. | `DEVELOPER_ONLY` |
| Dev/test command wrappers | `frontend/src/tauriDev.ts` | Not product UI | Test compatibility only. | Dangerous if mistaken for product authority. | `DEVELOPER_ONLY` |
| Stage1/Step6 E2E reports | `frontend/e2e/`, `frontend/scripts/` | Test-only | Evidence/blocked trial scripts. | High; historical. | `DEVELOPER_ONLY` |

Finding: Everyday trust states and diagnostics are mixed across product routes.
Evidence: `ProductShell` shows global readiness/safe-mode, Chat shows execution evidence and trace toggles, Runs shows task/run internals, Settings shows provider/router/policy/debug details.
File location: `frontend/src/components/ProductShell.tsx`; `frontend/src/pages/ChatPage.tsx`; `frontend/src/pages/RunsPage.tsx`; `frontend/src/pages/SettingsPage.tsx`; `frontend/src/pages/settings/tabs/`.
Confidence: High.
Impact: V2 should keep evidence available but define default visibility rules.

## Everyday User Surfaces

Should remain visible by default:

- Safe Mode and recovery guidance.
- Pending review count and links to `审核中心`.
- Current task status: running, waiting for confirmation, blocked, failed, cancelled, completed.
- Next recommended control: continue, retry, cancel, review, inspect.
- Model/privacy boundary summary: local, cloud, unknown, not sent/sent.
- Tool permission summary when a tool needs authorization or is blocked.
- Final result with completed actions, blockers, proposals, and pending user actions separated.

Finding: The backend and frontend already preserve these distinctions, but copy and IA are inconsistent.
Evidence: `LifeStateProjection` task counts; `runtimeDisclosure.ts` status/boundary labels; `MainChatExecutionEvidence`; `AgentControlPlane`.
File location: `src-tauri/src/life_state_projection.rs`; `frontend/src/utils/runtimeDisclosure.ts`; `frontend/src/components/MainChatExecutionEvidence.tsx`; `frontend/src/components/AgentControlPlane.tsx`.
Confidence: High.
Impact: V2 must not flatten blocked/pending/failed/completed states.

## Advanced Inspector Surfaces

Should be available but hidden behind explicit inspection:

- Reasoning trace.
- ReAct action lifecycle.
- Skill runtime trace.
- Kernel event stream.
- Durable event stream replay state.
- Full task transcript.
- Tool sanitized arguments and output hashes.
- Runtime route evidence rows.
- Provider health rows.
- Run JSON export.
- PolicyRouter authority chain.

Finding: Existing trace components already redact or summarize sensitive/raw payloads in several places.
Evidence: `ReasoningTracePanel` sanitizes paths/control characters; `RunTracePanel` presents metadata-safe traces; tests assert raw payloads are not rendered.
File location: `frontend/src/components/ReasoningTracePanel.tsx`; `frontend/src/components/RunTracePanel.tsx`; `frontend/src/components/RunTracePanel.test.tsx`.
Confidence: High.
Impact: V2 can reuse the safety posture while changing default visibility.

## Developer-only Surfaces

Should not appear in normal product navigation:

- `tauriDev.ts` old/development command wrappers.
- Stage1/Step6 retired E2E dogfood/product acceptance scripts and reports.
- Internal debug toggles such as ContextAssembler V2 and AgentLoop.
- Raw PolicyRouter old-router diagnostics unless a support mode is enabled.
- Metrics route unless product strategy explicitly makes metrics user-facing.

Finding: Dev/test/historical artifacts still exist and must remain classified by surface.
Evidence: `frontend/src/tauriDev.ts`, `frontend/src/test/archive/`, `frontend/e2e/`, `Settings AdvancedTab` internal debug gate.
File location: `frontend/src/tauriDev.ts`; `frontend/src/test/archive/`; `frontend/e2e/`; `frontend/src/pages/settings/tabs/AdvancedTab.tsx`.
Confidence: High.
Impact: Raw search hits must not drive product IA claims.

## Risks

1. Diagnostics-first UI makes OpenLife feel like an engineering console rather than a personal AI OS.
2. Internal labels such as `PolicyRouter`, `Provider`, `AgentRun`, `kernel`, and `finalDelivery` leak architecture before user meaning.
3. If pending/proposal states are hidden too aggressively, users may think durable writes completed.
4. If provider/external-transmission evidence is too hidden, privacy trust weakens.
5. If `tauriDev.ts` and historical test surfaces are treated as product, V2 may accidentally restore deleted Phase7 routes.

Human decisions required:

1. Which diagnostics belong to everyday trust affordances versus support/debug tools?
2. Should `版本控制` and `校准` be product routes or advanced tools?
3. What support mode should expose PolicyRouter/ModelRouter internals?
4. What default evidence must always be visible after an agent action?
