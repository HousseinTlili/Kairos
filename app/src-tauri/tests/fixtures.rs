//! The Hermes client against real responses recorded from Hermes 0.22.5 (fixtures/README.md).
//! Fixtures are the truth: if one of these fails, the code is wrong, not the fixture.

use std::fs;
use std::path::PathBuf;

use kairos_lib::hermes::client::{
    ApprovalResponse, Capabilities, CreateRunResponse, Health, RunStatus, StopResponse,
};
use kairos_lib::hermes::errors::{HermesError, error_from_response};
use kairos_lib::hermes::events::{RunEvent, RunEventEnvelope, parse_run_event};
use kairos_lib::hermes::sse::{SseItem, SseParser};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/hermes-0.22.5")
}

fn read(path: &str) -> String {
    let full = fixture_dir().join(path);
    fs::read_to_string(&full).unwrap_or_else(|err| panic!("reading {}: {err}", full.display()))
}

struct ParsedStream {
    events: Vec<RunEventEnvelope>,
    comments: Vec<String>,
    last_item_closes_stream: bool,
}

fn parse_items(items: Vec<SseItem>) -> ParsedStream {
    let last_item_closes_stream = items.last().is_some_and(SseItem::is_stream_closed);
    let mut events = Vec::new();
    let mut comments = Vec::new();
    for item in items {
        match item {
            SseItem::Frame(frame) => {
                events.push(parse_run_event(&frame).expect("every recorded frame is valid JSON"))
            }
            SseItem::Comment(text) => comments.push(text),
        }
    }
    ParsedStream {
        events,
        comments,
        last_item_closes_stream,
    }
}

/// Parses a recorded stream in one piece, and again one byte at a time to prove chunk
/// boundaries can't change the result.
fn parse_stream(path: &str) -> ParsedStream {
    let bytes = read(path).into_bytes();

    let mut whole = SseParser::new();
    let items = whole.push(&bytes);
    assert!(!whole.finish(), "{path}: recorded stream ends mid-event");

    let mut trickle = SseParser::new();
    let trickled: Vec<SseItem> = bytes.iter().flat_map(|b| trickle.push(&[*b])).collect();
    assert_eq!(items, trickled, "{path}: byte-at-a-time parse differs");

    parse_items(items)
}

/// Event names in order, with consecutive repeats collapsed to `name xN`.
fn sequence(events: &[RunEventEnvelope]) -> Vec<String> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for envelope in events {
        let name = match &envelope.event {
            RunEvent::MessageDelta { .. } => "message.delta",
            RunEvent::MessageInterim { .. } => "message.interim",
            RunEvent::ToolStarted { .. } => "tool.started",
            RunEvent::ToolCompleted { error: true, .. } => "tool.completed(error)",
            RunEvent::ToolCompleted { error: false, .. } => "tool.completed(ok)",
            RunEvent::ApprovalRequest { .. } => "approval.request",
            RunEvent::ApprovalSettled { .. } => "approval.settled",
            RunEvent::RunCompleted { .. } => "run.completed",
            RunEvent::RunCancelled { .. } => "run.cancelled",
            RunEvent::RunFailed { .. } => "run.failed",
            RunEvent::RunInterrupted { .. } => "run.interrupted",
            RunEvent::SubagentStart { .. } => "subagent.start",
            RunEvent::SubagentComplete { .. } => "subagent.complete",
            RunEvent::Unknown { .. } => "UNKNOWN",
        };
        match out.last_mut() {
            Some((last, count)) if last == name => *count += 1,
            _ => out.push((name.to_string(), 1)),
        }
    }
    out.into_iter()
        .map(|(name, count)| {
            if count == 1 {
                name
            } else {
                format!("{name} x{count}")
            }
        })
        .collect()
}

fn assert_well_formed(path: &str, stream: &ParsedStream) {
    assert!(
        stream.last_item_closes_stream,
        "{path}: no `stream closed` at the end"
    );
    assert_eq!(stream.comments.first().map(String::as_str), Some("open"));
    let run_ids: Vec<&str> = stream.events.iter().map(|e| e.run_id.as_str()).collect();
    assert!(
        run_ids.windows(2).all(|w| w[0] == w[1]),
        "{path}: events from more than one run"
    );
    let seqs: Vec<u64> = stream
        .events
        .iter()
        .map(|e| e.seq.expect("every event has a seq"))
        .collect();
    let expected: Vec<u64> = (0..seqs.len() as u64).collect();
    assert_eq!(seqs, expected, "{path}: seq is not 0, 1, 2, ...");
    let last = stream.events.last().expect("at least one event");
    assert!(
        last.event.is_terminal(),
        "{path}: last event isn't terminal"
    );
    assert!(
        stream.events[..stream.events.len() - 1]
            .iter()
            .all(|e| !e.event.is_terminal()),
        "{path}: terminal event before the end"
    );
}

/// The deltas joined. After a tool call Hermes streams a leading "\n\n" that the final `output`
/// doesn't have, so the UI must replace streamed text with `output` on completion (M1 §7).
fn streamed_text(events: &[RunEventEnvelope]) -> String {
    let joined: String = events
        .iter()
        .filter_map(|e| match &e.event {
            RunEvent::MessageDelta { delta } => Some(delta.as_str()),
            _ => None,
        })
        .collect();
    joined.trim().to_string()
}

fn final_output(events: &[RunEventEnvelope]) -> &str {
    match &events.last().expect("events").event {
        RunEvent::RunCompleted { output, .. } => output,
        other => panic!("expected run.completed, got {other:?}"),
    }
}

#[test]
fn every_recorded_stream_parses_without_errors_or_unknown_events() {
    let mut checked = 0;
    for entry in fs::read_dir(fixture_dir()).expect("fixture dir") {
        let dir = entry.expect("entry").path();
        if !dir.is_dir() {
            continue;
        }
        for file in fs::read_dir(&dir).expect("run dir") {
            let path = file.expect("file").path();
            if path.extension().and_then(|e| e.to_str()) != Some("sse") {
                continue;
            }
            let relative = path
                .strip_prefix(fixture_dir())
                .expect("inside fixtures")
                .to_string_lossy()
                .replace('\\', "/");
            let stream = parse_stream(&relative);
            assert_well_formed(&relative, &stream);
            assert!(
                !sequence(&stream.events).iter().any(|s| s == "UNKNOWN"),
                "{relative}: has events the client doesn't recognise: {:?}",
                sequence(&stream.events)
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 4, "expected the four recorded runs");
}

#[test]
fn a_simple_run_streams_its_answer_then_completes() {
    let stream = parse_stream("run-simple/02-events.sse");
    assert_eq!(
        sequence(&stream.events),
        ["message.delta x5", "run.completed"]
    );
    assert_eq!(streamed_text(&stream.events), "Kairos probe OK");
    assert_eq!(final_output(&stream.events), "Kairos probe OK");
    assert_eq!(stream.comments, ["open", "stream closed"]);
}

#[test]
fn smart_mode_ran_the_delete_without_an_approval_request() {
    let stream = parse_stream("run-smart-autoapprove/02-events.sse");
    assert_eq!(
        sequence(&stream.events),
        [
            "tool.started",
            "tool.completed(ok)",
            "message.delta x33",
            "run.completed"
        ]
    );
    assert_eq!(
        streamed_text(&stream.events),
        final_output(&stream.events),
        "streamed deltas add up to the final output"
    );
}

#[test]
fn a_denied_approval_blocks_the_tool_but_the_run_still_completes() {
    let stream = parse_stream("run-approval-deny/05-events.sse");
    assert_eq!(
        sequence(&stream.events),
        [
            "tool.started",
            "approval.request",
            "approval.settled",
            "tool.completed(error)",
            "message.delta x41",
            "run.completed"
        ]
    );
    assert!(stream.comments.iter().any(|c| c == "keepalive"));
    assert_eq!(streamed_text(&stream.events), final_output(&stream.events));

    let RunEvent::ToolStarted { preview, .. } = &stream.events[0].event else {
        panic!("first event is tool.started");
    };
    let RunEvent::ApprovalRequest {
        prompt_id,
        execution_generation,
        command,
        description,
        choices,
    } = &stream.events[1].event
    else {
        panic!("second event is approval.request");
    };
    assert_eq!(prompt_id, "5ca707fdc79b4076b0d1dddbcc756c64");
    assert_eq!(*execution_generation, 1);
    assert_eq!(description, "recursive delete");
    assert_eq!(choices, &["once", "deny", "session", "always"]);
    // M1 §9 rule 3: the command Hermes runs is not the tool.started preview.
    assert_eq!(preview, "rm -rf C:/Users/USER/kairos-probe-scratch");
    assert_eq!(
        command,
        "rm -rf C:/Users/USER/kairos-probe-scratch; echo \"EXIT_CODE=$?\""
    );

    let RunEvent::ApprovalSettled {
        prompt_id: settled_id,
        execution_generation: settled_generation,
    } = &stream.events[2].event
    else {
        panic!("third event is approval.settled");
    };
    assert_eq!(settled_id, prompt_id);
    assert_eq!(settled_generation, execution_generation);
}

#[test]
fn a_stopped_run_ends_cancelled_by_the_user() {
    let stream = parse_stream("run-stop/03-events.sse");
    assert_eq!(
        sequence(&stream.events),
        ["tool.started", "tool.completed(error)", "run.cancelled"]
    );
    assert_eq!(
        stream.events[2].event,
        RunEvent::RunCancelled {
            turn_exit_reason: Some("interrupted_by_user".into())
        }
    );
    assert!(stream.comments.iter().any(|c| c == "keepalive"));
}

fn json<T: serde::de::DeserializeOwned>(path: &str) -> T {
    serde_json::from_str(&read(path)).unwrap_or_else(|err| panic!("{path}: {err}"))
}

#[test]
fn health_reports_the_hermes_version() {
    let health: Health = json("meta/health.json");
    assert_eq!(health.status, "ok");
    assert_eq!(health.version.as_deref(), Some("0.22.5"));
}

#[test]
fn hermes_0_22_5_has_every_required_capability() {
    let caps: Capabilities = json("meta/capabilities.json");
    assert_eq!(caps.missing_required(), Vec::<String>::new());
}

#[test]
fn a_missing_or_false_capability_is_reported() {
    let mut caps: Capabilities = json("meta/capabilities.json");
    caps.features.remove("run_stop");
    caps.features
        .insert("approval_events".into(), serde_json::Value::Bool(false));
    assert_eq!(caps.missing_required(), ["run_stop", "approval_events"]);
}

#[test]
fn run_creation_and_its_idempotent_replay_parse() {
    let created: CreateRunResponse = json("run-simple/01-create.json");
    let replayed: CreateRunResponse = json("run-simple/04-retry-same-key.json");
    assert_eq!(created.status, "started");
    assert!(!created.replayed);
    assert_eq!(replayed.run_id, created.run_id);
    assert!(replayed.replayed);
}

#[test]
fn a_run_waiting_for_approval_lists_the_pending_control() {
    let status: RunStatus = json("run-approval-deny/02-status-waiting.json");
    assert_eq!(status.status, "waiting_for_approval");
    assert_eq!(status.pending_controls.len(), 1);
    let control = &status.pending_controls[0];
    assert_eq!(control.kind, "approval");
    assert_eq!(control.prompt_id, "5ca707fdc79b4076b0d1dddbcc756c64");
    assert_eq!(control.execution_generation, 1);

    let settled: RunStatus = json("run-approval-deny/06-status-final.json");
    assert_eq!(settled.status, "completed");
    assert!(settled.pending_controls.is_empty());
}

#[test]
fn approval_stop_and_cancelled_status_responses_parse() {
    let approval: ApprovalResponse = json("run-approval-deny/04-approval-deny.json");
    assert_eq!(approval.status, "resolved");
    let stop: StopResponse = json("run-stop/02-stop.json");
    assert_eq!(stop.status, "stopping");
    let status: RunStatus = json("run-stop/04-status.json");
    assert_eq!(status.status, "cancelled");
    assert_eq!(status.usage.total_tokens, 12435);
}

#[test]
fn reusing_an_idempotency_key_with_another_body_is_a_conflict() {
    let err = error_from_response(409, &read("run-simple/05-retry-changed-body.json"));
    assert!(matches!(
        err,
        HermesError::Conflict { ref code, .. } if code == "idempotency_key_conflict"
    ));
    assert_eq!(err.kind(), "conflict");
}

#[test]
fn an_out_of_date_approval_is_stale() {
    let err = error_from_response(
        409,
        &read("run-approval-deny/03-approval-stale-generation.json"),
    );
    assert!(matches!(err, HermesError::Stale { .. }));
    assert_eq!(err.kind(), "stale");
}

#[test]
fn a_wrong_key_is_unauthorized_and_its_message_names_no_key() {
    let err = error_from_response(401, &read("meta/unauthorized.json"));
    assert_eq!(err, HermesError::Unauthorized);
    assert_eq!(err.kind(), "auth");
}

#[test]
fn the_broken_skills_endpoint_is_a_plain_http_error() {
    let err = error_from_response(500, &read("meta/skills.error.json"));
    assert!(matches!(
        err,
        HermesError::Http { status: 500, code: None, ref message } if message == "Failed to enumerate skills"
    ));
}

#[test]
fn an_unknown_run_is_not_found() {
    // Seen live on 2026-10-23 (GET /v1/runs/run_doesnotexist) but not saved as a fixture.
    let body = r#"{"error": {"message": "Run not found: run_doesnotexist", "type": "invalid_request_error", "param": null, "code": "run_not_found"}}"#;
    let err = error_from_response(404, body);
    assert!(matches!(err, HermesError::NotFound { ref code, .. } if code == "run_not_found"));
}

#[test]
fn a_non_json_error_body_still_maps_by_status() {
    assert!(matches!(
        error_from_response(502, "Bad Gateway"),
        HermesError::Http { status: 502, ref message, .. } if message == "Bad Gateway"
    ));
    assert_eq!(error_from_response(401, "nope"), HermesError::Unauthorized);
}
