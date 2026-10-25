import { useEffect, useState } from "react";
import {
  personaState,
  restorePersona,
  savePersona,
  type CommandError,
  type PersonaState,
} from "../../lib/tauri";
import { personaLabel, sameText } from "./model";

/** Settings ▸ Persona (M10 §4): Hermes's SOUL.md, written only on Save (D-023). */
export function PersonaSettings() {
  const [state, setState] = useState<PersonaState | null>(null);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [confirmRestore, setConfirmRestore] = useState(false);

  const show = (next: PersonaState) => {
    setState(next);
    setDraft(next.text);
    setError(null);
  };
  const fail = (err: CommandError) => setError(err.message ?? String(err));

  useEffect(() => {
    void personaState().then(show).catch(fail);
  }, []);

  if (!state) return <p className="muted">{error ?? "Reading the persona…"}</p>;
  const changed = !sameText(state.text, draft);

  return (
    <div className="settings-section">
      <h2>Persona</h2>
      <p className="muted small">
        Who Kairos is and how it speaks. This is Hermes&apos;s own <code>SOUL.md</code>; Kairos
        writes it only when you press Save, and keeps the original so you can go back. Applies to
        new conversations.
      </p>
      <p className="persona-kind">{personaLabel(state.kind, state.text, draft)}</p>
      <textarea
        className="persona-editor"
        value={draft}
        onChange={(e) => {
          setDraft(e.target.value);
          setSaved(false);
        }}
        spellCheck={false}
        aria-label="Persona"
        placeholder="Empty: Hermes uses its built-in default."
      />
      <div className="actions">
        <button
          onClick={() => setDraft(state.kairos_text)}
          disabled={sameText(draft, state.kairos_text)}
        >
          Use the Kairos persona
        </button>
        <button
          className="primary"
          onClick={() =>
            void savePersona(draft)
              .then((next) => {
                show(next);
                setSaved(true);
              })
              .catch(fail)
          }
          disabled={!changed}
        >
          Save
        </button>
        {state.has_backup &&
          (confirmRestore ? (
            <>
              <span className="muted small">Put Hermes&apos;s original back?</span>
              <button
                className="deny"
                onClick={() => {
                  setConfirmRestore(false);
                  void restorePersona().then(show).catch(fail);
                }}
              >
                Restore
              </button>
              <button onClick={() => setConfirmRestore(false)}>Keep</button>
            </>
          ) : (
            <button className="link" onClick={() => setConfirmRestore(true)}>
              Restore the original
            </button>
          ))}
      </div>
      {saved && <p className="ok-text small">Saved. Start a new conversation to meet it.</p>}
      {error && <p className="bad-text">{error}</p>}
    </div>
  );
}
