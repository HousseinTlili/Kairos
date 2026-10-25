import type { SpeedTestProgress } from "../../lib/tauri";
import { useSystem } from "./store";

const mbps = (value: number) => `${value.toFixed(value >= 100 ? 0 : 1)} Mbps`;

function status(speed: SpeedTestProgress | null): string {
  switch (speed?.phase) {
    case undefined:
      return "Measures your connection against speed.cloudflare.com. Runs only when you ask.";
    case "latency":
      return "Checking latency…";
    case "download":
      return "Measuring download…";
    case "upload":
      return "Measuring upload…";
    case "done":
      return `Measured at ${new Date(speed.result.finished_at).toLocaleTimeString()}.`;
    case "failed":
      return speed.message;
  }
}

export function SpeedTestCard() {
  const { speed, runSpeedTest } = useSystem();
  const running = speed != null && !["done", "failed"].includes(speed.phase);
  const latency =
    speed?.phase === "done"
      ? speed.result.latency_ms
      : speed && "latency_ms" in speed
        ? speed.latency_ms
        : null;
  const download =
    speed?.phase === "done"
      ? speed.result.download_mbps
      : speed?.phase === "upload"
        ? speed.download_mbps
        : null;
  const upload = speed?.phase === "done" ? speed.result.upload_mbps : null;

  return (
    <section className="sys-card">
      <header>
        <h2>Internet speed</h2>
        <button onClick={() => void runSpeedTest()} disabled={running}>
          {running ? "Testing…" : "Run speed test"}
        </button>
      </header>
      <dl className="speed-figures">
        <dt>Download</dt>
        <dd>{download != null ? mbps(download) : "—"}</dd>
        <dt>Upload</dt>
        <dd>{upload != null ? mbps(upload) : "—"}</dd>
        <dt>Latency</dt>
        <dd>{latency != null ? `${latency.toFixed(0)} ms` : "—"}</dd>
      </dl>
      <p className={`sys-sub ${speed?.phase === "failed" ? "error-text" : ""}`} aria-live="polite">
        {status(speed)}
      </p>
    </section>
  );
}
