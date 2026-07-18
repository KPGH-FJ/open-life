import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const directory = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(directory, "../../..");
const requireFromFrontend = createRequire(path.join(repoRoot, "frontend/package.json"));
const { chromium } = requireFromFrontend("@playwright/test");

const baseUrl =
  process.env.OPENLIFE_BLUEPRINT_URL ||
  "http://127.0.0.1:4183/docs/phase3e_product_blueprints/prototype/index.html";
const artifactsDir = path.resolve(directory, "../artifacts");
fs.mkdirSync(artifactsDir, { recursive: true });

const screens = [
  "today-ready",
  "today-stale",
  "workspace",
  "tasks",
  "review-pending",
  "review-approved",
  "lifemodel",
  "settings",
];
const viewports = [
  { name: "1440x900", width: 1440, height: 900 },
  { name: "1280x800", width: 1280, height: 800 },
  { name: "390x844", width: 390, height: 844 },
];

const browser = await chromium.launch({ headless: true });
const errors = [];
const results = [];

try {
  for (const viewport of viewports) {
    const page = await browser.newPage({ viewport });
    page.on("console", message => {
      if (["error", "warning"].includes(message.type())) {
        errors.push(`${viewport.name} console ${message.type()}: ${message.text()}`);
      }
    });
    page.on("pageerror", error => errors.push(`${viewport.name} pageerror: ${error.message}`));

    for (const screen of screens) {
      await page.goto(`${baseUrl}?screen=${screen}`, { waitUntil: "networkidle" });
      await page.waitForSelector("#workSurface");

      const metrics = await page.evaluate(() => {
        const html = document.documentElement;
        const body = document.body;
        const visibleTextNodes = [
          ...document.querySelectorAll("button, h1, h2, h3, h4, p, dd, .nav-copy"),
        ]
          .filter(node => {
            const style = getComputedStyle(node);
            return (
              style.display !== "none" &&
              style.visibility !== "hidden" &&
              node.getBoundingClientRect().width > 0
            );
          })
          .map(node => {
            const rect = node.getBoundingClientRect();
            return {
              text: (node.textContent || "").trim().slice(0, 80),
              left: rect.left,
              right: rect.right,
              top: rect.top,
              bottom: rect.bottom,
            };
          });
        const escaped = visibleTextNodes.filter(
          item => item.left < -1 || item.right > window.innerWidth + 1
        );
        return {
          viewportWidth: window.innerWidth,
          documentOverflow: Math.max(html.scrollWidth, body.scrollWidth) - window.innerWidth,
          escaped,
          current: window.__OPENLIFE_BLUEPRINT__.getState(),
        };
      });

      if (metrics.documentOverflow > 1) {
        errors.push(
          `${viewport.name}/${screen}: horizontal overflow ${metrics.documentOverflow}px`
        );
      }
      if (metrics.escaped.length) {
        errors.push(
          `${viewport.name}/${screen}: escaped visible text ${JSON.stringify(metrics.escaped.slice(0, 3))}`
        );
      }

      const file = path.join(artifactsDir, `phase3e_${viewport.name}_${screen}.png`);
      await page.screenshot({ path: file, type: "png" });
      results.push({ viewport: viewport.name, screen, file, overflow: metrics.documentOverflow });
    }

    await page.close();
  }

  const mobile = await browser.newPage({ viewport: { width: 390, height: 844 } });
  await mobile.goto(`${baseUrl}?screen=today-ready`, { waitUntil: "networkidle" });
  await mobile.click("#openInspector");
  await mobile.waitForSelector("#workbenchShell.is-inspector-open");
  await mobile.screenshot({
    path: path.join(artifactsDir, "phase3e_390x844_today-evidence-sheet.png"),
    type: "png",
  });
  await mobile.click("#closeInspector");
  await mobile.click("#openMobileMenu");
  await mobile.screenshot({
    path: path.join(artifactsDir, "phase3e_390x844_navigation-drawer.png"),
    type: "png",
  });
  await mobile.close();

  const review = await browser.newPage({ viewport: { width: 390, height: 844 } });
  await review.goto(`${baseUrl}?screen=review-pending`, { waitUntil: "networkidle" });
  await review.click('[data-action-id="review:approve"]');
  await review.waitForSelector("#feedbackDialog[open]");
  await review.screenshot({
    path: path.join(artifactsDir, "phase3e_390x844_review-confirmation.png"),
    type: "png",
  });
  await review.close();

  const workspace = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  await workspace.goto(`${baseUrl}?screen=workspace`, { waitUntil: "networkidle" });
  await workspace.click('[data-action-id="workspace:view-scope"]');
  await workspace.waitForSelector("#workbenchShell.is-inspector-open");
  await workspace.screenshot({
    path: path.join(artifactsDir, "phase3e_1440x900_workspace-permission-inspector.png"),
    type: "png",
  });
  await workspace.close();
} finally {
  await browser.close();
}

if (errors.length) {
  console.error(errors.join("\n"));
  process.exit(1);
}

console.log(
  `Visual capture passed: ${results.length} screen/viewport images plus 4 interaction images.`
);
