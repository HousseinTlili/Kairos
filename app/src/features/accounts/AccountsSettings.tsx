import { useEffect, useState } from "react";
import type { AccountKind } from "../../lib/tauri";
import { useConnection } from "../connection/store";
import { ACCOUNT_NAME, ACCOUNT_SIGN_IN, disconnectQuestion, statusText } from "./model";
import { useAccounts } from "./store";

export function AccountsSettings() {
  const { accounts, busy, error, load, connect, disconnect } = useAccounts();
  const connected = useConnection((s) => s.info?.status === "connected");
  const [confirm, setConfirm] = useState<AccountKind | null>(null);

  useEffect(() => {
    void load();
  }, [load]);

  if (!accounts) return <p className="muted">{error ?? "Checking your accounts…"}</p>;

  return (
    <>
      <div className="settings-section">
        <h2>Accounts</h2>
        <p className="muted small">
          Kairos never asks for your passwords. Each account connects through its own official
          sign-in, reading is allowed, and sending, replying, deleting or changing anything always
          asks you first.
        </p>
        <ul className="account-list">
          {accounts.map((account) => (
            <li key={account.kind}>
              <div className="account-head">
                <strong>{ACCOUNT_NAME[account.kind]}</strong>
                <span
                  className={
                    account.status === "connected"
                      ? "ok-text"
                      : account.status === "unavailable"
                        ? "warn-text"
                        : "muted"
                  }
                >
                  {statusText(account)}
                </span>
              </div>
              <p className="muted small">{ACCOUNT_SIGN_IN[account.kind]}</p>
              <div className="actions">
                {account.status === "not_connected" && (
                  <button
                    onClick={() => void connect(account.kind)}
                    disabled={busy !== null || (account.kind === "google" && !connected)}
                    title={
                      account.kind === "google" && !connected ? "Hermes isn't connected" : undefined
                    }
                  >
                    Connect
                  </button>
                )}
                {account.status === "connected" &&
                  (confirm === account.kind ? (
                    <>
                      <span className="muted small">{disconnectQuestion(account.kind)}</span>
                      <button
                        className="deny"
                        onClick={() => {
                          setConfirm(null);
                          void disconnect(account.kind);
                        }}
                      >
                        Disconnect
                      </button>
                      <button onClick={() => setConfirm(null)}>Keep</button>
                    </>
                  ) : (
                    <button onClick={() => setConfirm(account.kind)} disabled={busy !== null}>
                      Disconnect
                    </button>
                  ))}
              </div>
            </li>
          ))}
        </ul>
        <div className="actions">
          <button onClick={() => void load()} disabled={busy !== null}>
            Check again
          </button>
        </div>
        <p className="muted small">
          Outlook isn&apos;t available yet: Hermes has no Microsoft sign-in, and Kairos won&apos;t
          use a password instead.
        </p>
      </div>
      {error && <p className="bad-text">{error}</p>}
    </>
  );
}
