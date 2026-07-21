import React from "react";
import ReactDOM from "react-dom/client";
import { Phase4dReadOnlyHarness } from "./Phase4dReadOnlyHarness";
import "@/ui/foundation/openlife.foundation.css";
import "@/ui/shell/openlife.shell.css";
import "@/ui/journeys/readOnly/readOnlySpine.css";
import "@/ui/journeys/governedAction/governedAction.css";
import "@/ui/journeys/durableTruth/durableTruth.css";
import "./phase4d-harness.css";

declare const __OPENLIFE_PHASE4D_HARNESS__: boolean;

if (!__OPENLIFE_PHASE4D_HARNESS__) {
  throw new Error("The Phase 4D desktop journey harness is unavailable in production builds.");
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <Phase4dReadOnlyHarness />
  </React.StrictMode>
);
