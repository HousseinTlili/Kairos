// Words and requests for Settings ▸ Accounts (M6 §3). Pure, for the tests.
import type { AccountKind, AccountState } from "../../lib/tauri";

export const ACCOUNT_NAME: Record<AccountKind, string> = {
  google: "Google (Gmail and Calendar)",
  github: "GitHub",
};

export const ACCOUNT_SIGN_IN: Record<AccountKind, string> = {
  google:
    "Through Hermes's Google Workspace skill: Google's own sign-in, with an OAuth client you create once in Google Cloud. The token stays with Hermes.",
  github:
    "Through GitHub's own sign-in (gh auth login), in a terminal window and your browser. The token stays with the GitHub CLI.",
};

export function statusText(state: AccountState): string {
  switch (state.status) {
    case "connected":
      return "Connected";
    case "not_connected":
      return "Not connected";
    case "unavailable":
      return state.reason;
  }
}

/** What "Disconnect" asks first: it removes Kairos's (and Hermes's) access. */
export function disconnectQuestion(kind: AccountKind): string {
  return kind === "google"
    ? "Disconnect Google? Hermes's token is revoked at Google and deleted; Kairos loses access to Gmail and Calendar until you connect again."
    : "Disconnect GitHub? The GitHub CLI signs out on this PC; Kairos loses access until you sign in again.";
}

/**
 * The conversation that connects Google. Hermes's skill drives the sign-in; the request limits it
 * to Gmail and Calendar and to Google's official OAuth (D-010: never a password).
 */
export function googleSetupRequest(): string {
  return [
    "Set up the Google Workspace skill so you can read my Gmail and Google Calendar.",
    "Use only the services email and calendar.",
    "Follow the skill's own OAuth steps: help me create the Google Cloud OAuth client (Desktop app),",
    "then give me the sign-in link. Never ask for my Google password or an app password.",
    "When it's done, check it with the skill's --check command and tell me the result.",
  ].join("\n");
}
