import { describe, expect, it } from "vitest";
import {
  HISTORY_LENGTH,
  formatBitRate,
  formatByteRate,
  formatBytes,
  formatDuration,
  formatPercent,
  level,
  pushHistory,
  sparklinePath,
  usedShare,
} from "./format";

describe("system page formatting", () => {
  it("shows sizes in binary units with sensible precision", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1536)).toBe("1.50 KB");
    expect(formatBytes(16 * 1024 ** 3)).toBe("16.0 GB");
    expect(formatBytes(476.9 * 1024 ** 3)).toBe("477 GB");
    expect(formatBytes(null)).toBe("—");
    expect(formatBytes(-1)).toBe("—");
  });

  it("shows network speeds in bits per second, as speeds are quoted", () => {
    expect(formatBitRate(100)).toBe("800 bps");
    expect(formatBitRate(1_250_000)).toBe("10.0 Mbps");
    expect(formatBitRate(125_000_000)).toBe("1.00 Gbps");
    expect(formatBitRate(null)).toBe("—");
  });

  it("shows disk throughput per second", () => {
    expect(formatByteRate(3 * 1024 * 1024)).toBe("3.00 MB/s");
    expect(formatByteRate(null)).toBe("—");
  });

  it("shows durations in days, hours and minutes", () => {
    expect(formatDuration(30)).toBe("less than a minute");
    expect(formatDuration(3 * 3600 + 5 * 60)).toBe("3 h 5 min");
    expect(formatDuration(86400 + 2 * 3600 + 3 * 60)).toBe("1 d 2 h");
    expect(formatDuration(null)).toBe("—");
  });

  it("shows a missing percentage as a dash, not zero", () => {
    expect(formatPercent(null)).toBe("—");
    expect(formatPercent(12.345, 1)).toBe("12.3 %");
  });

  it("never divides by a zero total", () => {
    expect(usedShare(5, 0)).toBe(0);
    expect(usedShare(50, 200)).toBe(25);
    expect(usedShare(300, 200)).toBe(100);
  });

  it("colours usage bars by how full they are", () => {
    expect(level(10)).toBe("ok");
    expect(level(75)).toBe("warn");
    expect(level(95)).toBe("bad");
  });
});

describe("sparkline history", () => {
  it("keeps the last two minutes and skips missing readings", () => {
    let history: number[] = [];
    for (let i = 0; i < HISTORY_LENGTH + 5; i++) history = pushHistory(history, i);
    expect(history).toHaveLength(HISTORY_LENGTH);
    expect(history[0]).toBe(5);
    expect(pushHistory(history, null)).toBe(history);
  });

  it("draws nothing until there are two points", () => {
    expect(sparklinePath([], 100, 20)).toBe("");
    expect(sparklinePath([5], 100, 20)).toBe("");
  });

  it("puts the newest point on the right edge and scales to the given maximum", () => {
    const path = sparklinePath([0, 50, 100], 118, 20, 100);
    expect(path.endsWith("L118.0 0.0")).toBe(true);
    expect(path.startsWith("M114.0 20.0")).toBe(true);
  });

  it("scales rates to their own peak when there is no maximum", () => {
    const path = sparklinePath([10, 20], 59, 10);
    expect(path).toBe("M58.0 5.0 L59.0 0.0");
  });
});
