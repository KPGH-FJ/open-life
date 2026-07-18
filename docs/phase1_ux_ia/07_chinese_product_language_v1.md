# Chinese Product Language v1

Status: Phase 1 language proposal.
Scope: Chinese-first naming, status, action, and diagnostics copy only.

## Classification Legend

- `VERIFIED_FACT`
- `DESIGN_DECISION`
- `DESIGN_ASSUMPTION`
- `CANDIDATE`
- `UNKNOWN`
- `PHASE_2_REQUIRED`

## Audience

`DESIGN_DECISION`: The first V2 product language should assume a Chinese-first user who wants a precise personal AI operating partner, not a developer console.

`VERIFIED_FACT`: Current navigation is mostly English while many page bodies already use Chinese. Source: `docs/phase0_5/05_ui_terminology_inventory.md`.

## Top-level Navigation

| Concept | Recommended Chinese | Classification | Notes |
| --- | --- | --- | --- |
| Today | 今日 | `DESIGN_DECISION` | Existing page title already supports this. |
| Workspace | 工作区 | `DESIGN_DECISION` | Merges Companion/Chat as current work surface. |
| Tasks | 任务 | `DESIGN_DECISION` | Replaces Runs for user-facing lifecycle. |
| Review Center | 审核中心 | `DESIGN_DECISION` | Replaces Mailbox framing. |
| LifeModel | LifeModel | `DESIGN_DECISION` with constraints | Keep English brand, explain in Chinese. |
| Memory | 记忆 | `CANDIDATE` / Accepted with constraints | Top-level only if boundaries are validated. |
| Settings | 设置 | `DESIGN_DECISION` | Split product settings from advanced/developer inspection. |

## Recommended Words

| Product concept | Recommended wording | Avoid by default | Classification |
| --- | --- | --- | --- |
| agent work | 工作 / 任务 | run / AgentRun | `DESIGN_DECISION` |
| current user goal | 目标 / 你想完成的事 | prompt | `DESIGN_DECISION` |
| system interpretation | OpenLife 理解为 | intent frame | `DESIGN_DECISION` |
| evidence | 依据 | raw trace | `DESIGN_DECISION` |
| source/provenance | 来源 / 来源与记录 | provenance | `DESIGN_DECISION` |
| review item | 待确认项 | proposal | `DESIGN_DECISION` |
| proposed change | 候选更新 / 建议变更 | proposal payload | `DESIGN_DECISION` |
| pending review | 待确认 | pending proposal | `DESIGN_DECISION` |
| tool permission | 工具权限 | tool manifest | `DESIGN_DECISION` |
| external write | 外部写入 / 外部操作 | external action runner | `DESIGN_DECISION` |
| blocked | 已阻断 | fallback | `DESIGN_DECISION` |
| safe mode | 安全模式 or Safe Mode（安全模式） | safety debug mode | `UNKNOWN`; human naming decision required |

## Forbidden / Default-hidden Words For Normal Users

These words may appear in `高级检查` or developer/support surfaces, but should not appear in ordinary product copy:

- run
- trace
- proposal
- kernel
- provider
- policy router
- final delivery
- AgentRun
- raw transcript
- mailbox

`DESIGN_DECISION`: Default copy should translate architecture into user outcomes and controls. It should not hide evidence, but it should avoid exposing implementation terms as the product model.

## Ordinary User Terms

| Need | Recommended copy |
| --- | --- |
| start work | 开始 |
| clarify | 补充说明 |
| continue | 继续 |
| retry | 重试 |
| cancel | 取消 |
| inspect evidence | 查看依据 |
| open review | 去审核 |
| no current work | 还没有正在进行的任务 |
| no review items | 没有待确认项 |
| stale state | 状态可能已过期 |
| external boundary | 可能会使用云端或外部服务 |

## Review Terms

| Concept | Recommended Chinese | Notes |
| --- | --- | --- |
| Review Center | 审核中心 | Top-level route. |
| Review item | 待确认项 | Default object name. |
| Candidate update | 候选更新 | For memory/LifeModel changes. |
| Approval | 批准 / 同意 | Use consistently by context. |
| Rejection | 拒绝 / 不同意 | Current Mailbox uses `不同意`; human approval needed. |
| Later | 稍后 | For postpone. |
| Edit | 修改 | For proposal/review edit. |
| Evidence | 查看依据 | ReviewAction. |
| Applied | 已应用 | Only after durable apply/materialization. |

## Advanced Inspection Terms

| Concept | Recommended Chinese | Notes |
| --- | --- | --- |
| Advanced inspection | 高级检查 | User-accessible evidence layer. |
| Runtime route | 执行路线 | Use only in details. |
| Tool details | 工具详情 | Hide arguments/output by default. |
| Provider health | 模型服务状态 | Advanced/support. |
| Transcript | 执行记录 | Prefer over raw transcript in user-facing details. |
| Debug export | 导出调试信息 | Developer/support action. |

## Developer-only Terms

`DESIGN_DECISION`: These remain developer/support terms unless human review explicitly promotes them:

- PolicyRouter
- ModelRouter
- MCP/A2A internals
- kernel event
- durable event stream
- raw JSON
- tauriDev
- stage/beta/migration/cutover historical surfaces
- metrics internals
- calibration internals

## Status Labels

| Concept | Chinese | Notes |
| --- | --- | --- |
| loading | 读取中 | Data loading. |
| stale | 可能已过期 | Do not silently show stale as current truth. |
| running | 运行中 | Current task action. |
| waiting_permission | 等待你确认 | More specific than `待处理`. |
| blocked | 已阻断 | Must show reason. |
| failed | 失败 | Do not soften to completed. |
| cancelled | 已取消 | User/system cancelled. |
| completed | 已完成 | Only when no pending durable change is implied. |
| completed_with_pending_items | 已完成，但有待确认项 | Important final-delivery distinction. |
| pending | 待确认 | Review state, not loading. |
| approved | 已同意 | Approval recorded. |
| rejected | 不同意 | Rejected. |
| applied | 已应用 | Durable application completed. |
| materialized | 已写入长期状态 | Only after materialization. |
| rolled_back | 已回滚 | Reverted. |
| revoked | 已撤销 | Permission/review revoked. |
| expired | 已过期 | Review item no longer actionable. |

## Action Verbs

| Action type | Recommended verbs |
| --- | --- |
| ProductAction | 开始, 继续, 重试, 取消, 查看任务, 去审核, 查看依据 |
| ReviewAction | 批准, 拒绝, 稍后, 修改, 查看依据 |
| DebugAction | 打开高级检查, 导出调试信息, 查看原始记录, 查看模型服务状态 |

## Review / Proposal Terminology

`DESIGN_DECISION`: Normal users should see `待确认项`, `候选更新`, `建议变更`, and `已应用`. The English word `proposal` is default-hidden.

`DESIGN_DECISION`: Copy must never say "已记住" or "已更新 LifeModel" when the system only created a review item.

## LifeModel / Memory Terminology

| Concept | Recommended wording |
| --- | --- |
| LifeModel | LifeModel |
| LifeModel subtitle | OpenLife 对你的长期理解 |
| Memory | 记忆 |
| Candidate memory | 候选记忆 |
| Confirmed memory | 已确认记忆 |
| Used in LifeModel | 已用于 LifeModel |
| Withdrawn/expired memory | 已撤回 / 已过期 |
| Evidence/provenance | 依据 / 来源 |
| Change | 变更 |

## Diagnostics Terminology

| Default product term | Advanced/developer equivalent |
| --- | --- |
| 执行状态 | runtime status |
| 依据 | trace / evidence refs |
| 工具权限 | manifest / permission policy |
| 模型路线 | provider/model route |
| 高级检查 | raw trace / transcript / debug |
| 安全模式 | safe mode |

## Agent Tone Guidelines

- `DESIGN_DECISION`: Be direct and concrete; avoid theatrical AI personality.
- `DESIGN_DECISION`: Say what is known, what is pending, what is blocked, and what the user can do.
- `DESIGN_DECISION`: Use humble uncertainty for unknowns: `我还不能确认...`, `需要你确认...`, `当前没有足够依据...`.
- `DESIGN_DECISION`: Never use assistant prose as write authorization.
- `DESIGN_DECISION`: Keep privacy and external transmission boundaries visible without turning default UI into diagnostics.

## Phase 2 Language Stop Rules

1. `PHASE_2_REQUIRED`: Do not ship final navigation labels until humans approve `工作区` vs `任务工作台`, `LifeModel`, `记忆`, and Safe Mode wording.
2. `PHASE_2_REQUIRED`: Do not mix `批准/拒绝` with `同意/不同意` in the same review flow without a final terminology decision.
3. `PHASE_2_REQUIRED`: Do not show default-hidden terms in ordinary product UI unless the surface is explicitly `高级检查`, support, or developer-only.
4. `PHASE_2_REQUIRED`: Do not use `已记住`, `已更新 LifeModel`, `已应用`, or `已写入长期状态` unless backend durable state proves the change happened.

## Open Questions

1. Should `Safe Mode` be fully localized as `安全模式`?
2. Should Review Center actions use `批准/拒绝` or `同意/不同意`?
3. Should `工作区` or `任务工作台` be the final workspace label?
4. How should `LifeModel` be introduced during first-run onboarding?
5. Which advanced terms are allowed in user-facing support mode?
