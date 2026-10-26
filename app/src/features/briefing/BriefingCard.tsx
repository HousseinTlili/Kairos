import { useConnection } from "../connection/store";
import { useVoice } from "../voice/store";
import { useBriefing } from "./store";

/** The welcome-back card above the chat (M5 §2.2): the instant facts, then the AI summary. */
export function BriefingCard() {
  const { briefing, open, summaryStarted, dismiss, startSummary } = useBriefing();
  const connected = useConnection((s) => s.info?.status === "connected");
  const { speaking, speak, stopSpeaking } = useVoice();
  if (!briefing || !open) return null;
  // Read only when asked (M9 §3): the facts, as one short spoken paragraph.
  const spoken =
    briefing.facts.length === 0
      ? "Welcome back. Nothing needs your attention."
      : `Welcome back. ${briefing.facts.map((f) => f.text.replace(/[.\s]*$/, ".")).join(" ")}`;

  return (
    <section className="briefing-card" aria-label="Welcome back">
      <header>
        <h2 className="illuminated small" aria-label="Welcome back, Master">
          <span className="initial" aria-hidden="true">
            W
          </span>
          <span aria-hidden="true">elcome back, Master</span>
        </h2>
        <span className="muted small">
          {new Date(briefing.at * 1000).toLocaleTimeString([], {
            hour: "2-digit",
            minute: "2-digit",
          })}
        </span>
      </header>
      {briefing.facts.length === 0 ? (
        <p className="muted">Nothing needs your attention.</p>
      ) : (
        <ul>
          {briefing.facts.map((fact, i) => (
            <li key={i} className={fact.tone === "warn" ? "warn" : undefined}>
              {fact.text}
            </li>
          ))}
        </ul>
      )}
      <div className="actions">
        {!summaryStarted && (
          <button
            onClick={() => void startSummary()}
            disabled={!connected}
            title={connected ? undefined : "Hermes isn't connected"}
          >
            Summarise with AI
          </button>
        )}
        <button
          onClick={() => (speaking === "briefing" ? stopSpeaking() : speak("briefing", spoken))}
        >
          {speaking === "briefing" ? "Stop reading" : "Read it to me"}
        </button>
        <button className="link" onClick={dismiss}>
          Dismiss
        </button>
      </div>
    </section>
  );
}
