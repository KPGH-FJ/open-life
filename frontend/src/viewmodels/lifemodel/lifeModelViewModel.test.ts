import { readdirSync, readFileSync } from "node:fs";
import { basename, join } from "node:path";
import { describe, expect, it } from "vitest";

describe("lifeModelViewModel backend ownership", () => {
  it("keeps the frontend adapter as a backend command delegate", () => {
    const adapterSource = readFileSync(
      join(process.cwd(), "src/viewmodels/lifemodel/lifeModelViewModelAdapter.ts"),
      "utf8"
    );

    expect(adapterSource).toContain("getLifeModelViewModel");
    expect(adapterSource).not.toContain("buildLifeModelViewModelEnvelope");
    expect(adapterSource).not.toContain("BuildLifeModelViewModelInput");
  });

  it("keeps the frontend contract as a tauri mirror, not a local owner", () => {
    const contractSource = readFileSync(
      join(process.cwd(), "src/viewmodels/lifemodel/lifeModelViewModel.ts"),
      "utf8"
    );

    expect(contractSource).toContain("openlife-core/src/agent/life_model_view_model.rs");
    expect(contractSource).toContain("LifeModelViewModelContract");
    expect(contractSource).not.toContain("export type BuildLifeModelViewModelInput");
  });

  it("keeps the production durable-truth owner off raw LifeModel reconstruction commands", () => {
    const source = readFileSync(
      join(process.cwd(), "src/ui/journeys/durableTruth/durableTruthDataSource.ts"),
      "utf8"
    );
    const forbiddenCalls = [
      ["get", "Life", "Model", "("].join(""),
      ["get", "Life", "Model", "Current", "View", "("].join(""),
      ["get", "System", "Diagnostics", "("].join(""),
      ["get", "Model", "4D", "Completion", "("].join(""),
      ["count", "Memory", "Chunks", "("].join(""),
      ["get", "Memory", "Tier", "Stats", "("].join(""),
      ["list", "Proposals", "("].join(""),
    ];

    expect(source).toContain("getLifeModelViewModel");
    for (const forbidden of forbiddenCalls) {
      expect(source, `durable truth data source should not call ${forbidden}`).not.toContain(
        forbidden
      );
    }
  });

  it("keeps forbidden bridge and write symbols out of the LifeModel ViewModel package", () => {
    const forbiddenSymbols = [
      ["get", "System", "Diagnostics"].join(""),
      ["save", "Life", "Model"].join(""),
      ["accept", "Proposal"].join(""),
      ["batch", "Accept", "Low", "Risk", "Proposals"].join(""),
      ["edit", "Proposal"].join(""),
      ["reject", "Proposal"].join(""),
      ["postpone", "Proposal"].join(""),
      ["tauri", "Dev"].join(""),
      ["safe", "Invoke"].join(""),
      ["invoke", "("].join(""),
    ];
    const packageDir = join(process.cwd(), "src/viewmodels/lifemodel");
    const sources = readdirSync(packageDir)
      .filter(fileName => fileName.endsWith(".ts"))
      .map(fileName => ({
        fileName: basename(fileName),
        source: readFileSync(join(packageDir, fileName), "utf8"),
      }));

    for (const { fileName, source } of sources) {
      for (const symbol of forbiddenSymbols) {
        expect(source, `${fileName} should not contain ${symbol}`).not.toContain(symbol);
      }
    }
  });
});
