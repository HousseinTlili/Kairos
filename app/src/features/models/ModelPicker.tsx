import { useEffect, useMemo, useRef, useState } from "react";
import { ChevronIcon } from "../shell/icons";
import { groupName, modelGroups, openAtFirst, priceLabel } from "./model";
import { useModels } from "./store";

const OPEN_KEY = "kairos.models.open";

function savedOpen(): string[] | null {
  try {
    const text = localStorage.getItem(OPEN_KEY);
    return text ? (JSON.parse(text) as string[]) : null;
  } catch {
    return null;
  }
}

/**
 * The model menu on the message box's toolbar (M7 §4): the models grouped by where they come from
 * (Nous Portal, Google AI Studio, GitHub Copilot, your own endpoints…), each group folding open
 * and shut, with a search over all of them. It opens upwards, above the message box.
 */
export function ModelPicker({ disabled }: { disabled: boolean }) {
  const { catalog, choice, loadCatalog, choose } = useModels();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  // The groups the Master opened, kept between sessions; null until he opens or shuts one.
  const [unfolded, setUnfolded] = useState<string[] | null>(savedOpen);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!catalog) void loadCatalog();
  }, [catalog, loadCatalog]);

  // Shut on a click elsewhere or Esc.
  useEffect(() => {
    if (!open) return;
    const away = (e: MouseEvent) => {
      if (root.current && !root.current.contains(e.target as Node)) setOpen(false);
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", key);
    return () => {
      document.removeEventListener("mousedown", away);
      document.removeEventListener("keydown", key);
    };
  }, [open]);

  const groups = useMemo(() => modelGroups(catalog, query), [catalog, query]);
  const searching = query.trim().length > 0;
  const openGroups = unfolded ?? openAtFirst(groups, choice);

  const toggle = (slug: string) => {
    const next = openGroups.includes(slug)
      ? openGroups.filter((s) => s !== slug)
      : [...openGroups, slug];
    setUnfolded(next);
    try {
      localStorage.setItem(OPEN_KEY, JSON.stringify(next));
    } catch {
      // A private profile just starts with the chosen group open next time.
    }
  };

  const pick = (provider: string | null, model: string | null) => {
    choose({ provider, model });
    setOpen(false);
    setQuery("");
  };

  const defaultModel = catalog?.current_model ?? "Hermes's choice";
  const chosenGroup = catalog?.providers.find((p) => p.slug === choice.provider);
  const label =
    choice.provider && choice.model
      ? `${choice.model} · ${chosenGroup ? groupName(chosenGroup.slug, chosenGroup.name) : choice.provider}`
      : `Hermes default (${defaultModel})`;

  return (
    <div className="model-picker" ref={root}>
      <button
        type="button"
        className="model-picker-button"
        disabled={disabled || !catalog}
        aria-haspopup="listbox"
        aria-expanded={open}
        title="The model for this conversation"
        onClick={() => setOpen((o) => !o)}
      >
        <span className="model-picker-label">{label}</span>
        <ChevronIcon open={open} />
      </button>

      {open && catalog && (
        <div className="model-menu" role="listbox" aria-label="Models">
          <input
            className="model-search"
            autoFocus
            placeholder="Find a model"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            aria-label="Find a model"
          />
          <div className="model-menu-scroll">
            {!searching && (
              <button
                type="button"
                role="option"
                aria-selected={!choice.provider}
                className={`model-option ${!choice.provider ? "chosen" : ""}`}
                onClick={() => pick(null, null)}
              >
                <span>Hermes default</span>
                <span className="muted small">{defaultModel}</span>
              </button>
            )}
            {groups.length === 0 && <p className="muted small model-none">No model matches.</p>}
            {groups.map((g) => {
              const shown = searching || openGroups.includes(g.slug);
              return (
                <section key={g.slug} className="model-group">
                  <button
                    type="button"
                    className="model-group-head"
                    aria-expanded={shown}
                    onClick={() => !searching && toggle(g.slug)}
                  >
                    <ChevronIcon open={shown} />
                    <span className="model-group-name">{g.name}</span>
                    {g.isDefault && <span className="model-group-tag">default</span>}
                    <span className="muted small">
                      {searching && g.models.length !== g.total
                        ? `${g.models.length} of ${g.total}`
                        : g.total}
                    </span>
                  </button>
                  {shown &&
                    g.models.map((m) => {
                      const chosen = choice.provider === g.slug && choice.model === m.id;
                      const price = priceLabel(m.price);
                      return (
                        <button
                          type="button"
                          role="option"
                          aria-selected={chosen}
                          key={m.id}
                          className={`model-option ${chosen ? "chosen" : ""}`}
                          onClick={() => pick(g.slug, m.id)}
                        >
                          <span>{m.id}</span>
                          {price && <span className="muted small">{price}</span>}
                        </button>
                      );
                    })}
                </section>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
