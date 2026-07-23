import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const directory = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(directory, "../../..");
const requireFromFrontend = createRequire(path.join(repoRoot, "frontend/package.json"));
const { chromium } = requireFromFrontend("@playwright/test");

const baseUrl =
  process.env.OPENLIFE_REVIEW_URL ||
  "http://127.0.0.1:4183/docs/phase3e_product_blueprints/review/index.html";
const artifactsDir = path.resolve(directory, "../artifacts");
const viewports = [
  { name: "1440x900", width: 1440, height: 900 },
  { name: "390x844", width: 390, height: 844 },
];
const failures = [];

const browser = await chromium.launch({ headless: true });

try {
  for (const viewport of viewports) {
    const page = await browser.newPage({ viewport });
    page.on("console", message => {
      if (["error", "warning"].includes(message.type())) {
        failures.push(`${viewport.name} console ${message.type()}: ${message.text()}`);
      }
    });
    page.on("pageerror", error => failures.push(`${viewport.name} pageerror: ${error.message}`));
    await page.goto(baseUrl, { waitUntil: "networkidle" });

    const result = await page.evaluate(() => {
      const images = [...document.images].map(image => ({
        src: image.getAttribute("src"),
        loaded: image.complete && image.naturalWidth > 0,
      }));
      const targetIds = [...document.querySelectorAll('a[href^="#"]')]
        .map(link => link.getAttribute("href")?.slice(1))
        .filter(Boolean);
      return {
        overflow:
          Math.max(document.documentElement.scrollWidth, document.body.scrollWidth) -
          window.innerWidth,
        brokenImages: images.filter(image => !image.loaded),
        missingTargets: targetIds.filter(id => !document.getElementById(id)),
      };
    });

    if (result.overflow > 1)
      failures.push(`${viewport.name}: horizontal overflow ${result.overflow}px`);
    if (result.brokenImages.length)
      failures.push(`${viewport.name}: broken images ${JSON.stringify(result.brokenImages)}`);
    if (result.missingTargets.length)
      failures.push(`${viewport.name}: missing anchors ${result.missingTargets.join(", ")}`);

    await page.screenshot({
      path: path.join(artifactsDir, `phase3e_review-board_${viewport.name}.png`),
      fullPage: true,
      type: "png",
    });
    await page.close();
  }
} finally {
  await browser.close();
}

if (failures.length) {
  console.error(failures.map(failure => `- ${failure}`).join("\n"));
  process.exit(1);
}

console.log("Review board QA passed: 2 viewports, all images and section anchors resolved.");
