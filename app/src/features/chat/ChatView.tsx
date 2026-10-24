import { useEffect, useRef, useState, type FormEvent, type KeyboardEvent } from "react";
import { useChat } from "./store";
import { isFinished, type Turn, type TurnItem } from "./turn";

const STATUS_TEXT: Record<Turn["status"], string> = {
  sending: "Sending…",
  running: "Working…",
  waiting_for_approval: "Waiting for your approval",
  completed: "",
  incomplete: "Stopped before finishing (Hermes ran out of steps)",
  cancelled: "Stopped",
  failed: "Failed",
  interrupted: "Interrupted: Hermes shut down during this request",
  error: "Error",
};

function ItemRow({ item }: { item: TurnItem }) {
  if (item.kind === "tool") {
    const label = { running: "running", ok: "done", error: "failed" }[item.state];
    return (
      <li className={`tool-row ${item.state}`}>
        <span className="tool-name">{item.tool}</span>
        <code className="tool-preview">{item.preview}</code>
        <span className="tool-state">{label}</span>
      </li>
    );
  }
  return (
    <li className={`approval-row ${item.state}`}>
      <strong>{item.state === "waiting" ? "Needs your approval" : "Approval answered"}:</strong>{" "}
      {item.description}
      {/* M1 §9 rule 3: the command Hermes will actually run. */}
      <code className="command">{item.command}</code>
      {item.state === "waiting" && (
        <p className="muted">
          Approval buttons arrive in M1 Step 6; until then this run waits here.
        </p>
      )}
    </li>
  );
}

function TurnView({ turn }: { turn: Turn }) {
  const blocked = turn.items.some((item) => item.kind === "tool" && item.state === "error");
  const status =
    turn.status === "completed" && blocked
      ? "Completed — an action was blocked or failed"
      : STATUS_TEXT[turn.status];
  return (
    <article className="turn">
      <p className="user-message">{turn.input}</p>
      {turn.items.length > 0 && (
        <ul className="turn-items">
          {turn.items.map((item, i) => (
            <ItemRow key={i} item={item} />
          ))}
        </ul>
      )}
      {turn.text && <div className="assistant-message">{turn.text}</div>}
      {(status || turn.error) && (
        <p className={`turn-status ${turn.status}`}>
          {status}
          {turn.error ? `: ${turn.error}` : ""}
        </p>
      )}
    </article>
  );
}

export function ChatView() {
  const { turns, send, newConversation } = useChat();
  const [draft, setDraft] = useState("");
  const endRef = useRef<HTMLDivElement>(null);
  const busy = turns.some((turn) => !isFinished(turn.status));

  useEffect(() => {
    endRef.current?.scrollIntoView({ block: "end" });
  }, [turns]);

  function submit(event?: FormEvent) {
    event?.preventDefault();
    if (busy || !draft.trim()) return;
    void send(draft);
    setDraft("");
  }

  function onKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (event.key === "Enter" && !event.shiftKey) submit();
  }

  return (
    <section className="chat">
      <div className="chat-toolbar">
        <button onClick={newConversation} disabled={busy}>
          New conversation
        </button>
      </div>
      <div className="turns">
        {turns.length === 0 && <p className="muted">Ask Kairos something.</p>}
        {turns.map((turn, i) => (
          <TurnView key={turn.taskId ?? `pending-${i}`} turn={turn} />
        ))}
        <div ref={endRef} />
      </div>
      <form className="composer" onSubmit={submit}>
        <textarea
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={onKeyDown}
          placeholder={busy ? "Kairos is working…" : "Message Kairos (Enter to send)"}
          rows={2}
          aria-label="Message"
        />
        <button type="submit" disabled={busy || !draft.trim()}>
          Send
        </button>
      </form>
    </section>
  );
}
