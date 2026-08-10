import { useEffect, useMemo, useState } from "react";
import type {
  DraftLegacyLifeModelMigrationRequest,
  LegacyLifeModelMigrationCandidateV2,
  LegacyLifeModelMigrationCandidateValueV2,
  LegacyLifeModelMigrationPreviewV2,
} from "@/tauri";
import { FoundationActionButton, FoundationNotice } from "@/ui/foundation";

type Decision = "include" | "exclude";

const sectionLabels: Record<LegacyLifeModelMigrationCandidateV2["targetSection"], string> = {
  identity: "身份",
  values: "价值观",
  long_term_goals: "长期目标",
  stable_preferences: "稳定偏好",
  personal_boundaries: "个人边界",
  important_relationships: "重要关系",
  capabilities: "能力",
  resources: "资源",
  decision_principles: "决策原则",
  collaboration_preferences: "协作方式",
};

function cloneValue(
  value: LegacyLifeModelMigrationCandidateValueV2
): LegacyLifeModelMigrationCandidateValueV2 {
  return structuredClone(value);
}

function CandidateEditor({
  value,
  onChange,
}: {
  value: LegacyLifeModelMigrationCandidateValueV2;
  onChange: (value: LegacyLifeModelMigrationCandidateValueV2) => void;
}) {
  const field = (label: string, key: string, current: string) => (
    <label>
      <span>{label}</span>
      <textarea
        value={current}
        onChange={event =>
          onChange({
            ...value,
            value: { ...value.value, [key]: event.target.value },
          } as LegacyLifeModelMigrationCandidateValueV2)
        }
      />
    </label>
  );
  switch (value.kind) {
    case "statement":
      return field("内容", "statement", value.value.statement);
    case "long_term_goal":
      return (
        <>
          {field("方向", "direction", value.value.direction)}
          {field("意义", "meaning", value.value.meaning)}
        </>
      );
    case "relationship":
      return (
        <>
          {field("人物", "person_label", value.value.person_label)}
          {field("关系", "relationship", value.value.relationship)}
          {field("长期意义", "significance", value.value.significance)}
        </>
      );
    case "capability":
    case "resource":
      return (
        <>
          {field("名称", "name", value.value.name)}
          {field("说明", "description", value.value.description)}
        </>
      );
  }
}

export function LifeModelMigrationPanel({
  preview,
  submitting,
  proposalId,
  error,
  onSubmit,
  onOpenReview,
}: {
  preview: LegacyLifeModelMigrationPreviewV2;
  submitting: boolean;
  proposalId?: string;
  error?: string;
  onSubmit: (request: DraftLegacyLifeModelMigrationRequest) => Promise<boolean>;
  onOpenReview?: () => void;
}) {
  const [decisions, setDecisions] = useState<Record<string, Decision>>({});
  const [values, setValues] = useState<Record<string, LegacyLifeModelMigrationCandidateValueV2>>(
    {}
  );
  const [acknowledged, setAcknowledged] = useState(false);
  useEffect(() => {
    setDecisions({});
    setValues(
      Object.fromEntries(
        preview.candidates.map(candidate => [
          candidate.candidateId,
          cloneValue(candidate.proposedValue),
        ])
      )
    );
    setAcknowledged(false);
  }, [preview.sourceDigest, preview.candidates]);

  const otherCount = preview.items.length - preview.reviewRequiredCount;
  const complete = preview.candidates.every(candidate => decisions[candidate.candidateId]);
  const canSubmit = complete && (otherCount === 0 || acknowledged) && !submitting && !proposalId;
  const selectionSummary = useMemo(
    () => ({
      included: Object.values(decisions).filter(decision => decision === "include").length,
      excluded: Object.values(decisions).filter(decision => decision === "exclude").length,
    }),
    [decisions]
  );

  const submit = async () => {
    if (!canSubmit) return;
    await onSubmit({
      sourceDigest: preview.sourceDigest,
      selections: preview.candidates.map(candidate => ({
        candidateId: candidate.candidateId,
        decision: decisions[candidate.candidateId],
        editedValue:
          decisions[candidate.candidateId] === "include" ? values[candidate.candidateId] : null,
      })),
      nonLifemodelItemsAcknowledged: acknowledged,
    });
  };

  return (
    <div className="ol-lifemodel-migration-review">
      <p>逐项决定哪些长期个人信息进入新模型。默认不选择；敏感内容也不会被预先勾选。</p>
      <ol>
        {preview.candidates.map(candidate => {
          const decision = decisions[candidate.candidateId];
          const value = values[candidate.candidateId] ?? candidate.proposedValue;
          return (
            <li key={candidate.candidateId}>
              <header>
                <strong>{sectionLabels[candidate.targetSection]}</strong>
                {candidate.sensitive ? <span>敏感</span> : null}
              </header>
              <small>来源：{candidate.sourcePaths.join("、")}</small>
              <div role="group" aria-label={`${sectionLabels[candidate.targetSection]}迁移决定`}>
                <label>
                  <input
                    type="radio"
                    name={candidate.candidateId}
                    checked={decision === "include"}
                    onChange={() =>
                      setDecisions(current => ({ ...current, [candidate.candidateId]: "include" }))
                    }
                  />
                  纳入
                </label>
                <label>
                  <input
                    type="radio"
                    name={candidate.candidateId}
                    checked={decision === "exclude"}
                    onChange={() =>
                      setDecisions(current => ({ ...current, [candidate.candidateId]: "exclude" }))
                    }
                  />
                  不纳入
                </label>
              </div>
              {decision === "include" ? (
                <CandidateEditor
                  value={value}
                  onChange={next =>
                    setValues(current => ({ ...current, [candidate.candidateId]: next }))
                  }
                />
              ) : null}
            </li>
          );
        })}
      </ol>
      {otherCount > 0 ? (
        <label>
          <input
            type="checkbox"
            checked={acknowledged}
            onChange={event => setAcknowledged(event.target.checked)}
          />
          我已查看另外 {otherCount} 个不属于 LifeModel v2 的旧字段；它们不会在本次迁移中写入。
        </label>
      ) : null}
      <p>
        已纳入 {selectionSummary.included} 项 · 不纳入 {selectionSummary.excluded} 项 · 待决定{" "}
        {preview.candidates.length - selectionSummary.included - selectionSummary.excluded} 项
      </p>
      {error ? (
        <FoundationNotice title="迁移建议未创建" tone="error">
          <p>{error}</p>
        </FoundationNotice>
      ) : null}
      {proposalId ? (
        <FoundationNotice title="等待 Review" tone="neutral">
          <p>建议 {proposalId} 已创建。接受前不会备份、写入 v2 或切换权威源。</p>
          {onOpenReview ? (
            <FoundationActionButton label="前往 Review" onClick={onOpenReview} />
          ) : null}
        </FoundationNotice>
      ) : (
        <FoundationActionButton
          label="提交到 Review"
          loading={submitting}
          loadingLabel="正在创建建议"
          disabled={!canSubmit && !submitting}
          disabledReason={
            !canSubmit && !submitting
              ? proposalId
                ? "迁移建议已经创建。"
                : !complete
                  ? "请先逐项决定纳入或不纳入。"
                  : "请确认已查看不属于 LifeModel v2 的旧字段。"
              : undefined
          }
          onClick={() => void submit()}
        />
      )}
    </div>
  );
}
