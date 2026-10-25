import { useEffect, useState } from "react";
import { costCap, setCostCap, terminalBackend, type CommandError } from "../../lib/tauri";
import { sandboxText } from "./model";

export function ModelsSettings() {
  const [backend, setBackend] = useState<string | null | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    terminalBackend()
      .then(setBackend)
      .catch((err: CommandError) => setError(err.message ?? String(err)));
  }, []);

  const [cap, setCap] = useState<number | null | undefined>(undefined);
  const [capDraft, setCapDraft] = useState<string | null>(null);

  useEffect(() => {
    costCap()
      .then(setCap)
      .catch(() => setCap(null));
  }, []);

  const saveCap = (text: string) => {
    const value = Number(text.replace(",", "."));
    const next = text.trim() === "" || !(value > 0) ? null : value;
    setCostCap(next)
      .then(setCap)
      .catch((err: CommandError) => setError(err.message ?? String(err)));
    setCapDraft(null);
  };

  const sandbox = backend === undefined ? null : sandboxText(backend);

  return (
    <>
      <div className="settings-section">
        <h2>Where commands run</h2>
        {sandbox ? (
          <p
            className={
              sandbox.tone === "ok" ? "ok-text" : sandbox.tone === "warn" ? "warn-text" : "muted"
            }
          >
            {sandbox.text}
          </p>
        ) : (
          <p className="muted">{error ?? "Asking Hermes…"}</p>
        )}
        <p className="muted small">
          To run the agent&apos;s commands in a container (Docker Desktop with WSL2), run this in a
          terminal from the Kairos folder; it asks first and can be undone with -Undo:
        </p>
        <code className="command">
          powershell -NoProfile -ExecutionPolicy Bypass -File scripts/setup-container.ps1
        </code>
      </div>

      <div className="settings-section">
        <h2>Models</h2>
        <p className="muted small">
          Pick the model for each conversation in the menu above the message box; &quot;Hermes
          default&quot; uses Hermes&apos;s own choice and its fallback providers (set them up with{" "}
          <code>hermes fallback add</code>). A local model downloaded in Hermes Desktop&apos;s Local
          Models appears in the same menu. Under each answer, Kairos shows the model that actually
          served it, so a fallback is visible.
        </p>
      </div>

      <div className="settings-section">
        <h2>Cost</h2>
        <label className="cap-row">
          <span>Stop a request that costs more than $</span>
          <input
            type="text"
            inputMode="decimal"
            placeholder="no limit"
            value={capDraft ?? (cap != null ? String(cap) : "")}
            disabled={cap === undefined}
            onChange={(e) => setCapDraft(e.target.value)}
            onBlur={(e) => saveCap(e.target.value)}
          />
        </label>
        <p className="muted small">
          Costs are estimates from Hermes&apos;s own price list. Kairos checks a running request
          every 10 seconds and stops it when it passes the limit. Leave empty for no limit.
        </p>
      </div>
    </>
  );
}
