import { useConnection } from "../connection/store";
import { useBriefing } from "./store";

/** The welcome-back card above the chat (M5 §2.2): the instant facts, then the AI summary. */
export function BriefingCard() {
  const { briefing, open, summaryStarted, dismiss, startSummary } = useBriefing();
  const connected = useConnection((s) => s.info?.status === "connected");
  if (!briefing || !open) return null;

  return (
    <section className="briefing-card" aria-label="Welcome back">
      <header>
        <h2>Welcome back</h2>
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
        <button className="link" onClick={dismiss}>
          Dismiss
        </button>
      </div>
    </section>
  );
}
