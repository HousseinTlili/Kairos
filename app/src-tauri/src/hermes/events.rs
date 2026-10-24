//! `RunEvent`, the run event types in M1 §7.
//!
//! Every field a Hermes event might omit is `#[serde(default)]`, so a missing field never turns a
//! known event into an error. Events Kairos doesn't know, or known events whose shape doesn't
//! match, become `RunEvent::Unknown` with the raw JSON kept for logging (M1 §6).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::errors::HermesError;
use super::sse::SseFrame;

/// The fields every run event carries, plus the event itself.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RunEventEnvelope {
    pub run_id: String,
    pub seq: Option<u64>,
    /// Unix seconds.
    pub timestamp: Option<f64>,
    pub event: RunEvent,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum RunEvent {
    #[serde(rename = "message.delta")]
    MessageDelta { delta: String },

    #[serde(rename = "message.interim")]
    MessageInterim {
        #[serde(default)]
        text: String,
        #[serde(default)]
        already_streamed: bool,
    },

    #[serde(rename = "tool.started")]
    ToolStarted {
        tool: String,
        #[serde(default)]
        preview: String,
        #[serde(default)]
        tool_call_id: Option<String>,
    },

    #[serde(rename = "tool.completed")]
    ToolCompleted {
        tool: String,
        /// Seconds; Hermes 0.22.5 reported 0 for every call recorded so far.
        #[serde(default)]
        duration: f64,
        /// True for a failed, stopped or denied tool call (M1 §9 rules 4–5).
        #[serde(default)]
        error: bool,
        #[serde(default)]
        tool_call_id: Option<String>,
        #[serde(default)]
        preview: String,
    },

    #[serde(rename = "approval.request")]
    ApprovalRequest {
        prompt_id: String,
        execution_generation: u32,
        /// The command Hermes will actually run, which can differ from `tool.started.preview`
        /// (M1 §9 rule 3).
        #[serde(default)]
        command: String,
        #[serde(default)]
        description: String,
        #[serde(default)]
        choices: Vec<String>,
    },

    /// Hermes doesn't say which choice settled it (M1 §7).
    #[serde(rename = "approval.settled")]
    ApprovalSettled {
        prompt_id: String,
        execution_generation: u32,
    },

    #[serde(rename = "run.completed")]
    RunCompleted {
        #[serde(default)]
        output: String,
        #[serde(default)]
        usage: Usage,
        #[serde(default)]
        completed: bool,
        #[serde(default)]
        partial: bool,
        #[serde(default)]
        interrupted: bool,
    },

    #[serde(rename = "run.cancelled")]
    RunCancelled {
        #[serde(default)]
        turn_exit_reason: Option<String>,
    },

    /// Documented, not yet recorded live: the shape of `error` is unknown, so it is kept raw.
    #[serde(rename = "run.failed")]
    RunFailed {
        #[serde(default)]
        error: Option<Value>,
    },

    /// Documented, not yet recorded live.
    #[serde(rename = "run.interrupted")]
    RunInterrupted {
        #[serde(default)]
        error: Option<Value>,
    },

    /// Documented, not yet recorded live; the payload is kept raw until a fixture exists.
    #[serde(rename = "subagent.start")]
    SubagentStart {
        #[serde(flatten)]
        raw: Value,
    },

    #[serde(rename = "subagent.complete")]
    SubagentComplete {
        #[serde(flatten)]
        raw: Value,
    },

    /// Never produced by Hermes; Kairos's catch-all. `name` is the event's `event` field.
    #[serde(rename = "unknown", skip_deserializing)]
    Unknown { name: String, raw: Value },
}

impl RunEvent {
    /// Ends the run's event stream.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            RunEvent::RunCompleted { .. }
                | RunEvent::RunCancelled { .. }
                | RunEvent::RunFailed { .. }
                | RunEvent::RunInterrupted { .. }
        )
    }
}

/// Turns one SSE frame into a run event. Only data that isn't a JSON object is an error; an
/// unrecognised or malformed event is returned as `RunEvent::Unknown`.
pub fn parse_run_event(frame: &SseFrame) -> Result<RunEventEnvelope, HermesError> {
    let value: Value = serde_json::from_str(&frame.data)
        .map_err(|err| HermesError::Parse(format!("event data is not JSON: {err}")))?;
    let Value::Object(fields) = &value else {
        return Err(HermesError::Parse("event data is not a JSON object".into()));
    };

    let run_id = fields
        .get("run_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let seq = fields.get("seq").and_then(Value::as_u64);
    let timestamp = fields.get("timestamp").and_then(Value::as_f64);
    let name = fields
        .get("event")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let event = serde_json::from_value::<RunEvent>(value.clone())
        .unwrap_or(RunEvent::Unknown { name, raw: value });

    Ok(RunEventEnvelope {
        run_id,
        seq,
        timestamp,
        event,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(data: &str) -> SseFrame {
        SseFrame {
            id: None,
            event: None,
            data: data.to_string(),
        }
    }

    #[test]
    fn an_unknown_event_is_kept_raw_not_rejected() {
        let parsed = parse_run_event(&frame(
            r#"{"event": "brand.new", "run_id": "run_1", "seq": 3, "extra": [1]}"#,
        ))
        .expect("unknown events are not errors");
        assert_eq!(parsed.seq, Some(3));
        match parsed.event {
            RunEvent::Unknown { name, raw } => {
                assert_eq!(name, "brand.new");
                assert_eq!(raw["extra"][0], 1);
            }
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[test]
    fn a_known_event_with_a_missing_required_field_becomes_unknown() {
        let parsed = parse_run_event(&frame(r#"{"event": "approval.settled", "run_id": "r"}"#))
            .expect("not an error");
        assert!(
            matches!(parsed.event, RunEvent::Unknown { ref name, .. } if name == "approval.settled")
        );
    }

    #[test]
    fn data_that_is_not_json_is_a_parse_error() {
        assert!(matches!(
            parse_run_event(&frame("not json")),
            Err(HermesError::Parse(_))
        ));
        assert!(matches!(
            parse_run_event(&frame("[1, 2]")),
            Err(HermesError::Parse(_))
        ));
    }

    #[test]
    fn a_run_failed_event_with_any_error_shape_is_terminal() {
        let parsed = parse_run_event(&frame(
            r#"{"event": "run.failed", "run_id": "r", "error": {"message": "boom"}}"#,
        ))
        .expect("parses");
        assert!(parsed.event.is_terminal());
    }

    #[test]
    fn missing_optional_fields_take_defaults() {
        let parsed = parse_run_event(&frame(
            r#"{"event": "tool.completed", "run_id": "r", "tool": "x"}"#,
        ))
        .expect("parses");
        assert_eq!(
            parsed.event,
            RunEvent::ToolCompleted {
                tool: "x".into(),
                duration: 0.0,
                error: false,
                tool_call_id: None,
                preview: String::new(),
            }
        );
    }
}
