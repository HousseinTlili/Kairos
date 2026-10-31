//! The Master's own AI providers (M18, D-038), added through Hermes's documented ways: a known
//! provider's key as its line in Hermes's `.env` (e.g. `OPENAI_API_KEY`), and an OpenAI-compatible
//! endpoint as a `providers.<name>` entry (`hermes config set`) whose key is a `.env` line it names
//! with `key_env`. Kairos keeps no key, never puts one on a command line, and never shows one
//! again. Every `.env` line it writes follows a marker comment, so it only ever removes its own.
//! The pure parts (the `.env` text, the entry, the checks) are tested; the file and CLI calls are
//! thin.

use serde::Serialize;

/// The comment Kairos writes above each of its `.env` lines: `# kairos: <what>`.
const MARKER: &str = "# kairos: ";

/// A provider Hermes knows, with the `.env` name it reads the key from (Hermes 0.22.5's
/// `environment-variables.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Known {
    pub id: &'static str,
    pub name: &'static str,
    pub env: &'static str,
    /// Where to make a key.
    pub keys_at: &'static str,
}

pub const KNOWN: &[Known] = &[
    Known {
        id: "openai-api",
        name: "OpenAI",
        env: "OPENAI_API_KEY",
        keys_at: "https://platform.openai.com/api-keys",
    },
    Known {
        id: "anthropic",
        name: "Anthropic (Claude)",
        env: "ANTHROPIC_API_KEY",
        keys_at: "https://console.anthropic.com/settings/keys",
    },
    Known {
        id: "gemini",
        name: "Google Gemini",
        env: "GOOGLE_API_KEY",
        keys_at: "https://aistudio.google.com/app/apikey",
    },
    Known {
        id: "openrouter",
        name: "OpenRouter",
        env: "OPENROUTER_API_KEY",
        keys_at: "https://openrouter.ai/keys",
    },
    Known {
        id: "deepseek",
        name: "DeepSeek",
        env: "DEEPSEEK_API_KEY",
        keys_at: "https://platform.deepseek.com/api_keys",
    },
    Known {
        id: "xai",
        name: "xAI (Grok)",
        env: "XAI_API_KEY",
        keys_at: "https://console.x.ai",
    },
    Known {
        id: "huggingface",
        name: "Hugging Face",
        env: "HF_TOKEN",
        keys_at: "https://huggingface.co/settings/tokens",
    },
    Known {
        id: "nvidia",
        name: "NVIDIA Build",
        env: "NVIDIA_API_KEY",
        keys_at: "https://build.nvidia.com",
    },
    Known {
        id: "fireworks",
        name: "Fireworks AI",
        env: "FIREWORKS_API_KEY",
        keys_at: "https://fireworks.ai/account/api-keys",
    },
    Known {
        id: "novita",
        name: "NovitaAI",
        env: "NOVITA_API_KEY",
        keys_at: "https://novita.ai/settings/key-management",
    },
    Known {
        id: "zai",
        name: "z.ai (GLM)",
        env: "GLM_API_KEY",
        keys_at: "https://z.ai/manage-apikey/apikey-list",
    },
    Known {
        id: "kimi-coding",
        name: "Kimi (Moonshot)",
        env: "KIMI_API_KEY",
        keys_at: "https://platform.moonshot.ai/console/api-keys",
    },
    Known {
        id: "minimax",
        name: "MiniMax",
        env: "MINIMAX_API_KEY",
        keys_at: "https://www.minimax.io/platform",
    },
    Known {
        id: "alibaba",
        name: "Qwen (Alibaba DashScope)",
        env: "DASHSCOPE_API_KEY",
        keys_at: "https://dashscope.console.aliyun.com/apiKey",
    },
    Known {
        id: "copilot",
        name: "GitHub Copilot",
        env: "COPILOT_GITHUB_TOKEN",
        keys_at: "https://github.com/settings/personal-access-tokens/new",
    },
];

/// Ready-made OpenAI-compatible endpoints for services Hermes has no provider for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Preset {
    pub name: &'static str,
    pub api: &'static str,
    pub needs_key: bool,
}

pub const PRESETS: &[Preset] = &[
    Preset {
        name: "Groq",
        api: "https://api.groq.com/openai/v1",
        needs_key: true,
    },
    Preset {
        name: "Mistral",
        api: "https://api.mistral.ai/v1",
        needs_key: true,
    },
    Preset {
        name: "Together AI",
        api: "https://api.together.xyz/v1",
        needs_key: true,
    },
    Preset {
        name: "Ollama (this PC)",
        api: "http://127.0.0.1:11434/v1",
        needs_key: false,
    },
    Preset {
        name: "LM Studio (this PC)",
        api: "http://127.0.0.1:1234/v1",
        needs_key: false,
    },
];

pub fn known(id: &str) -> Option<&'static Known> {
    KNOWN.iter().find(|k| k.id == id)
}

/// What Kairos added, read back from its markers in `.env`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Added {
    /// A known provider's id, or an endpoint's name.
    pub id: String,
    pub endpoint: bool,
    /// The endpoint's address (empty for a known provider).
    pub api: String,
    pub env: String,
}

/// The `.env` name of an endpoint's key.
pub fn endpoint_env(slug: &str) -> String {
    format!("KAIROS_{}_API_KEY", slug.to_uppercase().replace('-', "_"))
}

/// An endpoint's name as Hermes's provider id: lower-case letters, digits and dashes.
pub fn slug(name: &str) -> Option<String> {
    let mut out = String::new();
    for c in name.trim().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_string();
    (!out.is_empty() && out.len() <= 32 && !KNOWN.iter().any(|k| k.id == out)).then_some(out)
}

/// A key Kairos will write: one line of printable characters, nothing a `.env` reader could cut
/// or read as something else (spaces, quotes, `#`, `\`, `=` at the start).
pub fn key_problem(key: &str) -> Option<&'static str> {
    if key.len() < 8 {
        return Some("That key is too short.");
    }
    if key.len() > 1024 {
        return Some("That key is too long.");
    }
    if key
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '"' | '\'' | '#' | '\\' | '`'))
    {
        return Some("A key has no spaces, quotes or # in it; check what was pasted.");
    }
    None
}

/// Copilot reads only some GitHub tokens (Hermes's Copilot docs).
pub fn copilot_problem(token: &str) -> Option<&'static str> {
    if token.starts_with("ghp_") {
        Some(
            "Copilot doesn't accept classic tokens (ghp_…). Make a fine-grained token with the Copilot Requests permission.",
        )
    } else if !(token.starts_with("github_pat_")
        || token.starts_with("gho_")
        || token.starts_with("ghu_"))
    {
        Some("That isn't a GitHub token (github_pat_…).")
    } else {
        None
    }
}

/// An endpoint's address: http(s), no spaces.
pub fn api_problem(api: &str) -> Option<&'static str> {
    let api = api.trim();
    if !(api.starts_with("https://") || api.starts_with("http://"))
        || api.contains(char::is_whitespace)
    {
        Some(
            "The address starts with https:// (or http:// for this PC), like https://api.example.com/v1.",
        )
    } else {
        None
    }
}

fn is_line_for(line: &str, name: &str) -> bool {
    let line = line.trim_start();
    let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
    line.split_once('=').is_some_and(|(k, _)| k.trim() == name)
}

/// Whether `.env` already sets `name` (by Kairos or not).
pub fn env_has(text: &str, name: &str) -> bool {
    text.lines().any(|l| is_line_for(l, name))
}

/// `.env` with `name=value` set by Kairos: any line already setting `name` is replaced (with the
/// marker before it if it was Kairos's), and the new line goes at the end under its marker.
/// Everything else is kept as it was, line endings included.
pub fn set_env(text: &str, name: &str, value: &str, what: &str) -> String {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut kept: Vec<&str> = Vec::new();
    for line in text.lines() {
        if is_line_for(line, name) {
            if kept.last().is_some_and(|l| l.starts_with(MARKER)) {
                kept.pop();
            }
            continue;
        }
        kept.push(line);
    }
    while kept.last().is_some_and(|l| l.trim().is_empty()) {
        kept.pop();
    }
    let mut out = kept.join(newline);
    if !out.is_empty() {
        out.push_str(newline);
    }
    out.push_str(&format!("{MARKER}{what}{newline}{name}={value}{newline}"));
    out
}

/// `.env` without Kairos's line for `name`; another program's line of the same name is left alone.
/// The second value says whether anything was removed.
pub fn remove_env(text: &str, name: &str) -> (String, bool) {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<&str> = Vec::new();
    let mut removed = false;
    let mut i = 0;
    while i < lines.len() {
        if lines[i].starts_with(MARKER) && i + 1 < lines.len() && is_line_for(lines[i + 1], name) {
            removed = true;
            i += 2;
            continue;
        }
        out.push(lines[i]);
        i += 1;
    }
    let mut joined = out.join(newline);
    if text.ends_with('\n') && !joined.is_empty() {
        joined.push_str(newline);
    }
    (joined, removed)
}

/// What Kairos added, from its markers: `known <id>` or `endpoint <slug> <api>`.
pub fn added(text: &str) -> Vec<Added> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    for pair in lines.windows(2) {
        let Some(what) = pair[0].strip_prefix(MARKER) else {
            continue;
        };
        let Some((env, _)) = pair[1].split_once('=') else {
            continue;
        };
        let mut words = what.split_whitespace();
        match (words.next(), words.next(), words.next()) {
            (Some("known"), Some(id), _) => out.push(Added {
                id: id.into(),
                endpoint: false,
                api: String::new(),
                env: env.trim().into(),
            }),
            (Some("endpoint"), Some(id), api) => out.push(Added {
                id: id.into(),
                endpoint: true,
                api: api.unwrap_or_default().into(),
                env: env.trim().into(),
            }),
            _ => {}
        }
    }
    out
}

/// The `providers.<slug>` entry for `hermes config set`, as a JSON literal (Hermes parses YAML/JSON
/// literals there). The key is named, never given.
pub fn endpoint_entry(api: &str, key_env: Option<&str>, models: &[String]) -> String {
    let mut entry = serde_json::Map::new();
    entry.insert("api".into(), serde_json::Value::from(api.trim()));
    if let Some(env) = key_env {
        entry.insert("key_env".into(), serde_json::Value::from(env));
    }
    let models: Vec<&str> = models
        .iter()
        .map(|m| m.trim())
        .filter(|m| !m.is_empty())
        .collect();
    if !models.is_empty() {
        entry.insert("models".into(), serde_json::Value::from(models));
    }
    serde_json::Value::Object(entry).to_string()
}

// --- The file and the CLI ------------------------------------------------------------------------

use std::path::{Path, PathBuf};

fn env_path(home: &Path) -> PathBuf {
    home.join(".env")
}

fn read_env(home: &Path) -> Result<String, String> {
    match std::fs::read_to_string(env_path(home)) {
        Ok(text) => Ok(text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(err) => Err(format!("couldn't read Hermes's .env: {err}")),
    }
}

/// Written next to it, then moved over it: a crash mid-write never leaves half a `.env`.
fn write_env(home: &Path, text: &str) -> Result<(), String> {
    let path = env_path(home);
    let tmp = home.join(".env.kairos-tmp");
    std::fs::write(&tmp, text).map_err(|err| format!("couldn't write Hermes's .env: {err}"))?;
    std::fs::rename(&tmp, &path).map_err(|err| {
        let _ = std::fs::remove_file(&tmp);
        format!("couldn't write Hermes's .env: {err}")
    })
}

/// What the page shows: the providers Kairos can add, and what it added.
#[derive(Debug, Clone, Serialize)]
pub struct ProvidersState {
    pub known: Vec<KnownState>,
    pub presets: &'static [Preset],
    pub endpoints: Vec<Added>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KnownState {
    #[serde(flatten)]
    pub provider: Known,
    /// Hermes has a key for it (from Kairos or set up before).
    pub has_key: bool,
    /// The key is Kairos's line, so Kairos may remove it.
    pub by_kairos: bool,
}

pub fn state(home: &Path) -> Result<ProvidersState, String> {
    let text = read_env(home)?;
    let mine = added(&text);
    Ok(ProvidersState {
        known: KNOWN
            .iter()
            .map(|k| KnownState {
                provider: *k,
                // Gemini reads either name.
                has_key: env_has(&text, k.env)
                    || (k.id == "gemini" && env_has(&text, "GEMINI_API_KEY")),
                by_kairos: mine.iter().any(|a| !a.endpoint && a.id == k.id),
            })
            .collect(),
        presets: PRESETS,
        endpoints: mine.into_iter().filter(|a| a.endpoint).collect(),
    })
}

pub fn add_key(home: &Path, id: &str, key: &str) -> Result<(), String> {
    let provider = known(id).ok_or("Kairos doesn't know that provider")?;
    let key = key.trim();
    if let Some(problem) = key_problem(key) {
        return Err(problem.into());
    }
    if provider.id == "copilot"
        && let Some(problem) = copilot_problem(key)
    {
        return Err(problem.into());
    }
    let text = read_env(home)?;
    write_env(
        home,
        &set_env(&text, provider.env, key, &format!("known {}", provider.id)),
    )
}

pub fn add_endpoint(
    home: &Path,
    name: &str,
    api: &str,
    key: Option<&str>,
    models: &[String],
) -> Result<String, String> {
    let slug = slug(name)
        .ok_or("Give it a short name of letters and digits (not one of Hermes's own providers).")?;
    if let Some(problem) = api_problem(api) {
        return Err(problem.into());
    }
    let key = key.map(str::trim).filter(|k| !k.is_empty());
    if let Some(k) = key
        && let Some(problem) = key_problem(k)
    {
        return Err(problem.into());
    }
    let env = endpoint_env(&slug);
    let before = read_env(home)?;
    if let Some(k) = key {
        write_env(
            home,
            &set_env(&before, &env, k, &format!("endpoint {slug} {}", api.trim())),
        )?;
    }
    let entry = endpoint_entry(api, key.map(|_| env.as_str()), models);
    let path = format!("providers.{slug}");
    if let Err(err) = crate::hermes::local::run_cli(home, &["config", "set", &path, &entry]) {
        // Hermes refused the entry: its key line goes too.
        let _ = write_env(home, &before);
        return Err(err);
    }
    if key.is_none() {
        // Still marked, so the page lists it and Remove can find it.
        let text = read_env(home)?;
        write_env(
            home,
            &set_env(
                &text,
                &env,
                "none",
                &format!("endpoint {slug} {}", api.trim()),
            ),
        )?;
    }
    Ok(slug)
}

/// Removes what Kairos added: a known provider's key line, or an endpoint and its key line.
pub fn remove(home: &Path, id: &str, endpoint: bool) -> Result<(), String> {
    let text = read_env(home)?;
    let mine = added(&text);
    let item = mine
        .iter()
        .find(|a| a.id == id && a.endpoint == endpoint)
        .ok_or("Kairos didn't add that, so it leaves it alone")?;
    if endpoint {
        crate::hermes::local::run_cli(home, &["config", "unset", &format!("providers.{id}")])?;
    }
    let (text, _) = remove_env(&text, &item.env);
    write_env(home, &text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENV: &str = "# Hermes\nAPI_SERVER_KEY=abc123\nOPENAI_API_KEY=sk-theirs\n\nTELEGRAM=1\n";

    #[test]
    fn a_kairos_key_goes_at_the_end_under_its_marker_and_everything_else_stays() {
        let out = set_env(ENV, "DEEPSEEK_API_KEY", "sk-new-key-1", "known deepseek");
        assert!(out.starts_with(
            "# Hermes\nAPI_SERVER_KEY=abc123\nOPENAI_API_KEY=sk-theirs\n\nTELEGRAM=1\n"
        ));
        assert!(out.ends_with("# kairos: known deepseek\nDEEPSEEK_API_KEY=sk-new-key-1\n"));
    }

    #[test]
    fn setting_a_key_again_replaces_it_and_its_marker_once() {
        let once = set_env(ENV, "XAI_API_KEY", "xai-11111111", "known xai");
        let twice = set_env(&once, "XAI_API_KEY", "xai-22222222", "known xai");
        assert_eq!(twice.matches("XAI_API_KEY=").count(), 1);
        assert_eq!(twice.matches("# kairos: known xai").count(), 1);
        assert!(twice.contains("XAI_API_KEY=xai-22222222"));
    }

    #[test]
    fn windows_line_endings_are_kept() {
        let crlf = ENV.replace('\n', "\r\n");
        let out = set_env(&crlf, "HF_TOKEN", "hf_abcdefgh", "known huggingface");
        assert!(out.ends_with("\r\nHF_TOKEN=hf_abcdefgh\r\n"));
        assert!(!out.replace("\r\n", "").contains('\n'));
    }

    #[test]
    fn removing_takes_only_kaiross_own_line() {
        let with = set_env(
            ENV,
            "GROQ_KEY",
            "gsk_12345678",
            "endpoint groq https://api.groq.com/openai/v1",
        );
        let (back, removed) = remove_env(&with, "GROQ_KEY");
        assert!(removed);
        assert_eq!(back.trim_end(), ENV.trim_end());
        // A key the Master set up himself (no marker) is never removed.
        let (same, removed) = remove_env(ENV, "OPENAI_API_KEY");
        assert!(!removed);
        assert_eq!(same, ENV);
    }

    #[test]
    fn what_kairos_added_is_read_back_from_its_markers() {
        let text = set_env(ENV, "DEEPSEEK_API_KEY", "sk-11111111", "known deepseek");
        let text = set_env(
            &text,
            "KAIROS_GROQ_API_KEY",
            "gsk_11111111",
            "endpoint groq https://api.groq.com/openai/v1",
        );
        let found = added(&text);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].id, "deepseek");
        assert!(!found[0].endpoint);
        assert_eq!(found[1].id, "groq");
        assert_eq!(found[1].api, "https://api.groq.com/openai/v1");
        assert_eq!(found[1].env, "KAIROS_GROQ_API_KEY");
    }

    #[test]
    fn a_key_that_a_dotenv_reader_could_cut_is_refused() {
        assert!(key_problem("sk-proj-abcDEF123_-.").is_none());
        assert!(key_problem("sk abc defgh").is_some(), "a space");
        assert!(key_problem("sk-abc#defgh").is_some(), "a comment sign");
        assert!(key_problem("\"sk-abcdefg\"").is_some(), "quotes");
        assert!(key_problem("short").is_some());
    }

    #[test]
    fn copilot_takes_only_tokens_it_accepts() {
        assert!(copilot_problem("ghp_classic123456").is_some());
        assert!(copilot_problem("github_pat_11ABCDEF").is_none());
        assert!(copilot_problem("sk-something").is_some());
    }

    #[test]
    fn an_endpoint_name_becomes_a_safe_provider_id() {
        assert_eq!(slug("My Gateway!").as_deref(), Some("my-gateway"));
        assert_eq!(slug("  Groq  ").as_deref(), Some("groq"));
        assert_eq!(slug("!!!"), None);
        assert_eq!(slug("openrouter"), None, "one of Hermes's own");
        assert_eq!(endpoint_env("my-gateway"), "KAIROS_MY_GATEWAY_API_KEY");
    }

    #[test]
    fn the_endpoint_entry_names_its_key_and_never_holds_it() {
        let entry = endpoint_entry(
            " https://api.groq.com/openai/v1 ",
            Some("KAIROS_GROQ_API_KEY"),
            &["llama-3.3-70b".into(), " ".into()],
        );
        let v: serde_json::Value = serde_json::from_str(&entry).expect("json");
        assert_eq!(v["api"], "https://api.groq.com/openai/v1");
        assert_eq!(v["key_env"], "KAIROS_GROQ_API_KEY");
        assert_eq!(v["models"], serde_json::json!(["llama-3.3-70b"]));
        assert!(!entry.contains("gsk_"));
        let local = endpoint_entry("http://127.0.0.1:11434/v1", None, &[]);
        assert_eq!(local, r#"{"api":"http://127.0.0.1:11434/v1"}"#);
    }

    #[test]
    fn an_address_is_http_or_https() {
        assert!(api_problem("https://api.example.com/v1").is_none());
        assert!(api_problem("http://127.0.0.1:1234/v1").is_none());
        assert!(api_problem("api.example.com").is_some());
        assert!(api_problem("javascript:alert(1)").is_some());
    }
}
