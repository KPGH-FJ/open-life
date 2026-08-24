import { useEffect, useMemo, useState } from "react";
import { ArrowRight, ShieldCheck } from "lucide-react";
import type {
  DraftLegacyLifeModelMigrationRequest,
  LegacyLifeModelInventoryV2,
  LegacyLifeModelMigrationCandidateV2,
  LifeModelUserValueV2,
} from "@/tauri";
import { FoundationActionButton, FoundationNotice } from "@/ui/foundation";

type MigrationAction = {
  kind: "migration" | "change" | "rollback" | "export";
  status: "submitting" | "review_required" | "failed";
  proposalId?: string;
  error?: string;
} | null;

function candidateSummary(candidate: LegacyLifeModelMigrationCandidateV2): string {
  const value = candidate.proposedValue;
  switch (value.kind) {
    case "statement":
      return value.value.statement;
    case "long_term_goal":
      return [value.value.direction, value.value.meaning].filter(Boolean).join(" — ");
    case "relationship":
      return [value.value.person_label, value.value.relationship, value.value.significance]
        .filter(Boolean)
        .join(" — ");
    case "capability":
    case "resource":
      return [value.value.name, value.value.description].filter(Boolean).join(" — ");
  }
}

function CandidateEditor({
  value,
  onChange,
}: {
  value: LifeModelUserValueV2;
  onChange: (value: LifeModelUserValueV2) => void;
}) {
  if (value.kind === "statement") {
    return (
      <label>
        迁移内容
        <textarea
          value={value.value.statement}
          onChange={event =>
            onChange({ kind: "statement", value: { statement: event.currentTarget.value } })
          }
        />
      </label>
    );
  }
  if (value.kind === "long_term_goal") {
    return (
      <>
        <label>
          长期方向
          <input
            value={value.value.direction}
            onChange={event =>
              onChange({
                kind: "long_term_goal",
                value: { ...value.value, direction: event.currentTarget.value },
              })
            }
          />
        </label>
        <label>
          意义
          <textarea
            value={value.value.meaning}
            onChange={event =>
              onChange({
                kind: "long_term_goal",
                value: { ...value.value, meaning: event.currentTarget.value },
              })
            }
          />
        </label>
      </>
    );
  }
  if (value.kind === "relationship") {
    return (
      <>
        {[
          ["人物称呼", "person_label"],
          ["关系", "relationship"],
          ["重要性说明", "significance"],
        ].map(([label, field]) => (
          <label key={field}>
            {label}
            <input
              value={value.value[field as keyof typeof value.value]}
              onChange={event =>
                onChange({
                  kind: "relationship",
                  value: { ...value.value, [field]: event.currentTarget.value },
                })
              }
            />
          </label>
        ))}
      </>
    );
  }
  const kind = value.kind;
  return (
    <>
      <label>
        名称
        <input
          value={value.value.name}
          onChange={event =>
            onChange({ kind, value: { ...value.value, name: event.currentTarget.value } })
          }
        />
      </label>
      <label>
        说明
        <textarea
          value={value.value.description}
          onChange={event =>
            onChange({ kind, value: { ...value.value, description: event.currentTarget.value } })
          }
        />
      </label>
    </>
  );
}

export function LegacyLifeModelMigrationPanel({
  inventory,
  action,
  onDraft,
  onOpenReview,
}: {
  inventory: LegacyLifeModelInventoryV2;
  action: MigrationAction;
  onDraft: (request: DraftLegacyLifeModelMigrationRequest) => Promise<boolean>;
  onOpenReview?: () => void;
}) {
  const preview = inventory.preview;
  const [decisions, setDecisions] = useState<Record<string, "include" | "exclude">>({});
  const [editedValues, setEditedValues] = useState<Record<string, LifeModelUserValueV2>>({});
  const [acknowledged, setAcknowledged] = useState(false);
  useEffect(() => {
    setDecisions({});
    setEditedValues(
      Object.fromEntries(
        (preview?.candidates ?? []).map(candidate => [
          candidate.candidateId,
          candidate.proposedValue,
        ])
      )
    );
    setAcknowledged(false);
  }, [preview?.sourceDigest]);

  const nonLifeModelCount = preview ? preview.items.length - preview.reviewRequiredCount : 0;
  const undecidedCount = useMemo(
    () => (preview?.candidates ?? []).filter(candidate => !decisions[candidate.candidateId]).length,
    [decisions, preview?.candidates]
  );
  const busy = action?.kind === "migration" && action.status === "submitting";
  const proposalCreated = action?.kind === "migration" && action.status === "review_required";
  const canSubmit = Boolean(
    preview &&
    undecidedCount === 0 &&
    (nonLifeModelCount === 0 || acknowledged) &&
    !busy &&
    !proposalCreated
  );

  if (!preview) {
    return (
      <FoundationNotice title="旧版迁移暂时不可开始" tone="error" live>
        <p>检测到历史文件，但当前 YAML 不存在。系统不会从历史版本中猜测哪一个应成为当前版本。</p>
      </FoundationNotice>
    );
  }

  return (
    <section className="ol-intelligence-current" aria-labelledby="legacy-migration-title">
      <header className="ol-intelligence-section-heading">
        <div>
          <span>迁移前审核</span>
          <h2 id="legacy-migration-title">逐项决定旧版长期信息</h2>
        </div>
      </header>
      <p>
        每一项都必须明确选择迁移或保留在旧档案中。提交只会创建 Review 项，不会立即写入
        v2、删除旧文件或改变 Agent 使用的长期信息。
      </p>

      {preview.candidates.length === 0 ? (
        <FoundationNotice title="没有可直接迁移的长期项" tone="neutral">
          <p>仍可通过 Review 建立一个明确的空 v2 所有者；旧文件和历史继续作为迁移证据保留。</p>
        </FoundationNotice>
      ) : (
        <ol className="ol-lifemodel-migration-details" aria-label="旧版 LifeModel 迁移候选">
          {preview.candidates.map(candidate => {
            const decision = decisions[candidate.candidateId];
            const editedValue = editedValues[candidate.candidateId] ?? candidate.proposedValue;
            return (
              <li key={candidate.candidateId}>
                <div>
                  <strong>{candidateSummary(candidate) || "空内容"}</strong>
                  <span>
                    {candidate.targetSection}
                    {candidate.sensitive ? " · 敏感信息" : ""}
                  </span>
                </div>
                <small>来源字段：{candidate.sourcePaths.join("、")}</small>
                <fieldset disabled={busy || proposalCreated}>
                  <legend>这项如何处理？</legend>
                  <label>
                    <input
                      type="radio"
                      name={`migration-${candidate.candidateId}`}
                      checked={decision === "include"}
                      onChange={() =>
                        setDecisions(current => ({
                          ...current,
                          [candidate.candidateId]: "include",
                        }))
                      }
                    />
                    审核后迁移到 v2
                  </label>
                  <label>
                    <input
                      type="radio"
                      name={`migration-${candidate.candidateId}`}
                      checked={decision === "exclude"}
                      onChange={() =>
                        setDecisions(current => ({
                          ...current,
                          [candidate.candidateId]: "exclude",
                        }))
                      }
                    />
                    不迁移，保留在旧档案
                  </label>
                </fieldset>
                {decision === "include" ? (
                  <div className="ol-lifemodel-migration-editor">
                    <CandidateEditor
                      value={editedValue}
                      onChange={value =>
                        setEditedValues(current => ({
                          ...current,
                          [candidate.candidateId]: value,
                        }))
                      }
                    />
                  </div>
                ) : null}
              </li>
            );
          })}
        </ol>
      )}

      {nonLifeModelCount > 0 ? (
        <label>
          <input
            type="checkbox"
            checked={acknowledged}
            disabled={busy || proposalCreated}
            onChange={event => setAcknowledged(event.currentTarget.checked)}
          />
          我理解还有 {nonLifeModelCount}{" "}
          个字段属于任务、状态、Memory、运行时、元数据或需要人工分类；它们不会被静默塞入 LifeModel
          v2。
        </label>
      ) : null}

      {proposalCreated ? (
        <FoundationNotice title="迁移建议已进入 Review" tone="protection" live>
          <p>当前旧文件与规范 LifeModel 都没有因创建建议而改变。</p>
          {onOpenReview ? (
            <FoundationActionButton
              label="前往需处理"
              icon={<ArrowRight size={16} aria-hidden="true" />}
              onClick={onOpenReview}
            />
          ) : null}
        </FoundationNotice>
      ) : (
        <FoundationActionButton
          label="创建迁移审核建议"
          icon={<ShieldCheck size={16} aria-hidden="true" />}
          loading={busy}
          loadingLabel="正在创建"
          disabled={!canSubmit}
          disabledReason={
            busy
              ? "迁移审核建议正在创建。"
              : undecidedCount > 0
                ? `还有 ${undecidedCount} 项没有决定。`
                : nonLifeModelCount > 0 && !acknowledged
                  ? "请先确认未迁移字段的归属边界。"
                  : undefined
          }
          onClick={() => {
            if (!canSubmit) return;
            void onDraft({
              sourceDigest: preview.sourceDigest,
              selections: preview.candidates.map(candidate => ({
                candidateId: candidate.candidateId,
                decision: decisions[candidate.candidateId],
                editedValue:
                  decisions[candidate.candidateId] === "include"
                    ? (editedValues[candidate.candidateId] ?? candidate.proposedValue)
                    : null,
              })),
              nonLifemodelItemsAcknowledged: nonLifeModelCount === 0 || acknowledged,
            });
          }}
        />
      )}
      {action?.kind === "migration" && action.status === "failed" ? (
        <FoundationNotice title="迁移建议没有创建" tone="error" live>
          <p>{action.error}</p>
        </FoundationNotice>
      ) : null}
    </section>
  );
}
