import { useEffect, useState } from "react";
import {
  computerUseSet,
  computerUseState,
  type CommandError,
  type ComputerUseState,
} from "../../lib/tauri";
import { useShell } from "../shell/store";

const messageOf = (err: unknown) => (err as CommandError)?.message ?? String(err);

/**
 * Settings ▸ Safety ▸ Computer use (M20, D-040): the switch, off by default, and plainly what it means.
 */
export function ComputerUseSettings() {
  const [state, setState] = useState<ComputerUseState | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const openTab = useShell((s) => s.showSettings);

  useEffect(() => {
    void computerUseState()
      .then(setState)
      .catch((err) => setError(messageOf(err)));
  }, []);

  async function flip(on: boolean) {
    setBusy(true);
    setError(null);
    try {
      setState(await computerUseSet(on));
    } catch (err) {
      setError(messageOf(err));
    } finally {
      setBusy(false);
    }
  }

  if (!state) return <p className="muted">{error ?? "Reading the setting…"}</p>;

  const blocked = !state.on && (!state.ready || !state.rules_ready);

  return (
    <>
      <div className="settings-section">
        <h2>Computer use</h2>
        <p className="muted small">
          Lets Kairos use an app beside you, in the background, without taking your mouse or
          keyboard: it reads a window, clicks and types.
        </p>
        <label className="switch-row">
          <input
            type="checkbox"
            checked={state.on}
            disabled={busy || blocked}
            onChange={(e) => void flip(e.target.checked)}
          />
          <span>Let Kairos use my apps, in its own conversations</span>
        </label>
        {!state.ready && (
          <p className="warn-text small">Hermes's desktop driver isn't ready on this PC.</p>
        )}
        {!state.rules_ready && (
          <p className="warn-text small">
            The rules that keep it out of certain windows aren't in place, so it can't be turned on.{" "}
            <button className="link" onClick={() => openTab("rules")}>
              Open Rules
            </button>{" "}
            and press &ldquo;Add the new rules&rdquo;.
          </p>
        )}
        {error && <p className="bad-text small">{error}</p>}
      </div>

      <div className="settings-section">
        <h2>What it means</h2>
        <ul className="computer-use-notes">
          <li>
            <strong>It asks.</strong> Once per app to see a window, and before every click. Typing
            and keys are granted one at a time, showing the exact text. Nothing is ever granted for
            good.
          </li>
          <li>
            <strong>What it sees leaves this PC.</strong> A picture or the contents of each window
            you allow go to your AI provider.
          </li>
          <li>
            <strong>Some windows are off limits.</strong> Kairos itself, terminals, code editors,
            File Explorer, Settings, security tools, password managers, browsers and banking. The
            list is the rule <code>computer-use-off-limits</code>: add your own bank&apos;s app
            there, as a title only matches when it names a bank.
          </li>
          <li>
            <strong>It isn&apos;t airtight.</strong> Kairos checks the window it knows about, so a
            dialog that opens after a click can differ from what it remembers, and button names come
            from the app. A window&apos;s text can also carry instructions to the AI, so read each
            card.
          </li>
          <li>
            <strong>It costs more.</strong> Each step resends the conversation: 200,000 input tokens
            for a five-step task in testing. Stop it from the top bar at any time.
          </li>
        </ul>
      </div>
    </>
  );
}
