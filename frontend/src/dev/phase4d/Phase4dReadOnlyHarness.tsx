import { useEffect, useMemo, useState } from "react";
import { FoundationStatusLabel } from "@/ui/foundation";
import {
  ReadOnlySpineJourney,
  tauriReadOnlySpineDataSource,
  type ReadOnlySpineDataSource,
} from "@/ui/journeys/readOnly";
import {
  phase4dFixtureDataSource,
  phase4dFixtureLabels,
  type Phase4dFixtureId,
} from "./phase4d-fixtures";

const HARNESS_MARKER = "OPENLIFE_PHASE4D_READ_ONLY_SPINE_HARNESS";
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

export function Phase4dReadOnlyHarness() {
  const tauriAvailable = isTauriWindow();
  const [sourceId, setSourceId] = useState<Phase4dSourceId>(
    tauriAvailable ? "tauri" : "fixture-ready"
  );
  const [probeStatus, setProbeStatus] = useState(
    tauriAvailable ? "正在核对 Today 与 Tasks 命令" : "浏览器 fixture，不代表后端状态"
  );
  const dataSource: ReadOnlySpineDataSource = useMemo(
    () =>
      sourceId === "tauri" ? tauriReadOnlySpineDataSource : phase4dFixtureDataSource(sourceId),
    [sourceId]
  );
  useEffect(() => {
    if (sourceId !== "tauri") {
      setProbeStatus(`${phase4dFixtureLabels[sourceId]} · 非后端状态`);
      return;
    }
    let cancelled = false;
    setProbeStatus("正在核对 Today 与 Tasks 命令");
    Promise.allSettled([dataSource.loadToday(), dataSource.loadTasks()]).then(results => {
      if (cancelled) return;
      setProbeStatus(
        `命令探针：${probeResultLabel("Today", results[0], false)} · ${probeResultLabel(
          "Tasks",
          results[1]
        )}`
      );
    });
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
          <span>桌面只读业务旅程 · DEV ONLY</span>
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
          <FoundationStatusLabel label="只读" status="waiting" />
        </div>
      </header>

      <div className="phase4d-shell-stage" key={sourceId}>
        <ReadOnlySpineJourney dataSource={dataSource} />
      </div>
    </div>
  );
}
