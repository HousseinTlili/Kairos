import { useConnection } from "../connection/store";
import { useAutomations } from "./store";

/** An automation's offer above the chat (M17 §3.3): Kairos starts only on the Master's click. */
export function OfferCards() {
  const { offers, acceptOffer, dismissOffer } = useAutomations();
  const connected = useConnection((s) => s.info?.status === "connected");
  if (offers.length === 0) return null;
  return (
    <>
      {offers.map(({ fired, text }) => (
        <section key={fired.run_id} className="offer-card" aria-label={fired.name}>
          <header>
            <h2>{fired.name}</h2>
            {fired.test && <span className="muted small">test</span>}
          </header>
          <p>{text}</p>
          {/* What happened, unless the Master's text already says it (as with {crash}). */}
          {!text.includes(fired.event) && <p className="muted small">{fired.event}</p>}
          <div className="actions">
            <button
              className="primary"
              onClick={() => void acceptOffer(fired.run_id)}
              disabled={!connected}
              title={connected ? undefined : "Hermes isn't connected"}
            >
              Ask Kairos
            </button>
            <button className="link" onClick={() => dismissOffer(fired.run_id)}>
              Not now
            </button>
          </div>
        </section>
      ))}
    </>
  );
}
