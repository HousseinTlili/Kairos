//! Models and cost (M7 §4–§5, D-020): the catalogue from `/api/model/options`, read leniently
//! (there is no recorded fixture of it yet; the shape is from Hermes's source), prices in dollars
//! per million tokens, and the estimated cost of a run from its `usage`.

use serde::Serialize;
use serde_json::Value;

use crate::hermes::events::Usage;

/// Dollars per million tokens. `cache` is the price of a cached input token, when the provider
/// has one.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Price {
    pub input: f64,
    pub output: f64,
    pub cache: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelEntry {
    pub id: String,
    /// `None` when Hermes has no price for it ("price unknown").
    pub price: Option<Price>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProviderModels {
    pub slug: String,
    pub name: String,
    pub is_current: bool,
    pub models: Vec<ModelEntry>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct Catalog {
    pub current_provider: Option<String>,
    pub current_model: Option<String>,
    pub providers: Vec<ProviderModels>,
}

/// Hermes's price text: `"$1.25"` (per Mtok), `"free"`, or `"?"` / `""` when unknown
/// [source: `hermes_cli/models_pricing.py` `_format_price_per_mtok`].
pub fn parse_price(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.eq_ignore_ascii_case("free") {
        return Some(0.0);
    }
    text.trim_start_matches('$')
        .replace(',', "")
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0)
}

fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn price_entry(value: &Value) -> Option<Price> {
    let input = parse_price(value.get("input")?.as_str()?)?;
    // A missing output price with a free input is free (Hermes marks such entries `free`).
    let output = match value.get("output").and_then(Value::as_str) {
        Some(out) if !out.is_empty() => parse_price(out)?,
        _ if value.get("free").and_then(Value::as_bool) == Some(true) => 0.0,
        _ => return None,
    };
    let cache = value
        .get("cache")
        .and_then(Value::as_str)
        .and_then(parse_price);
    Some(Price {
        input,
        output,
        cache,
    })
}

/// Reads the payload; anything unexpected is skipped rather than failing the whole picker.
pub fn parse_catalog(payload: &Value) -> Catalog {
    let providers = payload
        .get("providers")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let slug = text(row, "slug")?;
                    // A provider Hermes isn't signed in to can't serve a request.
                    if row.get("authenticated").and_then(Value::as_bool) == Some(false) {
                        return None;
                    }
                    let pricing = row.get("pricing");
                    let models = row
                        .get("models")
                        .and_then(Value::as_array)
                        .map(|list| {
                            list.iter()
                                .filter_map(Value::as_str)
                                .map(|id| ModelEntry {
                                    id: id.to_string(),
                                    price: pricing.and_then(|p| p.get(id)).and_then(price_entry),
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    Some(ProviderModels {
                        name: text(row, "name").unwrap_or_else(|| slug.clone()),
                        is_current: row.get("is_current").and_then(Value::as_bool) == Some(true),
                        slug,
                        models,
                    })
                })
                .filter(|p| !p.models.is_empty())
                .collect()
        })
        .unwrap_or_default();
    Catalog {
        current_provider: text(payload, "provider"),
        current_model: text(payload, "model"),
        providers,
    }
}

impl Catalog {
    pub fn price(&self, provider: &str, model: &str) -> Option<Price> {
        self.providers
            .iter()
            .find(|p| p.slug.eq_ignore_ascii_case(provider))?
            .models
            .iter()
            .find(|m| m.id == model)?
            .price
    }
}

/// The estimated cost in dollars. `input_tokens` is taken to include the cache reads, which are
/// priced at the cache price when there is one (D-020: an estimate, labelled as such).
pub fn estimate_cost(usage: &Usage, price: &Price) -> f64 {
    let cached = usage.cache_read_tokens.min(usage.input_tokens);
    let uncached = usage.input_tokens - cached;
    let per_token = |dollars_per_mtok: f64| dollars_per_mtok / 1_000_000.0;
    uncached as f64 * per_token(price.input)
        + cached as f64 * per_token(price.cache.unwrap_or(price.input))
        + usage.output_tokens as f64 * per_token(price.output)
}

/// Whether a running request has gone over its cap (D-020). No cap or no price: never.
#[allow(dead_code)] // M7: the cap watcher arrives in Step 5.
pub fn over_cap(cost: Option<f64>, cap: Option<f64>) -> bool {
    matches!((cost, cap), (Some(cost), Some(cap)) if cap > 0.0 && cost > cap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Invented, in the shape of `build_model_options_payload` [source]; not a recording.
    fn payload() -> Value {
        json!({
            "model": "example/large",
            "provider": "openrouter",
            "providers": [
                {
                    "slug": "openrouter", "name": "OpenRouter", "is_current": true,
                    "models": ["example/large", "example/free", "example/unpriced"],
                    "pricing": {
                        "example/large": {"input": "$3.00", "output": "$15.00", "cache": "$0.30", "free": false},
                        "example/free": {"input": "free", "output": "free", "cache": null, "free": true},
                        "example/unpriced": {"input": "?", "output": "?", "cache": null, "free": false}
                    }
                },
                {"slug": "llamacpp", "name": "Local models", "models": ["qwen-7b-q4"]},
                {"slug": "anthropic", "name": "Anthropic", "authenticated": false, "models": ["x"]},
                {"slug": "empty", "name": "Nothing", "models": []},
                {"name": "no slug", "models": ["y"]}
            ]
        })
    }

    #[test]
    fn prices_read_as_dollars_per_million_tokens() {
        assert_eq!(parse_price("$3.00"), Some(3.0));
        assert_eq!(parse_price("$0.0005"), Some(0.0005));
        assert_eq!(parse_price("free"), Some(0.0));
        assert_eq!(parse_price("$1,200.00"), Some(1200.0));
        for unknown in ["?", "", "soon", "$-1"] {
            assert_eq!(parse_price(unknown), None, "{unknown}");
        }
    }

    #[test]
    fn the_catalogue_keeps_usable_providers_and_their_prices() {
        let catalog = parse_catalog(&payload());
        assert_eq!(catalog.current_provider.as_deref(), Some("openrouter"));
        assert_eq!(catalog.current_model.as_deref(), Some("example/large"));
        let slugs: Vec<&str> = catalog.providers.iter().map(|p| p.slug.as_str()).collect();
        assert_eq!(
            slugs,
            ["openrouter", "llamacpp"],
            "unauthenticated and empty rows dropped"
        );
        assert!(catalog.providers[0].is_current);
        assert_eq!(
            catalog.price("openrouter", "example/large"),
            Some(Price {
                input: 3.0,
                output: 15.0,
                cache: Some(0.3)
            })
        );
        assert_eq!(
            catalog
                .price("openrouter", "example/free")
                .map(|p| p.output),
            Some(0.0)
        );
        assert_eq!(catalog.price("openrouter", "example/unpriced"), None);
        assert_eq!(catalog.price("llamacpp", "qwen-7b-q4"), None);
        assert_eq!(catalog.price("nobody", "x"), None);
    }

    #[test]
    fn a_payload_of_the_wrong_shape_is_an_empty_catalogue_not_a_crash() {
        for odd in [
            json!(null),
            json!([]),
            json!({"providers": "lots"}),
            json!({"providers": [1, 2]}),
        ] {
            assert!(parse_catalog(&odd).providers.is_empty());
        }
    }

    #[test]
    fn cost_prices_cached_input_at_the_cache_price() {
        let usage = Usage {
            input_tokens: 10_000,
            output_tokens: 1_000,
            total_tokens: 11_000,
            cache_read_tokens: 4_000,
            cache_write_tokens: 0,
        };
        let price = Price {
            input: 3.0,
            output: 15.0,
            cache: Some(0.3),
        };
        // 6,000 × $3 + 4,000 × $0.30 + 1,000 × $15, per million.
        let cost = estimate_cost(&usage, &price);
        assert!((cost - (0.018 + 0.0012 + 0.015)).abs() < 1e-12, "{cost}");
        let no_cache_price = Price {
            cache: None,
            ..price
        };
        assert!((estimate_cost(&usage, &no_cache_price) - (0.03 + 0.015)).abs() < 1e-12);
    }

    #[test]
    fn more_cache_reads_than_input_never_goes_negative() {
        let usage = Usage {
            input_tokens: 10,
            output_tokens: 0,
            total_tokens: 10,
            cache_read_tokens: 50,
            cache_write_tokens: 0,
        };
        let price = Price {
            input: 1.0,
            output: 1.0,
            cache: Some(0.1),
        };
        assert!(estimate_cost(&usage, &price) >= 0.0);
    }

    #[test]
    fn the_cap_stops_only_a_priced_run_that_went_over() {
        assert!(over_cap(Some(0.6), Some(0.5)));
        assert!(!over_cap(Some(0.5), Some(0.5)), "at the cap is not over it");
        assert!(!over_cap(None, Some(0.5)), "unknown price never stops");
        assert!(!over_cap(Some(9.0), None), "no cap");
        assert!(!over_cap(Some(9.0), Some(0.0)), "a zero cap means off");
    }
}
