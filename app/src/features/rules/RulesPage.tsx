import { useEffect, useState } from "react";
import { ChevronIcon } from "../shell/icons";
import {
  ACTION_LABEL,
  MATCH_KINDS,
  MATCH_LABEL,
  addNewerRules,
  addRule,
  isEnabled,
  missingNewerRules,
  moveRule,
  removeRule,
  setMatch,
  summarizeRule,
  toggleRule,
  updateRule,
  type RuleAction,
  type RuleDoc,
} from "./model";
import { isDirty, useRules } from "./store";

function RuleEditor({ rule, index }: { rule: RuleDoc; index: number }) {
  const edit = useRules((s) => s.edit);
  return (
    <div className="rule-editor">
      <label>
        <span>Description (what the model and you are told)</span>
        <input
          value={rule.description ?? ""}
          onChange={(e) => edit((d) => updateRule(d, index, { description: e.target.value }))}
        />
      </label>
      <div className="rule-editor-row">
        <label>
          <span>When it matches</span>
          <select
            value={rule.action}
            onChange={(e) =>
              edit((d) => updateRule(d, index, { action: e.target.value as RuleAction }))
            }
          >
            {(["deny", "ask", "allow"] as const).map((a) => (
              <option key={a} value={a}>
                {ACTION_LABEL[a]}
              </option>
            ))}
          </select>
        </label>
        <label className="grow">
          <span>Tools (comma-separated, or * for any)</span>
          <input
            value={rule.tools.join(", ")}
            onChange={(e) =>
              edit((d) =>
                updateRule(d, index, {
                  tools: e.target.value
                    .split(",")
                    .map((t) => t.trim())
                    .filter(Boolean),
                }),
              )
            }
          />
        </label>
      </div>
      {MATCH_KINDS.map((kind) => (
        <label key={kind}>
          <span>{MATCH_LABEL[kind]}, one per line</span>
          <textarea
            rows={Math.min(8, Math.max(2, (rule.match?.[kind]?.length ?? 0) + 1))}
            spellCheck={false}
            value={(rule.match?.[kind] ?? []).join("\n")}
            onChange={(e) => edit((d) => setMatch(d, index, kind, e.target.value.split("\n")))}
          />
        </label>
      ))}
      <p className="muted small">
        Every kind you fill in must match (paths and commands, for example); within one kind, any
        line is enough. With nothing filled in, the rule covers every call to its tools.
      </p>
      <button className="danger-link" onClick={() => edit((d) => removeRule(d, index))}>
        Remove this rule
      </button>
    </div>
  );
}

function RuleRow({ rule, index, count }: { rule: RuleDoc; index: number; count: number }) {
  const edit = useRules((s) => s.edit);
  const [open, setOpen] = useState(false);
  const enabled = isEnabled(rule);
  return (
    <li className={`rule ${enabled ? "" : "off"}`}>
      <div className="rule-head">
        <input
          type="checkbox"
          className="switch"
          checked={enabled}
          onChange={() => edit((d) => toggleRule(d, index))}
          aria-label={`${enabled ? "Turn off" : "Turn on"} ${rule.id}`}
          title={enabled ? "On" : "Off"}
        />
        <button className="rule-title" onClick={() => setOpen(!open)} aria-expanded={open}>
          <ChevronIcon open={open} />
          <span className="rule-id">{rule.id}</span>
          <span className={`action-badge ${rule.action}`}>{ACTION_LABEL[rule.action]}</span>
          <span className="rule-summary">{summarizeRule(rule)}</span>
        </button>
        <div className="rule-order">
          <button
            onClick={() => edit((d) => moveRule(d, index, -1))}
            disabled={index === 0}
            aria-label={`Move ${rule.id} up`}
            title="Move up"
          >
            ↑
          </button>
          <button
            onClick={() => edit((d) => moveRule(d, index, 1))}
            disabled={index === count - 1}
            aria-label={`Move ${rule.id} down`}
            title="Move down"
          >
            ↓
          </button>
        </div>
      </div>
      {rule.description && !open && <p className="rule-description">{rule.description}</p>}
      {open && <RuleEditor rule={rule} index={index} />}
    </li>
  );
}

function DraftStatus() {
  const { checking, draftValidation, draft } = useRules();
  if (checking) return <span className="muted">Checking…</span>;
  if (!draft) return <span className="bad-text">The text isn't a rules document yet.</span>;
  if (!draftValidation) return null;
  switch (draftValidation.result) {
    case "valid":
      return <span className="ok-text">Valid</span>;
    case "invalid":
      return <span className="bad-text">{draftValidation.reason}</span>;
    case "unavailable":
      return <span className="warn-text">Can't check: {draftValidation.reason}</span>;
  }
}

export function RulesPage() {
  const store = useRules();
  const { saved, draft, draftText, draftValidation, saving, error, load, edit, setRaw, save } =
    store;
  const [raw, setRawMode] = useState(false);
  const [confirmReset, setConfirmReset] = useState(false);
  const dirty = isDirty(store);
  const canSave = dirty && draftValidation?.result === "valid" && !store.checking && !saving;

  useEffect(() => {
    void load();
  }, [load]);

  if (!saved) {
    return <p className="muted">{error ?? "Loading rules…"}</p>;
  }

  return (
    <div className="rules-page">
      <p className="muted">
        Kairos checks every action Hermes takes against these rules, top to bottom, and the first
        one that matches decides. They apply to Hermes Desktop too. Saved changes take effect on the
        next action, without restarting Hermes.
      </p>

      {saved.validation.result === "invalid" && (
        <div className="status-card bad" role="alert">
          <h2>
            <span className="dot" aria-hidden="true" />
            Your rules file can't be used
          </h2>
          <p>{saved.validation.reason}</p>
          <p className="detail">
            Until it is fixed, every action Kairos or Hermes tries is blocked. Fix it below, or
            restore the starter rules.
          </p>
        </div>
      )}
      {saved.validation.result === "unavailable" && (
        <div className="status-card warn" role="alert">
          <p>Kairos can't check the rules: {saved.validation.reason}</p>
        </div>
      )}

      {draft && saved.validation.result === "valid" && missingNewerRules(draft).length > 0 && (
        <div className="status-card" role="status">
          <p>
            New starter rules came with this version of Kairos:{" "}
            {missingNewerRules(draft)
              .map((r) => r.id)
              .join(", ")}
            . Your rules file was written before them, so they aren't in it yet.
          </p>
          <div className="actions" style={{ marginTop: 10 }}>
            <button onClick={() => edit(addNewerRules)}>Add the new rules</button>
          </div>
          <p className="detail">They are added to the list below; review them, then save.</p>
        </div>
      )}

      <div className="rules-toolbar">
        <button onClick={() => edit(addRule)} disabled={!draft || raw}>
          Add rule
        </button>
        <button onClick={() => setRawMode(!raw)}>
          {raw ? "Back to the list" : "Edit as JSON"}
        </button>
        <span className="spacer" />
        {confirmReset ? (
          <>
            <span className="muted small">Replace all rules with the starter set?</span>
            <button
              className="deny"
              onClick={() => {
                setConfirmReset(false);
                void store.restoreStarter();
              }}
            >
              Replace
            </button>
            <button onClick={() => setConfirmReset(false)}>Keep mine</button>
          </>
        ) : (
          <button onClick={() => setConfirmReset(true)}>Restore starter rules</button>
        )}
      </div>

      {raw || !draft ? (
        <textarea
          className="rules-raw"
          spellCheck={false}
          value={draftText}
          onChange={(e) => setRaw(e.target.value)}
          aria-label="Rules as JSON"
        />
      ) : (
        <ol className="rule-list">
          {draft.rules.map((rule, i) => (
            <RuleRow key={`${rule.id}-${i}`} rule={rule} index={i} count={draft.rules.length} />
          ))}
        </ol>
      )}

      <div className="rules-savebar" aria-live="polite">
        <div className="rules-status">
          {dirty ? <DraftStatus /> : <span className="muted">No unsaved changes</span>}
          {error && <span className="bad-text">{error}</span>}
        </div>
        <button onClick={store.discard} disabled={!dirty || saving}>
          Discard changes
        </button>
        <button className="primary" onClick={() => void save()} disabled={!canSave}>
          {saving ? "Saving…" : "Save rules"}
        </button>
      </div>
      <p className="muted small">Stored in {saved.path}</p>
    </div>
  );
}
