# Hallucination And Contract Risk Audit

Status: `QA_PASS_WITH_NAMED_CONTRACT_BLOCKERS`

## 1. Audit Method

For every important product claim, check three layers:

1. authority owner exists in current source;
2. shipped command/read model exposes the required fact;
3. UI wording does not exceed bounded verification evidence.

If layer 1 exists but layer 2 does not, Phase 3F calls it a projection gap. If
layer 2 exists but verification is bounded, Phase 3F narrows the wording. A
fixture never closes either gap.

## 2. Claim Audit

| Claim | Evidence result | Allowed wording | Forbidden wording |
|---|---|---|---|
| Roadshow backend work produced a verified freeze | current state file and freeze tag support a bounded freeze | `后端能力冻结已验证（限定范围）` | `Roadshow/Phase7/全部后端已完成` |
| External provider generation works | bounded direct/stream/live journey evidence exists | `部分当前代码路径已有外部 live 证据` | `所有供应商均已就绪` |
| Web search works | current DeepSeek/generic fetch bounded evidence exists | `受治理搜索路径已有验证` | `联网搜索始终可用` |
| Resource import works | shipped picker/import/status/detach and resource gates exist | `支持受限格式的本地导入` | `任意附件均可安全读取` |
| Today uses canonical task truth | StateStore owns product daily tasks, exposed through compatibility DTO | `今日任务来自 StateStore 投影` | `已有完整 TodayViewModel` |
| Exact one-time permission exists | action-bound allow-once grant/consume exists | `后端支持精确的一次性动作授权` | `当前 ReviewItem 已能完整解释范围` |
| Review can decide proposals | ReviewItem actions and proposal commands exist | `可批准/拒绝/修改/稍后` | `批准后已写入长期状态` |
| LifeModel applied state is proven | VM checks proposal, patch, snapshots and current value | `满足全部证据后显示已应用` | `accepted 就是 applied` |
| Local model is configured | sanitized AppConfig may prove configuration | `已配置本地模型` | `所有内容都在本地处理` |
| Connection test succeeds | exact test receipt may prove one test | `本次连接验证成功` | `后续请求一定成功/私密` |
| Workspace is ready for full V2 | current contract explicitly says limited | `有限工作区投影可作为输入` | `WorkspaceViewModel 是完整 Frontend V2` |
| MCP/A2A/plugins exist | code exists under dev extensions | `开发扩展能力` | production primary product capability |

## 3. Current Contract Blockers

### P0-1 Rich Review Decision Context

`ReviewItem` does not project readable before/after, reason, impact, affected
objects, or a typed evidence body. `AgentProposal` has some values, but a
frontend join would create a second authority. This blocks rich Review React
porting.

### P0-2 Readable Exact Permission Context

The backend now enforces exact action-bound scope, but `ReviewItem` exposes only
refs/actions. The frontend cannot safely answer: what tool, which resolved
target, which capabilities, which exact input digest, which action, and what
transmission boundary. Known-scope Phase 3F scenarios are `TARGET_CONTRACT`.

### P0-3 Complete Workspace Composition

`WorkspaceViewModel` is intentionally limited. Task, resource, Web, review,
provider and transcript truths are available through separate authorities. A
reviewed backend composition or strict adapter contract is required before
React; unrestricted page-level joins are rejected.

### P0-4 Today Authority Shape

StateStore daily tasks are canonical, but Today still composes a broader
experience through a frontend adapter. The next phase must either freeze that
adapter as a bounded formatter or add a backend Today read model.

### P1-1 Settings Composition

Config, provider validation, privacy boundary, tool permissions, network policy,
data recovery and memory settings have separate owners. The interaction spec is
clear, but the product needs a strict orchestration contract and tests.

### P1-2 Artifact/Product Evidence Projection

Reviewed artifact mechanics are real; a standalone artifact list/detail model
is not. Workspace may show turn-local proposal/effect status only when tied to
current task evidence.

## 4. Prototype-Specific Risks

| Risk | Mitigation |
|---|---|
| fixture transition looks like backend success | QA toolbar always visible; live feedback says static demonstration |
| known permission fixture hides current projection gap | known and unknown scenarios both exist; Inspector labels target contract |
| sample provider/model looks supported | option copy is layout fixture; no provider readiness claim |
| real-looking task data is mistaken for current data | scenario data file labels every user story as fixture |
| approve navigation appears to materialize | intermediate approved-not-applied state is mandatory |
| fake buttons | every enabled control has a verified static result; unsupported controls are disabled/unavailable |
| evidence metadata is mistaken for evidence body | Inspector separates `EvidenceRef` metadata from fixture explanatory summary |

## 5. Required Scans Before Handoff

Run and classify:

```sh
git diff --check
rg -n "backend ready|后端已完成|全部就绪|已应用|已完成|本地处理|allow_until_revoked|WorkspaceViewModel" \
  docs/phase3f_ux_interaction_blueprint
rg -n "ProductShell|productShellContract|frontend/src/App|src-tauri|openlife-core" \
  <changed-file-list>
```

The second scan is not an automatic failure. Each hit must be checked for
negation, explicit bounded evidence, or a prohibited claim.

## 6. Audit Gate

```text
UNKNOWN_AND_STALE_FAIL_CLOSED = REQUIRED
APPROVED_DISTINCT_FROM_APPLIED = REQUIRED
FIXTURE_DISTINCT_FROM_BACKEND = REQUIRED
DEV_ONLY_DISTINCT_FROM_PRODUCT = REQUIRED
PROJECTION_GAPS_EXPLICIT = REQUIRED
GLOBAL_COMPLETION_CLAIM = FORBIDDEN
```

Interaction tests, screenshots, wording scans, and changed-file scope checks
passed on 2026-07-18. The audit therefore passes for review handoff, while the
P0 projection gaps in section 3 remain open and keep `REACT_PORT_READY = NO`.
