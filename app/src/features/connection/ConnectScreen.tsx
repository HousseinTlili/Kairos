import { useEffect } from "react";
import type { ConnectInfo } from "../../lib/tauri";
import { approvalWarning, useConnection } from "./store";

interface StatusView {
  tone: "ok" | "bad";
  title: string;
  body: string;
  command?: string;
}

function describe(info: ConnectInfo): StatusView {
  switch (info.status) {
    case "connected":
      return {
        tone: "ok",
        title: "Connected to Hermes",
        body: `Hermes ${info.version ?? "(unknown version)"} at ${info.base_url}.`,
      };
    case "not_running":
      return {
        tone: "bad",
        title: "Hermes isn't running",
        body: `Nothing answered at ${info.base_url}. Start the Hermes gateway, then check again.`,
        command: "hermes gateway restart",
      };
    case "key_rejected":
      return {
        tone: "bad",
        title: "Key rejected",
        body: "Hermes is running but didn't accept the stored API key. If the key in Hermes's .env changed, re-read it.",
      };
    case "capability_missing":
      return {
        tone: "bad",
        title: "This Hermes is missing features Kairos needs",
        body: `Missing: ${info.missing_capabilities.join(", ")}. Kairos was tested with Hermes 0.22.5.`,
      };
    case "no_key":
      return {
        tone: "bad",
        title: "No API key",
        body: info.detail ?? "Run scripts/setup.ps1 to enable Hermes's API server.",
        command: "powershell -NoProfile -ExecutionPolicy Bypass -File scripts/setup.ps1",
      };
  }
}

export function ConnectScreen() {
  const { info, error, checking, check, reloadKey } = useConnection();

  useEffect(() => {
    void check();
  }, [check]);

  if (error) {
    return (
      <section className="connect">
        <div className="status-card bad" role="alert">
          <h2>Couldn't check Hermes</h2>
          <p>{error.message}</p>
        </div>
        <button onClick={() => void check()} disabled={checking}>
          {checking ? "Checking…" : "Try again"}
        </button>
      </section>
    );
  }

  if (!info) {
    return (
      <section className="connect">
        <p className="muted">Looking for Hermes…</p>
      </section>
    );
  }

  const view = describe(info);
  const warning = info.status === "connected" ? approvalWarning(info.approval_mode) : null;

  return (
    <section className="connect">
      <div className={`status-card ${view.tone}`} role={view.tone === "bad" ? "alert" : "status"}>
        <h2>
          <span className="dot" aria-hidden="true" />
          {view.title}
        </h2>
        <p>{view.body}</p>
        {view.command && <code className="command">{view.command}</code>}
        {info.status !== "connected" && info.detail && info.status !== "no_key" && (
          <p className="detail">{info.detail}</p>
        )}
      </div>

      {warning && (
        <div className="status-card warn" role="alert">
          <h2>Approval mode</h2>
          <p>{warning}</p>
          <code className="command">hermes config set approvals.mode manual</code>
        </div>
      )}

      {info.status === "connected" && (
        <dl className="facts">
          <dt>Approval mode</dt>
          <dd>{info.approval_mode ?? "unknown"}</dd>
          <dt>Key</dt>
          <dd>
            {info.key_source === "env"
              ? "Read from Hermes's .env and saved to Windows Credential Manager"
              : "Windows Credential Manager"}
          </dd>
          <dt>Hermes home</dt>
          <dd>{info.hermes_home ?? "unknown"}</dd>
        </dl>
      )}

      <div className="actions">
        <button onClick={() => void check()} disabled={checking}>
          {checking ? "Checking…" : "Check again"}
        </button>
        {info.status === "key_rejected" && (
          <button onClick={() => void reloadKey()} disabled={checking}>
            Re-read key from .env
          </button>
        )}
      </div>
    </section>
  );
}
