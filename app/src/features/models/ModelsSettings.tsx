import { useEffect, useState } from "react";
import { terminalBackend, type CommandError } from "../../lib/tauri";
import { sandboxText } from "./model";

export function ModelsSettings() {
  const [backend, setBackend] = useState<string | null | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    terminalBackend()
      .then(setBackend)
      .catch((err: CommandError) => setError(err.message ?? String(err)));
  }, []);

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
    </>
  );
}
