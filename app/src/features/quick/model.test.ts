import { describe, expect, it } from "vitest";
import { hidesForCapture, hotkeyLabel, quickSessionKey } from "./model";

describe("the quick box", () => {
  it("keeps one conversation per local day", () => {
    expect(quickSessionKey(new Date(2026, 9, 25, 8))).toBe("kairos-quick-session-2026-10-25");
    expect(quickSessionKey(new Date(2026, 9, 25, 23, 59))).toBe(
      quickSessionKey(new Date(2026, 9, 25, 0, 1)),
    );
    expect(quickSessionKey(new Date(2026, 9, 26))).not.toBe(quickSessionKey(new Date(2026, 9, 25)));
  });

  it("hides only for a screen capture", () => {
    expect(hidesForCapture("screen_capture")).toBe(true);
    expect(hidesForCapture("terminal")).toBe(false);
  });

  it("shows the hotkey readably", () => {
    expect(hotkeyLabel("Ctrl+Alt+Space")).toBe("Ctrl + Alt + Space");
  });
});
