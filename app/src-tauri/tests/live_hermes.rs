//! Tests against the real local Hermes (M1 Step 9). Ignored by default; run with
//! `cargo test --manifest-path app/src-tauri/Cargo.toml -- --ignored` while Hermes is running.
//! They read the API key from Hermes's `.env` and never print it. Each run uses the free model
//! and creates a `kairos-live-test-*` session in Hermes.

use std::time::{SystemTime, UNIX_EPOCH};

use kairos_lib::hermes::client::{ApiKey, CreateRunRequest, DEFAULT_BASE_URL, HermesClient};
use kairos_lib::hermes::errors::HermesError;
use kairos_lib::hermes::events::RunEvent;
use kairos_lib::hermes::local;

fn client() -> HermesClient {
    let home = local::hermes_home().expect("Hermes home");
    let key: ApiKey = local::read_api_key(&home).expect("API_SERVER_KEY in Hermes's .env");
    HermesClient::new(DEFAULT_BASE_URL, key).expect("client")
}

fn unique(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    format!("{prefix}-{nanos}")
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tauri::async_runtime::block_on(future)
}

#[test]
#[ignore = "needs a running Hermes"]
fn hermes_is_up_and_has_every_capability_kairos_needs() {
    block_on(async {
        let client = client();
        let health = client.health().await.expect("health");
        assert_eq!(health.status, "ok");
        println!("Hermes version: {:?}", health.version);
        let caps = client
            .capabilities()
            .await
            .expect("capabilities with the real key");
        assert_eq!(caps.missing_required(), Vec::<String>::new());
    });
}

#[test]
#[ignore = "needs a running Hermes"]
fn a_wrong_key_is_rejected() {
    block_on(async {
        let client = HermesClient::new(DEFAULT_BASE_URL, ApiKey::new("not-the-key-0000000000"))
            .expect("client");
        assert_eq!(
            client.capabilities().await.expect_err("must be rejected"),
            HermesError::Unauthorized
        );
    });
}

#[test]
#[ignore = "needs a running Hermes"]
fn a_simple_run_streams_to_completion_and_a_retry_never_runs_twice() {
    block_on(async {
        let client = client();
        let session = unique("kairos-live-test");
        let key = unique("kairos-live-key");
        let request = CreateRunRequest {
            input: "Reply with exactly: Kairos live test OK",
            session_id: &session,
            model: None,
            provider: None,
        };

        let created = client.create_run(&request, &key).await.expect("create run");
        assert!(!created.replayed);

        let mut stream = client.run_events(&created.run_id).await.expect("events");
        let mut last = None;
        let mut deltas = 0;
        while let Some(item) = stream.next().await {
            let envelope = item.expect("every live event parses");
            assert!(
                !matches!(envelope.event, RunEvent::Unknown { .. }),
                "unrecognised live event: {:?}",
                envelope.event
            );
            if matches!(envelope.event, RunEvent::MessageDelta { .. }) {
                deltas += 1;
            }
            last = Some(envelope.event);
        }
        match last.expect("at least one event") {
            RunEvent::RunCompleted { output, .. } => {
                assert!(
                    output.contains("Kairos live test OK"),
                    "output was {output:?}"
                );
            }
            other => panic!("run ended with {other:?}"),
        }
        assert!(deltas > 0, "the answer should stream");

        // M1 §9 rule 1: the same key returns the same run instead of starting another.
        let retried = client.create_run(&request, &key).await.expect("retry");
        assert_eq!(retried.run_id, created.run_id);
        assert!(retried.replayed);

        // Reusing the key for a different request is refused.
        let other = CreateRunRequest {
            input: "something else",
            session_id: &session,
            model: None,
            provider: None,
        };
        assert!(matches!(
            client.create_run(&other, &key).await,
            Err(HermesError::Conflict { ref code, .. }) if code == "idempotency_key_conflict"
        ));

        let status = client.run_status(&created.run_id).await.expect("status");
        assert_eq!(status.status, "completed");
    });
}

#[test]
#[ignore = "needs a running Hermes"]
fn an_unknown_run_is_not_found() {
    block_on(async {
        assert!(matches!(
            client().run_status("run_doesnotexist").await,
            Err(HermesError::NotFound { ref code, .. }) if code == "run_not_found"
        ));
    });
}
