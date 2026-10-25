//! The System page's internet speed test (M3 §5.2, D-017): latency, download and upload against
//! Cloudflare's public speed endpoints, only when the user presses the button.

use std::time::{Duration, Instant};

use serde::Serialize;

const BASE: &str = "https://speed.cloudflare.com";
const LATENCY_PINGS: usize = 5;
const DOWNLOAD_BYTES: u64 = 25_000_000;
const UPLOAD_BYTES: usize = 10_000_000;
const PING_TIMEOUT: Duration = Duration::from_secs(5);
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpeedTestResult {
    pub latency_ms: f64,
    pub download_mbps: f64,
    pub upload_mbps: f64,
    /// Unix milliseconds.
    pub finished_at: u64,
}

/// Sent as `speed-test` events while a test runs.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum SpeedTestProgress {
    Latency,
    Download { latency_ms: f64 },
    Upload { latency_ms: f64, download_mbps: f64 },
    Done { result: SpeedTestResult },
    Failed { message: String },
}

/// Megabits per second for `bytes` moved in `seconds`; `None` for a transfer too short to time.
pub fn mbps(bytes: u64, seconds: f64) -> Option<f64> {
    (seconds > 0.0).then(|| bytes as f64 * 8.0 / seconds / 1_000_000.0)
}

/// The middle value, which ignores the odd slow ping; the mean of the two middle ones for an even
/// count.
pub fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mid = sorted.len() / 2;
    Some(if sorted.len().is_multiple_of(2) {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    })
}

fn failed(what: &str, err: reqwest::Error) -> String {
    if err.is_timeout() {
        format!("The {what} took too long.")
    } else if err.is_connect() {
        "Couldn't reach speed.cloudflare.com. Are you online?".to_string()
    } else {
        format!("The {what} failed: {err}")
    }
}

async fn latency(http: &reqwest::Client) -> Result<f64, String> {
    let mut times = Vec::with_capacity(LATENCY_PINGS);
    for _ in 0..LATENCY_PINGS {
        let started = Instant::now();
        let response = http
            .get(format!("{BASE}/__down?bytes=0"))
            .timeout(PING_TIMEOUT)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|err| failed("latency check", err))?;
        times.push(started.elapsed().as_secs_f64() * 1000.0);
        let _ = response.bytes().await;
    }
    median(&times).ok_or_else(|| "No latency measured.".to_string())
}

async fn download(http: &reqwest::Client) -> Result<f64, String> {
    let mut response = http
        .get(format!("{BASE}/__down?bytes={DOWNLOAD_BYTES}"))
        .timeout(TRANSFER_TIMEOUT)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|err| failed("download", err))?;
    let mut received = 0u64;
    let mut first_byte: Option<Instant> = None;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|err| failed("download", err))?
    {
        first_byte.get_or_insert_with(Instant::now);
        received += chunk.len() as u64;
    }
    let seconds = first_byte.map_or(0.0, |at| at.elapsed().as_secs_f64());
    mbps(received, seconds).ok_or_else(|| "The download was too short to time.".to_string())
}

async fn upload(http: &reqwest::Client) -> Result<f64, String> {
    let body = vec![0u8; UPLOAD_BYTES];
    let started = Instant::now();
    http.post(format!("{BASE}/__up"))
        .timeout(TRANSFER_TIMEOUT)
        .body(body)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|err| failed("upload", err))?;
    mbps(UPLOAD_BYTES as u64, started.elapsed().as_secs_f64())
        .ok_or_else(|| "The upload was too short to time.".to_string())
}

/// Runs the three measurements in turn, reporting each phase as it starts.
pub async fn run(progress: impl Fn(SpeedTestProgress)) -> Result<SpeedTestResult, String> {
    let outcome: Result<SpeedTestResult, String> = async {
        let http = reqwest::Client::builder()
            .user_agent("Kairos speed test")
            .build()
            .map_err(|err| format!("Couldn't start the speed test: {err}"))?;
        progress(SpeedTestProgress::Latency);
        let latency_ms = latency(&http).await?;
        progress(SpeedTestProgress::Download { latency_ms });
        let download_mbps = download(&http).await?;
        progress(SpeedTestProgress::Upload {
            latency_ms,
            download_mbps,
        });
        let upload_mbps = upload(&http).await?;
        Ok(SpeedTestResult {
            latency_ms,
            download_mbps,
            upload_mbps,
            finished_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64),
        })
    }
    .await;
    match &outcome {
        Ok(result) => progress(SpeedTestProgress::Done {
            result: result.clone(),
        }),
        Err(message) => progress(SpeedTestProgress::Failed {
            message: message.clone(),
        }),
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn megabits_per_second_counts_bits_not_bytes() {
        // 25 MB in 2 s is 100 Mbps.
        assert_eq!(mbps(25_000_000, 2.0), Some(100.0));
        assert_eq!(mbps(1_000, 0.0), None);
    }

    #[test]
    fn latency_is_the_median_so_one_slow_ping_does_not_count() {
        assert_eq!(median(&[12.0, 11.0, 250.0, 13.0, 12.5]), Some(12.5));
        assert_eq!(median(&[10.0, 20.0]), Some(15.0));
        assert_eq!(median(&[]), None);
    }

    #[test]
    fn progress_events_name_their_phase_for_the_page() {
        let json = serde_json::to_value(SpeedTestProgress::Download { latency_ms: 9.5 })
            .expect("serialise");
        assert_eq!(json["phase"], "download");
        assert_eq!(json["latency_ms"], 9.5);
    }
}
