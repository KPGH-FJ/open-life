import { Check, FolderOpen, RefreshCw, Save, Unplug, Wifi } from "lucide-react";
import type { ReviewItem } from "@/tauri";
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
  searchCredentialState,
  settingsBoundaryLabels,
  settingsProviderLabels,
  settingsProviderOptions,
  settingsSearchProviderOptions,
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
  const busy =
    controller.loading ||
    ["testing", "saving", "refreshing_boundary"].includes(controller.state.phase) ||
    controller.artifactDirectorySelection.phase === "selecting";
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
        <SafeModeNotice controller={controller} />
        <CredentialInitializationPanel controller={controller} />
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
  const searchProvider = draft.system?.search_provider ?? "duckduckgo";
  const searchCredential = searchCredentialState(draft);
  const artifactOutputDirectory = draft.system?.safe_paths?.[0];

  return (
    <div
      className="ol-settings-page"
      data-settings-surface={surface}
      data-settings-phase={controller.state.phase}
    >
      <SafeModeNotice controller={controller} />
      <CredentialInitializationPanel controller={controller} />

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

          <section className="ol-settings-section" aria-labelledby="ol-settings-search-title">
            <div className="ol-settings-section-heading">
              <span>独立工具连接，不继承模型凭据</span>
              <h2 id="ol-settings-search-title">网页搜索</h2>
            </div>
            <label className="ol-settings-select-field" htmlFor="ol-settings-search-provider">
              <span className="ol-settings-select-field__label">Search Provider</span>
              <span className="ol-settings-select-field__description">
                这是 web.search
                工具的后端，不是生成模型自身的联网能力。保存配置不等于搜索已经验证成功。
              </span>
              <select
                id="ol-settings-search-provider"
                aria-label="Search Provider"
                value={searchProvider}
                onChange={event =>
                  controller.edit({
                    field: "search_provider",
                    value: event.target.value as NonNullable<
                      NonNullable<typeof draft.system>["search_provider"]
                    >,
                  })
                }
              >
                {settingsSearchProviderOptions.map(option => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>
            {searchProvider === "searxng" && (
              <FoundationTextField
                id="ol-settings-searxng-url"
                label="SearXNG 地址"
                description="填写你信任的 SearXNG 实例根地址；实际请求仍受网络权限控制。"
                value={draft.system?.searxng_url ?? ""}
                onChange={event =>
                  controller.edit({ field: "searxng_url", value: event.target.value })
                }
                error={
                  endpointHost(draft.system?.searxng_url ?? "")
                    ? undefined
                    : "请输入完整的 HTTP 或 HTTPS 地址。"
                }
                spellCheck={false}
                autoCapitalize="none"
              />
            )}
            {(searchProvider === "deepseek" || searchProvider === "brave") && (
              <FoundationTextField
                id="ol-settings-search-api-key"
                label="搜索凭据"
                description="与生成模型凭据分开保存；真实值不会返回网页层。"
                type="password"
                value={
                  searchCredential === "stored" ? "" : (draft.system?.search_provider_key ?? "")
                }
                placeholder={searchCredential === "stored" ? "已保存搜索凭据" : "输入搜索凭据"}
                stateMessage={
                  searchCredential === "stored"
                    ? "后端只返回凭据存在状态；同一搜索目标下留空会按后端规则保留。"
                    : searchCredential === "entered"
                      ? "新搜索凭据只存在于当前草稿，保存后才会进入系统凭据存储。"
                      : "当前搜索目标缺少必需凭据，设置不能保存。"
                }
                onChange={event =>
                  controller.edit({ field: "search_credential", value: event.target.value })
                }
                autoComplete="new-password"
                spellCheck={false}
              />
            )}
            {searchProvider === "duckduckgo" && (
              <FoundationNotice title="无需凭据，但不保证可用">
                <p>DuckDuckGo HTML 端点可能返回挑战页；只有真实任务成功后才算本次搜索证据。</p>
              </FoundationNotice>
            )}
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

          <section className="ol-settings-section" aria-labelledby="ol-settings-artifact-title">
            <div className="ol-settings-section-heading">
              <span>原生选择，精确目录</span>
              <h2 id="ol-settings-artifact-title">Artifact 输出目录</h2>
            </div>
            <p>
              {artifactOutputDirectory
                ? `当前目录：${artifactOutputDirectory}`
                : "尚未配置。生成 artifact 时会被后端明确阻止，不会回退到进程当前目录。"}
            </p>
            {controller.artifactDirectorySelection.phase === "failed" && (
              <FoundationNotice title="目录没有保存" tone="error" live>
                <p>原生选择或后端持久化失败；现有路径保持不变。</p>
              </FoundationNotice>
            )}
            <FoundationActionButton
              label="选择输出文件夹"
              variant="secondary"
              icon={<FolderOpen size={18} strokeWidth={1.75} aria-hidden="true" />}
              loading={controller.artifactDirectorySelection.phase === "selecting"}
              loadingLabel="等待系统选择"
              disabled={
                controller.protectionState !== "normal" ||
                controller.state.draftRevision !== controller.state.savedRevision
              }
              disabledReason={
                controller.protectionState !== "normal"
                  ? "后端保护状态不是正常态，目录修改保持关闭。"
                  : "请先保存当前设置草稿，避免目录选择覆盖未保存内容。"
              }
              onClick={controller.selectArtifactOutputDirectory}
            />
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

function SafeModeNotice({ controller }: { controller: SettingsPrivacyJourneyController }) {
  if (controller.protectionState === "normal" || controller.protectionState === "loading") {
    return null;
  }

  const active = controller.protectionState === "active";
  const recoveryEligible = controller.eligibleCredentialPurposes.length > 0;

  return (
    <FoundationNotice title={active ? "安全模式保持生效" : "保护状态未知"} tone="protection" live>
      <p>
        {active
          ? recoveryEligible
            ? "后端报告了保护状态；连接测试与设置保存继续关闭。下方只开放后端启动快照明确列出的凭据初始化或访问恢复，且仍需原生确认。"
            : "后端报告了保护状态；连接测试与设置保存继续关闭。当前读模型没有提供凭据恢复资格，页面不会从自由文本原因推导并开放系统凭据操作。"
          : "LifeStateProjection 没有提供可核对的保护状态；连接测试、设置保存和本地确定态全部保持关闭。"}
      </p>
    </FoundationNotice>
  );
}

function CredentialInitializationPanel({
  controller,
}: {
  controller: SettingsPrivacyJourneyController;
}) {
  const phase = controller.credentialInitialization.phase;
  const eligibleCount = controller.eligibleCredentialPurposes.length;
  if (eligibleCount === 0 && phase === "idle") return null;

  const report = controller.credentialInitialization.report;
  const running = phase === "running";
  const restartRequired = phase === "restart_required";
  const cleanupUnknown = report?.cleanupStatus === "unknown";
  const accessRecovery = (controller.snapshot?.credentialBootstrap?.purposes ?? []).some(
    purpose => purpose.status === "unavailable"
  );
  return (
    <section className="ol-settings-section" aria-labelledby="ol-settings-credential-title">
      <div className="ol-settings-section-heading">
        <span>后端启动快照</span>
        <h2 id="ol-settings-credential-title">
          {accessRecovery ? "凭据访问恢复" : "系统凭据初始化"}
        </h2>
      </div>
      {restartRequired ? (
        <FoundationNotice
          title={accessRecovery ? "访问恢复完成，需要重启" : "初始化完成，需要重启"}
          live
        >
          <p>当前进程仍保持受限；完全退出并重新启动 OpenLife 后才会重新读取这些凭据。</p>
        </FoundationNotice>
      ) : phase === "blocked" ? (
        <FoundationNotice title="初始化未完成" tone="error" live>
          <p>
            {report?.blockedReason ??
              "后端没有证明全部初始化步骤和补偿步骤完成；当前继续保持阻塞。"}
          </p>
        </FoundationNotice>
      ) : phase === "failed" ? (
        <FoundationNotice title="初始化已取消或失败" tone="protection" live>
          <p>没有获得成功证明；当前进程和后端快照都不会被标记为可用。</p>
        </FoundationNotice>
      ) : (
        <p>
          {accessRecovery
            ? `后端确认有 ${eligibleCount} 类既有凭据需要恢复访问。此操作不创建、不覆盖且不返回密钥。`
            : `后端确认有 ${eligibleCount} 类系统凭据可以首次初始化。`}
          点击后仍需在 macOS 原生系统对话框中确认。
        </p>
      )}
      <FoundationActionButton
        label={restartRequired ? "等待重启" : accessRecovery ? "恢复凭据访问" : "初始化系统凭据"}
        icon={<Check size={18} strokeWidth={1.75} aria-hidden="true" />}
        loading={running}
        loadingLabel="等待系统确认"
        disabled={restartRequired || cleanupUnknown}
        disabledReason={
          restartRequired
            ? "初始化已完成；必须重启后重新读取状态。"
            : cleanupUnknown
              ? "后端无法证明补偿完成；必须先重启并重新检查状态。"
              : undefined
        }
        onClick={controller.initializeRequiredCredentials}
      />
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
  const canRetryBoundary =
    controller.state.phase === "unknown" &&
    controller.state.failureStage === "boundary_refresh" &&
    !controller.state.boundaryAppliesToSavedRevision;
  return (
    <section className="ol-settings-actions" aria-label="设置动作">
      <div>
        <span>明确操作</span>
        <p>测试与保存互不替代；保存后必须重新读取模型传输边界。</p>
      </div>
      <div className="ol-settings-actions__buttons">
        {canRetryBoundary && (
          <FoundationActionButton
            label="重新读取保存结果"
            variant="secondary"
            icon={<RefreshCw size={18} strokeWidth={1.75} aria-hidden="true" />}
            onClick={controller.retryBoundaryRefresh}
          />
        )}
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
