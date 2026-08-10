import { useMemo, useState } from "react";
import { Copy, Download, History, Plus, RotateCcw, Trash2 } from "lucide-react";
import type {
  DraftLifeModelV2ChangeRequest,
  DraftLifeModelV2ExportRequest,
  DraftLifeModelV2RollbackRequest,
  LifeModelCanonicalSummary,
  LifeModelDocumentV2,
  LifeModelSectionV2,
  LifeModelUserValueV2,
  LifeModelVersionHistoryEntryV2,
} from "@/tauri";
import { FoundationNotice, FoundationStatusLabel } from "@/ui/foundation";

type CanonicalItem = {
  id: string;
  section: LifeModelSectionV2;
  title: string;
  detail: string;
  value: LifeModelUserValueV2;
  confirmedAt: string;
};

const sections: { id: LifeModelSectionV2; label: string }[] = [
  { id: "identity", label: "身份" },
  { id: "values", label: "价值观" },
  { id: "long_term_goals", label: "长期目标" },
  { id: "stable_preferences", label: "稳定偏好" },
  { id: "personal_boundaries", label: "个人边界" },
  { id: "important_relationships", label: "重要关系" },
  { id: "capabilities", label: "能力" },
  { id: "resources", label: "长期资源" },
  { id: "decision_principles", label: "决策原则" },
  { id: "collaboration_preferences", label: "协作方式" },
];

const statementSections = new Set<LifeModelSectionV2>([
  "identity",
  "values",
  "stable_preferences",
  "personal_boundaries",
  "decision_principles",
  "collaboration_preferences",
]);

function sectionItems(document: LifeModelDocumentV2): CanonicalItem[] {
  const statements = (
    section: LifeModelSectionV2,
    items: LifeModelDocumentV2["identity"]
  ): CanonicalItem[] =>
    items.map(item => ({
      id: item.id,
      section,
      title: item.statement,
      detail: "",
      confirmedAt: item.confirmedAt,
      value: { kind: "statement", value: { statement: item.statement } },
    }));
  return [
    ...statements("identity", document.identity),
    ...statements("values", document.values),
    ...document.longTermGoals.map(item => ({
      id: item.id,
      section: "long_term_goals" as const,
      title: item.direction,
      detail: item.meaning,
      confirmedAt: item.confirmedAt,
      value: {
        kind: "long_term_goal" as const,
        value: { direction: item.direction, meaning: item.meaning },
      },
    })),
    ...statements("stable_preferences", document.stablePreferences),
    ...statements("personal_boundaries", document.personalBoundaries),
    ...document.importantRelationships.map(item => ({
      id: item.id,
      section: "important_relationships" as const,
      title: item.personLabel,
      detail: `${item.relationship} · ${item.significance}`,
      confirmedAt: item.confirmedAt,
      value: {
        kind: "relationship" as const,
        value: {
          person_label: item.personLabel,
          relationship: item.relationship,
          significance: item.significance,
        },
      },
    })),
    ...document.capabilities.map(item => ({
      id: item.id,
      section: "capabilities" as const,
      title: item.name,
      detail: item.description,
      confirmedAt: item.confirmedAt,
      value: {
        kind: "capability" as const,
        value: { name: item.name, description: item.description },
      },
    })),
    ...document.resources.map(item => ({
      id: item.id,
      section: "resources" as const,
      title: item.name,
      detail: item.description,
      confirmedAt: item.confirmedAt,
      value: {
        kind: "resource" as const,
        value: { name: item.name, description: item.description },
      },
    })),
    ...statements("decision_principles", document.decisionPrinciples),
    ...statements("collaboration_preferences", document.collaborationPreferences),
  ];
}

function valueFor(
  section: LifeModelSectionV2,
  primary: string,
  secondary: string,
  tertiary: string
): LifeModelUserValueV2 | null {
  const one = primary.trim();
  const two = secondary.trim();
  const three = tertiary.trim();
  if (statementSections.has(section)) {
    return one ? { kind: "statement", value: { statement: one } } : null;
  }
  if (section === "long_term_goals") {
    return one && two ? { kind: "long_term_goal", value: { direction: one, meaning: two } } : null;
  }
  if (section === "important_relationships") {
    return one && two && three
      ? {
          kind: "relationship",
          value: { person_label: one, relationship: two, significance: three },
        }
      : null;
  }
  const kind = section === "capabilities" ? "capability" : "resource";
  return one && two ? { kind, value: { name: one, description: two } } : null;
}

function fieldsFor(value: LifeModelUserValueV2): [string, string, string] {
  switch (value.kind) {
    case "statement":
      return [value.value.statement, "", ""];
    case "long_term_goal":
      return [value.value.direction, value.value.meaning, ""];
    case "relationship":
      return [value.value.person_label, value.value.relationship, value.value.significance];
    case "capability":
    case "resource":
      return [value.value.name, value.value.description, ""];
  }
}

export function LifeModelV2ControlsPanel({
  canonical,
  history,
  disabledReason,
  action,
  onChange,
  onRollback,
  onExport,
  onOpenReview,
}: {
  canonical: LifeModelCanonicalSummary;
  history: LifeModelVersionHistoryEntryV2[];
  disabledReason?: string;
  action: {
    kind: "change" | "rollback" | "export";
    status: "submitting" | "review_required" | "failed";
    proposalId?: string;
    error?: string;
  } | null;
  onChange: (request: DraftLifeModelV2ChangeRequest) => Promise<boolean>;
  onRollback: (request: DraftLifeModelV2RollbackRequest) => Promise<boolean>;
  onExport: (request: DraftLifeModelV2ExportRequest) => Promise<boolean>;
  onOpenReview?: () => void;
}) {
  const items = useMemo(() => sectionItems(canonical.document), [canonical.document]);
  const [section, setSection] = useState<LifeModelSectionV2>("identity");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [primary, setPrimary] = useState("");
  const [secondary, setSecondary] = useState("");
  const [tertiary, setTertiary] = useState("");
  const [confirming, setConfirming] = useState<string | null>(null);
  const [exportFormat, setExportFormat] = useState<"yaml" | "json">("yaml");
  const [exportPath, setExportPath] = useState("");
  const [localMessage, setLocalMessage] = useState<string | null>(null);
  const busy = action?.status === "submitting";
  const blocked = Boolean(disabledReason) || busy;
  const selectedSectionLabel =
    sections.find(candidate => candidate.id === section)?.label ?? section;

  const resetEditor = () => {
    setEditingId(null);
    setPrimary("");
    setSecondary("");
    setTertiary("");
  };

  const submit = async () => {
    const value = valueFor(section, primary, secondary, tertiary);
    if (!value || blocked) return;
    const ok = await onChange({
      baseVersion: canonical.humanProjection.modelVersion,
      baseDocumentDigest: canonical.documentDigest,
      change: editingId
        ? { operation: "replace", section, item_id: editingId, value }
        : { operation: "add", section, value },
    });
    if (ok) resetEditor();
  };

  const copy = async (format: "yaml" | "json") => {
    const content =
      format === "yaml"
        ? canonical.humanProjection.yaml
        : `${JSON.stringify(
            {
              schemaVersion: "openlife.lifemodel.v2.export.v1",
              modelId: canonical.document.modelId,
              modelVersion: canonical.humanProjection.modelVersion,
              documentDigest: canonical.documentDigest,
              document: canonical.document,
            },
            null,
            2
          )}\n`;
    try {
      await navigator.clipboard.writeText(content);
      setLocalMessage(`${format.toUpperCase()} 已复制到剪贴板；没有写入文件。`);
    } catch {
      setLocalMessage("剪贴板写入失败；没有导出任何内容。");
    }
  };

  return (
    <section className="ol-lifemodel-controls" aria-labelledby="lifemodel-controls-title">
      <header className="ol-durable-section-heading">
        <div>
          <span>管理长期模型</span>
          <h2 id="lifemodel-controls-title">编辑、历史与导出</h2>
        </div>
        <FoundationStatusLabel
          label={disabledReason ? "只读" : "受审核操作"}
          status={disabledReason ? "unknown" : "neutral"}
        />
      </header>
      <p>修改、删除、清空和回滚只会创建绑定当前版本的建议；批准并成功应用后才会新增规范版本。</p>
      {disabledReason ? (
        <FoundationNotice title="当前只允许查看" tone="protection">
          <p>{disabledReason}</p>
        </FoundationNotice>
      ) : null}
      {action?.status === "review_required" ? (
        <FoundationNotice title="建议已进入 Review" tone="neutral" live>
          <p>当前模型尚未改变。建议编号：{action.proposalId}</p>
          {onOpenReview ? (
            <button type="button" className="ol-lifemodel-link-button" onClick={onOpenReview}>
              前往审核中心
            </button>
          ) : null}
        </FoundationNotice>
      ) : action?.status === "failed" ? (
        <FoundationNotice title="操作没有创建" tone="error" live>
          <p>{action.error}</p>
        </FoundationNotice>
      ) : null}

      <div className="ol-lifemodel-controls__editor">
        <h3>{editingId ? `修改“${selectedSectionLabel}”内容` : "添加长期信息"}</h3>
        <label>
          类别
          <select
            value={section}
            disabled={Boolean(editingId) || blocked}
            onChange={event => {
              setSection(event.target.value as LifeModelSectionV2);
              resetEditor();
            }}
          >
            {sections.map(candidate => (
              <option key={candidate.id} value={candidate.id}>
                {candidate.label}
              </option>
            ))}
          </select>
        </label>
        <label>
          {section === "important_relationships"
            ? "人物"
            : section === "long_term_goals"
              ? "方向"
              : statementSections.has(section)
                ? "内容"
                : "名称"}
          <input
            value={primary}
            disabled={blocked}
            onChange={event => setPrimary(event.target.value)}
          />
        </label>
        {!statementSections.has(section) ? (
          <label>
            {section === "important_relationships"
              ? "关系"
              : section === "long_term_goals"
                ? "长期意义"
                : "说明"}
            <textarea
              value={secondary}
              disabled={blocked}
              onChange={event => setSecondary(event.target.value)}
            />
          </label>
        ) : null}
        {section === "important_relationships" ? (
          <label>
            重要性
            <textarea
              value={tertiary}
              disabled={blocked}
              onChange={event => setTertiary(event.target.value)}
            />
          </label>
        ) : null}
        <div className="ol-durable-actions">
          <button
            type="button"
            disabled={blocked || !valueFor(section, primary, secondary, tertiary)}
            onClick={() => void submit()}
          >
            <Plus size={16} aria-hidden="true" /> {editingId ? "创建修改建议" : "创建添加建议"}
          </button>
          {editingId ? (
            <button type="button" disabled={busy} onClick={resetEditor}>
              取消修改
            </button>
          ) : null}
        </div>
      </div>

      <div className="ol-lifemodel-controls__items">
        <h3>当前版本内容（{items.length}）</h3>
        {items.length ? (
          <ol>
            {items.map(item => (
              <li key={item.id}>
                <div>
                  <small>{sections.find(candidate => candidate.id === item.section)?.label}</small>
                  <strong>{item.title}</strong>
                  {item.detail ? <p>{item.detail}</p> : null}
                </div>
                <div>
                  <button
                    type="button"
                    disabled={blocked}
                    onClick={() => {
                      const [one, two, three] = fieldsFor(item.value);
                      setSection(item.section);
                      setEditingId(item.id);
                      setPrimary(one);
                      setSecondary(two);
                      setTertiary(three);
                    }}
                  >
                    修改
                  </button>
                  <button
                    type="button"
                    disabled={blocked}
                    onClick={() => {
                      if (confirming !== `remove:${item.id}`) {
                        setConfirming(`remove:${item.id}`);
                        return;
                      }
                      setConfirming(null);
                      void onChange({
                        baseVersion: canonical.humanProjection.modelVersion,
                        baseDocumentDigest: canonical.documentDigest,
                        change: { operation: "remove", section: item.section, item_id: item.id },
                      });
                    }}
                  >
                    <Trash2 size={15} aria-hidden="true" />
                    {confirming === `remove:${item.id}` ? "再次确认删除" : "删除"}
                  </button>
                </div>
              </li>
            ))}
          </ol>
        ) : (
          <p>当前是已版本化的空模型，不会回退到旧 LifeModel。</p>
        )}
        {items.length ? (
          <button
            type="button"
            disabled={blocked}
            onClick={() => {
              if (confirming !== "clear") {
                setConfirming("clear");
                return;
              }
              setConfirming(null);
              void onChange({
                baseVersion: canonical.humanProjection.modelVersion,
                baseDocumentDigest: canonical.documentDigest,
                change: { operation: "clear" },
              });
            }}
          >
            <Trash2 size={15} aria-hidden="true" />
            {confirming === "clear" ? "再次确认清空全部" : "清空全部内容"}
          </button>
        ) : null}
      </div>

      <div className="ol-lifemodel-controls__history">
        <h3>
          <History size={17} aria-hidden="true" /> 版本历史
        </h3>
        <ol>
          {history.map(entry => (
            <li key={entry.modelVersion}>
              <div>
                <strong>版本 {entry.modelVersion}</strong>
                <small>
                  {entry.createdAt} · {entry.itemCount} 项 · +{entry.changeSummary.added} / ~
                  {entry.changeSummary.replaced} / -{entry.changeSummary.removed} · 来源
                  {entry.sourceRefs.length} 条
                </small>
                <p>{entry.summary}</p>
              </div>
              {entry.modelVersion < canonical.humanProjection.modelVersion ? (
                <button
                  type="button"
                  disabled={blocked}
                  onClick={() => {
                    if (confirming !== `rollback:${entry.modelVersion}`) {
                      setConfirming(`rollback:${entry.modelVersion}`);
                      return;
                    }
                    setConfirming(null);
                    void onRollback({
                      baseVersion: canonical.humanProjection.modelVersion,
                      baseDocumentDigest: canonical.documentDigest,
                      targetVersion: entry.modelVersion,
                      targetDocumentDigest: entry.documentDigest,
                    });
                  }}
                >
                  <RotateCcw size={15} aria-hidden="true" />
                  {confirming === `rollback:${entry.modelVersion}` ? "再次确认恢复" : "恢复此内容"}
                </button>
              ) : (
                <span>当前</span>
              )}
            </li>
          ))}
        </ol>
      </div>

      <div className="ol-lifemodel-controls__export">
        <h3>导出当前精确版本</h3>
        <div className="ol-durable-actions">
          <button type="button" onClick={() => void copy("yaml")}>
            <Copy size={15} aria-hidden="true" /> 复制 YAML
          </button>
          <button type="button" onClick={() => void copy("json")}>
            <Copy size={15} aria-hidden="true" /> 复制 JSON
          </button>
        </div>
        <label>
          文件格式
          <select
            value={exportFormat}
            disabled={blocked}
            onChange={event => setExportFormat(event.target.value as "yaml" | "json")}
          >
            <option value="yaml">YAML</option>
            <option value="json">JSON</option>
          </select>
        </label>
        <label>
          本机绝对路径
          <input
            value={exportPath}
            disabled={blocked}
            placeholder={`/Users/you/Documents/lifemodel.${exportFormat}`}
            onChange={event => setExportPath(event.target.value)}
          />
        </label>
        <button
          type="button"
          disabled={blocked || !exportPath.trim()}
          onClick={() =>
            void onExport({
              modelVersion: canonical.humanProjection.modelVersion,
              documentDigest: canonical.documentDigest,
              projectionDigest:
                exportFormat === "yaml" ? canonical.humanProjection.projectionDigest : null,
              format: exportFormat,
              targetPath: exportPath.trim(),
            })
          }
        >
          <Download size={15} aria-hidden="true" /> 创建文件导出建议
        </button>
        {localMessage ? <small role="status">{localMessage}</small> : null}
      </div>
    </section>
  );
}
