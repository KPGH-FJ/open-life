import { useCallback, useEffect, useMemo, useState } from "react";
import { Check, Pencil, Plus, Trash2, Wifi } from "lucide-react";
import type {
  LlmConnectionTestResult,
  ProviderConnectionViewModel,
  ProviderConnectionsViewModel,
  ReviewItem,
  SaveProviderConnectionInput,
} from "@/tauri";
import { productErrorMessage } from "@/shared/productError";
import {
  FoundationActionButton,
  FoundationDialog,
  FoundationNotice,
  FoundationStatusLabel,
  FoundationTextField,
} from "@/ui/foundation";
import { settingsProviderOptions } from "./settingsPresentation";
import type { ProviderConnectionDataSource } from "./settingsDataSource";

const providerDefaults: Record<string, string> = {
  deepseek: "https://api.deepseek.com",
  openai: "https://api.openai.com/v1",
  openrouter: "https://openrouter.ai/api/v1",
  gemini: "https://generativelanguage.googleapis.com/v1beta/openai",
  siliconflow: "https://api.siliconflow.cn/v1",
  moonshot: "https://api.moonshot.cn/v1",
  dashscope: "https://dashscope.aliyuncs.com/compatible-mode/v1",
  zhipu: "https://open.bigmodel.cn/api/paas/v4",
  custom: "",
};

type EditorDraft = SaveProviderConnectionInput & { credential: string };

function emptyDraft(): EditorDraft {
  return {
    providerId: "openrouter",
    displayName: "OpenRouter",
    endpoint: providerDefaults.openrouter,
    modelId: "",
    credential: "",
  };
}

function editorDraft(connection: ProviderConnectionViewModel): EditorDraft {
  const model = connection.models.find(candidate => candidate.selected) ?? connection.models[0];
  return {
    id: connection.id,
    providerId: connection.providerId,
    displayName: connection.displayName,
    endpoint: connection.endpoint,
    modelId: model?.modelId ?? "",
    credential: "",
  };
}

function connectionStatus(connection: ProviderConnectionViewModel) {
  const model = connection.models.find(candidate => candidate.selected) ?? connection.models[0];
  if (model?.validationState === "ready" && connection.credentialState === "stored") {
    return { label: "可用", status: "success" as const };
  }
  if (connection.credentialState !== "stored") {
    return { label: "凭据不可用", status: "error" as const };
  }
  return { label: "待测试", status: "waiting" as const };
}

export function ProviderConnectionsPanel({
  disabled = false,
  disabledReason,
  onOpenReview,
  onViewModelChange,
  dataSource,
}: {
  disabled?: boolean;
  disabledReason?: string;
  onOpenReview?: (item: ReviewItem) => void;
  onViewModelChange?: (viewModel: ProviderConnectionsViewModel) => void;
  dataSource?: ProviderConnectionDataSource | null;
}) {
  const [viewModel, setViewModel] = useState<ProviderConnectionsViewModel | null>(null);
  const [editor, setEditor] = useState<EditorDraft | null>(null);
  const [phase, setPhase] = useState<"loading" | "idle" | "saving" | "testing" | "deleting">(
    "loading"
  );
  const [error, setError] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<LlmConnectionTestResult | null>(null);
  const [testReviewItem, setTestReviewItem] = useState<ReviewItem | null>(null);
  const [deleteCandidate, setDeleteCandidate] = useState<ProviderConnectionViewModel | null>(null);

  const load = useCallback(async () => {
    setPhase("loading");
    setError(null);
    try {
      if (!dataSource) throw new Error("provider_connection_data_source_unavailable");
      const next = await dataSource.loadProviderConnections();
      setViewModel(next);
      onViewModelChange?.(next);
    } catch (loadError) {
      setError(productErrorMessage(loadError));
    } finally {
      setPhase("idle");
    }
  }, [dataSource, onViewModelChange]);

  useEffect(() => {
    void load();
  }, [load]);

  const validationError = useMemo(() => {
    if (!editor) return null;
    if (!editor.displayName.trim()) return "请填写连接名称。";
    if (!/^https?:\/\//i.test(editor.endpoint.trim())) return "请输入完整的 HTTP 或 HTTPS 地址。";
    if (!editor.modelId.trim()) return "请填写精确模型 ID。";
    if (!editor.id && !editor.credential.trim()) return "新连接需要 API 凭据。";
    return null;
  }, [editor]);

  const save = async () => {
    if (!editor || validationError) return;
    setPhase("saving");
    setError(null);
    setTestResult(null);
    setTestReviewItem(null);
    try {
      if (!dataSource) throw new Error("provider_connection_data_source_unavailable");
      const next = await dataSource.saveProviderConnection({
        ...editor,
        credential: editor.credential.trim() || undefined,
      });
      setViewModel(next);
      onViewModelChange?.(next);
      setEditor(null);
    } catch (saveError) {
      setError(productErrorMessage(saveError));
    } finally {
      setPhase("idle");
    }
  };

  const test = async (connection: ProviderConnectionViewModel) => {
    const model = connection.models.find(candidate => candidate.selected) ?? connection.models[0];
    if (!model) return;
    setPhase("testing");
    setError(null);
    setTestResult(null);
    setTestReviewItem(null);
    try {
      if (!dataSource) throw new Error("provider_connection_data_source_unavailable");
      const outcome = await dataSource.testSavedProviderConnection(connection.id, model.profileId);
      const result = outcome.result;
      setTestResult(result);
      setTestReviewItem(outcome.reviewItem);
      await load();
    } catch (testError) {
      setError(productErrorMessage(testError));
      setPhase("idle");
    }
  };

  const remove = async () => {
    if (!deleteCandidate) return;
    setPhase("deleting");
    setError(null);
    try {
      if (!dataSource) throw new Error("provider_connection_data_source_unavailable");
      const next = await dataSource.deleteProviderConnection(deleteCandidate.id);
      setViewModel(next);
      onViewModelChange?.(next);
      setDeleteCandidate(null);
      if (editor?.id === deleteCandidate.id) setEditor(null);
    } catch (deleteError) {
      setError(productErrorMessage(deleteError));
    } finally {
      setPhase("idle");
    }
  };

  return (
    <section className="ol-settings-section" aria-labelledby="ol-settings-cloud-title">
      <div className="ol-settings-section-heading ol-provider-connections__heading">
        <div>
          <span>独立保存，按连接选择模型</span>
          <h2 id="ol-settings-cloud-title">供应商连接</h2>
        </div>
        <FoundationActionButton
          label="添加连接"
          variant="secondary"
          icon={<Plus size={17} aria-hidden="true" />}
          disabled={disabled}
          disabledReason={disabled ? (disabledReason ?? "当前不能修改供应商连接。") : undefined}
          onClick={() => {
            setEditor(emptyDraft());
            setError(null);
            setTestResult(null);
            setTestReviewItem(null);
          }}
        />
      </div>

      {phase === "loading" && !viewModel && <p>正在读取连接…</p>}
      {viewModel?.connections.length === 0 && !editor && (
        <p>还没有云端连接。添加后测试一次，即可在对话中选择它的模型。</p>
      )}
      {viewModel?.connections.length ? (
        <ul className="ol-provider-connections" aria-label="已保存的供应商连接">
          {viewModel.connections.map(connection => {
            const status = connectionStatus(connection);
            const model =
              connection.models.find(candidate => candidate.selected) ?? connection.models[0];
            return (
              <li key={connection.id}>
                <div className="ol-provider-connections__identity">
                  <strong>{connection.displayName}</strong>
                  <span>{model?.displayName ?? "尚未配置模型"}</span>
                  <small>{connection.endpoint}</small>
                </div>
                <FoundationStatusLabel
                  label={status.label}
                  status={status.status}
                  verified={status.status === "success"}
                />
                <div className="ol-provider-connections__actions">
                  <button type="button" disabled={disabled} onClick={() => void test(connection)}>
                    <Wifi size={16} aria-hidden="true" />
                    测试
                  </button>
                  <button
                    type="button"
                    disabled={disabled}
                    onClick={() => setEditor(editorDraft(connection))}
                  >
                    <Pencil size={16} aria-hidden="true" />
                    编辑
                  </button>
                  <button
                    type="button"
                    disabled={disabled}
                    onClick={() => setDeleteCandidate(connection)}
                  >
                    <Trash2 size={16} aria-hidden="true" />
                    删除
                  </button>
                </div>
              </li>
            );
          })}
        </ul>
      ) : null}

      {editor && (
        <div className="ol-provider-connection-editor">
          <label className="ol-settings-select-field" htmlFor="ol-provider-connection-kind">
            <span className="ol-settings-select-field__label">供应商</span>
            <select
              id="ol-provider-connection-kind"
              value={editor.providerId}
              onChange={event => {
                const providerId = event.target.value as EditorDraft["providerId"];
                const option = settingsProviderOptions.find(
                  candidate => candidate.value === providerId
                );
                setEditor(current =>
                  current
                    ? {
                        ...current,
                        providerId,
                        displayName: option?.label ?? current.displayName,
                        endpoint: providerDefaults[providerId] ?? "",
                        credential: "",
                      }
                    : current
                );
              }}
            >
              {settingsProviderOptions.map(option => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>
          <FoundationTextField
            id="ol-provider-connection-name"
            label="连接名称"
            value={editor.displayName}
            onChange={event =>
              setEditor(current =>
                current ? { ...current, displayName: event.target.value } : current
              )
            }
          />
          <FoundationTextField
            id="ol-provider-connection-endpoint"
            label="API 地址"
            value={editor.endpoint}
            onChange={event =>
              setEditor(current =>
                current ? { ...current, endpoint: event.target.value } : current
              )
            }
            spellCheck={false}
          />
          <FoundationTextField
            id="ol-provider-connection-model"
            label="模型 ID"
            description="保存这一精确模型；其他模型可稍后从对话模型选择器添加。"
            value={editor.modelId}
            onChange={event =>
              setEditor(current =>
                current ? { ...current, modelId: event.target.value } : current
              )
            }
            spellCheck={false}
          />
          <FoundationTextField
            id="ol-provider-connection-credential"
            label="API 凭据"
            description={
              editor.id ? "留空会保留当前凭据；输入新值会安全轮换。" : "凭据不会返回网页层。"
            }
            type="password"
            value={editor.credential}
            placeholder={editor.id ? "保留当前凭据" : "输入 API 凭据"}
            onChange={event =>
              setEditor(current =>
                current ? { ...current, credential: event.target.value } : current
              )
            }
            autoComplete="new-password"
            spellCheck={false}
          />
          {validationError && (
            <p className="ol-provider-connection-editor__error">{validationError}</p>
          )}
          <div className="ol-settings-inline-actions">
            <FoundationActionButton
              label="保存连接"
              variant="primary"
              icon={<Check size={17} aria-hidden="true" />}
              loading={phase === "saving"}
              loadingLabel="正在保存"
              disabled={disabled || Boolean(validationError)}
              disabledReason={
                disabled
                  ? (disabledReason ?? "当前不能修改供应商连接。")
                  : (validationError ?? undefined)
              }
              onClick={() => void save()}
            />
            <FoundationActionButton label="取消" variant="quiet" onClick={() => setEditor(null)} />
          </div>
        </div>
      )}

      {testResult && (
        <FoundationNotice
          title={testResult.ok ? "连接可用" : "连接测试未通过"}
          tone={testResult.ok ? "neutral" : "error"}
          live
        >
          <p>{testResult.message}</p>
          {testResult.review_proposal_id && (
            <FoundationActionButton
              label="查看并确认"
              variant="secondary"
              disabled={!testReviewItem || !onOpenReview}
              disabledReason={
                testReviewItem && onOpenReview
                  ? undefined
                  : "当前无法从需处理事项精确定位这次连接确认。"
              }
              onClick={() => testReviewItem && onOpenReview?.(testReviewItem)}
            />
          )}
        </FoundationNotice>
      )}
      {error && (
        <FoundationNotice title="连接操作没有完成" tone="error" live>
          <p>{error}</p>
        </FoundationNotice>
      )}

      <FoundationDialog
        open={deleteCandidate !== null}
        title="删除这个供应商连接？"
        description="连接凭据和模型档案会一并删除；已有历史记录仍保留当时使用的模型标识。"
        busy={phase === "deleting"}
        onClose={() => setDeleteCandidate(null)}
        footer={
          <>
            <FoundationActionButton
              label="取消"
              variant="quiet"
              onClick={() => setDeleteCandidate(null)}
            />
            <FoundationActionButton
              label="删除连接"
              variant="danger"
              icon={<Trash2 size={17} aria-hidden="true" />}
              loading={phase === "deleting"}
              onClick={() => void remove()}
            />
          </>
        }
      >
        <p>{deleteCandidate?.displayName}</p>
      </FoundationDialog>
    </section>
  );
}
