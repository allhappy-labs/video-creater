import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { AppErrorBoundary } from "./components/runtime/app-error-boundary";
import "./index.css";
import { bootstrapRuntime } from "./lib/runtime/bootstrap";
import { runPackagedSettingsAcceptanceIfEnabled } from "./lib/settings-acceptance-enablement";

async function main() {
  const runtime = await bootstrapRuntime();
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <AppErrorBoundary onRecover={() => window.location.reload()}>
        <App runtime={runtime} />
      </AppErrorBoundary>
    </React.StrictMode>,
  );

  if (runtime.mode !== "desktop") return;
  window.setTimeout(() => {
    void import("./lib/runtime/adapters/tauri-settings-acceptance-bridge")
      .then(({ createTauriSettingsAcceptanceBridge }) =>
        runPackagedSettingsAcceptanceIfEnabled(createTauriSettingsAcceptanceBridge()),
      )
      .catch(() => {
        console.error("Packaged Settings acceptance failed");
      });
  }, 0);
}

void main();
