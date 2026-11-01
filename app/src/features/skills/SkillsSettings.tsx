import { useEffect, useMemo, useState } from "react";
import type { AuditedSkill, FoundSkill, InstalledSkill, SkillFinding } from "../../lib/tauri";
import { ChevronIcon } from "../shell/icons";
import {
  byTrust,
  canInstall,
  categoryName,
  groupSkills,
  ORIGIN_LABEL,
  scanWords,
  SKILL_SOURCES,
  TRUST_LABEL,
  TRUST_WORDS,
  verdictName,
} from "./model";
import { SkillText } from "./SkillText";
import { useSkills } from "./store";

const OPEN_KEY = "kairos.skills.open";

function savedOpen(): string[] {
  try {
    const saved = JSON.parse(localStorage.getItem(OPEN_KEY) ?? "[]");
    return Array.isArray(saved) ? saved : [];
  } catch {
    return [];
  }
}

/**
 * Settings ▸ Kairos ▸ Skills (M19, D-039): what Hermes knows how to do, and new skills from its
 * registries. A skill is instructions the model will follow, so installing one goes through its
 * page: who it's from, what it says, the scan, and only then Install.
 */
export function SkillsSettings() {
  const { page, load, busy, error, notice } = useSkills();
  const [tab, setTab] = useState<"installed" | "find">("installed");

  useEffect(() => {
    void load();
  }, [load]);

  if (page) return <SkillPage />;

  return (
    <div className="settings-section">
      <h2>Skills</h2>
      <p className="muted small">
        What Hermes knows how to do. A skill belongs to Hermes, so it also works from its command
        line and messaging apps, and whatever a skill leads Kairos to do still follows your rules.
      </p>
      <div className="segmented" role="tablist">
        {(
          [
            ["installed", "Installed"],
            ["find", "Find new"],
          ] as const
        ).map(([id, label]) => (
          <button
            key={id}
            role="tab"
            aria-selected={tab === id}
            className={tab === id ? "active" : ""}
            onClick={() => setTab(id)}
          >
            {label}
          </button>
        ))}
      </div>
      {tab === "installed" ? <Installed /> : <Find />}
      {busy && (
        <p className="muted small" role="status">
          {busy}
        </p>
      )}
      {notice && (
        <p className="ok-text small" role="status">
          {notice}
        </p>
      )}
      {error && <p className="bad-text small">{error}</p>}
    </div>
  );
}

function Installed() {
  const { installed, updates, audited, busy, check, rescan } = useSkills();
  const [filter, setFilter] = useState("");
  const [unfolded, setUnfolded] = useState<string[]>(savedOpen);
  const groups = useMemo(() => groupSkills(installed ?? [], filter), [installed, filter]);
  const searching = filter.trim().length > 0;

  if (!installed) return null;

  const fold = (category: string) => {
    const next = unfolded.includes(category)
      ? unfolded.filter((c) => c !== category)
      : [...unfolded, category];
    setUnfolded(next);
    try {
      localStorage.setItem(OPEN_KEY, JSON.stringify(next));
    } catch {
      // A private profile just starts folded next time.
    }
  };

  return (
    <>
      <div className="skills-bar">
        <input
          type="search"
          className="skills-filter"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          placeholder={`Filter ${installed.length} skills`}
          aria-label="Filter the skills"
        />
        <button disabled={!!busy} onClick={() => void check()}>
          Check for updates
        </button>
        <button disabled={!!busy} onClick={() => void rescan()}>
          Re-scan
        </button>
      </div>

      {audited && <Rescan audited={audited} />}

      {groups.length === 0 && <p className="muted small">No skill matches.</p>}
      <div className="skill-groups">
        {groups.map((g) => {
          const shown = searching || unfolded.includes(g.category);
          return (
            <section key={g.category} className="skill-group">
              <button
                className="skill-group-head"
                aria-expanded={shown}
                onClick={() => fold(g.category)}
              >
                <ChevronIcon open={shown} />
                <span className="skill-name">{categoryName(g.category)}</span>
                <span className="muted small">
                  {g.skills.length}
                  {g.off > 0 ? ` · ${g.off} off` : ""}
                </span>
              </button>
              {shown && (
                <ul className="skill-list">
                  {g.skills.map((s) => (
                    <SkillRow key={s.name} skill={s} update={updates?.[s.name]} />
                  ))}
                </ul>
              )}
            </section>
          );
        })}
      </div>
    </>
  );
}

function SkillRow({ skill, update }: { skill: InstalledSkill; update?: string }) {
  const { busy, toggle, remove, update: updateSkill } = useSkills();
  const [confirm, setConfirm] = useState<"remove" | "update" | null>(null);
  return (
    <li className={skill.enabled ? "" : "skill-off"}>
      <div className="skill-row">
        <div className="skill-about">
          <span className="skill-name">{skill.name}</span>
          <span className={`skill-badge ${skill.origin}`}>{ORIGIN_LABEL[skill.origin]}</span>
          {skill.description && <p className="muted small">{skill.description}</p>}
          {update && update !== "up_to_date" && (
            <p className="warn-text small">
              {update === "update_available" ? "An update is available." : `Hermes says: ${update}`}
            </p>
          )}
        </div>
        {skill.origin === "hub" && (
          <div className="skill-actions">
            {update === "update_available" && (
              <button className="link" onClick={() => setConfirm("update")}>
                Update
              </button>
            )}
            <button className="link" onClick={() => setConfirm("remove")}>
              Remove
            </button>
          </div>
        )}
        <input
          type="checkbox"
          className="skill-switch"
          aria-label={`Use ${skill.name}`}
          title={skill.own ? "Kairos's own skills stay on" : undefined}
          checked={skill.enabled}
          disabled={skill.own || !!busy}
          onChange={(e) => void toggle(skill.name, e.target.checked)}
        />
      </div>
      {confirm === "remove" && (
        <div className="status-card warn skill-confirm" role="alertdialog">
          <p>Remove {skill.name}? Any edits you made to it are lost.</p>
          <button
            className="deny"
            disabled={!!busy}
            onClick={() => {
              setConfirm(null);
              void remove(skill.name);
            }}
          >
            Remove
          </button>
          <button className="link" onClick={() => setConfirm(null)}>
            Keep
          </button>
        </div>
      )}
      {confirm === "update" && (
        <div className="status-card warn skill-confirm" role="alertdialog">
          <p>
            An update removes this copy and installs the new one through the same scan. Any edits
            you made are lost, and if the new version fails the scan, the skill stays removed.
          </p>
          <button
            className="primary"
            disabled={!!busy}
            onClick={() => {
              setConfirm(null);
              void updateSkill(skill.name);
            }}
          >
            Update
          </button>
          <button className="link" onClick={() => setConfirm(null)}>
            Keep this one
          </button>
        </div>
      )}
    </li>
  );
}

function Rescan({ audited }: { audited: AuditedSkill[] }) {
  return (
    <div className="status-card skill-rescan">
      <h3>Re-scan</h3>
      {audited.length === 0 ? (
        <p>No skills from the hub to re-scan. Built-in ones come with Hermes.</p>
      ) : (
        <>
          <p className="muted small">
            A re-scan reads each skill as if anyone could have written it, so an official skill can
            look alarming. Read what it found.
          </p>
          {audited.map((a) => (
            <div key={a.name}>
              <p>
                <span className="skill-name">{a.name}</span>: {verdictName(a.verdict)}
              </p>
              <Findings findings={a.findings} />
            </div>
          ))}
        </>
      )}
    </div>
  );
}

function Findings({ findings }: { findings: SkillFinding[] }) {
  if (findings.length === 0) return null;
  return (
    <ul className="skill-findings">
      {findings.map((f, i) => (
        <li key={`${f.place}-${i}`}>
          <span className={`skill-severity ${f.severity}`}>{f.severity}</span>
          <span className="muted small">
            {f.category} · {f.place}
          </span>
          <code>{f.text}</code>
        </li>
      ))}
    </ul>
  );
}

function Find() {
  const { results, busy, search, open } = useSkills();
  const [query, setQuery] = useState("");
  const [source, setSource] = useState("all");
  const ordered = useMemo(() => byTrust(results ?? []), [results]);
  return (
    <>
      <form
        className="skills-bar"
        onSubmit={(e) => {
          e.preventDefault();
          void search(query, source);
        }}
      >
        <input
          type="search"
          className="skills-filter"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="What should Kairos learn to do?"
          aria-label="Search the registries"
        />
        <select
          value={source}
          onChange={(e) => setSource(e.target.value)}
          aria-label="Which registry"
        >
          {SKILL_SOURCES.map((s) => (
            <option key={s.id} value={s.id}>
              {s.label}
            </option>
          ))}
        </select>
        <button className="primary" type="submit" disabled={!!busy || !query.trim()}>
          Search
        </button>
      </form>
      {results?.length === 0 && <p className="muted small">Nothing found.</p>}
      {ordered.length > 0 && (
        <ul className="skill-list skill-results">
          {ordered.map((f) => (
            <FoundRow key={f.identifier} found={f} onOpen={() => void open(f.identifier)} />
          ))}
        </ul>
      )}
    </>
  );
}

function FoundRow({ found, onOpen }: { found: FoundSkill; onOpen: () => void }) {
  const busy = useSkills((s) => s.busy);
  return (
    <li>
      <button className="skill-result" disabled={!!busy} onClick={onOpen}>
        <span className="skill-name">{found.name}</span>
        <span className={`skill-badge ${found.trust}`}>{TRUST_LABEL[found.trust]}</span>
        <span className="muted small">{found.source}</span>
        {found.description && <p className="muted small">{found.description}</p>}
      </button>
    </li>
  );
}

/** A skill before it's installed. The order is the point: who, what it says, the scan, Install. */
function SkillPage() {
  const { page, scan, busy, error, notice, close, scanOpen, installOpen, installed } = useSkills();
  if (!page) return null;
  const words = scan && scanWords(scan, page.trust);
  const isInstalled = installed?.some((s) => s.name === page.name && s.origin === "hub");
  return (
    <div className="settings-section skill-page">
      <button className="link skill-back" onClick={close}>
        ← Skills
      </button>
      <h2>{page.name}</h2>
      {page.description && <p className="muted">{page.description}</p>}

      <h3>Who it comes from</h3>
      <p>
        <span className={`skill-badge ${page.trust}`}>{TRUST_LABEL[page.trust]}</span>{" "}
        {TRUST_WORDS[page.trust]}
      </p>
      <p className="muted small">
        {page.source} · <span className="skill-id">{page.identifier}</span>
      </p>

      <h3>What it says</h3>
      <SkillText text={page.text} />
      <p className="muted small">
        Hermes shows only the first 50 lines
        {page.more_lines > 0 ? `; ${page.more_lines} more aren't shown` : ""}, and its preview can
        drop text in [square brackets]. The scan reads every file.
      </p>

      <h3>The scan</h3>
      {!words || !scan ? (
        <>
          <p className="muted small">
            Hermes downloads the skill to a quarantine folder, scans it, and deletes the copy.
            Nothing is installed or run.
          </p>
          <button disabled={!!busy || !page.text} onClick={() => void scanOpen()}>
            Scan it
          </button>
        </>
      ) : (
        <div className={`status-card ${words.tone}`} role="status">
          <p>{words.text}</p>
          {scan.origin && (
            <p className="muted small">
              Scanned from <span className="skill-id">{scan.origin}</span>
            </p>
          )}
          <Findings findings={scan.findings} />
        </div>
      )}

      {canInstall(scan) && (
        <div className="actions">
          <button className="primary" disabled={!!busy} onClick={() => void installOpen()}>
            Install
          </button>
        </div>
      )}
      {isInstalled && !scan && !busy && <p className="muted small">This skill is installed.</p>}
      {busy && (
        <p className="muted small" role="status">
          {busy}
        </p>
      )}
      {notice && (
        <p className="ok-text small" role="status">
          {notice}
        </p>
      )}
      {error && <p className="bad-text small">{error}</p>}
    </div>
  );
}
