import { useEffect, useState } from "react";
import type { BriefingSettings as Settings } from "../../lib/tauri";
import { minutesToTime, timeToMinutes } from "./model";
import { useBriefing } from "./store";

export function BriefingSettings() {
  const { settings, error, loadSettings, saveSettings, briefNow } = useBriefing();
  // The text being typed; null shows the saved list.
  const [draft, setDraft] = useState<string | null>(null);

  useEffect(() => {
    void loadSettings();
  }, [loadSettings]);

  if (!settings) return <p className="muted">{error ?? "Reading briefing settings…"}</p>;
  const save = (patch: Partial<Settings>) => void saveSettings({ ...settings, ...patch });

  return (
    <>
      <div className="settings-section">
        <h2>Welcome-back briefing</h2>
        <label className="switch-row">
          <input
            type="checkbox"
            checked={settings.enabled}
            onChange={(e) => save({ enabled: e.target.checked })}
          />
          <span>Brief me when I come back to the PC</span>
        </label>
        <div className="briefing-fields">
          <label>
            <span>After at least (minutes away)</span>
            <input
              type="number"
              min={1}
              max={1440}
              value={settings.min_away_min}
              onChange={(e) => {
                const value = Number(e.target.value);
                if (Number.isInteger(value) && value > 0) save({ min_away_min: value });
              }}
            />
          </label>
          <label>
            <span>Quiet hours from</span>
            <input
              type="time"
              value={minutesToTime(settings.quiet_start)}
              onChange={(e) => save({ quiet_start: timeToMinutes(e.target.value) })}
            />
          </label>
          <label>
            <span>to</span>
            <input
              type="time"
              value={minutesToTime(settings.quiet_end)}
              onChange={(e) => save({ quiet_end: timeToMinutes(e.target.value) })}
            />
          </label>
        </div>
        <p className="muted small">
          Also the first time you come back each day. Never during quiet hours (clear both times to
          have none).
        </p>
        <label className="switch-row">
          <input
            type="checkbox"
            checked={settings.ai_summary}
            onChange={(e) => save({ ai_summary: e.target.checked })}
          />
          <span>Then start a short AI summary by itself (in a &quot;Welcome back&quot; chat)</span>
        </label>
      </div>

      <div className="settings-section">
        <h2>Projects to check</h2>
        <p className="muted small">
          Folders the AI summary may look at for uncommitted changes and failing tests, one per
          line. It is told not to change anything there; your rules still apply.
        </p>
        <textarea
          className="projects-input"
          rows={3}
          spellCheck={false}
          value={draft ?? settings.projects.join("\n")}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={() => {
            if (draft === null) return;
            save({
              projects: draft
                .split("\n")
                .map((p) => p.trim())
                .filter(Boolean),
            });
            setDraft(null);
          }}
        />
      </div>

      <div className="settings-section">
        <button onClick={() => void briefNow()}>Brief me now</button>
        {error && <p className="bad-text">{error}</p>}
      </div>
    </>
  );
}
