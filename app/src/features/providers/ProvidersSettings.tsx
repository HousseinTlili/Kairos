import { useEffect, useState } from "react";
import { useModels } from "../models/store";
import { addedWords, modelsListed, parseModels, slugOf } from "./model";
import { useProviders } from "./store";

type Way = "known" | "endpoint" | "copilot";

/**
 * Settings ▸ Accounts and models ▸ AI providers (M18, D-038): the services Hermes can use, and
 * three ways to add one: a known provider's key, your own OpenAI-compatible endpoint, or GitHub
 * Copilot. The keys go into Hermes; Kairos keeps none and never shows one again.
 */
export function ProvidersSettings() {
  const { state, busy, error, justAdded, load, addKey, addEndpoint, remove, restart } =
    useProviders();
  const catalog = useModels((s) => s.catalog);
  const [way, setWay] = useState<Way>("known");
  const [confirmRemove, setConfirmRemove] = useState<string | null>(null);

  useEffect(() => {
    void load();
  }, [load]);

  if (!state) return <p className="muted">{error ?? "Reading your providers…"}</p>;

  const listed = justAdded ? modelsListed(catalog, justAdded) : null;
  const mine = [
    ...state.known
      .filter((k) => k.by_kairos)
      .map((k) => ({ id: k.id, name: k.name, endpoint: false, detail: k.env })),
    ...state.endpoints.map((e) => ({ id: e.id, name: e.id, endpoint: true, detail: e.api })),
  ];

  return (
    <>
      <div className="settings-section">
        <h2>AI providers</h2>
        <p className="muted small">
          The services Hermes can use for Kairos&apos;s answers. Add your own with its key; its
          models then appear in the model menu above the message box. Keys go straight into
          Hermes&apos;s settings: Kairos keeps none and never shows one again.
        </p>
        {catalog && (
          <ul className="provider-list">
            {catalog.providers.map((p) => (
              <li key={p.slug}>
                <span className="provider-name">{p.name}</span>
                <span className="muted small">
                  {p.models.length} model{p.models.length === 1 ? "" : "s"}
                  {p.is_current ? " · Hermes's default" : ""}
                </span>
              </li>
            ))}
          </ul>
        )}
      </div>

      {mine.length > 0 && (
        <div className="settings-section">
          <h2>Added here</h2>
          <ul className="provider-list">
            {mine.map((m) => {
              const key = `${m.endpoint ? "e" : "k"}:${m.id}`;
              return (
                <li key={key}>
                  <span className="provider-name">{m.name}</span>
                  <span className="muted small">
                    {m.detail} · {addedWords(modelsListed(catalog, m.id))}
                  </span>
                  <span className="spacer" />
                  {confirmRemove === key ? (
                    <>
                      <button
                        className="deny"
                        disabled={busy}
                        onClick={() => {
                          setConfirmRemove(null);
                          void remove(m.id, m.endpoint);
                        }}
                      >
                        Remove
                      </button>
                      <button className="link" onClick={() => setConfirmRemove(null)}>
                        Keep
                      </button>
                    </>
                  ) : (
                    <button className="link" onClick={() => setConfirmRemove(key)}>
                      Remove
                    </button>
                  )}
                </li>
              );
            })}
          </ul>
        </div>
      )}

      <div className="settings-section">
        <h2>Add a provider</h2>
        <div className="segmented" role="tablist">
          {(
            [
              ["known", "A known provider"],
              ["endpoint", "Your own endpoint"],
              ["copilot", "GitHub Copilot"],
            ] as const
          ).map(([id, label]) => (
            <button
              key={id}
              role="tab"
              aria-selected={way === id}
              className={way === id ? "active" : ""}
              onClick={() => setWay(id)}
            >
              {label}
            </button>
          ))}
        </div>
        {way === "known" ? (
          <KnownForm onAdd={addKey} busy={busy} />
        ) : way === "endpoint" ? (
          <EndpointForm onAdd={addEndpoint} busy={busy} />
        ) : (
          <CopilotForm onAdd={(token) => addKey("copilot", token)} busy={busy} />
        )}

        {justAdded && (
          <div className={`status-card ${listed === null ? "warn" : "ok"}`} role="status">
            <p>
              Saved.{" "}
              {listed === null
                ? "Hermes doesn't list it yet: it reads new providers when it starts."
                : `Hermes lists ${addedWords(listed)}; pick one in the model menu.`}
            </p>
            {listed === null && (
              <button onClick={() => void restart()} disabled={busy}>
                {busy ? "Restarting Hermes…" : "Restart Hermes"}
              </button>
            )}
          </div>
        )}
        {error && <p className="bad-text">{error}</p>}
      </div>
    </>
  );
}

function KnownForm({
  onAdd,
  busy,
}: {
  onAdd: (id: string, key: string) => Promise<boolean>;
  busy: boolean;
}) {
  const known = useProviders((s) => s.state?.known ?? []).filter((k) => k.id !== "copilot");
  const [id, setId] = useState(known[0]?.id ?? "");
  const [key, setKey] = useState("");
  const chosen = known.find((k) => k.id === id);
  return (
    <form
      className="provider-form"
      onSubmit={(e) => {
        e.preventDefault();
        void onAdd(id, key).then((ok) => ok && setKey(""));
      }}
    >
      <label>
        <span>Provider</span>
        <select value={id} onChange={(e) => setId(e.target.value)}>
          {known.map((k) => (
            <option key={k.id} value={k.id}>
              {k.name}
              {k.has_key ? " (has a key)" : ""}
            </option>
          ))}
        </select>
      </label>
      <label>
        <span>API key</span>
        <input
          type="password"
          autoComplete="off"
          spellCheck={false}
          value={key}
          onChange={(e) => setKey(e.target.value)}
          placeholder="Paste the key"
        />
      </label>
      {chosen && (
        <p className="muted small">
          {chosen.has_key && !chosen.by_kairos
            ? "Hermes already has a key for it, set up outside Kairos; saving replaces it. "
            : ""}
          Get a key at{" "}
          <a href={chosen.keys_at} target="_blank" rel="noreferrer">
            {chosen.keys_at.replace(/^https:\/\//, "")}
          </a>
          .
        </p>
      )}
      <div className="actions">
        <button className="primary" type="submit" disabled={busy || key.trim().length < 8}>
          Save the key
        </button>
      </div>
    </form>
  );
}

function EndpointForm({
  onAdd,
  busy,
}: {
  onAdd: (name: string, api: string, key: string | null, models: string[]) => Promise<boolean>;
  busy: boolean;
}) {
  const presets = useProviders((s) => s.state?.presets ?? []);
  const [name, setName] = useState("");
  const [api, setApi] = useState("");
  const [key, setKey] = useState("");
  const [models, setModels] = useState("");
  return (
    <form
      className="provider-form"
      onSubmit={(e) => {
        e.preventDefault();
        void onAdd(name, api, key.trim() || null, parseModels(models)).then((ok) => {
          if (ok) {
            setName("");
            setApi("");
            setKey("");
            setModels("");
          }
        });
      }}
    >
      <label>
        <span>Start from</span>
        <select
          value=""
          onChange={(e) => {
            const preset = presets.find((p) => p.name === e.target.value);
            if (preset) {
              setName(preset.name.replace(/\s*\(.*\)$/, ""));
              setApi(preset.api);
            }
          }}
        >
          <option value="">Any OpenAI-compatible API…</option>
          {presets.map((p) => (
            <option key={p.name} value={p.name}>
              {p.name}
            </option>
          ))}
        </select>
      </label>
      <label>
        <span>Name</span>
        <input value={name} onChange={(e) => setName(e.target.value)} placeholder="My gateway" />
      </label>
      {name.trim() && <p className="muted small">Hermes will know it as “{slugOf(name)}”.</p>}
      <label>
        <span>Address</span>
        <input
          value={api}
          onChange={(e) => setApi(e.target.value)}
          placeholder="https://api.example.com/v1"
          spellCheck={false}
        />
      </label>
      <label>
        <span>API key (empty if it needs none)</span>
        <input
          type="password"
          autoComplete="off"
          spellCheck={false}
          value={key}
          onChange={(e) => setKey(e.target.value)}
        />
      </label>
      <label>
        <span>Models (optional, commas between; empty to ask the endpoint)</span>
        <input
          value={models}
          onChange={(e) => setModels(e.target.value)}
          placeholder="llama-3.3-70b-versatile, mixtral-8x7b"
          spellCheck={false}
        />
      </label>
      <div className="actions">
        <button
          className="primary"
          type="submit"
          disabled={busy || !slugOf(name) || !/^https?:\/\//.test(api.trim())}
        >
          Add the endpoint
        </button>
      </div>
    </form>
  );
}

function CopilotForm({
  onAdd,
  busy,
}: {
  onAdd: (token: string) => Promise<boolean>;
  busy: boolean;
}) {
  const [token, setToken] = useState("");
  return (
    <form
      className="provider-form"
      onSubmit={(e) => {
        e.preventDefault();
        void onAdd(token).then((ok) => ok && setToken(""));
      }}
    >
      <p className="muted small">
        Copilot&apos;s GPT, Claude and Gemini models, on your Copilot plan. Make a{" "}
        <a
          href="https://github.com/settings/personal-access-tokens/new"
          target="_blank"
          rel="noreferrer"
        >
          fine-grained GitHub token
        </a>{" "}
        with the <strong>Copilot Requests</strong> permission (classic ghp_ tokens don&apos;t work),
        and paste it here.
      </p>
      <p className="muted small">
        Worth knowing: a student plan has a limited agent allowance, and each Kairos request makes
        several model calls. GitHub officially supports Copilot in some other tools; Hermes
        isn&apos;t one of the announced ones.
      </p>
      <label>
        <span>GitHub token</span>
        <input
          type="password"
          autoComplete="off"
          spellCheck={false}
          value={token}
          onChange={(e) => setToken(e.target.value)}
          placeholder="github_pat_…"
        />
      </label>
      <div className="actions">
        <button className="primary" type="submit" disabled={busy || token.trim().length < 8}>
          Save the token
        </button>
      </div>
    </form>
  );
}
