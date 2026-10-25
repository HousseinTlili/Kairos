import { describe, expect, it } from "vitest";
import type { StartupState } from "../../lib/tauri";
import { describePresence, hermesAtLogin } from "./format";

const state = (hermes: StartupState["hermes_at_login"]): StartupState => ({
  kairos_at_login: true,
  hermes_at_login: hermes,
  launched_at_login: false,
  recent: [],
});

describe("Settings ▸ Startup", () => {
  it("names each kind of return to the PC, with its detail", () => {
    expect(describePresence({ at: 1, kind: "startup", detail: "autostart" })).toBe(
      "Kairos started at login",
    );
    expect(describePresence({ at: 1, kind: "startup", detail: null })).toBe("Kairos started");
    expect(describePresence({ at: 1, kind: "resume", detail: "slept 2 h" })).toBe(
      "PC woke from sleep (slept 2 h)",
    );
    expect(describePresence({ at: 1, kind: "unlock", detail: null })).toBe("PC unlocked");
  });

  it("says how Hermes starts at login, or that it doesn't", () => {
    expect(hermesAtLogin(state({ scheduled_task: true, startup_entry: false })).ok).toBe(true);
    expect(hermesAtLogin(state({ scheduled_task: false, startup_entry: true })).text).toContain(
      "Startup folder",
    );
    expect(hermesAtLogin(state({ scheduled_task: false, startup_entry: false })).ok).toBe(false);
    expect(hermesAtLogin(state(null)).ok).toBeNull();
  });
});
