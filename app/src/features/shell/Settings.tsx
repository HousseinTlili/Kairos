import { approvalWarning, useConnection } from "../connection/store";
import { RulesPage } from "../rules/RulesPage";
import { useShell, type SettingsTab } from "./store";

const TABS: { id: SettingsTab; label: string }[] = [
  { id: "connection", label: "Connection" },
  { id: "rules", label: "Rules" },
];

const MODE_TEXT: Record<string, string> = {
  manual: "Hermes asks you before any command it considers dangerous.",
  smart:
    "Your Kairos rules ask you about destructive, system and outbound actions; Hermes's helper model decides the rest by itself, and Kairos records every such decision.",
  off: "Hermes runs commands without asking.",
};

export function Settings() {
  const { settingsTab, showSettings } = useShell();
  return (
    <section className="settings" aria-labelledby="settings-title">
      <h1 id="settings-title">Settings</h1>
      <div className="tabs" role="tablist">
        {TABS.map((tab) => (
          <button
            key={tab.id}
            role="tab"
            aria-selected={settingsTab === tab.id}
            className={settingsTab === tab.id ? "tab active" : "tab"}
            onClick={() => showSettings(tab.id)}
          >
            {tab.label}
          </button>
        ))}
      </div>
      {settingsTab === "rules" ? <RulesPage /> : <ConnectionSettings />}
    </section>
  );
}

function ConnectionSettings() {
  const { info, checking, check, reloadKey } = useConnection();
  const warning = info ? approvalWarning(info.approval_mode) : null;

  return (
    <>
      <div className="settings-section">
        <h2>Hermes connection</h2>
        <dl className="facts">
          <dt>Status</dt>
          <dd>{info?.status === "connected" ? "Connected" : (info?.detail ?? "Not connected")}</dd>
          <dt>Version</dt>
          <dd>{info?.version ?? "Unknown"}</dd>
          <dt>Address</dt>
          <dd>{info?.base_url ?? "http://127.0.0.1:8642"}</dd>
          <dt>Hermes folder</dt>
          <dd>{info?.hermes_home ?? "Unknown"}</dd>
          <dt>API key</dt>
          <dd>Stored in Windows Credential Manager</dd>
        </dl>
        <div className="actions">
          <button onClick={() => void check()} disabled={checking}>
            {checking ? "Checking…" : "Check connection"}
          </button>
          <button onClick={() => void reloadKey()} disabled={checking}>
            Re-read key from Hermes
          </button>
        </div>
      </div>

      <div className="settings-section">
        <h2>Approvals</h2>
        <p className="setting-value">
          <strong>{info?.approval_mode ?? "Unknown"}</strong>
          {info?.approval_mode && MODE_TEXT[info.approval_mode] && (
            <span className="muted"> — {MODE_TEXT[info.approval_mode]}</span>
          )}
        </p>
        {warning ? (
          <div className="status-card warn" role="alert">
            <p>{warning}</p>
            <code className="command">hermes config set approvals.mode smart</code>
          </div>
        ) : (
          <p className="muted">
            Chosen on 2026-10-25 (decision D-009). Manual mode is the stricter choice: it adds
            Hermes's own prompt on top of your rules, so some commands ask twice.
          </p>
        )}
      </div>

      <div className="settings-section">
        <h2>About</h2>
        <p className="muted">
          Kairos is a personal assistant built on Hermes Agent by Nous Research. Hermes does the
          reasoning and runs the tools; Kairos is your window onto it and, from M2, the rules it
          works within.
        </p>
      </div>
    </>
  );
}
