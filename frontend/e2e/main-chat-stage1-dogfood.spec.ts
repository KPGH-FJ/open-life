import { test, expect } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

import {
  STAGE1_REQUIRED_BROWSER_JOURNEYS,
  buildStage1BlockedBrowserEvidenceReport,
  type Stage1BrowserEvidenceReport,
} from "../src/stage1BrowserEvidence";

function writeReport(report: Stage1BrowserEvidenceReport) {
  const reportPath = path.resolve(
    process.cwd(),
    "test-results/main-chat-stage1-dogfood-report.json"
  );
  fs.mkdirSync(path.dirname(reportPath), { recursive: true });
  fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));
  return report;
}

test.describe("main-chat-stage1-dogfood", () => {
  test.setTimeout(300_000);

  test("exports blocked report when real Tauri browser command surface is unavailable", async ({
    page,
  }) => {
    await page.goto("/#/chat");
    const tauriAvailable = await page.evaluate(() => Boolean((window as any).__TAURI_INTERNALS__));

    if (!tauriAvailable) {
      const report = writeReport(
        buildStage1BlockedBrowserEvidenceReport(["real_tauri_browser_command_surface_unavailable"])
      );

      expect(report.selfContainedRunner).toBe(true);
      expect(report.smokePassed).toBe(false);
      expect(report.requiredJourneys).toEqual(STAGE1_REQUIRED_BROWSER_JOURNEYS);
      expect(report.passedJourneys).toEqual([]);
      expect(report.failedJourneys).toEqual(STAGE1_REQUIRED_BROWSER_JOURNEYS);
      expect(report.observedScenarios).toEqual([]);
      expect(report.evidenceSource).not.toContain("frontend");
      expect(report.evidenceSource).not.toContain("fixture");
      return;
    }

    const report = writeReport(
      buildStage1BlockedBrowserEvidenceReport([
        "real_tauri_browser_per_scenario_observation_unavailable",
      ])
    );
    expect(report.smokePassed).toBe(false);
    expect(report.passedJourneys).toEqual([]);
    expect(report.failedJourneys).toEqual(STAGE1_REQUIRED_BROWSER_JOURNEYS);
    expect(report.observedScenarios).toEqual([]);
  });
});
