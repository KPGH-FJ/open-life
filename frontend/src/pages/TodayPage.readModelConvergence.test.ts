import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

function source(pathFromFrontendRoot: string): string {
  return readFileSync(join(process.cwd(), pathFromFrontendRoot), "utf8");
}

function expectContains(filePath: string, patterns: string[]) {
  const contents = source(filePath);
  for (const pattern of patterns) {
    expect(contents, `${filePath} should contain ${pattern}`).toContain(pattern);
  }
}

function expectNotContains(filePath: string, patterns: string[]) {
  const contents = source(filePath);
  for (const pattern of patterns) {
    expect(contents, `${filePath} should not contain ${pattern}`).not.toContain(pattern);
  }
}

describe("frontend read-model convergence guards", () => {
  it("keeps repaired product authority on backend read-model consumers", () => {
    expectContains("src/pages/MailboxPage.tsx", [
      "getReviewCenterViewModel",
      "allowedActions.find",
      "item.materializationStatus",
      "ReviewCenterViewModel 尚未提供该确认项的后端操作状态",
    ]);
    expectNotContains("src/pages/MailboxPage.tsx", [
      "function canAccept(",
      "function isPathInSafePaths(",
      "function appliedNotice(",
      "setProposals(prev =>",
    ]);

    expectContains("src/pages/ChatPage.tsx", [
      "getTasksViewModel",
      "currentTaskViewItem",
      "enabledTaskViewControl",
      "taskViewItem?.lifecycleStatus",
    ]);
    expectNotContains("src/pages/ChatPage.tsx", [
      'taskState?.canResume ? ["resume"]',
      'taskState?.canRetry ? ["retry"]',
      'canCancel ? ["cancel"]',
      'taskStatus === "completed" ||',
      'runStatus === "completed" ||',
    ]);

    expectContains("src/pages/RunsPage.tsx", [
      "getTasksViewModel",
      "TaskViewModelItem",
      "item.lifecycleStatus",
      "enabledActionControls(item)",
      "control.effect",
    ]);
    expectNotContains("src/pages/RunsPage.tsx", [
      "listMainChatAgentTasks",
      "getMainChatAgentTaskDetail",
      "taskSummaryByRunId",
      "allowedControlsForSummary",
      "lifecycleForRun",
    ]);
  });

  it("keeps LifeModel, Memory, and provider/privacy truth behind backend owners", () => {
    expectContains("src/pages/LifeModelPage.tsx", [
      "getLifeModelViewModel",
      "viewModel?.pendingUpdateCounts.pendingReview",
      "viewModel?.memoryLinkage",
      "viewModel?.candidateChanges",
    ]);
    expectNotContains("src/pages/LifeModelPage.tsx", [
      "getLifeModel(",
      "getLifeModelCurrentView(",
      "getSystemDiagnostics(",
      "countMemoryChunks(",
      "getMemoryTierStats(",
      "listProposals(",
      "buildLifeModelViewModelEnvelope",
    ]);

    expectContains("src/pages/MemorySearch.tsx", [
      "getMemoryViewModel",
      "memoryViewModel?.summary",
      "lifecycleSummary",
      "向量层级只是存储遥测",
    ]);
    expectNotContains("src/pages/MemorySearch.tsx", ["getMemoryTierStats("]);

    expectContains("src/pages/SettingsPage.tsx", [
      "getMemoryViewModel",
      "getProviderPrivacyBoundarySummary",
      "providerPrivacyBoundary",
    ]);
    expectNotContains("src/pages/settings/tabs/ProviderTab.tsx", ["buildProviderReadinessView"]);
    expectNotContains("src/pages/settings/tabs/OverviewTab.tsx", ["buildProviderReadinessView"]);
  });

  it("keeps Today limited and the future adapter explicit without a production preview route", () => {
    expectContains("src/pages/TodayPage.tsx", [
      "getLifeStateProjection",
      "getDailyGoals",
      "reviewRequiredCountFromProjection",
    ]);
    expectNotContains("src/pages/TodayPage.tsx", [
      "listProposals(",
      "getSystemDiagnostics(",
      "getMemoryTierStats(",
      "buildProviderReadinessView",
      "getProviderPrivacyBoundarySummary",
    ]);

    expectContains("src/viewmodels/today/todayViewModelAdapter.ts", [
      "providerPrivacyBoundary",
      "Provider/privacy boundary is not backend-owned by the Today limited slice.",
    ]);
    expectNotContains("src/App.tsx", ["TodayV2PreviewPage", "/today-v2-preview"]);
  });

  it("keeps frontend helpers display-only", () => {
    expectNotContains("src/utils/runtimeDisclosure.ts", [
      "safeInvoke",
      "getSystemDiagnostics",
      "getProviderPrivacyBoundarySummary",
      "listMainChatAgentTasks",
      "resumeMainChatAgentTask",
      "ReviewCenterViewModel",
      "TasksViewModel",
      "MemoryViewModel",
    ]);
    expectNotContains("src/utils/lifeStateProjection.ts", [
      "safeInvoke",
      "getSystemDiagnostics",
      "listProposals",
      "getProviderPrivacyBoundarySummary",
      "getMemoryTierStats",
    ]);
  });
});
