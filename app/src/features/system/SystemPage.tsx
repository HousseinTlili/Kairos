import { useEffect, type CSSProperties, type ReactNode } from "react";
import type { BatteryReading, ProcessGroup, SystemSnapshot } from "../../lib/tauri";
import {
  formatBitRate,
  formatByteRate,
  formatBytes,
  formatDuration,
  formatPercent,
  level,
  sparklinePath,
  usedShare,
} from "./format";
import { SpeedTestCard } from "./SpeedTestCard";
import { useSystem } from "./store";

const SPARK_W = 236;
const SPARK_H = 36;

function Sparkline({ values, max, label }: { values: number[]; max?: number; label: string }) {
  const path = sparklinePath(values, SPARK_W, SPARK_H, max);
  return (
    <svg
      className="sparkline"
      viewBox={`0 0 ${SPARK_W} ${SPARK_H}`}
      preserveAspectRatio="none"
      role="img"
      aria-label={label}
    >
      {path && <path d={path} />}
    </svg>
  );
}

function Bar({ percent, label }: { percent: number; label: string }) {
  return (
    <div
      className="bar"
      role="meter"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(percent)}
    >
      <span className={`bar-fill ${level(percent)}`} style={{ width: `${percent}%` }} />
    </div>
  );
}

function Card({
  title,
  value,
  children,
}: {
  title: string;
  value?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="sys-card">
      <header>
        <h2>{title}</h2>
        {value != null && <span className="sys-value">{value}</span>}
      </header>
      {children}
    </section>
  );
}

function CpuCard({ snapshot, history }: { snapshot: SystemSnapshot; history: number[] }) {
  const { cpu } = snapshot;
  return (
    <Card title="CPU" value={formatPercent(cpu.total)}>
      <Sparkline values={history} max={100} label="CPU use over the last two minutes" />
      <p className="sys-sub">
        {cpu.name || "Processor"}
        {cpu.frequency_mhz > 0 && `, ${(cpu.frequency_mhz / 1000).toFixed(2)} GHz`}
      </p>
      {cpu.per_core.length > 0 && (
        <div className="cores" aria-label="Use per logical core">
          {cpu.per_core.map((value, i) => (
            <span
              key={i}
              className={`core ${level(value)}`}
              title={`Core ${i}: ${formatPercent(value)}`}
              style={{ "--fill": `${Math.min(100, value)}%` } as CSSProperties}
            />
          ))}
        </div>
      )}
    </Card>
  );
}

function MemoryCard({ snapshot, history }: { snapshot: SystemSnapshot; history: number[] }) {
  const { memory } = snapshot;
  const share = usedShare(memory.used, memory.total);
  return (
    <Card title="Memory" value={formatPercent(share)}>
      <Sparkline values={history} max={100} label="Memory use over the last two minutes" />
      <p className="sys-sub">
        {formatBytes(memory.used)} of {formatBytes(memory.total)} in use
        {memory.swap_total > 0 && `, page file ${formatBytes(memory.swap_used)}`}
      </p>
    </Card>
  );
}

function NetworkCard({
  snapshot,
  down,
  up,
}: {
  snapshot: SystemSnapshot;
  down: number[];
  up: number[];
}) {
  const { network, wifi } = snapshot;
  return (
    <Card title="Network">
      <div className="net-rows">
        <div>
          <span className="sys-label">Down</span>
          <strong>{formatBitRate(network.down_per_s)}</strong>
          <Sparkline values={down} label="Download speed over the last two minutes" />
        </div>
        <div>
          <span className="sys-label">Up</span>
          <strong>{formatBitRate(network.up_per_s)}</strong>
          <Sparkline values={up} label="Upload speed over the last two minutes" />
        </div>
      </div>
      <p className="sys-sub">
        {wifi
          ? `Wi-Fi: ${wifi.ssid}${wifi.signal_percent != null ? `, signal ${wifi.signal_percent} %` : ""}`
          : "Not on Wi-Fi"}
      </p>
    </Card>
  );
}

function batteryLine(battery: BatteryReading): string {
  switch (battery.state) {
    case "charging":
      return battery.time_to_full_s != null
        ? `Charging, full in ${formatDuration(battery.time_to_full_s)}`
        : "Charging";
    case "discharging":
      return battery.time_to_empty_s != null
        ? `On battery, ${formatDuration(battery.time_to_empty_s)} left`
        : "On battery";
    case "full":
      return "Fully charged";
    default:
      return "Plugged in";
  }
}

function BatteryCard({ battery }: { battery: BatteryReading }) {
  return (
    <Card title="Battery" value={formatPercent(battery.percent)}>
      <Bar percent={100 - battery.percent} label="Battery used" />
      <p className="sys-sub">{batteryLine(battery)}</p>
      {battery.health_percent != null && (
        <p className="sys-sub">
          Health {formatPercent(battery.health_percent)} (wear{" "}
          {formatPercent(100 - battery.health_percent)})
        </p>
      )}
    </Card>
  );
}

function GpuCards({ snapshot }: { snapshot: SystemSnapshot }) {
  return (
    <>
      {snapshot.gpus.map((gpu, i) => (
        <Card
          key={i}
          title={snapshot.gpus.length > 1 ? `GPU ${i}` : "GPU"}
          value={formatPercent(gpu.load_percent)}
        >
          {gpu.memory_used != null && gpu.memory_total != null && (
            <Bar percent={usedShare(gpu.memory_used, gpu.memory_total)} label="GPU memory used" />
          )}
          <p className="sys-sub">{gpu.name}</p>
          <p className="sys-sub">
            {gpu.memory_used != null && gpu.memory_total != null
              ? `${formatBytes(gpu.memory_used)} of ${formatBytes(gpu.memory_total)} VRAM`
              : "VRAM unknown"}
            {gpu.temperature_c != null && `, ${gpu.temperature_c.toFixed(0)} °C`}
          </p>
        </Card>
      ))}
    </>
  );
}

function DiskList({ snapshot }: { snapshot: SystemSnapshot }) {
  return (
    <section className="sys-card wide">
      <header>
        <h2>Disks</h2>
      </header>
      <ul className="disk-list">
        {snapshot.disks.map((disk) => {
          const used = usedShare(disk.total - disk.available, disk.total);
          return (
            <li key={disk.mount}>
              <div className="disk-head">
                <strong>{disk.mount}</strong>
                <span className="muted">
                  {[
                    disk.name,
                    disk.kind !== "unknown" ? disk.kind.toUpperCase() : null,
                    disk.removable ? "removable" : null,
                  ]
                    .filter(Boolean)
                    .join(", ")}
                </span>
                <span className="disk-free">
                  {formatBytes(disk.available)} free of {formatBytes(disk.total)}
                </span>
              </div>
              <Bar percent={used} label={`${disk.mount} used`} />
              <p className="sys-sub">
                Read {formatByteRate(disk.read_per_s)}, write {formatByteRate(disk.write_per_s)}
              </p>
            </li>
          );
        })}
      </ul>
    </section>
  );
}

function ProcessTable({
  title,
  rows,
  show,
}: {
  title: string;
  rows: ProcessGroup[];
  show: "cpu" | "memory";
}) {
  return (
    <section className="sys-card">
      <header>
        <h2>{title}</h2>
      </header>
      <table className="proc-table">
        <tbody>
          {rows.map((row) => (
            <tr key={row.name}>
              <td className="proc-name" title={row.name}>
                {row.name}
                {row.count > 1 && <span className="muted"> ({row.count})</span>}
              </td>
              <td className="proc-num">
                {show === "cpu" ? formatPercent(row.cpu_percent, 1) : formatBytes(row.memory)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}

export function SystemPage() {
  const { snapshot, history, error, setVisible } = useSystem();

  useEffect(() => {
    setVisible(true);
    return () => setVisible(false);
  }, [setVisible]);

  return (
    <section className="system" aria-labelledby="system-title">
      <div className="system-head">
        <h1 id="system-title">System</h1>
        {snapshot && <span className="muted">Up {formatDuration(snapshot.uptime_s)}</span>}
      </div>
      {error && (
        <div className="status-card bad" role="alert">
          <p>{error}</p>
        </div>
      )}
      {!snapshot ? (
        <p className="muted">Reading the system…</p>
      ) : (
        <>
          <div className="sys-grid">
            <CpuCard snapshot={snapshot} history={history.cpu} />
            <MemoryCard snapshot={snapshot} history={history.memory} />
            <NetworkCard snapshot={snapshot} down={history.down} up={history.up} />
            {snapshot.battery && <BatteryCard battery={snapshot.battery} />}
            <GpuCards snapshot={snapshot} />
            <SpeedTestCard />
          </div>
          <DiskList snapshot={snapshot} />
          <div className="sys-grid two">
            <ProcessTable title="Top by CPU" rows={snapshot.top_cpu} show="cpu" />
            <ProcessTable title="Top by memory" rows={snapshot.top_memory} show="memory" />
          </div>
          {snapshot.notes.length > 0 && (
            <ul className="sys-notes">
              {snapshot.notes.map((note) => (
                <li key={note}>{note}</li>
              ))}
            </ul>
          )}
          <p className="muted small">
            Read directly from Windows every few seconds. Nothing on this page goes to Hermes or a
            model.
          </p>
        </>
      )}
    </section>
  );
}
