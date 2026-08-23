import React from "react";
import ReactDOM from "react-dom/client";
import { HashRouter } from "react-router-dom";
import App from "./App.tsx";
import "./index.css";
import "@/ui/foundation/openlife.foundation.css";
import "@/ui/shell/openlife.shell.css";
import "@/app/productWorkbench.css";
import "@/app/workbench.css";
import "@/features/personalIntelligence/personalIntelligence.css";
import "@/features/settings/settings.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <HashRouter future={{ v7_startTransition: true, v7_relativeSplatPath: true }}>
      <App />
    </HashRouter>
  </React.StrictMode>
);
