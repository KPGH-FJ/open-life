# OpenLife vNext P11 Trial Path Matrix

Date: 2026-05-08

Status: current

This matrix defines the repeatable manual smoke paths for Beta Trial Readiness.
It is intentionally product-facing: a tester should be able to follow it without
reading Rust, TypeScript, or architecture documents.

## How to Use This Matrix

1. Choose your profile (clean or existing).
2. Follow the must-pass smoke paths in order.
3. For each path, perform the steps and verify the expected result.
4. If you encounter a failure signal, follow the recovery instructions.
5. Record results using the Tester Report Template at the bottom.
6. Optional exploratory paths can be run after must-pass paths are complete.

## Trial Profiles

| Profile | Purpose | Data State |
|---|---|---|
| Clean profile | Validate first-run experience and onboarding | Empty app data directory or fresh test profile |
| Existing profile | Validate continuity and recovery | Existing LifeModel, sessions, proposals, runs, and settings |

## Must-Pass Smoke Paths

### P11-S1: First Launch and Diagnostics

**Setup:** Clean profile (or existing profile after data reset).

**Steps:**
1. Launch the OpenLife app.
2. Wait for the initial loading screen to complete.
3. Observe the Workspace/Dashboard page loads.
4. Navigate to Settings (齿轮图标).
5. Open the Overview tab in Settings.
6. Review the Trial Checklist and Beta Readiness state.

**Expected Result:**
- App renders without crash or blank screen.
- Workspace shows system status cards.
- Settings Overview shows a trial checklist with clear pass/fail indicators.
- If no model is configured, checklist shows actionable blocker with link to fix.
- Diagnostics state is visible (green = ready, amber = partial, red = blocked).

**Failure Signals:**
- Blank screen or permanent loading spinner.
- Settings page shows no checklist or unclear readiness state.
- Error banner with unactionable message.
- App crashes during launch.

**Recovery:**
- Blank screen: Check developer console (if available) or restart app.
- Check Settings → Data → Export Diagnostics to capture launch state.
- If persistent, file issue with platform info and launch logs.

---

### P11-S2: Provider Configuration

**Setup:** Clean profile or existing profile with missing/invalid provider.

**Steps:**
1. Open Settings → Provider tab.
2. Select a provider (DeepSeek, OpenAI, OpenRouter, or Ollama).
3. Enter API Key (for cloud providers) or verify Ollama is running (for local).
4. Click Test Connection.
5. Wait for test result.
6. Save configuration.
7. Return to Settings Overview and verify checklist updates.

**Expected Result:**
- Test connection shows success or gives actionable error (e.g., "Invalid API key", "Ollama not reachable at localhost:11434").
- Saved configuration persists after app restart.
- Settings Overview checklist updates: model provider shows green checkmark.
- Chat readiness status updates to reflect available backend.

**Failure Signals:**
- API key rejected without specific reason.
- Saved config not reflected in UI after restart.
- Chat remains blocked despite successful test.
- Test hangs indefinitely (>30s).

**Recovery:**
- Re-enter provider/base URL/model name and retest.
- For cloud: verify API key is valid and has credits.
- For Ollama: ensure `ollama serve` is running and model is pulled.
- Use Settings Overview diagnostic export to inspect configuration state.
- Switch to alternative provider if one fails.

---

### P11-S3: Quick LifeModel Build

**Setup:** Clean profile with configured provider.

**Steps:**
1. Navigate to Builder page.
2. Select Quick Build mode.
3. Answer the guided questions (identity, goals, capabilities, state).
4. Complete the build session.
5. Review the generated Proposal(s) in Builder or navigate to Review page.
6. Do NOT apply directly - verify proposals are created for confirmation.

**Expected Result:**
- Builder creates one or more Proposals.
- Proposals do NOT directly mutate high-risk LifeModel fields without confirmation.
- Review page shows pending proposals with source, evidence, and risk level.
- Builder completion percentage increases in Dashboard.

**Failure Signals:**
- Builder silently applies changes without creating proposals.
- No Review link or pending proposals after build.
- Build session gets stuck or loops.
- Proposals created but cannot be found in Review.

**Recovery:**
- Open Review Center directly from navigation.
- Reload Builder page and check for unfinished sessions.
- Inspect pending proposals via Settings → Overview.
- If proposals are missing, check diagnostics for builder session state.

---

### P11-S4: Chat to Proposal

**Setup:** LifeModel exists (built or existing profile).

**Steps:**
1. Navigate to Chat/Agent page.
2. Send a message containing a goal, state, or preference statement (e.g., "I want to learn Spanish" or "I'm feeling stressed about work").
3. Wait for assistant response.
4. Observe if a proposal banner appears (may take a few messages depending on confidence threshold).
5. If banner appears, click it to view the proposal.
6. Navigate to Runs page and verify a new run was created.

**Expected Result:**
- Assistant responds with relevant, personalized content based on LifeModel.
- A new AgentRun is created and visible in Runs page.
- Proposal banner may appear if message triggers extraction (not guaranteed on first message).
- Run detail shows model route, tool calls, and any generated proposals.

**Failure Signals:**
- No response or permanent "thinking" state.
- No run created in Runs page.
- Proposal created without banner or clear source indication.
- Generic response ignoring LifeModel context.

**Recovery:**
- Check Settings → Overview for chat readiness issues.
- Open Runs page to inspect latest run status and error details.
- Check if model backend is responding (Settings → Provider → Test Connection).
- If no proposals generated, verify chat_proposal is enabled in config.

---

### P11-S5: Proposal Review and Apply

**Setup:** Pending proposal exists (from P11-S3 or P11-S4).

**Steps:**
1. Navigate to Review page.
2. Inspect pending proposal(s):
   - Read the reason and affected path.
   - Check risk level (low/medium/high).
   - View source/evidence if available.
3. Accept a low-risk proposal.
4. Reject a test proposal (if high-risk or unwanted).
5. Verify accepted proposal applies successfully.
6. Verify rejected proposal does not mutate data.
7. Check that source run link works (if proposal has run_id).

**Expected Result:**
- Accepted proposal applies safely and updates LifeModel/memory accordingly.
- Rejected proposal remains in rejected state without mutating data.
- Source run link navigates to correct run detail.
- Dashboard reflects applied changes.
- Version snapshot may be auto-created for significant changes.

**Failure Signals:**
- Apply failure marks proposal as accepted but data not updated.
- High-risk proposal can be batch-applied without individual review.
- Source run link is broken or missing.
- Proposal apply crashes or hangs.

**Recovery:**
- Keep proposal pending on apply failure.
- Use Version Control (Snapshots) to restore previous state if needed.
- Check Settings → Data → Export Diagnostics for proposal apply errors.
- Inspect run trace for detailed error information.

---

### P11-S6: Run Trace Inspection

**Setup:** At least one run exists (from P11-S4).

**Steps:**
1. Navigate to Runs page.
2. Select the latest run.
3. Expand the run detail view.
4. Review timeline, tool observations, and proposal evidence panels.
5. Check for redaction/truncation markers.
6. Verify run metadata (model, provider, tool count, proposal count).

**Expected Result:**
- Trace shows event summaries with clear phase labels.
- Redaction/truncation markers visible where sensitive content was masked.
- Tool observations show bounded outputs with risk scope.
- Linked proposals are clickable and navigable.
- No raw sensitive prompts or memory content exposed by default.

**Failure Signals:**
- Raw sensitive prompt exposed in trace.
- Unknown event type crashes UI.
- Tool output unbounded or missing risk indicators.
- Run metadata missing or incorrect.

**Recovery:**
- Export diagnostic summary (Settings → Data → Export Diagnostics).
- Avoid sharing raw trace screenshots with sensitive data.
- File issue with run ID and description of UI crash.

---

### P11-S7: Plan Inspection and Legal Operation

**Setup:** A plan exists for a run (may require running a planning task or using existing profile with plans).

**Steps:**
1. Navigate to Runs page.
2. Select a run with an associated plan.
3. Open plan detail view.
4. Inspect plan steps, status, and risk level.
5. Attempt legal operations: confirm, reject, cancel (only if in legal state).
6. Verify operation updates plan status and creates trace events.

**Expected Result:**
- Buttons match legal state (e.g., cannot cancel completed plan).
- Operation updates status correctly.
- Trace records plan operation events.
- Illegal operations are disabled or rejected with clear message.

**Failure Signals:**
- Terminal plan shows retry/cancel buttons illegally.
- Operation bypasses confirmation when required.
- Plan status not updated after operation.
- UI allows illegal state transitions.

**Recovery:**
- Refresh plan detail page.
- Inspect Runs trace for plan operation events.
- Report plan ID and current status in issue.

---

### P11-S8: Backup/Export and Safe Mode Recovery

**Setup:** Existing profile preferred (has data to export).

**Steps:**
1. Navigate to Settings → Data tab.
2. Click Export All Data.
3. Save the export file to a known location.
4. Verify export file contains data structure (JSON with version, app_version, life_model, messages, vectors).
5. If in Safe Mode (indicated by amber banner in Settings):
   - Follow Safe Mode guidance in Settings Overview.
   - Export diagnostics before attempting recovery.
   - Use recovery actions (rebuild index, run tier maintenance) as guided.

**Expected Result:**
- Export completes successfully with clear success message.
- Export file is valid JSON with expected structure.
- Safe Mode explains restricted operations clearly.
- Safe Mode provides actionable next steps (export, rebuild, check data).
- Export does NOT include raw sensitive content by default ( LifeModel and messages are present but diagnostic exports exclude raw memory/tool output).

**Failure Signals:**
- Export includes raw private data unexpectedly.
- Safe Mode has no next action or unclear guidance.
- Export fails silently or creates invalid file.
- Safe Mode restrictions not enforced (allows dangerous operations).

**Recovery:**
- Stop applying proposals if in Safe Mode.
- Export diagnostics for analysis.
- Restore from snapshot/backup if available (Settings → Data or Version Control).
- Check data directory permissions if export fails.

---

## Optional Exploratory Paths

These paths are not required for Beta readiness but help validate edge cases.

| ID | Path | What To Explore | Guardrail |
|---|---|---|---|
| P11-E1 | Memory search and governance | Search memories, create explicit "remember this" proposal, archive memory | Memory writes remain proposal-first |
| P11-E2 | Tool permission flow | Trigger a permission-gated tool and inspect Review Center | No write action executes without policy/proposal |
| P11-E3 | MCP/A2A pages | Inspect configured tools and A2A state | Disabled/declarative-only tools must not appear executable |
| P11-E4 | Long run continuity | Continue a longer chat and inspect compaction trace if triggered | Sensitive context remains summarized/redacted |
| P11-E5 | Safe Mode boundary | Simulate data corruption (e.g., stop app mid-write) and verify Safe Mode detection | Safe Mode should detect and block risky operations |

---

## Release Gate Checklist

Before declaring a trial build ready:

- [ ] `make ci` passes (Rust tests, frontend tests, frontend build, typecheck).
- [ ] P11-S1 through P11-S8 are run on a clean profile.
- [ ] P11-S1, P11-S2, P11-S4, P11-S5, P11-S6, and P11-S8 are run on an existing profile.
- [ ] Any failed smoke path has a filed issue with:
  - Profile type (clean/existing)
  - Run ID / Proposal ID / Plan ID (when available)
  - Expected result
  - Actual result
  - Recovery attempted
- [ ] P9 shell checks remain true:
  - [ ] No terminal UI in the app.
  - [ ] No generic chat shell prompt.
  - [ ] `shell.run` excluded from generic tools prompt.
  - [ ] Scheduled/proactive/sub-agent shell disabled by default.
- [ ] Feedback/diagnostic export excludes raw sensitive content by default.
- [ ] README.md points to this trial path matrix.

---

## Tester Report Template

Copy and fill this template when reporting trial results:

```markdown
## Trial Report

**Profile:** clean | existing
**Build/Date:** 
**Tester:** 

### Smoke Path Results

| Path | Status | Notes |
|---|---|---|
| P11-S1 First Launch | pass / fail / blocked | |
| P11-S2 Provider Config | pass / fail / blocked | |
| P11-S3 LifeModel Build | pass / fail / blocked | |
| P11-S4 Chat to Proposal | pass / fail / blocked | |
| P11-S5 Proposal Review | pass / fail / blocked | |
| P11-S6 Run Trace | pass / fail / blocked | |
| P11-S7 Plan Inspection | pass / fail / blocked | |
| P11-S8 Backup/Recovery | pass / fail / blocked | |

### Detailed Issue Report (if any failure)

**Path:** 
**Expected:** 
**Actual:** 
**Run ID:** 
**Proposal ID:** 
**Plan ID:** 
**Diagnostics State:** 
**Recovery Attempted:** 
**Privacy Concern Observed:** yes / no

### Environment

- OS: 
- App Version: 
- Provider: 
- Local Model (if any): 

### Additional Notes

```

## Quick Reference

| If you see... | Check... |
|---|---|
| "模型后端还没完全就绪" | Settings → Provider → Test Connection |
| "人生模型还没有建立起来" | Builder page → Quick Build |
| Safe Mode banner | Settings → Overview → Recovery Console |
| Pending proposals alert | Review page |
| Chat no response | Settings → Overview → Readiness Issues |
| Export fail | Settings → Data → Export Diagnostics first |
