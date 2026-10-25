import { describe, expect, it } from "vitest";
import type { SystemAlert } from "../../lib/tauri";
import { applyAlerts, closeRequest } from "./alerts";

const alert = (id: string, extra: Partial<SystemAlert> = {}): SystemAlert => ({
  id,
  kind: "battery_low",
  title: "t",
  message: "m",
  process: null,
  cpu_percent: null,
  ...extra,
});

describe("alert banners", () => {
  it("show new alerts first and drop cleared ones", () => {
    const shown = applyAlerts([alert("battery")], { fired: [alert("disk:D:\\")], cleared: [] });
    expect(shown.map((a) => a.id)).toEqual(["disk:D:\\", "battery"]);
    expect(applyAlerts(shown, { fired: [], cleared: ["battery"] }).map((a) => a.id)).toEqual([
      "disk:D:\\",
    ]);
  });

  it("replace an earlier alert with the same id instead of stacking", () => {
    const low = alert("battery", { kind: "battery_low" });
    const critical = alert("battery", { kind: "battery_critical" });
    const shown = applyAlerts([low], { fired: [critical], cleared: [] });
    expect(shown).toEqual([critical]);
  });
});

describe("Close it?", () => {
  it("asks Hermes to close the busy app, so the close-apps rule asks the user", () => {
    const hog = alert("cpu:chrome", { kind: "cpu_hog", process: "chrome.exe", cpu_percent: 86.4 });
    expect(closeRequest(hog)).toBe(
      "Close chrome.exe. It has been using about 86 % of the CPU for over a minute.",
    );
  });

  it("is only offered for a busy process", () => {
    expect(closeRequest(alert("battery"))).toBeNull();
  });
});
