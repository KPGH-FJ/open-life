import { ArrowRight, Pause, RefreshCw, Sparkles } from "lucide-react";
import { FoundationActionButton, FoundationNotice, FoundationStatusLabel } from "@/ui/foundation";
import type { LifeModelBuilderController } from "./useLifeModelBuilder";

function actionData(action: ReturnType<LifeModelBuilderController["startAction"]>) {
  return {
    "data-action-category": "product",
    "data-action-id": action.id,
    "data-action-kind": action.kind,
    "data-action-enabled": String(action.enabled),
    "data-action-disabled-reason": action.disabledReason ?? "",
    "data-action-target-ref": action.targetRef ?? "",
  } as const;
}

function signalValue(value: unknown): string {
  if (typeof value === "string") return value;
  return JSON.stringify(value, null, 2);
}

export function LifeModelBuilderPanel({
  controller,
  disabledReason,
  onOpenReview,
}: {
  controller: LifeModelBuilderController;
  disabledReason?: string;
  onOpenReview: () => void;
}) {
  const startAction = controller.startAction(disabledReason);
  const answerAction = controller.answerAction(disabledReason);
  const proposalAction = controller.proposalAction(disabledReason);

  return (
    <section className="ol-lifemodel-builder" aria-labelledby="lifemodel-builder-title">
      <header className="ol-lifemodel-builder__header">
        <div>
          <span>首次建立</span>
          <h2 id="lifemodel-builder-title">从真实情况开始</h2>
        </div>
        <FoundationStatusLabel
          label={
            controller.phase === "reviewing"
              ? "候选待选择"
              : controller.phase === "created"
                ? "建议待审核"
                : "尚未写入"
          }
          status={
            controller.phase === "reviewing" || controller.phase === "created"
              ? "waiting"
              : "neutral"
          }
        />
      </header>

      {controller.error && (
        <FoundationNotice title="建立过程暂时不可用" tone="error" live>
          <p>后端未能启动或继续建立流程；没有创建审核建议，也没有写入 LifeModel。</p>
        </FoundationNotice>
      )}

      {(controller.phase === "idle" ||
        controller.phase === "loading" ||
        controller.phase === "starting" ||
        controller.phase === "error") && (
        <div className="ol-lifemodel-builder__start">
          <p>
            先回答一组简短问题。形成的内容会进入审核中心；只有后续读模型证明已应用，才会成为长期状态。
          </p>
          {controller.unfinished.length > 0 && (
            <div
              className="ol-lifemodel-builder__unfinished"
              role="list"
              aria-label="未完成的建立会话"
            >
              {controller.unfinished.map(session => (
                <div key={session.session_id} role="listitem">
                  <div>
                    <strong>继续上次建立</strong>
                    <small>
                      第 {session.step_index + 1} 步 · {session.pending_signal_count} 个候选
                    </small>
                  </div>
                  <FoundationActionButton
                    label="继续"
                    icon={<ArrowRight size={17} aria-hidden="true" />}
                    disabled={!startAction.enabled}
                    disabledReason={startAction.disabledReason}
                    onClick={() => void controller.resume(session, disabledReason)}
                  />
                </div>
              ))}
            </div>
          )}
          <div className="ol-durable-actions">
            <FoundationActionButton
              label={startAction.label}
              icon={<Sparkles size={17} aria-hidden="true" />}
              variant="primary"
              loading={controller.phase === "starting" || controller.phase === "loading"}
              loadingLabel={controller.phase === "loading" ? "正在读取" : "正在开始"}
              disabled={!startAction.enabled}
              disabledReason={startAction.disabledReason}
              {...actionData(startAction)}
              onClick={() => void controller.start(disabledReason)}
            />
            {controller.phase === "error" && (
              <FoundationActionButton
                label="重新读取"
                icon={<RefreshCw size={17} aria-hidden="true" />}
                variant="quiet"
                onClick={() => void controller.reload()}
              />
            )}
          </div>
        </div>
      )}

      {(controller.phase === "asking" || controller.phase === "answering") && (
        <form
          className="ol-lifemodel-builder__question"
          onSubmit={event => {
            event.preventDefault();
            void controller.answer(disabledReason);
          }}
        >
          {controller.progress && (
            <div className="ol-lifemodel-builder__progress">
              <span>{controller.progress.current_step_label}</span>
              <progress max={100} value={controller.progress.progress}>
                {controller.progress.progress}%
              </progress>
            </div>
          )}
          <h3>{controller.prompt}</h3>
          <label htmlFor="lifemodel-builder-answer">你的回答</label>
          <textarea
            id="lifemodel-builder-answer"
            rows={5}
            value={controller.answerDraft}
            disabled={controller.busy}
            onChange={event => controller.setAnswerDraft(event.target.value)}
          />
          <div className="ol-lifemodel-builder__footer">
            <FoundationActionButton
              label="稍后继续"
              icon={<Pause size={17} aria-hidden="true" />}
              variant="quiet"
              disabled={controller.busy}
              disabledReason={controller.busy ? "当前回答正在提交。" : undefined}
              onClick={controller.pause}
            />
            <FoundationActionButton
              label={answerAction.label}
              icon={<ArrowRight size={17} aria-hidden="true" />}
              variant="primary"
              loading={controller.phase === "answering"}
              loadingLabel="正在继续"
              disabled={!answerAction.enabled}
              disabledReason={answerAction.disabledReason}
              {...actionData(answerAction)}
              type="submit"
            />
          </div>
        </form>
      )}

      {(controller.phase === "reviewing" || controller.phase === "submitting") && (
        <div className="ol-lifemodel-builder__review">
          <div className="ol-lifemodel-builder__review-heading">
            <div>
              <span>候选理解</span>
              <h3>逐项决定哪些内容进入审核</h3>
            </div>
            <small>默认全部未选择</small>
          </div>
          {controller.summary && (
            <p className="ol-lifemodel-builder__summary">
              {controller.summary.identity_summary || controller.summary.goals_summary}
            </p>
          )}
          <div className="ol-lifemodel-builder__candidates">
            {controller.candidates.map(candidate => (
              <fieldset key={candidate.signal.id}>
                <legend>{candidate.signal.affected_path}</legend>
                <p>{signalValue(candidate.signal.proposed_value)}</p>
                <small>{candidate.signal.reason}</small>
                <div
                  className="ol-lifemodel-builder__decision"
                  role="radiogroup"
                  aria-label={`决定 ${candidate.signal.affected_path}`}
                >
                  {[
                    ["accepted", "纳入审核"],
                    ["rejected", "忽略"],
                    ["edited", "修改后纳入"],
                  ].map(([value, label]) => (
                    <label key={value}>
                      <input
                        type="radio"
                        name={`builder-signal:${candidate.signal.id}`}
                        value={value}
                        checked={candidate.decision === value}
                        disabled={controller.busy}
                        onChange={() =>
                          controller.setCandidateDecision(
                            candidate.signal.id,
                            value as "accepted" | "rejected" | "edited"
                          )
                        }
                      />
                      <span>{label}</span>
                    </label>
                  ))}
                </div>
                {candidate.decision === "edited" && (
                  <label className="ol-lifemodel-builder__edit">
                    <span>调整后的内容</span>
                    <textarea
                      rows={3}
                      value={candidate.editedValue}
                      disabled={controller.busy}
                      onChange={event =>
                        controller.setCandidateEditedValue(candidate.signal.id, event.target.value)
                      }
                    />
                  </label>
                )}
              </fieldset>
            ))}
          </div>
          <div className="ol-lifemodel-builder__footer">
            <FoundationActionButton
              label="稍后处理"
              icon={<Pause size={17} aria-hidden="true" />}
              variant="quiet"
              disabled={controller.busy}
              disabledReason={controller.busy ? "审核建议正在创建。" : undefined}
              onClick={controller.pause}
            />
            <FoundationActionButton
              label={proposalAction.label}
              icon={<ArrowRight size={17} aria-hidden="true" />}
              variant="primary"
              loading={controller.phase === "submitting"}
              loadingLabel="正在创建"
              disabled={!proposalAction.enabled}
              disabledReason={proposalAction.disabledReason}
              {...actionData(proposalAction)}
              onClick={() => void controller.createProposals(disabledReason)}
            />
          </div>
        </div>
      )}

      {controller.phase === "created" && controller.receipt && (
        <div className="ol-lifemodel-builder__created">
          <FoundationNotice title="审核建议已创建" tone="protection" live>
            <p>
              已创建 {controller.receipt.created_count} 项、复用 {controller.receipt.reused_count}
              项；它们尚未批准，也尚未应用到 LifeModel。
            </p>
          </FoundationNotice>
          <FoundationActionButton
            label="前往审核中心"
            icon={<ArrowRight size={17} aria-hidden="true" />}
            variant="primary"
            onClick={onOpenReview}
          />
        </div>
      )}
    </section>
  );
}
