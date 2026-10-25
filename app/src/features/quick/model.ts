// The quick box's small rules (M8 §3). Pure, for the tests.

/** One quick conversation per local day: the key its session id is remembered under. */
export function quickSessionKey(date: Date = new Date()): string {
  const day = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
  return `kairos-quick-session-${day}`;
}

/** The quick box hides while the screen is being captured, so it isn't in the picture. */
export function hidesForCapture(tool: string): boolean {
  return tool === "screen_capture";
}

/** "Ctrl + Alt + Space" for display. */
export function hotkeyLabel(hotkey: string): string {
  return hotkey
    .split("+")
    .map((part) => part.trim())
    .filter(Boolean)
    .join(" + ");
}
