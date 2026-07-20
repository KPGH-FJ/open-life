import { useEffect, useMemo, useState } from "react";
import { FoundationStatusLabel } from "@/ui/foundation";
import { ReadOnlySpineJourney, tauriReadOnlySpineDataSource } from "@/ui/journeys/readOnly";
import { tauriGovernedActionDataSource } from "@/ui/journeys/governedAction";
import { phase4dFixtureLabels, type Phase4dFixtureId } from "./phase4d-fixtures";
import {
  phase4dJourneyFixtureDataSource,
  type Phase4dJourneyDataSource,
} from "./phase4d-governed-fixtures";

const HARNESS_MARKER = "OPENLIFE_PHASE4D_GOVERNED_ACTION_HARNESS";
type Phase4dSourceId = "tauri" | Phase4dFixtureId;

function probeResultLabel<
  Snapshot extends {
    envelope: { status: string; warnings?: Array<{ code: string }> };
  },
>(label: string, result: PromiseSettledResult<Snapshot>, includeWarnings = true): string {
  if (result.status === "rejected") return `${label} rejected`;
  const warningCodes = result.value.envelope.warnings?.map(warning => warning.code) ?? [];
  const warnings =
    includeWarnings && warningCodes.length > 0 ? ` [${warningCodes.join(", ")}]` : "";
  return `${label} ${result.value.envelope.status}${warnings}`;
}

function isTauriWindow(): boolean {
  return (
    typeof window !== "undefined" &&
    Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__)
  );
}

const tauriJourneyDataSource: Phase4dJourneyDataSource = {
  ...tauriReadOnlySpineDataSource,
  ...tauriGovernedActionDataSource,
};

export function Phase4dReadOnlyHarness() {
  const tauriAvailable = isTauriWindow();
  const [sourceId, setSourceId] = useState<Phase4dSourceId>(
    tauriAvailable ? "tauri" : "fixture-ready"
  );
  const [probeStatus, setProbeStatus] = useState(
    tauriAvailable ? "正在核对五个后端读模型" : "浏览器 fixture，不代表后端状态"
  );
  const dataSource: Phase4dJourneyDataSource = useMemo(
    () =>
      sourceId === "tauri" ? tauriJourneyDataSource : phase4dJourneyFixtureDataSource(sourceId),
    [sourceId]
  );
  useEffect(() => {
    if (sourceId !== "tauri") {
      setProbeStatus(`${phase4dFixtureLabels[sourceId]} · 非后端状态`);
      return;
    }
    let cancelled = false;
    setProbeStatus("正在核对 Today、Tasks、Workspace 与 Review 命令");
    Promise.allSettled([dataSource.loadToday(), dataSource.loadTasks(), dataSource.load()]).then(
      results => {
        if (cancelled) return;
        const governed =
          results[2].status === "rejected"
            ? "Workspace / Review rejected"
            : `Workspace ${results[2].value.workspaceEnvelope.status} · Review ${results[2].value.reviewEnvelope.status} · Journey Tasks ${results[2].value.tasksEnvelope.status}`;
        setProbeStatus(
          `命令探针：${probeResultLabel("Today", results[0], false)} · ${probeResultLabel(
            "Tasks",
            results[1]
          )} · ${governed}`
        );
        const todayWarnings =
          results[0].status === "fulfilled"
            ? (results[0].value.envelope.warnings?.map(warning => warning.code) ?? [])
            : ["request_rejected"];
        const tasksWarnings =
          results[1].status === "fulfilled"
            ? (results[1].value.envelope.warnings?.map(warning => warning.code) ?? [])
            : ["request_rejected"];
        const governedStatuses =
          results[2].status === "fulfilled"
            ? {
                workspace: results[2].value.workspaceEnvelope.status,
                review: results[2].value.reviewEnvelope.status,
                journeyTasks: results[2].value.tasksEnvelope.status,
                diagnostics: results[2].value.diagnostics.map(item => ({
                  id: item.id,
                  status: item.status,
                })),
              }
            : {
                workspace: "rejected",
                review: "rejected",
                journeyTasks: "rejected",
                diagnostics: [],
              };
        void fetch("/__phase4d_tauri_probe", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            marker: "OPENLIFE_PHASE4D_REAL_TAURI_PROBE",
            today:
              results[0].status === "fulfilled" ? results[0].value.envelope.status : "rejected",
            tasks:
              results[1].status === "fulfilled" ? results[1].value.envelope.status : "rejected",
            todayWarnings,
            tasksWarnings,
            ...governedStatuses,
          }),
        }).catch(() => undefined);
      }
    );
    return () => {
      cancelled = true;
    };
  }, [dataSource, sourceId]);

  return (
    <div
      className="ol-foundation phase4d-harness"
      data-harness-marker={HARNESS_MARKER}
      data-source-id={sourceId}
    >
      <header className="phase4d-qa-toolbar" aria-label="Phase 4D QA 工具栏">
        <div className="phase4d-qa-identity">
          <strong>Phase 4D</strong>
          <span>桌面受治理动作旅程 · DEV ONLY</span>
        </div>
        <label className="phase4d-source-select">
          <span>数据来源</span>
          <select
            value={sourceId}
            onChange={event => setSourceId(event.target.value as Phase4dSourceId)}
          >
            <option value="tauri" disabled={!tauriAvailable}>
              真实 Tauri 后端{tauriAvailable ? "" : "（仅桌面窗口）"}
            </option>
            {(Object.keys(phase4dFixtureLabels) as Phase4dFixtureId[]).map(id => (
              <option key={id} value={id}>
                {phase4dFixtureLabels[id]}
              </option>
            ))}
          </select>
        </label>
        <p className="phase4d-qa-feedback" role="status" aria-live="polite">
          {probeStatus}
        </p>
        <div className="phase4d-qa-boundaries" aria-label="开发边界">
          <FoundationStatusLabel label="DESKTOP ≥1024" />
          {sourceId === "tauri" ? (
            <FoundationStatusLabel label="TAURI READ MODEL" />
          ) : (
            <FoundationStatusLabel label="静态 fixture · 非后端状态" status="unknown" />
          )}
          <FoundationStatusLabel label="决定与恢复分离" status="waiting" />
        </div>
        {sourceId === "tauri" && (
          <p className="phase4d-qa-warning" role="note">
            真实 Tauri 模式会记录审核决定或任务恢复请求；仅使用隔离的开发数据进行验证。
          </p>
        )}
      </header>

      <div className="phase4d-shell-stage" key={sourceId}>
        <ReadOnlySpineJourney dataSource={dataSource} governedActionDataSource={dataSource} />
      </div>
    </div>
  );
}
