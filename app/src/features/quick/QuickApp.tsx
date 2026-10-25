import { useEffect, useRef, useState, type FormEvent, type KeyboardEvent } from "react";
import { hideQuick, onQuickShown, onRunEvent, openInMain, showQuick } from "../../lib/tauri";
import { TurnView } from "../chat/ChatView";
import { startChatEvents, useChat } from "../chat/store";
import { useConnection } from "../connection/store";
import { hidesForCapture, quickSessionKey } from "./model";

function rememberedSession(): string | null {
  try {
    return localStorage.getItem(quickSessionKey());
  } catch {
    return null;
  }
}

function rememberSession(id: string) {
  try {
    localStorage.setItem(quickSessionKey(), id);
  } catch {
    // A private profile without storage just starts a new quick conversation next time.
  }
}

/** The quick box window (M8 §3): one question at a time, the latest answer under it. */
export function QuickApp() {
  const { turns, sessionId, send, openConversation } = useChat();
  const { info, check } = useConnection();
  const [draft, setDraft] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const connected = info?.status === "connected";
  const busy = turns.some((t) =>
    ["sending", "running", "waiting_for_approval", "stopping"].includes(t.status),
  );
  const last = turns[turns.length - 1];

  useEffect(() => {
    const stops = [
      startChatEvents(),
      onQuickShown(() => inputRef.current?.focus()),
      // Out of the picture while the screen is captured (M8 §4).
      onRunEvent(({ envelope }) => {
        const event = envelope.event;
        if (event.event === "tool.started" && hidesForCapture(event.tool)) void hideQuick();
        if (event.event === "tool.completed" && hidesForCapture(event.tool)) void showQuick();
      }),
    ];
    void check();
    return () => {
      for (const stop of stops) void stop.then((unlisten) => unlisten());
    };
  }, [check]);

  useEffect(() => {
    const remembered = rememberedSession();
    if (connected && remembered && !sessionId) void openConversation(remembered);
  }, [connected, sessionId, openConversation]);

  useEffect(() => {
    if (sessionId) rememberSession(sessionId);
  }, [sessionId]);

  function submit(event?: FormEvent) {
    event?.preventDefault();
    if (!draft.trim() || busy || !connected) return;
    void send(draft);
    setDraft("");
  }

  function onKeyDown(event: KeyboardEvent) {
    if (event.key === "Escape") void hideQuick();
  }

  return (
    <div className="quick" onKeyDown={onKeyDown}>
      <form className="quick-input" onSubmit={submit}>
        <input
          ref={inputRef}
          autoFocus
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          placeholder={connected ? "Ask Kairos…" : "Hermes isn't connected"}
          disabled={!connected}
          aria-label="Ask Kairos"
        />
      </form>
      <div className="quick-answer">
        {last ? (
          <TurnView turn={last} />
        ) : (
          <p className="muted small">
            Ask anything, or &quot;what&apos;s on my screen?&quot;. Esc to hide.
          </p>
        )}
      </div>
      <div className="quick-footer">
        {sessionId && (
          <button className="link" onClick={() => void openInMain(sessionId)}>
            Open in Kairos
          </button>
        )}
        <span className="muted small">Esc to hide</span>
      </div>
    </div>
  );
}
