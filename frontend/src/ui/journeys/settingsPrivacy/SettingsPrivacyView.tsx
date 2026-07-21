import { Check, KeyRound, RefreshCw, Save, ShieldAlert, Unplug, Wifi } from "lucide-react";
import type { CredentialRecoveryItem, ReviewItem } from "@/tauri";
import {
  FoundationActionButton,
  FoundationDialog,
  FoundationNotice,
  FoundationStatusLabel,
  FoundationTextField,
  FoundationToggle,
} from "@/ui/foundation";
import { boundaryPresentation } from "@/ui/journeys/readOnly";
import {
  credentialState,
  endpointHost,
  settingsBoundaryLabels,
  settingsProviderLabels,
  settingsProviderOptions,
  type SettingsPrivacySurfaceId,
} from "./settingsPrivacyPresentation";
import {
  settingsTestConfirmationTarget,
  type SettingsPrivacyJourneyController,
} from "./useSettingsPrivacyJourney";

export function SettingsPrivacyView({
  controller,
  surface,
  onOpenReview,
  onOpenInspector,
}: {
  controller: SettingsPrivacyJourneyController;
  surface: SettingsPrivacySurfaceId;
  onOpenReview: (item: ReviewItem) => void;
  onOpenInspector: () => void;
}) {
  const draft = controller.draft;
  const boundary = boundaryPresentation(controller.effectiveBoundaryEnvelope);
  const busy = ["testing", "saving", "refreshing_boundary"].includes(controller.state.phase);
  const testTarget = settingsTestConfirmationTarget(controller);

  if (!controller.snapshot && controller.loading) {
    return (
      <div className="ol-settings-page ol-settings-page--centered">
        <FoundationNotice title="正在读取设置" live>
          <p>正在同时读取清理后的配置和模型传输边界；完成前不显示本地确定态。</p>
        </FoundationNotice>
      </div>
    );
  }

  if (!draft) {
    const safeModeActive = controller.snapshot?.safeMode?.active === true;
    return (
      <div className={`ol-settings-page${safeModeActive ? "" : " ol-settings-page--centered"}`}>
        <CredentialRecoverySection controller={controller} />
        <FoundationNotice title="设置暂不可用" tone="error">
          <p>后端没有返回可编辑配置。页面不会用默认 Provider、地址或隐私结论代替。</p>
        </FoundationNotice>
        <FoundationActionButton
          label="重新读取"
          icon={<RefreshCw size={18} strokeWidth={1.75} aria-hidden="true" />}
          loading={controller.loading}
          loadingLabel="正在读取"
          onClick={() => void controller.load(true)}
        />
      </div>
    );
  }

  const credential = credentialState(draft);
  const endpointInvalid = draft.llm.openai_base.trim() && !endpointHost(draft.llm.openai_base);
  const networkEnabled = draft.system?.network_policy?.enabled;
  const networkDefault = draft.system?.network_policy?.default_decision;
  const reviewItem = controller.lastTestOutcome?.reviewItem ?? null;

  return (
    <div
      className="ol-settings-page"
      data-settings-surface={surface}
      data-settings-phase={controller.state.phase}
    >
      <CredentialRecoverySection controller={controller} />

      <section className="ol-settings-boundary" aria-labelledby="ol-settings-boundary-title">
        <div className="ol-settings-section-heading">
          <span>当前后端结论</span>
          <h2 id="ol-settings-boundary-title">模型与传输边界</h2>
        </div>
        <div className="ol-settings-boundary__summary">
          <FoundationStatusLabel
            label={boundary.label}
            status={boundary.status}
            verified={boundary.verified}
            live
          />
          <p>{boundary.detail}</p>
        </div>
        <button type="button" className="ol-settings-evidence-link" onClick={onOpenInspector}>
          查看依据与限制
        </button>
      </section>

      {controller.state.phase === "dirty" && (
        <FoundationNotice title="草稿尚未成为当前边界" live>
          <p>配置偏好已经更改。保存并重新读取边界前，不沿用之前的“本地”或“未外传”结论。</p>
        </FoundationNotice>
      )}
      {controller.state.phase === "refreshing_boundary" && (
        <FoundationNotice title="设置命令已返回，正在核对边界" live>
          <p>只有刷新后的清理配置和 ProviderPrivacyBoundarySummary 一致时，页面才显示保存结果。</p>
        </FoundationNotice>
      )}
      {controller.state.phase === "unknown" && (
        <FoundationNotice title="保存后的边界仍未知" live>
          <p>设置命令可能已经返回，但当前没有足够证据显示本地、外传或风险确定态。</p>
        </FoundationNotice>
      )}
      {controller.state.phase === "failed" && !controller.testPresentation && (
        <FoundationNotice title="设置操作失败" tone="error">
          <p>草稿仍保留；后端产品状态没有因此变为可用。</p>
        </FoundationNotice>
      )}

      {surface === "model-provider" ? (
        <fieldset className="ol-settings-form" disabled={busy}>
          <legend className="ol-sr-only">模型与供应商配置</legend>

          <section className="ol-settings-section" aria-labelledby="ol-settings-local-title">
            <div className="ol-settings-section-heading">
              <span>配置偏好，不是当前路由证明</span>
              <h2 id="ol-settings-local-title">本地模型</h2>
            </div>
            <FoundationToggle
              label="优先使用本地模型"
              description="这只改变配置偏好；不能单独证明当前请求留在本机。"
              state={draft.prefer_local_model ? "on" : "off"}
              onChange={next => controller.edit({ field: "prefer_local", value: next === "on" })}
            />
            <FoundationTextField
              id="ol-settings-local-model"
              label="本地模型名称"
              description="使用后端清理配置中的模型标识，不在页面探测已安装模型。"
              value={draft.local_model}
              onChange={event =>
                controller.edit({ field: "local_model", value: event.target.value })
              }
              error={
                draft.prefer_local_model && !draft.local_model.trim()
                  ? "启用本地优先时必须填写本地模型。"
                  : undefined
              }
            />
          </section>

          <section className="ol-settings-section" aria-labelledby="ol-settings-cloud-title">
            <div className="ol-settings-section-heading">
              <span>云端或 OpenAI 兼容连接</span>
              <h2 id="ol-settings-cloud-title">供应商连接</h2>
            </div>
            <label className="ol-settings-select-field" htmlFor="ol-settings-provider">
              <span className="ol-settings-select-field__label">供应商</span>
              <span className="ol-settings-select-field__description">
                更改供应商会清除当前草稿中的凭据；页面不会把旧凭据带到新目标。
              </span>
              <select
                id="ol-settings-provider"
                value={draft.llm.provider ?? ""}
                onChange={event =>
                  controller.edit({
                    field: "provider",
                    value: event.target.value as NonNullable<typeof draft.llm.provider>,
                  })
                }
              >
                <option value="" disabled>
                  选择供应商
                </option>
                {settingsProviderOptions.map(option => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>
            <FoundationTextField
              id="ol-settings-endpoint"
              label="API 地址"
              description="必须是完整 HTTP 或 HTTPS 地址；连接测试会明确显示目标主机。"
              value={draft.llm.openai_base}
              onChange={event => controller.edit({ field: "endpoint", value: event.target.value })}
              error={endpointInvalid ? "请输入完整的 HTTP 或 HTTPS 地址。" : undefined}
              spellCheck={false}
              autoCapitalize="none"
            />
            <FoundationTextField
              id="ol-settings-chat-model"
              label="模型"
              description="测试只验证这一精确模型，不代表其他模型可用。"
              value={draft.llm.chat_model}
              onChange={event =>
                controller.edit({ field: "chat_model", value: event.target.value })
              }
              error={!draft.llm.chat_model.trim() ? "请填写要使用的模型。" : undefined}
              spellCheck={false}
              autoCapitalize="none"
            />
            <FoundationTextField
              id="ol-settings-api-key"
              label="API 凭据"
              description="真实值不会显示、搜索或进入检查器。"
              type="password"
              value={credential === "stored" ? "" : (draft.llm.openai_key ?? "")}
              placeholder={credential === "stored" ? "已保存凭据" : "输入新的 API 凭据"}
              stateMessage={
                credential === "stored"
                  ? "后端返回遮罩凭据；留空仅在同一供应商和地址下按后端规则保留。"
                  : credential === "entered"
                    ? "新凭据仅存在于当前草稿；测试不会保存，保存需要单独操作。"
                    : "当前目标没有可用于测试的凭据。"
              }
              onChange={event =>
                controller.edit({ field: "credential", value: event.target.value })
              }
              autoComplete="new-password"
              spellCheck={false}
            />
          </section>

          <SettingsTestResult
            controller={controller}
            reviewItem={reviewItem}
            onOpenReview={onOpenReview}
            onOpenInspector={onOpenInspector}
          />

          <SettingsActions controller={controller} showTest />

          <details className="ol-settings-advanced">
            <summary>高级配置摘要</summary>
            <dl>
              <div>
                <dt>运行模式</dt>
                <dd>{draft.runtime_mode ?? "后端未返回"}</dd>
              </div>
              <div>
                <dt>Embedding</dt>
                <dd>{draft.llm.embedding_enabled === false ? "关闭" : "由当前配置决定"}</dd>
              </div>
              <div>
                <dt>配置代</dt>
                <dd>{draft.llm.credential_version ?? "未知"}</dd>
              </div>
            </dl>
          </details>
        </fieldset>
      ) : (
        <fieldset className="ol-settings-form" disabled={busy}>
          <legend className="ol-sr-only">隐私与网络配置</legend>
          <section className="ol-settings-section" aria-labelledby="ol-settings-privacy-title">
            <div className="ol-settings-section-heading">
              <span>读模型事实</span>
              <h2 id="ol-settings-privacy-title">当前传输状态</h2>
            </div>
            <dl className="ol-settings-fact-list">
              <div>
                <dt>路由</dt>
                <dd>
                  {controller.effectiveBoundaryEnvelope.data
                    ? settingsBoundaryLabels.routeType[
                        controller.effectiveBoundaryEnvelope.data.routeType
                      ]
                    : "尚未确认"}
                </dd>
              </div>
              <div>
                <dt>外部传输</dt>
                <dd>
                  {controller.effectiveBoundaryEnvelope.data
                    ? settingsBoundaryLabels.externalTransmission[
                        controller.effectiveBoundaryEnvelope.data.externalTransmission
                      ]
                    : "尚未确认"}
                </dd>
              </div>
              <div>
                <dt>供应商与模型</dt>
                <dd>
                  {controller.effectiveBoundaryEnvelope.data
                    ? `${controller.effectiveBoundaryEnvelope.data.providerLabel} · ${controller.effectiveBoundaryEnvelope.data.modelLabel}`
                    : "未知"}
                </dd>
              </div>
              <div>
                <dt>风险</dt>
                <dd>
                  {controller.effectiveBoundaryEnvelope.data
                    ? settingsBoundaryLabels.risk[controller.effectiveBoundaryEnvelope.data.risk]
                    : "尚未确认"}
                </dd>
              </div>
              <div>
                <dt>本地限定</dt>
                <dd>
                  {controller.effectiveBoundaryEnvelope.data?.localOnlyRequired === true
                    ? "后端要求仅本地"
                    : controller.effectiveBoundaryEnvelope.data
                      ? "未声明仅本地"
                      : "未知"}
                </dd>
              </div>
            </dl>
            {controller.effectiveBoundaryEnvelope.data?.blockedReason && (
              <FoundationNotice title="当前限制">
                <p>{controller.effectiveBoundaryEnvelope.data.blockedReason}</p>
              </FoundationNotice>
            )}
          </section>

          <section className="ol-settings-section" aria-labelledby="ol-settings-network-title">
            <div className="ol-settings-section-heading">
              <span>配置策略，不代表传输结果</span>
              <h2 id="ol-settings-network-title">网络访问</h2>
            </div>
            <FoundationToggle
              label="允许受策略控制的网络访问"
              description="关闭会阻止新的网络请求；它不能证明之前从未外传。"
              state={networkEnabled === undefined ? "unknown" : networkEnabled ? "on" : "off"}
              onChange={next => controller.edit({ field: "network_enabled", value: next === "on" })}
            />
            <label className="ol-settings-select-field" htmlFor="ol-settings-network-default">
              <span className="ol-settings-select-field__label">未匹配目标的默认处理</span>
              <span className="ol-settings-select-field__description">
                “每次询问”保留明确确认；允许或拒绝仍由后端策略执行。
              </span>
              <select
                id="ol-settings-network-default"
                value={networkDefault ?? "unknown"}
                disabled={networkDefault === undefined}
                onChange={event =>
                  controller.edit({
                    field: "network_default",
                    value: event.target.value as "ask" | "allow" | "deny",
                  })
                }
              >
                {networkDefault === undefined && <option value="unknown">后端未返回</option>}
                <option value="ask">每次询问</option>
                <option value="allow">允许</option>
                <option value="deny">拒绝</option>
              </select>
            </label>
          </section>

          <SettingsActions controller={controller} />
        </fieldset>
      )}

      <FoundationDialog
        open={controller.testConfirmationOpen}
        title="确认本次外部连接测试"
        description="这一步只测试连接，不会保存设置。后端网络策略仍可能要求独立审核。"
        busy={controller.state.phase === "testing"}
        onClose={controller.cancelTest}
        footer={
          <>
            <FoundationActionButton label="取消" variant="quiet" onClick={controller.cancelTest} />
            <FoundationActionButton
              label="确认并测试"
              variant="primary"
              icon={<Wifi size={18} strokeWidth={1.75} aria-hidden="true" />}
              onClick={controller.confirmTest}
            />
          </>
        }
      >
        <dl className="ol-settings-confirmation">
          <div>
            <dt>供应商</dt>
            <dd>
              {draft.llm.provider
                ? settingsProviderLabels[draft.llm.provider]
                : testTarget.provider}
            </dd>
          </div>
          <div>
            <dt>目标主机</dt>
            <dd>{testTarget.host}</dd>
          </div>
          <div>
            <dt>模型</dt>
            <dd>{testTarget.model}</dd>
          </div>
          <div>
            <dt>可能结果</dt>
            <dd>可能发送一次最小连接请求；API 凭据不会显示在确认信息中。</dd>
          </div>
        </dl>
      </FoundationDialog>
    </div>
  );
}

const credentialPurposeLabels: Record<CredentialRecoveryItem["purpose"], string> = {
  agent_run_receipts: "助手运行回执",
  main_chat_events: "对话事件",
  action_queue: "动作队列",
  task_store: "任务存储",
};

const credentialStatusLabels: Record<CredentialRecoveryItem["status"], string> = {
  available: "本次可访问",
  created: "本次已安全初始化",
  missing_existing_data: "已有数据但密钥缺失",
  invalid: "密钥格式无效",
  unavailable: "系统凭据库不可用",
};

function CredentialRecoverySection({
  controller,
}: {
  controller: SettingsPrivacyJourneyController;
}) {
  if (!controller.snapshot?.safeMode?.active) return null;

  const recoveryState = controller.credentialRecovery;
  const recoveryAction = controller.actions.recovery;
  const report = recoveryState.report;
  const recoveryBusy = recoveryState.phase === "recovering";
  const readyForRestart = Boolean(report?.allRequiredCredentialsReady && report.restartRequired);

  return (
    <section className="ol-settings-recovery" aria-labelledby="ol-settings-recovery-title">
      <div className="ol-settings-section-heading">
        <span>后端保护状态</span>
        <h2 id="ol-settings-recovery-title">安全模式</h2>
      </div>
      <div className="ol-settings-recovery__summary">
        <FoundationStatusLabel label="长期写入保持关闭" status="waiting" live />
        <p>
          OpenLife
          无法确认内部完整性凭据。你可以发起一次受保护检查；完整重启并由后端重新核对前，当前状态不会变为已恢复。
        </p>
      </div>
      <div className="ol-settings-inline-actions">
        <FoundationActionButton
          label={recoveryAction.label}
          icon={<ShieldAlert size={18} strokeWidth={1.75} aria-hidden="true" />}
          loading={recoveryBusy}
          loadingLabel="等待系统确认"
          disabled={!recoveryAction.enabled}
          disabledReason={recoveryAction.disabledReason}
          data-action-id={recoveryAction.id}
          data-action-kind={recoveryAction.kind}
          data-target-ref={recoveryAction.targetRef}
          onClick={controller.requestCredentialRecovery}
        />
        <span className="ol-settings-recovery__limit">当前状态只由后端重新读取后确认</span>
      </div>

      {report && (
        <FoundationNotice
          title={readyForRestart ? "本次检查均可访问，重启后重新核对" : "仍有系统凭据阻塞"}
          tone={readyForRestart ? "protection" : "error"}
          live
        >
          <p>
            {readyForRestart
              ? "交互式访问不证明下次启动仍可访问。请完全退出并重启 OpenLife；当前页面不会自行改写安全模式。"
              : "没有生成替代密钥，也没有覆盖已有数据。长期写入继续保持关闭。"}
          </p>
          <dl className="ol-settings-recovery-results">
            {report.items.map(item => (
              <div key={item.purpose}>
                <dt>{credentialPurposeLabels[item.purpose]}</dt>
                <dd>{credentialStatusLabels[item.status]}</dd>
              </div>
            ))}
          </dl>
        </FoundationNotice>
      )}

      {recoveryState.phase === "error" && (
        <FoundationNotice title="系统凭据检查未完成" tone="error" live>
          <p>安全模式保持不变。可以重新发起检查；原始失败信息仅在检查器中显示。</p>
        </FoundationNotice>
      )}

      <FoundationDialog
        open={controller.credentialRecoveryConfirmationOpen}
        title="确认系统凭据检查范围"
        description="下一步还会显示 OpenLife 原生确认；取消任一确认都不会执行检查。"
        onClose={controller.cancelCredentialRecovery}
        footer={
          <>
            <FoundationActionButton
              label="取消"
              variant="quiet"
              onClick={controller.cancelCredentialRecovery}
            />
            <FoundationActionButton
              label="继续到系统确认"
              variant="primary"
              icon={<KeyRound size={18} strokeWidth={1.75} aria-hidden="true" />}
              onClick={controller.confirmCredentialRecovery}
            />
          </>
        }
      >
        <ul className="ol-settings-recovery-confirmation">
          <li>检查 Agent 运行回执、主聊天事件、动作队列和任务存储四类完整性凭据。</li>
          <li>仅在对应的长期数据文件不存在时初始化缺失凭据。</li>
          <li>不会读取或显示密钥内容，也不会替换已有数据旁缺失或无效的密钥。</li>
          <li>macOS 若要求授权，普通“允许”可能只对当前进程有效；重启结果仍是唯一恢复证明。</li>
          <li>全部就绪后仍需完全退出并重启，由后端重新确认安全模式。</li>
        </ul>
      </FoundationDialog>
    </section>
  );
}

function SettingsTestResult({
  controller,
  reviewItem,
  onOpenReview,
  onOpenInspector,
}: {
  controller: SettingsPrivacyJourneyController;
  reviewItem: ReviewItem | null;
  onOpenReview: (item: ReviewItem) => void;
  onOpenInspector: () => void;
}) {
  const result = controller.lastTestOutcome?.result;
  const presentation = controller.testPresentation;
  if (!result || !presentation) return null;
  const hasReviewProposal = Boolean(result.review_proposal_id);

  return (
    <section className="ol-settings-test-result" aria-labelledby="ol-settings-test-title">
      <div className="ol-settings-section-heading">
        <span>只适用于本次请求</span>
        <h2 id="ol-settings-test-title">连接测试结果</h2>
      </div>
      <FoundationStatusLabel
        label={presentation.label}
        status={presentation.status}
        verified={presentation.verified}
        live
      />
      <p>{result.message}</p>
      <p className="ol-settings-test-result__limit">{presentation.detail}</p>
      <div className="ol-settings-inline-actions">
        {hasReviewProposal && (
          <FoundationActionButton
            label="查看并决定"
            variant="secondary"
            disabled={!reviewItem}
            disabledReason={
              reviewItem ? undefined : "当前无法从审核中心确认对应的待决定项；不会跳转到猜测目标。"
            }
            onClick={() => reviewItem && onOpenReview(reviewItem)}
          />
        )}
        <FoundationActionButton label="查看测试依据" variant="quiet" onClick={onOpenInspector} />
      </div>
    </section>
  );
}

function SettingsActions({
  controller,
  showTest = false,
}: {
  controller: SettingsPrivacyJourneyController;
  showTest?: boolean;
}) {
  const { test, save } = controller.actions;
  return (
    <section className="ol-settings-actions" aria-label="设置动作">
      <div>
        <span>明确操作</span>
        <p>测试与保存互不替代；保存后必须重新读取模型传输边界。</p>
      </div>
      <div className="ol-settings-actions__buttons">
        {showTest && (
          <FoundationActionButton
            label={test.label}
            icon={<Unplug size={18} strokeWidth={1.75} aria-hidden="true" />}
            loading={controller.state.phase === "testing"}
            loadingLabel="正在测试"
            disabled={!test.enabled}
            disabledReason={test.disabledReason}
            data-action-id={test.id}
            data-action-kind={test.kind}
            data-target-ref={test.targetRef}
            onClick={controller.requestTest}
          />
        )}
        <FoundationActionButton
          label={save.label}
          variant="primary"
          icon={
            controller.state.phase === "ready" ? (
              <Check size={18} strokeWidth={1.75} aria-hidden="true" />
            ) : (
              <Save size={18} strokeWidth={1.75} aria-hidden="true" />
            )
          }
          loading={["saving", "refreshing_boundary"].includes(controller.state.phase)}
          loadingLabel={
            controller.state.phase === "refreshing_boundary" ? "正在核对边界" : "正在保存"
          }
          disabled={!save.enabled}
          disabledReason={save.disabledReason}
          data-action-id={save.id}
          data-action-kind={save.kind}
          data-target-ref={save.targetRef}
          onClick={controller.save}
        />
      </div>
    </section>
  );
}
