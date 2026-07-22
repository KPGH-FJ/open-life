import { useEffect } from "react";
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type {
  SettingsPrivacyDataSource,
  SettingsPrivacySnapshot,
} from "./settingsPrivacyDataSource";
import { SettingsPrivacyView } from "./SettingsPrivacyView";
import { useSettingsPrivacyJourney } from "./useSettingsPrivacyJourney";

function SafeModeSettings({ source }: { source: SettingsPrivacyDataSource }) {
  const controller = useSettingsPrivacyJourney(source, vi.fn());
  useEffect(() => {
    void controller.ensureLoaded();
  }, [controller.ensureLoaded]);
  return (
    <SettingsPrivacyView
      controller={controller}
      surface="model-provider"
      onOpenReview={vi.fn()}
      onOpenInspector={vi.fn()}
    />
  );
}

describe("SettingsPrivacyView", () => {
  it("renders the sanitized native config shape when the secret field is omitted", async () => {
    const snapshot: SettingsPrivacySnapshot = {
      config: {
        llm: {
          provider: "openai",
          openai_base: "https://api.openai.com/v1",
          openai_key_ref: "keychain://com.openlife.desktop/provider-api-key",
          embedding_model: "text-embedding-3-small",
          chat_model: "gpt-4o-mini",
        },
        prefer_local_model: false,
        local_model: "qwen2.5:14b",
      },
      boundaryEnvelope: {
        data: null,
        status: "error",
        lastUpdatedAt: null,
        source: "backend-readmodel",
        evidenceRefs: [],
        warnings: [],
        actions: { primary: [], review: [], debugOnly: [] },
      },
      safeMode: { active: false, reason: "", sourceRefs: [] },
      diagnostics: [
        { id: "sanitized_config", status: "loaded" },
        { id: "provider_privacy_boundary", status: "loaded" },
        { id: "life_state_projection", status: "loaded" },
        { id: "review_item_resolution", status: "not_requested" },
      ],
    };
    const source: SettingsPrivacyDataSource = {
      loadSettingsPrivacy: vi.fn().mockResolvedValue(snapshot),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };

    render(<SafeModeSettings source={source} />);

    expect(await screen.findByRole("heading", { name: "模型与传输边界" })).toBeInTheDocument();
    expect(screen.getByLabelText("API 凭据")).toHaveValue("");
    expect(screen.getByText(/后端返回遮罩凭据/)).toBeInTheDocument();
  });

  it("does not infer credential recovery eligibility from generic Safe Mode", async () => {
    const snapshot: SettingsPrivacySnapshot = {
      config: null,
      boundaryEnvelope: {
        data: null,
        status: "error",
        lastUpdatedAt: null,
        source: "backend-readmodel",
        evidenceRefs: [],
        warnings: [],
        actions: { primary: [], review: [], debugOnly: [] },
      },
      safeMode: {
        active: true,
        reason: "credential_store_unavailable",
        sourceRefs: ["safe-mode:credential-store"],
      },
      diagnostics: [
        { id: "sanitized_config", status: "failed", message: "config unavailable" },
        { id: "provider_privacy_boundary", status: "failed", message: "boundary unavailable" },
        { id: "life_state_projection", status: "loaded" },
        { id: "review_item_resolution", status: "not_requested" },
      ],
    };
    const source: SettingsPrivacyDataSource = {
      loadSettingsPrivacy: vi.fn().mockResolvedValue(snapshot),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };

    render(<SafeModeSettings source={source} />);

    expect(await screen.findByText("安全模式保持生效")).toBeInTheDocument();
    expect(screen.getByText("设置暂不可用")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "检查系统凭据" })).not.toBeInTheDocument();
    expect(screen.getByText(/不会从自由文本原因推导/)).toBeInTheDocument();
  });

  it("shows unknown protection and closes actions when LifeStateProjection is unavailable", async () => {
    const snapshot: SettingsPrivacySnapshot = {
      config: {
        llm: {
          provider: "custom",
          openai_base: "http://127.0.0.1:11434/v1",
          openai_key: "***",
          embedding_model: "nomic-embed-text",
          chat_model: "qwen2.5:14b",
        },
        prefer_local_model: true,
        local_model: "qwen2.5:14b",
      },
      boundaryEnvelope: {
        data: {
          routeType: "local",
          externalTransmission: "not_sent",
          providerLabel: "本机模型服务",
          modelLabel: "qwen2.5:14b",
          privacyLabel: "仅本机处理",
          risk: "none",
          localOnlyRequired: true,
          evidenceRefs: [],
        },
        status: "ready",
        lastUpdatedAt: "2026-07-22T00:00:00Z",
        source: "backend-readmodel",
        evidenceRefs: [],
        warnings: [],
        actions: { primary: [], review: [], debugOnly: [] },
      },
      safeMode: null,
      diagnostics: [
        { id: "sanitized_config", status: "loaded" },
        { id: "provider_privacy_boundary", status: "loaded" },
        {
          id: "life_state_projection",
          status: "failed",
          message: "projection unavailable",
        },
        { id: "review_item_resolution", status: "not_requested" },
      ],
    };
    const source: SettingsPrivacyDataSource = {
      loadSettingsPrivacy: vi.fn().mockResolvedValue(snapshot),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };

    render(<SafeModeSettings source={source} />);

    expect(await screen.findByText("保护状态未知")).toBeInTheDocument();
    expect(screen.getByText(/LifeStateProjection 没有提供可核对的保护状态/)).toBeInTheDocument();
    expect(screen.getByText("是否外传未知")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "测试连接" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "保存设置" })).toBeDisabled();
  });
});
