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
import { BriefingCard } from "../briefing/BriefingCard";
import { ApprovalCard } from "./ApprovalCard";
import { MarkdownView } from "./MarkdownView";
import { useChat } from "./store";
import {
  blockedCount,
  conversationCost,
  decisionText,
  isFinished,
  plainEnding,
  toolLabel,
  type Turn,
} from "./turn";
import { ModelPicker } from "../models/ModelPicker";
import { usageLine } from "../models/model";
import { useVoiceInChat } from "../voice/hooks";
import { useVoice } from "../voice/store";
import { MicButton, SpeakButton, VoiceStatus } from "../voice/VoiceControls";

// Things Kairos does on this PC, so the first conversation shows what it is for.
const SUGGESTIONS = [
  "Which apps are using the most memory right now?",
  "Summarise what I missed while I was away",
  "List the 10 largest files in my Downloads folder",
  "Which of my actions do you ask me about, and why?",
];

function greeting(now: Date): string {
  const hour = now.getHours();
  if (hour < 5) return "You keep late hours, Master";
  if (hour < 12) return "Good morning, Master";
  if (hour < 18) return "Good afternoon, Master";
  return "Good evening, Master";
}

/** Kairos's line with an illuminated initial (BRAND.md, Ornament 2). */
export function Illuminated({ text, className }: { text: string; className?: string }) {
  return (
    <h1 className={`illuminated ${className ?? ""}`} aria-label={text}>
      <span className="initial" aria-hidden="true">
        {text.slice(0, 1)}
      </span>
      <span aria-hidden="true">{text.slice(1)}</span>
    </h1>
  );
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
      // An ending Hermes gave as a code is already shown in words in the answer's place.
      return turn.text && plainEnding(turn.text) !== turn.text
        ? null
        : { text: "Stopped", tone: "muted" };
    case "failed":
      return {
        text: `Failed: ${turn.error ? plainEnding(turn.error) : "Hermes reported a failure."}`,
        tone: "bad",
      };
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
      return { text: turn.error ? plainEnding(turn.error) : "Something went wrong.", tone: "bad" };
  }
}

export function TurnView({ turn }: { turn: Turn }) {
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
        {turn.text && plainEnding(turn.text) !== turn.text ? (
          <p className="turn-status muted">{plainEnding(turn.text)}</p>
        ) : turn.text ? (
          <MarkdownView text={turn.text} />
        ) : (
          working && <span className="thinking" aria-label="Kairos is working" />
        )}
        {status && <p className={`turn-status ${status.tone}`}>{status.text}</p>}
        {turn.costCap && (
          <p className="turn-status bad">
            Kairos stopped this request: it passed your cost cap of ${turn.costCap.cap.toFixed(2)}{" "}
            (≈ ${turn.costCap.cost.toFixed(4)} so far).
          </p>
        )}
        {turn.taskId &&
          turn.text &&
          plainEnding(turn.text) === turn.text &&
          !working &&
          isFinished(turn.status) && <SpeakButton id={turn.taskId} text={turn.text} />}
        {turn.usage && <p className="turn-usage">{usageLine(turn.usage)}</p>}
      </div>
    </article>
  );
}

function EmptyState({ onPick }: { onPick: (text: string) => void }) {
  return (
    <div className="empty-state">
      <Illuminated className="greeting" text={`${greeting(new Date())}.`} />
      <p className="kairos-says">What is your will?</p>
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
  const stopSpeaking = useVoice((s) => s.stopSpeaking);
  useVoiceInChat(turns);
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

  const spent = conversationCost(turns);

  if (peekId) return <PeekView id={peekId} />;

  function submit(event?: FormEvent) {
    event?.preventDefault();
    if (active || !draft.trim()) return;
    stopSpeaking();
    void send(draft);
    setDraft("");
  }

  function sendSpoken(text: string) {
    if (active) {
      setDraft(text);
      return;
    }
    stopSpeaking();
    void send(text);
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
        {spent.total > 0 && (
          <span
            className="cost-badge"
            title={
              spent.unknown > 0
                ? `Estimate; ${spent.unknown} request(s) had no price`
                : "Estimated cost of this conversation"
            }
          >
            ≈ ${spent.total < 0.01 ? spent.total.toPrecision(2) : spent.total.toFixed(2)}
          </span>
        )}
        {approvalMode && (
          <span
            className={`mode-badge ${approvalMode === "manual" || approvalMode === "smart" ? "" : "warn"}`}
            title="Hermes approval mode"
          >
            Approvals: {approvalMode}
          </span>
        )}
      </header>

      <div className="chat-scroll" ref={scrollRef}>
        <div className="chat-column">
          {/* Kairos speaks first when you come back: the briefing opens the conversation. */}
          <BriefingCard />
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
        <ModelPicker disabled={Boolean(active)} />
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
            <>
              <MicButton onText={sendSpoken} />
              <button
                type="submit"
                className="composer-button"
                disabled={!draft.trim()}
                aria-label="Send"
                title="Send (Enter)"
              >
                <SendIcon />
              </button>
            </>
          )}
        </form>
        <VoiceStatus />
        <p className="composer-hint">Enter to send, Shift+Enter for a new line</p>
      </div>
    </section>
  );
}
