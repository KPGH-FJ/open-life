import { ArrowRight, Plus, RotateCcw, UserRound } from "lucide-react";
import { useState } from "react";
import type {
  DraftLifeModelV2ChangeRequest,
  LifeModelSectionV2,
  LifeModelUserValueV2,
} from "@/tauri";
import { FoundationActionButton, FoundationNotice, FoundationStatusLabel } from "@/ui/foundation";

type SectionDefinition = {
  id: LifeModelSectionV2;
  label: string;
  prompt: string;
  primaryLabel: string;
  secondaryLabel?: string;
  tertiaryLabel?: string;
};

const sections: SectionDefinition[] = [
  {
    id: "identity",
    label: "身份与自我定义",
    prompt: "你希望 OpenLife 长期如何理解你是谁？",
    primaryLabel: "长期稳定的自我描述",
  },
  {
    id: "values",
    label: "价值观",
    prompt: "什么价值在长期决策中对你最重要？",
    primaryLabel: "一条价值观",
  },
  {
    id: "long_term_goals",
    label: "长期目标",
    prompt: "你长期想走向哪里，这件事对你意味着什么？",
    primaryLabel: "长期方向",
    secondaryLabel: "长期意义",
  },
  {
    id: "stable_preferences",
    label: "稳定偏好",
    prompt: "哪项跨场景、长期稳定的偏好值得 OpenLife 记住？",
    primaryLabel: "稳定偏好",
  },
  {
    id: "personal_boundaries",
    label: "个人边界",
    prompt: "OpenLife 在协助你时必须长期尊重什么边界？",
    primaryLabel: "个人边界",
  },
  {
    id: "important_relationships",
    label: "重要关系",
    prompt: "哪段关系会长期影响你的判断？敏感关系不会默认纳入。",
    primaryLabel: "人物称呼",
    secondaryLabel: "与你的关系",
    tertiaryLabel: "长期重要性",
  },
  {
    id: "capabilities",
    label: "长期能力",
    prompt: "你已经形成了哪项相对稳定的个人能力？",
    primaryLabel: "能力名称",
    secondaryLabel: "能力说明",
  },
  {
    id: "resources",
    label: "长期资源",
    prompt: "你长期可以依靠的个人资源是什么？",
    primaryLabel: "资源名称",
    secondaryLabel: "资源说明",
  },
  {
    id: "decision_principles",
    label: "决策原则",
    prompt: "你通常希望依据什么长期原则作决定？",
    primaryLabel: "决策原则",
  },
  {
    id: "collaboration_preferences",
    label: "长期协作方式",
    prompt: "你希望 OpenLife 长期采用怎样的协作方式？",
    primaryLabel: "协作偏好",
  },
];

const statementSections = new Set<LifeModelSectionV2>([
  "identity",
  "values",
  "stable_preferences",
  "personal_boundaries",
  "decision_principles",
  "collaboration_preferences",
]);

function reviewedValue(
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
  return one && two
    ? section === "capabilities"
      ? { kind: "capability", value: { name: one, description: two } }
      : { kind: "resource", value: { name: one, description: two } }
    : null;
}

export function LifeModelBuilderPanel({
  disabledReason,
  action,
  onChange,
  onOpenReview,
}: {
  disabledReason?: string;
  action: {
    kind: "change" | "rollback" | "export";
    status: "submitting" | "review_required" | "failed";
    proposalId?: string;
    error?: string;
  } | null;
  onChange: (request: DraftLifeModelV2ChangeRequest) => Promise<boolean>;
  onOpenReview?: () => void;
}) {
  const [section, setSection] = useState<LifeModelSectionV2>("identity");
  const [primary, setPrimary] = useState("");
  const [secondary, setSecondary] = useState("");
  const [tertiary, setTertiary] = useState("");
  const [include, setInclude] = useState(false);
  const definition = sections.find(candidate => candidate.id === section) ?? sections[0];
  const value = reviewedValue(section, primary, secondary, tertiary);
  const busy = action?.status === "submitting";
  const blocked = Boolean(disabledReason) || busy;
  const submitDisabledReason = disabledReason
    ? disabledReason
    : busy
      ? "LifeModel 建议正在创建；请等待审核入口准备完成。"
      : !value
        ? "请完整填写当前类别需要的信息。"
        : !include
          ? "请明确选择将这条内容纳入审核。"
          : undefined;

  const resetAnswer = () => {
    setPrimary("");
    setSecondary("");
    setTertiary("");
    setInclude(false);
  };

  const submit = async () => {
    if (!value || !include || blocked) return;
    const created = await onChange({
      baseVersion: null,
      baseDocumentDigest: null,
      change: { operation: "add", section, value },
    });
    if (created) setInclude(false);
  };

  return (
    <section className="ol-lifemodel-builder" aria-labelledby="lifemodel-builder-title">
      <header className="ol-lifemodel-builder__header">
        <div>
          <span>首次建立</span>
          <h2 id="lifemodel-builder-title">从一条长期信息开始</h2>
        </div>
        <FoundationStatusLabel
          label={action?.status === "review_required" ? "建议待审核" : "尚未写入"}
          status={action?.status === "review_required" ? "waiting" : "neutral"}
        />
      </header>

      <p>这里只建立关于你的长期模型。近期任务、当前状态、工作过程和工具能力不会进入 LifeModel。</p>
      {disabledReason ? (
        <FoundationNotice title="当前不能建立" tone="protection">
          <p>{disabledReason}</p>
        </FoundationNotice>
      ) : null}
      {action?.status === "review_required" ? (
        <FoundationNotice title="审核建议已创建" tone="protection" live>
          <p>当前仍是空模型；只有 Review 批准并成功物化后，第一条长期信息才会成为规范版本。</p>
          {onOpenReview ? (
            <FoundationActionButton
              label="前往需处理"
              icon={<ArrowRight size={17} aria-hidden="true" />}
              variant="primary"
              onClick={onOpenReview}
            />
          ) : null}
        </FoundationNotice>
      ) : action?.status === "failed" ? (
        <FoundationNotice title="审核建议没有创建" tone="error" live>
          <p>{action.error}</p>
        </FoundationNotice>
      ) : null}

      <div className="ol-lifemodel-controls__editor">
        <label>
          长期信息类别
          <select
            value={section}
            disabled={blocked}
            onChange={event => {
              setSection(event.target.value as LifeModelSectionV2);
              resetAnswer();
            }}
          >
            {sections.map(candidate => (
              <option key={candidate.id} value={candidate.id}>
                {candidate.label}
              </option>
            ))}
          </select>
        </label>
        <h3>{definition.prompt}</h3>
        <label>
          {definition.primaryLabel}
          <textarea
            rows={3}
            value={primary}
            disabled={blocked}
            onChange={event => setPrimary(event.target.value)}
          />
        </label>
        {definition.secondaryLabel ? (
          <label>
            {definition.secondaryLabel}
            <textarea
              rows={3}
              value={secondary}
              disabled={blocked}
              onChange={event => setSecondary(event.target.value)}
            />
          </label>
        ) : null}
        {definition.tertiaryLabel ? (
          <label>
            {definition.tertiaryLabel}
            <textarea
              rows={3}
              value={tertiary}
              disabled={blocked}
              onChange={event => setTertiary(event.target.value)}
            />
          </label>
        ) : null}

        <label className="ol-lifemodel-builder__include">
          <input
            type="checkbox"
            checked={include}
            disabled={blocked || !value}
            onChange={event => setInclude(event.target.checked)}
          />
          <span>将这条内容纳入本次审核</span>
        </label>
        <small>默认不选择。系统不会从你的回答推断其他类别，也不会自动补写内容。</small>

        <div className="ol-intelligence-actions">
          <FoundationActionButton
            label="创建首条 LifeModel 建议"
            icon={<Plus size={17} aria-hidden="true" />}
            variant="primary"
            loading={busy}
            loadingLabel="正在创建"
            disabled={blocked || !value || !include}
            disabledReason={submitDisabledReason}
            onClick={() => void submit()}
          />
          <FoundationActionButton
            label="重新填写"
            icon={<RotateCcw size={17} aria-hidden="true" />}
            variant="quiet"
            disabled={busy}
            disabledReason={busy ? "LifeModel 建议正在创建；暂时不能重置回答。" : undefined}
            onClick={resetAnswer}
          />
        </div>
      </div>

      <FoundationNotice title="建立边界" tone="neutral">
        <p>
          <UserRound size={16} aria-hidden="true" /> LifeModel
          只描述长期稳定的用户；任务与状态由各自系统管理，Agent Memory 管理工作连续性。
        </p>
      </FoundationNotice>
    </section>
  );
}
