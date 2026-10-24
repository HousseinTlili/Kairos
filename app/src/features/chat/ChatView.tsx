import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type FormEvent,
  type KeyboardEvent,
} from "react";
import { useConnection } from "../connection/store";
import { LockIcon, SendIcon, StopIcon, TerminalIcon } from "../shell/icons";
import { useShell } from "../shell/store";
import { ApprovalCard } from "./ApprovalCard";
import { MarkdownView } from "./MarkdownView";
import { useChat } from "./store";
import { blockedCount, decisionText, isFinished, toolLabel, type Turn } from "./turn";

const SUGGESTIONS = [
  "What can you help me with?",
  "List the 10 largest files in my Downloads folder",
  "Explain what Hermes's manual approval mode protects me from",
  "Check which version of Node.js and Rust I have installed",
];

function greeting(now: Date): string {
  const hour = now.getHours();
  if (hour < 5) return "Working late";
  if (hour < 12) return "Good morning";
  if (hour < 18) return "Good afternoon";
  return "Good evening";
}

function statusLine(turn: Turn): { text: string; tone: "muted" | "bad" } | null {
  const blocked = blockedCount(turn);
  switch (turn.status) {
    case "sending":
      return { text: "Sending…", tone: "muted" };
    case "running":
      return { text: "Working…", tone: "muted" };
    case "waiting_for_approval":
      return { text: "Waiting for your approval", tone: "muted" };
    case "stopping":
      return { text: "Stopping…", tone: "muted" };
    case "completed":
      return blocked > 0
        ? {
            text: `Completed, ${blocked === 1 ? "1 action" : `${blocked} actions`} blocked`,
            tone: "muted",
          }
        : null;
    case "incomplete":
      return { text: "Stopped before finishing: Hermes ran out of steps.", tone: "muted" };
    case "cancelled":
      return { text: "Stopped", tone: "muted" };
    case "failed":
      return { text: `Failed: ${turn.error ?? "Hermes reported a failure."}`, tone: "bad" };
    case "interrupted":
      return { text: "Interrupted: Hermes shut down during this request.", tone: "bad" };
    case "unknown":
      return {
        text: `Outcome unknown${turn.error ? `: ${turn.error}` : ""}. Kairos won't resend it.`,
        tone: "bad",
      };
    case "lost":
      return { text: `${turn.error ?? "Hermes no longer knows this request."}`, tone: "bad" };
    case "error":
      return { text: turn.error ?? "Something went wrong.", tone: "bad" };
  }
}

function TurnView({ turn }: { turn: Turn }) {
  const answer = useChat((s) => s.answer);
  const status = statusLine(turn);
  const working = turn.status === "running" || turn.status === "sending";

  return (
    <article className="turn">
      <div className="user-message">{turn.input}</div>
      <div className="assistant">
        {turn.items.length > 0 && (
          <ul className="activity">
            {turn.items.map((item, i) =>
              item.kind === "tool" ? (
                <li key={i} className={`tool-row ${toolLabel(turn, item)}`}>
                  <TerminalIcon />
                  <span className="tool-name">{item.tool}</span>
                  <code className="tool-preview" title={item.preview}>
                    {item.preview}
                  </code>
                  <span className="tool-state">{toolLabel(turn, item)}</span>
                  {decisionText(item) && (
                    <span className={`tool-decision ${item.audit?.decision ?? ""}`}>
                      {decisionText(item)}
                    </span>
                  )}
                </li>
              ) : (
                <li key={i}>
                  <ApprovalCard
                    row={item}
                    onAnswer={(choice) =>
                      turn.taskId && void answer(turn.taskId, item.promptId, choice)
                    }
                  />
                </li>
              ),
            )}
          </ul>
        )}
        {turn.text ? (
          <MarkdownView text={turn.text} />
        ) : (
          working && <span className="thinking" aria-label="Kairos is working" />
        )}
        {status && <p className={`turn-status ${status.tone}`}>{status.text}</p>}
      </div>
    </article>
  );
}

function EmptyState({ onPick }: { onPick: (text: string) => void }) {
  return (
    <div className="empty-state">
      <h1 className="greeting">{greeting(new Date())}.</h1>
      <p className="muted">What should we get done?</p>
      <div className="suggestions">
        {SUGGESTIONS.map((text) => (
          <button key={text} className="suggestion" onClick={() => onPick(text)}>
            {text}
          </button>
        ))}
      </div>
    </div>
  );
}

function PeekView({ id }: { id: string }) {
  const other = useChat((s) => s.conversations.other.find((c) => c.id === id));
  const show = useShell((s) => s.show);
  return (
    <div className="empty-state peek">
      <LockIcon />
      <h1 className="peek-title">{other?.title ?? "Hermes conversation"}</h1>
      <p className="muted">
        This conversation belongs to Hermes{other?.source === "local" ? " Desktop" : ""}. Hermes
        lets each app continue only its own conversations, so open it there to keep going.
      </p>
      {other && (
        <p className="muted small">
          {other.message_count} messages{other.model ? `, ${other.model}` : ""}
        </p>
      )}
      <button onClick={() => show("chat")}>Back to Kairos</button>
    </div>
  );
}

export function ChatView() {
  const { turns, sessionId, conversations, notice, send, stop } = useChat();
  const peekId = useShell((s) => s.peekId);
  const approvalMode = useConnection((s) => s.info?.approval_mode);
  const [draft, setDraft] = useState("");
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  const active = turns.find((turn) => !isFinished(turn.status));
  const title =
    conversations.kairos.find((c) => c.session_id === sessionId)?.title ??
    turns[0]?.input ??
    "New conversation";

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight });
  }, [turns]);

  // Grow the composer with its text, up to a limit.
  useLayoutEffect(() => {
    const el = inputRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 200)}px`;
  }, [draft]);

  useEffect(() => {
    inputRef.current?.focus();
  }, [sessionId]);

  if (peekId) return <PeekView id={peekId} />;

  function submit(event?: FormEvent) {
    event?.preventDefault();
    if (active || !draft.trim()) return;
    void send(draft);
    setDraft("");
  }

  function onKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      submit();
    }
  }

  return (
    <section className="chat" aria-label="Conversation">
      <header className="chat-header">
        <h1 className="chat-title" title={title}>
          {title}
        </h1>
        {approvalMode && (
          <span
            className={`mode-badge ${approvalMode === "manual" ? "" : "warn"}`}
            title="Hermes approval mode"
          >
            Approvals: {approvalMode}
          </span>
        )}
      </header>

      <div className="chat-scroll" ref={scrollRef}>
        <div className="chat-column">
          {turns.length === 0 ? (
            <EmptyState onPick={(text) => void send(text)} />
          ) : (
            turns.map((turn, i) => <TurnView key={turn.taskId ?? `pending-${i}`} turn={turn} />)
          )}
        </div>
      </div>

      <div className="composer-wrap">
        {notice && (
          <p className="notice" role="status">
            {notice}
          </p>
        )}
        <form className="composer" onSubmit={submit}>
          <textarea
            ref={inputRef}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={onKeyDown}
            placeholder={active ? "Kairos is working…" : "Message Kairos"}
            rows={1}
            aria-label="Message"
          />
          {active ? (
            <button
              type="button"
              className="composer-button stop"
              onClick={() => active.taskId && void stop(active.taskId)}
              disabled={!active.taskId || active.status === "stopping"}
              aria-label="Stop"
              title="Stop"
            >
              <StopIcon />
            </button>
          ) : (
            <button
              type="submit"
              className="composer-button"
              disabled={!draft.trim()}
              aria-label="Send"
              title="Send (Enter)"
            >
              <SendIcon />
            </button>
          )}
        </form>
        <p className="composer-hint">Enter to send, Shift+Enter for a new line</p>
      </div>
    </section>
  );
}
