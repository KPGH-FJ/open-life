import { useEffect } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type {
  SettingsPrivacyDataSource,
  SettingsPrivacySnapshot,
} from "./settingsPrivacyDataSource";
import { SettingsPrivacyView } from "./SettingsPrivacyView";
import { useSettingsPrivacyJourney } from "./useSettingsPrivacyJourney";

function SafeModeSettings({ source }: { source: SettingsPrivacyDataSource }) {
  const controller = useSettingsPrivacyJourney(source, vi.fn());
  useEffect(() => controller.ensureLoaded(), [controller.ensureLoaded]);
  return (
    <SettingsPrivacyView
      controller={controller}
      surface="model-provider"
      onOpenReview={vi.fn()}
      onOpenInspector={vi.fn()}
    />
  );
}

describe("SettingsPrivacyView credential recovery", () => {
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
      recoverRequiredCredentialAccess: vi.fn(),
    };

    render(<SafeModeSettings source={source} />);

    expect(await screen.findByRole("heading", { name: "模型与传输边界" })).toBeInTheDocument();
    expect(screen.getByLabelText("API 凭据")).toHaveValue("");
    expect(screen.getByText(/后端返回遮罩凭据/)).toBeInTheDocument();
  });

  it("keeps recovery reachable from proven Safe Mode even when editable config is unavailable", async () => {
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
    const recoverRequiredCredentialAccess = vi.fn();
    const source: SettingsPrivacyDataSource = {
      loadSettingsPrivacy: vi.fn().mockResolvedValue(snapshot),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
      recoverRequiredCredentialAccess,
    };
    const user = userEvent.setup();

    render(<SafeModeSettings source={source} />);

    expect(await screen.findByRole("heading", { name: "安全模式" })).toBeInTheDocument();
    expect(screen.getByText("设置暂不可用")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "检查系统凭据" }));

    expect(screen.getByRole("dialog", { name: "确认系统凭据检查范围" })).toBeInTheDocument();
    expect(screen.getByText(/仅在对应的长期数据文件不存在时/)).toBeInTheDocument();
    expect(recoverRequiredCredentialAccess).not.toHaveBeenCalled();
  });
});
