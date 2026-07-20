import React from "react";
import ReactDOM from "react-dom/client";
import { DesktopShellHarness } from "./DesktopShellHarness";
import "@/ui/foundation/openlife.foundation.css";
import "@/ui/shell/openlife.shell.css";
import "./phase4c-harness.css";

declare const __OPENLIFE_PHASE4C_HARNESS__: boolean;

if (!__OPENLIFE_PHASE4C_HARNESS__) {
  throw new Error("The Phase 4C desktop shell harness is unavailable in production builds.");
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <DesktopShellHarness />
  </React.StrictMode>
);
