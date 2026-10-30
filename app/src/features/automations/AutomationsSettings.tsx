import { useEffect, useState } from "react";
import type { Automation, AutomationAction, Conditions, Trigger } from "../../lib/tauri";
import { minutesToTime, timeToMinutes } from "../briefing/model";
import { callModule, filterApps, type AppEntry } from "../island/panel/api";
import {
  ACTION_KINDS,
  DAY_LABELS,
  PLACEHOLDERS,
  TRIGGER_KINDS,
  describeAction,
  describeConditions,
  describeTrigger,
  newAutomation,
  outcomeLabel,
  problem,
} from "./model";
import { useAutomations } from "./store";

const when = (unix: number) =>
  new Date(unix * 1000).toLocaleString([], {
    weekday: "short",
    hour: "2-digit",
    minute: "2-digit",
  });

/** Settings ▸ Kairos ▸ Automations (M17 Step 6). */
export function AutomationsSettings() {
  const { state, error, load, save, remove, setEnabled, setIgnore, test } = useAutomations();
  // The automation being edited (a copy), or null for the list.
  const [editing, setEditing] = useState<Automation | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [ignoreDraft, setIgnoreDraft] = useState<string | null>(null);

  useEffect(() => {
    void load();
  }, [load]);

  if (!state) return <p className="muted">{error ?? "Reading your automations…"}</p>;

  if (editing) {
    return (
      <Editor
        draft={editing}
        error={error}
        onChange={setEditing}
        onCancel={() => setEditing(null)}
        onSave={async () => {
          if (await save(editing)) setEditing(null);
        }}
        onTest={() => void test(editing)}
      />
    );
  }

  const lastRun = (id: string) => state.runs.find((r) => r.automation_id === id && !r.test);

  return (
    <>
      <div className="settings-section">
        <h2>Automations</h2>
        <p className="muted small">
          When something happens on the PC, Kairos does what you ask: a notification, a few words in
          its voice, an app, or a request to Kairos. Requests follow your rules like any other:
          anything that would ask you still waits for you. At most 6 requests by automations an
          hour, 30 a day.
        </p>
        <label className="switch-row">
          <input
            type="checkbox"
            checked={state.enabled}
            onChange={(e) => void setEnabled(e.target.checked)}
          />
          <span>Run my automations</span>
        </label>
        <ul className={`automation-list ${state.enabled ? "" : "all-off"}`}>
          {state.automations.map((a) => {
            const last = lastRun(a.id);
            const conditions = describeConditions(a.only_if);
            return (
              <li key={a.id} className="automation-row">
                <input
                  type="checkbox"
                  checked={a.enabled}
                  aria-label={`${a.name} on`}
                  onChange={(e) => void save({ ...a, enabled: e.target.checked })}
                  disabled={a.actions.length === 0 && !a.enabled}
                  title={a.actions.length === 0 ? "Choose what it does first" : undefined}
                />
                <div className="automation-words">
                  <p className="automation-name">{a.name}</p>
                  <p className="small muted">
                    {describeTrigger(a.trigger)}
                    {conditions ? ` (${conditions})` : ""}
                    {a.actions.length > 0
                      ? `: ${a.actions.map(describeAction).join(", ")}`
                      : ": nothing chosen yet"}
                  </p>
                  {last && (
                    <p className="small muted">
                      Last: {when(last.at)}, {outcomeLabel(last).toLowerCase()}
                    </p>
                  )}
                </div>
                <div className="automation-buttons">
                  <button onClick={() => setEditing(structuredClone(a))}>Edit</button>
                  {confirmDelete === a.id ? (
                    <>
                      <button className="deny" onClick={() => void remove(a.id)}>
                        Delete
                      </button>
                      <button className="link" onClick={() => setConfirmDelete(null)}>
                        Keep
                      </button>
                    </>
                  ) : (
                    <button className="link" onClick={() => setConfirmDelete(a.id)}>
                      Delete
                    </button>
                  )}
                </div>
              </li>
            );
          })}
        </ul>
        <div className="actions">
          <button
            className="primary"
            onClick={() => setEditing(newAutomation(crypto.randomUUID()))}
          >
            New automation
          </button>
        </div>
        {error && <p className="bad-text">{error}</p>}
      </div>

      <div className="settings-section">
        <h2>Recent runs</h2>
        {state.runs.length === 0 ? (
          <p className="muted small">Nothing has run yet.</p>
        ) : (
          <ul className="run-list">
            {state.runs.slice(0, 30).map((run) => (
              <li key={run.id} className={`run-row ${run.outcome}`}>
                <span className="run-when small muted">{when(run.at)}</span>
                <span className="run-what">
                  <strong>{run.name}</strong>
                  {run.test && <span className="run-test small">test</span>}
                  <span className="small muted">{run.event}</span>
                </span>
                <span className="run-outcome small">{outcomeLabel(run)}</span>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="settings-section">
        <h2>Programs whose crashes don&apos;t count</h2>
        <p className="muted small">
          One per line. Windows&apos;s own background programs are here to begin with; Kairos itself
          is always left out.
        </p>
        <textarea
          className="projects-input"
          rows={4}
          spellCheck={false}
          value={ignoreDraft ?? state.crash_ignore.join("\n")}
          onChange={(e) => setIgnoreDraft(e.target.value)}
          onBlur={() => {
            if (ignoreDraft === null) return;
            void setIgnore(ignoreDraft.split("\n"));
            setIgnoreDraft(null);
          }}
        />
      </div>
    </>
  );
}

function TriggerFields({
  trigger,
  onChange,
}: {
  trigger: Trigger;
  onChange: (t: Trigger) => void;
}) {
  const number = (value: string, min: number, max: number) => {
    const n = Number(value);
    return Number.isInteger(n) && n >= min && n <= max ? n : null;
  };
  switch (trigger.kind) {
    case "back":
      return (
        <label>
          <span>Away at least (minutes)</span>
          <input
            type="number"
            min={1}
            max={1440}
            value={trigger.away_min}
            onChange={(e) => {
              const n = number(e.target.value, 1, 1440);
              if (n) onChange({ ...trigger, away_min: n });
            }}
          />
        </label>
      );
    case "battery_low":
      return (
        <label>
          <span>Under (%)</span>
          <input
            type="number"
            min={1}
            max={99}
            value={trigger.below}
            onChange={(e) => {
              const n = number(e.target.value, 1, 99);
              if (n) onChange({ ...trigger, below: n });
            }}
          />
        </label>
      );
    case "charger":
      return (
        <label>
          <span>When it is</span>
          <select
            value={trigger.plugged ? "in" : "out"}
            onChange={(e) => onChange({ ...trigger, plugged: e.target.value === "in" })}
          >
            <option value="out">unplugged</option>
            <option value="in">plugged in</option>
          </select>
        </label>
      );
    case "wifi_joined":
      return (
        <label>
          <span>Network (empty for any)</span>
          <input
            value={trigger.network}
            placeholder="Any network"
            onChange={(e) => onChange({ ...trigger, network: e.target.value })}
          />
        </label>
      );
    case "crash":
      return (
        <label className="wide">
          <span>Only these programs (comma-separated, empty for any)</span>
          <input
            value={trigger.programs.join(", ")}
            placeholder="Any program, e.g. code.exe, steam.exe"
            onChange={(e) =>
              onChange({
                ...trigger,
                programs: e.target.value
                  .split(",")
                  .map((p) => p.trim())
                  .filter(Boolean),
              })
            }
          />
        </label>
      );
    default:
      return null;
  }
}

function ConditionFields({
  value,
  onChange,
}: {
  value: Conditions;
  onChange: (c: Conditions) => void;
}) {
  const windowOn = value.from_min !== null && value.to_min !== null;
  return (
    <>
      <div className="automation-fields">
        <label className="switch-row">
          <input
            type="checkbox"
            checked={windowOn}
            onChange={(e) =>
              onChange({
                ...value,
                from_min: e.target.checked ? 8 * 60 : null,
                to_min: e.target.checked ? 12 * 60 : null,
              })
            }
          />
          <span>Only between</span>
        </label>
        <input
          type="time"
          aria-label="From"
          disabled={!windowOn}
          value={minutesToTime(value.from_min)}
          onChange={(e) => onChange({ ...value, from_min: timeToMinutes(e.target.value) })}
        />
        <span className="muted">and</span>
        <input
          type="time"
          aria-label="To"
          disabled={!windowOn}
          value={minutesToTime(value.to_min)}
          onChange={(e) => onChange({ ...value, to_min: timeToMinutes(e.target.value) })}
        />
      </div>
      <div className="day-picks" role="group" aria-label="Days">
        {DAY_LABELS.map((label, day) => {
          const on = value.days.length === 0 || value.days.includes(day);
          return (
            <button
              key={label}
              className={on ? "day on" : "day"}
              aria-pressed={on}
              onClick={() => {
                const all = value.days.length === 0 ? [0, 1, 2, 3, 4, 5, 6] : value.days;
                const next = on ? all.filter((d) => d !== day) : [...all, day].sort();
                // Every day, or none picked, means no condition.
                onChange({ ...value, days: next.length === 7 || next.length === 0 ? [] : next });
              }}
            >
              {label}
            </button>
          );
        })}
      </div>
      <div className="automation-fields">
        <label>
          <span>Power</span>
          <select
            value={value.power ?? ""}
            onChange={(e) =>
              onChange({
                ...value,
                power: e.target.value === "" ? null : (e.target.value as "battery" | "plugged"),
              })
            }
          >
            <option value="">Either</option>
            <option value="battery">On battery</option>
            <option value="plugged">Plugged in</option>
          </select>
        </label>
        <label className="switch-row">
          <input
            type="checkbox"
            checked={value.during_quiet_hours}
            onChange={(e) => onChange({ ...value, during_quiet_hours: e.target.checked })}
          />
          <span>Also during quiet hours (Settings ▸ Briefing)</span>
        </label>
      </div>
    </>
  );
}

function AppPicker({
  action,
  onChange,
}: {
  action: Extract<AutomationAction, { kind: "open_app" }>;
  onChange: (a: AutomationAction) => void;
}) {
  const [catalog, setCatalog] = useState<AppEntry[] | null>(null);
  const [query, setQuery] = useState("");
  const [failed, setFailed] = useState<string | null>(null);
  useEffect(() => {
    void callModule<AppEntry[]>("pinned", "catalog")
      .then(setCatalog)
      .catch((err: unknown) => setFailed(String(err)));
  }, []);
  if (action.app_id && !query) {
    return (
      <p className="app-chosen">
        <strong>{action.name}</strong>
        <button className="link" onClick={() => onChange({ ...action, app_id: "", name: "" })}>
          Change
        </button>
      </p>
    );
  }
  return (
    <div className="pin-picker">
      <input placeholder="Find an app" value={query} onChange={(e) => setQuery(e.target.value)} />
      {failed && <p className="bad-text small">{failed}</p>}
      {!catalog && !failed && <p className="muted small">Reading the Start menu…</p>}
      {catalog && (
        <ul>
          {filterApps(catalog, query)
            .slice(0, 12)
            .map((app) => (
              <li key={app.id}>
                <button
                  onClick={() => {
                    setQuery("");
                    onChange({ ...action, app_id: app.id, name: app.name });
                  }}
                >
                  {app.name}
                </button>
              </li>
            ))}
        </ul>
      )}
    </div>
  );
}

function Editor({
  draft,
  error,
  onChange,
  onCancel,
  onSave,
  onTest,
}: {
  draft: Automation;
  error: string | null;
  onChange: (a: Automation) => void;
  onCancel: () => void;
  onSave: () => Promise<void>;
  onTest: () => void;
}) {
  const [tested, setTested] = useState(false);
  const issue = problem(draft);
  const setAction = (i: number, action: AutomationAction) =>
    onChange({ ...draft, actions: draft.actions.map((a, j) => (j === i ? action : a)) });
  const placeholders = PLACEHOLDERS[draft.trigger.kind];

  return (
    <div className="automation-editor">
      <div className="settings-section">
        <h2>{draft.name.trim() || "New automation"}</h2>
        <label className="automation-name-field">
          <span>Name</span>
          <input
            value={draft.name}
            placeholder="Morning apps"
            onChange={(e) => onChange({ ...draft, name: e.target.value })}
          />
        </label>
      </div>

      <div className="settings-section">
        <h3 className="editor-step">When</h3>
        <div className="automation-fields">
          <label>
            <span>Something happens</span>
            <select
              value={draft.trigger.kind}
              onChange={(e) => {
                const kind = TRIGGER_KINDS.find((t) => t.kind === e.target.value);
                if (kind) onChange({ ...draft, trigger: kind.make() });
              }}
            >
              {TRIGGER_KINDS.map((t) => (
                <option key={t.kind} value={t.kind}>
                  {t.label}
                </option>
              ))}
            </select>
          </label>
          <TriggerFields
            trigger={draft.trigger}
            onChange={(trigger) => onChange({ ...draft, trigger })}
          />
        </div>
      </div>

      <div className="settings-section">
        <h3 className="editor-step">Only if</h3>
        <ConditionFields
          value={draft.only_if}
          onChange={(only_if) => onChange({ ...draft, only_if })}
        />
      </div>

      <div className="settings-section">
        <h3 className="editor-step">Do</h3>
        {placeholders.length > 0 && (
          <p className="muted small">
            Texts can use {placeholders.map((p) => `{${p}}`).join(", ")}.
          </p>
        )}
        <ol className="action-list">
          {draft.actions.map((action, i) => {
            const kind = ACTION_KINDS.find((k) => k.kind === action.kind);
            return (
              <li key={i} className="action-row">
                <div className="action-head">
                  <select
                    value={action.kind}
                    aria-label="What to do"
                    onChange={(e) => {
                      const next = ACTION_KINDS.find((k) => k.kind === e.target.value);
                      if (!next) return;
                      const made = next.make();
                      // Keep the text when switching between kinds that have one.
                      if ("text" in made && "text" in action) made.text = action.text;
                      setAction(i, made);
                    }}
                  >
                    {ACTION_KINDS.map((k) => (
                      <option key={k.kind} value={k.kind}>
                        {k.label}
                      </option>
                    ))}
                  </select>
                  <span className="muted small">{kind?.hint}</span>
                  <span className="spacer" />
                  <button
                    className="link"
                    onClick={() =>
                      onChange({ ...draft, actions: draft.actions.filter((_, j) => j !== i) })
                    }
                  >
                    Remove
                  </button>
                </div>
                {action.kind === "open_app" ? (
                  <AppPicker action={action} onChange={(a) => setAction(i, a)} />
                ) : (
                  <textarea
                    rows={action.kind === "ask" || action.kind === "offer" ? 3 : 2}
                    value={action.text}
                    placeholder={
                      action.kind === "ask"
                        ? "What's left on my calendar today?"
                        : action.kind === "offer"
                          ? "{crash}. Shall I look into why?"
                          : "Master, the battery is at {battery}."
                    }
                    onChange={(e) => setAction(i, { ...action, text: e.target.value })}
                  />
                )}
              </li>
            );
          })}
        </ol>
        {draft.actions.length < 8 && (
          <button
            onClick={() =>
              onChange({ ...draft, actions: [...draft.actions, { kind: "notify", text: "" }] })
            }
          >
            Add an action
          </button>
        )}
        <div className="automation-fields">
          <label>
            <span>Not again within (minutes)</span>
            <input
              type="number"
              min={1}
              max={1440}
              value={draft.cooldown_min}
              onChange={(e) => {
                const n = Number(e.target.value);
                if (Number.isInteger(n) && n >= 1) onChange({ ...draft, cooldown_min: n });
              }}
            />
          </label>
        </div>
      </div>

      <div className="settings-section">
        <div className="actions">
          <button className="primary" disabled={!!issue} onClick={() => void onSave()}>
            Save
          </button>
          <button
            disabled={!!issue}
            onClick={() => {
              setTested(true);
              onTest();
            }}
            title="Runs it now with a sample event, marked as a test"
          >
            Test now
          </button>
          <button className="link" onClick={onCancel}>
            Cancel
          </button>
        </div>
        {issue && <p className="muted small">{issue}</p>}
        {tested && !issue && (
          <p className="muted small">
            Ran with a sample event; it shows in Recent runs as a test. An &quot;Ask Kairos&quot;
            starts a real request.
          </p>
        )}
        {error && <p className="bad-text">{error}</p>}
      </div>
    </div>
  );
}
