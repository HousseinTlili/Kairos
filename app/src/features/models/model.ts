// Settings ▸ Models & sandbox wording (M7). Pure, for the tests.

/** What the terminal backend means for the user (D-019). */
export function sandboxText(backend: string | null): {
  tone: "ok" | "warn" | "muted";
  text: string;
} {
  switch (backend) {
    case "docker":
      return {
        tone: "ok",
        text: "In a container: each conversation gets a fresh Docker container that sees only your chosen project folders.",
      };
    case "local":
      return {
        tone: "warn",
        text: "Directly on this PC: the rules check every command, but nothing else contains it.",
      };
    case null:
      return { tone: "muted", text: "Kairos couldn't ask Hermes." };
    default:
      return { tone: "muted", text: `Hermes's "${backend}" backend.` };
  }
}
