// Pure helpers for the System page: number formatting, the short history behind the sparklines,
// and the sparkline path itself. Kept out of the components so they can be tested.

const UNITS = ["B", "KB", "MB", "GB", "TB"];

/** 1536 → "1.5 KB". Binary multiples, as Windows shows them, with Windows's unit names. */
export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null || !Number.isFinite(bytes) || bytes < 0) return "—";
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = unit === 0 || value >= 100 ? 0 : value >= 10 ? 1 : 2;
  return `${value.toFixed(digits)} ${UNITS[unit]}`;
}

/** Bytes per second as bits per second, the way network speeds are quoted: "12.4 Mbps". */
export function formatBitRate(bytesPerSecond: number | null | undefined): string {
  if (bytesPerSecond == null || !Number.isFinite(bytesPerSecond) || bytesPerSecond < 0) return "—";
  const bits = bytesPerSecond * 8;
  if (bits < 1000) return `${bits.toFixed(0)} bps`;
  if (bits < 1_000_000) return `${(bits / 1000).toFixed(bits < 10_000 ? 1 : 0)} Kbps`;
  if (bits < 1_000_000_000) return `${(bits / 1_000_000).toFixed(1)} Mbps`;
  return `${(bits / 1_000_000_000).toFixed(2)} Gbps`;
}

/** Disk throughput, as Task Manager shows it: "3.2 MB/s". */
export function formatByteRate(bytesPerSecond: number | null | undefined): string {
  const text = formatBytes(bytesPerSecond);
  return text === "—" ? text : `${text}/s`;
}

export function formatPercent(value: number | null | undefined, digits = 0): string {
  if (value == null || !Number.isFinite(value)) return "—";
  return `${value.toFixed(digits)} %`;
}

/** 93784 s → "1 d 2 h 3 min"; under a minute → "less than a minute". */
export function formatDuration(seconds: number | null | undefined): string {
  if (seconds == null || !Number.isFinite(seconds) || seconds < 0) return "—";
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const parts: string[] = [];
  if (days) parts.push(`${days} d`);
  if (hours) parts.push(`${hours} h`);
  if (minutes && days === 0) parts.push(`${minutes} min`);
  return parts.length ? parts.join(" ") : "less than a minute";
}

/** Share used, 0–100, guarding against a zero total. */
export function usedShare(used: number, total: number): number {
  return total > 0 ? Math.min(100, Math.max(0, (used / total) * 100)) : 0;
}

/** Two minutes of history at the page's 2 s rate. */
export const HISTORY_LENGTH = 60;

/** Appends a reading, dropping the oldest beyond `limit`. Missing readings are skipped. */
export function pushHistory(
  history: number[],
  value: number | null | undefined,
  limit = HISTORY_LENGTH,
): number[] {
  if (value == null || !Number.isFinite(value)) return history;
  const next = [...history, value];
  return next.length > limit ? next.slice(next.length - limit) : next;
}

/**
 * An SVG path for a sparkline in a `width` × `height` box. Values are scaled to `max`, or to the
 * largest value when no max is given (rates have no natural ceiling). The newest point sits on
 * the right edge; fewer than two points draw nothing.
 */
export function sparklinePath(
  values: number[],
  width: number,
  height: number,
  max?: number,
): string {
  if (values.length < 2) return "";
  const top = max ?? Math.max(...values, 1e-9);
  const step = width / (HISTORY_LENGTH - 1);
  const start = width - step * (values.length - 1);
  return values
    .map((value, i) => {
      const x = start + step * i;
      const y = height - (Math.min(Math.max(value, 0), top) / top) * height;
      return `${i === 0 ? "M" : "L"}${x.toFixed(1)} ${y.toFixed(1)}`;
    })
    .join(" ");
}

/** "ok" under 70 %, "warn" under 90 %, "bad" above: the colour of a usage bar. */
export function level(percent: number): "ok" | "warn" | "bad" {
  return percent >= 90 ? "bad" : percent >= 70 ? "warn" : "ok";
}
