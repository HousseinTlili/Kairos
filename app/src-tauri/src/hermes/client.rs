//! `HermesClient`: the HTTP calls in M1 §5.
//!
//! All Hermes traffic goes through here, from Rust, never from the web view (CORS stays off).

use std::collections::VecDeque;
use std::fmt;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::errors::{HermesError, error_from_response};
use super::events::{RunEventEnvelope, Runtime, Usage, parse_run_event};
use super::sse::{SseItem, SseParser};

pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8642";

/// Capability flags Kairos can't work without (M1 §5).
pub const REQUIRED_CAPABILITIES: &[&str] = &[
    "run_submission",
    "run_status",
    "run_events_sse",
    "run_stop",
    "run_approval_response",
    "approval_events",
    "tool_progress_events",
    "session_resources",
];

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
// Ordinary calls answer quickly; event streams have no overall timeout because runs can be long.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// The Hermes `API_SERVER_KEY`. Debug output never shows it, and there is no `Display`.
#[derive(Clone)]
pub struct ApiKey(String);

impl ApiKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// The raw key, for the Authorization header and the keyring only. Never log or display it.
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl PartialEq for ApiKey {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey(<redacted>)")
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Health {
    pub status: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Capabilities {
    #[serde(default)]
    pub features: Map<String, Value>,
}

impl Capabilities {
    /// Required flags that are absent or not `true`.
    pub fn missing_required(&self) -> Vec<String> {
        REQUIRED_CAPABILITIES
            .iter()
            .filter(|name| self.features.get(**name) != Some(&Value::Bool(true)))
            .map(|name| name.to_string())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CreateRunRequest<'a> {
    pub input: &'a str,
    pub session_id: &'a str,
    /// The conversation's chosen model (M7 §4); left out for Hermes's own choice and fallbacks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CreateRunResponse {
    pub run_id: String,
    pub status: String,
    /// True when Hermes recognised the Idempotency-Key and returned the existing run.
    #[serde(default)]
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PendingControl {
    pub kind: String,
    pub prompt_id: String,
    pub execution_generation: u32,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub choices: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RunStatus {
    pub run_id: String,
    pub status: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub pending_controls: Vec<PendingControl>,
    #[serde(default)]
    pub output: String,
    #[serde(default)]
    pub usage: Usage,
    /// The provider and model serving the run (M7); documented for completed runs.
    #[serde(default)]
    pub runtime: Option<Runtime>,
    #[serde(default)]
    pub execution_generation: Option<u32>,
    #[serde(default)]
    pub admission_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalChoice {
    Once,
    Session,
    Always,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct ApprovalRequestBody<'a> {
    request_id: &'a str,
    execution_generation: u32,
    choice: ApprovalChoice,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ApprovalResponse {
    pub run_id: String,
    pub status: String,
    #[serde(default)]
    pub prompt_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct StopResponse {
    pub run_id: String,
    pub status: String,
    #[serde(default)]
    pub admission_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub started_at: Option<f64>,
    #[serde(default)]
    pub last_active: Option<f64>,
    #[serde(default)]
    pub preview: Option<String>,
    #[serde(default)]
    pub message_count: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SessionList {
    #[serde(default)]
    pub data: Vec<SessionSummary>,
    #[serde(default)]
    pub has_more: bool,
}

#[derive(Debug, Clone)]
pub struct HermesClient {
    base_url: String,
    key: ApiKey,
    http: reqwest::Client,
}

impl HermesClient {
    pub fn new(base_url: impl Into<String>, key: ApiKey) -> Result<Self, HermesError> {
        let http = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .map_err(|err| HermesError::Unreachable(err.to_string()))?;
        Ok(Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            key,
            http,
        })
    }

    /// `GET /health`. Needs no key, so it tells "not running" apart from "key rejected".
    pub async fn health(&self) -> Result<Health, HermesError> {
        let request = self.http.get(self.url("/health")).timeout(REQUEST_TIMEOUT);
        read_json(request.send().await).await
    }

    pub async fn capabilities(&self) -> Result<Capabilities, HermesError> {
        self.get_json("/v1/capabilities").await
    }

    /// `POST /v1/runs`. The caller stores `idempotency_key` before calling and reuses it on every
    /// retry of the same request (M1 §9 rule 1).
    pub async fn create_run(
        &self,
        request: &CreateRunRequest<'_>,
        idempotency_key: &str,
    ) -> Result<CreateRunResponse, HermesError> {
        let builder = self
            .authed(self.http.post(self.url("/v1/runs")))
            .header("Idempotency-Key", idempotency_key)
            .json(request);
        read_json(builder.send().await).await
    }

    pub async fn run_status(&self, run_id: &str) -> Result<RunStatus, HermesError> {
        self.get_json(&format!("/v1/runs/{}", path_segment(run_id)?))
            .await
    }

    pub async fn answer_approval(
        &self,
        run_id: &str,
        prompt_id: &str,
        execution_generation: u32,
        choice: ApprovalChoice,
    ) -> Result<ApprovalResponse, HermesError> {
        let body = ApprovalRequestBody {
            request_id: prompt_id,
            execution_generation,
            choice,
        };
        self.post_json(
            &format!("/v1/runs/{}/approval", path_segment(run_id)?),
            &body,
        )
        .await
    }

    pub async fn stop_run(&self, run_id: &str) -> Result<StopResponse, HermesError> {
        self.post_json(
            &format!("/v1/runs/{}/stop", path_segment(run_id)?),
            &serde_json::json!({}),
        )
        .await
    }

    /// `GET /api/model/options` (M7 §4): providers, models and prices, read leniently by
    /// `models::parse_catalog`.
    pub async fn model_options(&self) -> Result<serde_json::Value, HermesError> {
        self.get_json("/api/model/options").await
    }

    pub async fn list_sessions(&self) -> Result<SessionList, HermesError> {
        self.get_json("/api/sessions").await
    }

    /// `GET /v1/runs/{id}/events`. The stream replays from `seq` 0 on every subscription
    /// (M1 §6), so callers decide what to show from `run_status`, not from old events.
    pub async fn run_events(&self, run_id: &str) -> Result<RunEventStream, HermesError> {
        let url = self.url(&format!("/v1/runs/{}/events", path_segment(run_id)?));
        let response = self
            .authed(self.http.get(url))
            .header("Accept", "text/event-stream")
            .send()
            .await
            .map_err(unreachable)?;
        let response = check_status(response).await?;
        Ok(RunEventStream::new(response))
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn authed(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        builder.bearer_auth(self.key.expose())
    }

    async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, HermesError> {
        let builder = self
            .authed(self.http.get(self.url(path)))
            .timeout(REQUEST_TIMEOUT);
        read_json(builder.send().await).await
    }

    async fn post_json<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, HermesError> {
        let builder = self
            .authed(self.http.post(self.url(path)))
            .timeout(REQUEST_TIMEOUT)
            .json(body);
        read_json(builder.send().await).await
    }
}

/// Run and prompt IDs go into URL paths; anything but Hermes's own ID characters is refused
/// rather than escaped, since a valid Hermes ID never needs escaping.
fn path_segment(id: &str) -> Result<&str, HermesError> {
    let valid = !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if valid {
        Ok(id)
    } else {
        Err(HermesError::InvalidArgument(format!(
            "not a Hermes id: {id:?}"
        )))
    }
}

// reqwest's own text for a refused connection is just "error sending request", which tells the
// user nothing; name the common cases. Its text never includes headers, so it carries no key.
fn unreachable(err: reqwest::Error) -> HermesError {
    let reason = if err.is_connect() {
        "nothing is accepting connections on that address".to_string()
    } else if err.is_timeout() {
        "it didn't answer in time".to_string()
    } else {
        err.without_url().to_string()
    };
    HermesError::Unreachable(reason)
}

async fn check_status(response: reqwest::Response) -> Result<reqwest::Response, HermesError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let body = response.text().await.unwrap_or_default();
    Err(error_from_response(status.as_u16(), &body))
}

async fn read_json<T: DeserializeOwned>(
    sent: Result<reqwest::Response, reqwest::Error>,
) -> Result<T, HermesError> {
    let response = check_status(sent.map_err(unreachable)?).await?;
    let body = response.text().await.map_err(unreachable)?;
    serde_json::from_str(&body).map_err(|err| HermesError::Parse(err.to_string()))
}

/// Events of one run, in order. Ends after the terminal event and Hermes's `stream closed`.
pub struct RunEventStream {
    response: reqwest::Response,
    parser: Option<SseParser>,
    pending: VecDeque<Result<RunEventEnvelope, HermesError>>,
    done: bool,
}

impl RunEventStream {
    fn new(response: reqwest::Response) -> Self {
        Self {
            response,
            parser: Some(SseParser::new()),
            pending: VecDeque::new(),
            done: false,
        }
    }

    /// The next event, an error for one frame that couldn't be read, or `None` at the end.
    /// A connection that drops mid-event yields an `Unreachable` error before `None`.
    pub async fn next(&mut self) -> Option<Result<RunEventEnvelope, HermesError>> {
        loop {
            if let Some(item) = self.pending.pop_front() {
                return Some(item);
            }
            if self.done {
                return None;
            }
            match self.response.chunk().await {
                Ok(Some(bytes)) => {
                    let Some(parser) = self.parser.as_mut() else {
                        self.done = true;
                        continue;
                    };
                    for item in parser.push(&bytes) {
                        match item {
                            SseItem::Frame(frame) => {
                                self.pending.push_back(parse_run_event(&frame))
                            }
                            SseItem::Comment(_) if item.is_stream_closed() => self.done = true,
                            SseItem::Comment(_) => {}
                        }
                    }
                }
                Ok(None) => {
                    self.done = true;
                    if self.parser.take().is_some_and(SseParser::finish) {
                        self.pending.push_back(Err(HermesError::Unreachable(
                            "the event stream ended in the middle of an event".into(),
                        )));
                    }
                }
                Err(err) => {
                    self.done = true;
                    self.pending.push_back(Err(unreachable(err)));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_request_names_a_model_only_when_the_conversation_chose_one() {
        let default = serde_json::to_value(CreateRunRequest {
            input: "hi",
            session_id: "kairos-s",
            model: None,
            provider: None,
        })
        .expect("json");
        assert_eq!(
            default,
            serde_json::json!({"input": "hi", "session_id": "kairos-s"})
        );
        let chosen = serde_json::to_value(CreateRunRequest {
            input: "hi",
            session_id: "kairos-s",
            model: Some("example/large"),
            provider: Some("openrouter"),
        })
        .expect("json");
        assert_eq!(chosen["model"], "example/large");
        assert_eq!(chosen["provider"], "openrouter");
    }

    #[test]
    fn the_api_key_never_appears_in_debug_output() {
        let key = ApiKey::new("0123456789abcdef0123456789abcdef");
        let client = HermesClient::new(DEFAULT_BASE_URL, key.clone()).expect("client builds");
        assert!(!format!("{key:?}").contains("0123456789abcdef"));
        assert!(!format!("{client:?}").contains("0123456789abcdef"));
    }

    #[test]
    fn ids_with_path_characters_are_refused() {
        assert!(path_segment("run_8790e213c48849b7b918204bd99b9a79").is_ok());
        for bad in ["", "../health", "run/1", "run 1", "run?x=1"] {
            assert!(
                matches!(path_segment(bad), Err(HermesError::InvalidArgument(_))),
                "{bad:?} accepted"
            );
        }
    }

    #[test]
    fn approval_choices_serialise_as_hermes_expects() {
        let body = ApprovalRequestBody {
            request_id: "p",
            execution_generation: 1,
            choice: ApprovalChoice::Deny,
        };
        assert_eq!(
            serde_json::to_value(&body).expect("serialises"),
            serde_json::json!({"request_id": "p", "execution_generation": 1, "choice": "deny"})
        );
    }

    #[test]
    fn a_trailing_slash_on_the_base_url_is_ignored() {
        let client =
            HermesClient::new("http://127.0.0.1:8642/", ApiKey::new("k")).expect("client builds");
        assert_eq!(client.url("/health"), "http://127.0.0.1:8642/health");
    }
}
