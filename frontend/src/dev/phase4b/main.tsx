import React from "react";
import ReactDOM from "react-dom/client";
import { FoundationHarness } from "./FoundationHarness";
import "@/ui/foundation/openlife.foundation.css";
import "./phase4b-harness.css";

declare const __OPENLIFE_PHASE4B_HARNESS__: boolean;

if (!__OPENLIFE_PHASE4B_HARNESS__) {
  throw new Error("The Phase 4B foundation harness is unavailable in production builds.");
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <FoundationHarness />
  </React.StrictMode>
);
