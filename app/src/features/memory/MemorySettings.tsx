import { useEffect, useState } from "react";
import type { MemoryFile } from "../../lib/tauri";
import { useChat } from "../chat/store";
import { useConnection } from "../connection/store";
import { useShell } from "../shell/store";
import { FORGET_SESSION, categoryLabel, fullness, type Forgettable } from "./model";
import { keyOf, useMemory } from "./store";

/** One item with its Forget button, which asks first. */
function Item({ item, children }: { item: Forgettable; children: React.ReactNode }) {
  const { forgetting, forget } = useMemory();
  const connected = useConnection((s) => s.info?.status === "connected");
  const [confirm, setConfirm] = useState(false);
  const busy = forgetting === keyOf(item);
  return (
    <li className="memory-item">
      <div className="memory-text">{children}</div>
      <div className="memory-actions">
        {busy ? (
          <span className="muted small">Forgetting…</span>
        ) : confirm ? (
          <>
            <button
              className="deny"
              onClick={() => {
                setConfirm(false);
                void forget(item);
              }}
            >
              Forget
            </button>
            <button onClick={() => setConfirm(false)}>Keep</button>
          </>
        ) : (
          <button
            className="link"
            onClick={() => setConfirm(true)}
            disabled={!connected || forgetting !== null}
            title={connected ? undefined : "Hermes isn't connected"}
          >
            Forget
          </button>
        )}
      </div>
    </li>
  );
}

function FileSection({
  title,
  about,
  file,
  kind,
  empty,
}: {
  title: string;
  about: string;
  file: MemoryFile;
  kind: "user" | "notes";
  empty: string;
}) {
  return (
    <div className="settings-section">
      <h2>{title}</h2>
      <p className="muted small">
        {about} {fullness(file.used, file.limit)}.
      </p>
      {file.entries.length === 0 ? (
        <p className="muted">{empty}</p>
      ) : (
        <ul className="memory-list">
          {file.entries.map((text) => (
            <Item key={text} item={{ kind, text }}>
              {text}
            </Item>
          ))}
        </ul>
      )}
    </div>
  );
}

/** Settings ▸ Memory (M11, D-026): what Kairos remembers, read-only, each item forgettable. */
export function MemorySettings() {
  const { state, error, stalled, load } = useMemory();
  const openConversation = useChat((s) => s.openConversation);
  const show = useShell((s) => s.show);

  useEffect(() => {
    void load();
  }, [load]);

  if (!state) return <p className="muted">{error ?? "Reading what Kairos remembers…"}</p>;

  return (
    <>
      <div className="settings-section">
        <h2>What Kairos remembers</h2>
        <p className="muted small">
          Everything Hermes keeps about you between conversations. Kairos never shares it, and
          forgetting an item asks Kairos to remove it with its own memory tools.{" "}
          {state.provider
            ? `Long-term facts are kept by the "${state.provider}" provider, on this PC.`
            : "Only Hermes's built-in memory is on."}
        </p>
        <div className="actions">
          <button onClick={() => void load()}>Refresh</button>
        </div>
        {error && <p className="bad-text">{error}</p>}
        {stalled && (
          <p className="warn-text small">
            Kairos hasn&apos;t finished forgetting yet; it may be waiting for your answer.{" "}
            <button
              className="link"
              onClick={() => {
                show("chat");
                void openConversation(FORGET_SESSION);
              }}
            >
              Open that conversation
            </button>
          </p>
        )}
      </div>

      <FileSection
        title="About you"
        about="Your profile: who you are and what you prefer."
        file={state.user}
        kind="user"
        empty="Nothing yet. Tell Kairos something lasting about yourself and it will keep it here."
      />
      <FileSection
        title="Kairos's notes"
        about="What Kairos noted for itself: how things are done here."
        file={state.notes}
        kind="notes"
        empty="No notes yet."
      />

      <div className="settings-section">
        <h2>Facts</h2>
        {state.facts === null ? (
          <p className="muted">
            {state.provider
              ? "No facts stored yet; the store is created with the first one."
              : "Turn on a memory provider to keep long-term facts."}
          </p>
        ) : state.facts.length === 0 ? (
          <p className="muted">No facts stored yet.</p>
        ) : (
          <ul className="memory-list">
            {state.facts.map((fact) => (
              <Item key={fact.id} item={{ kind: "fact", fact }}>
                <span>{fact.content}</span>
                <span className="memory-meta muted small">
                  {categoryLabel(fact.category)}, trusted {Math.round(fact.trust * 100)} %,{" "}
                  {fact.updated_at.slice(0, 10)}
                </span>
              </Item>
            ))}
          </ul>
        )}
      </div>
    </>
  );
}
