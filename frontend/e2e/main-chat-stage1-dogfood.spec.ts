import { test, expect } from '@playwright/test';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const requiredJourneys = Array.from({ length: 36 }, (_, index) =>
  `D${String(index + 1).padStart(2, '0')}`
);

function digestLabel(input: string): string {
  const bytes = Buffer.byteLength(input);
  const hash = crypto.createHash('sha256').update(input).digest('hex');
  return `bytes:${bytes} hash:sha256:${hash}`;
}

function writeBlockedReport(blockers: string[]) {
  const reportPath = path.resolve(
    process.cwd(),
    'test-results/main-chat-stage1-dogfood-report.json'
  );
  const runId = `stage1-browser-e2e-blocked-${Date.now()}`;
  const generatedAt = new Date().toISOString();
  const report = {
    browserE2eEnvironmentReady: false,
    selfContainedRunner: true,
    smokePassed: false,
    reportPath: 'frontend/test-results/main-chat-stage1-dogfood-report.json',
    evidenceSource: 'tauri_command_surface_unavailable',
    runId,
    generatedAt,
    reportDigest: digestLabel(`${runId}:${generatedAt}:${requiredJourneys.join(',')}`),
    requiredJourneys,
    passedJourneys: [],
    failedJourneys: requiredJourneys,
    blockers,
  };

  fs.mkdirSync(path.dirname(reportPath), { recursive: true });
  fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));
  return report;
}

test.describe('main-chat-stage1-dogfood', () => {
  test('exports blocked report when real Tauri browser command surface is unavailable', async ({
    page,
  }) => {
    await page.goto('/#/chat');
    const tauriAvailable = await page.evaluate(() => Boolean((window as any).__TAURI_INTERNALS__));

    if (!tauriAvailable) {
      const report = writeBlockedReport([
        'not_ready_browser_e2e_blocked',
        'real_tauri_browser_command_surface_unavailable',
      ]);

      expect(report.selfContainedRunner).toBe(true);
      expect(report.smokePassed).toBe(false);
      expect(report.requiredJourneys).toEqual(requiredJourneys);
      expect(report.passedJourneys).toEqual([]);
      expect(report.failedJourneys).toEqual(requiredJourneys);
      expect(report.evidenceSource).not.toContain('frontend');
      expect(report.evidenceSource).not.toContain('fixture');
      return;
    }

    const report = writeBlockedReport([
      'not_ready_browser_e2e_blocked',
      'real_tauri_browser_stage1_smoke_not_implemented',
    ]);
    expect(report.smokePassed).toBe(false);
  });
});
