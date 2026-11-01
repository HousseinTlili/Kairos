// Computer use in words (M20, D-040). Hermes's card says "computer_use: click element #1" and the
// stream says nothing about the target, so the words come from the audit row, where the plugin puts
// the app and the element's name (desktop.py). Pure, for the tests.
import type { ApprovalChoice } from "../../lib/tauri";
import type { ApprovalRow, ToolRow, Turn } from "./turn";

export const DESKTOP_TOOL = "computer_use";

interface DesktopArgs {
  action?: string;
  text?: string;
  value?: string;
  keys?: string;
  direction?: string;
  kairos_target?: string;
  kairos_element?: string;
}

function argsOf(row: ToolRow): DesktopArgs | null {
  if (row.tool !== DESKTOP_TOOL || !row.audit) return null;
  try {
    return JSON.parse(row.audit.args_redacted) as DesktopArgs;
  } catch {
    return null;
  }
}

/** One call, in words: "Click Select in Character Map". Empty when it can't be told. */
export function desktopPhrase(row: ToolRow): string {
  const args = argsOf(row);
  if (!args?.action) return "";
  const where = args.kairos_target ? ` in ${args.kairos_target}` : "";
  const what = args.kairos_element || "a spot";
  switch (args.action) {
    case "capture":
      return args.kairos_target ? `See ${args.kairos_target}` : "See a window";
    case "list_apps":
      return "List your apps";
    case "list_windows":
      return "List your open windows";
    case "wait":
      return "Wait";
    case "focus_app":
      return args.kairos_target ? `Switch to ${args.kairos_target}` : "Switch app";
    case "click":
      return `Click ${what}${where}`;
    case "double_click":
      return `Double-click ${what}${where}`;
    case "right_click":
      return `Right-click ${what}${where}`;
    case "middle_click":
      return `Middle-click ${what}${where}`;
    case "drag":
      return `Drag${where}`;
    case "scroll":
      return `Scroll ${args.direction ?? ""}${where}`.replace("  ", " ");
    case "type":
      return `Type “${args.text ?? ""}”${where}`;
    case "key":
      return `Press ${args.keys ?? ""}${where}`;
    case "set_value":
      return `Set ${what} to “${args.value ?? ""}”${where}`;
    default:
      return "";
  }
}

/** The tool row the card is about: Hermes starts the tool, then its gate asks. */
function callBefore(turn: Turn, row: ApprovalRow): ToolRow | null {
  const at = turn.items.indexOf(row);
  for (let i = at - 1; i >= 0; i--) {
    const item = turn.items[i];
    if (item.kind === "tool" && item.tool === DESKTOP_TOOL) return item;
    if (item.kind === "approval") return null;
  }
  return null;
}

export function isDesktopApproval(row: ApprovalRow): boolean {
  return row.command.startsWith(`${DESKTOP_TOOL}:`);
}

/** What a desktop card should say, if it's one and the words are known. */
export function desktopCardWords(turn: Turn, row: ApprovalRow): string {
  if (!isDesktopApproval(row) || row.source !== "hermes") return "";
  const call = callBefore(turn, row);
  return call ? desktopPhrase(call) : "";
}

// Hermes's grants are per kind of action: "this conversation" on typing would cover anything typed
// anywhere in it, unread. So what goes into an app is granted once, each time.
const ONCE_ONLY = /^computer_use: (type|key|set_value)\b/;

/**
 * The choices a card offers, in Hermes's order. "Always" is never offered for the desktop: one
 * "always" on a click would be every click, in every conversation, for good.
 */
export function offeredChoices(row: ApprovalRow): string[] {
  const desktop = isDesktopApproval(row) || row.rule?.startsWith("computer-use") === true;
  if (!desktop) return row.choices;
  return row.choices.filter(
    (choice: string) =>
      (choice as ApprovalChoice) !== "always" &&
      !((choice as ApprovalChoice) === "session" && ONCE_ONLY.test(row.command)),
  );
}

/** Where the turn is working, while it is: for the top bar's line and its Stop. */
export function desktopTarget(turn: Turn | undefined): string | null {
  if (!turn) return null;
  if (!["running", "waiting_for_approval", "stopping"].includes(turn.status)) return null;
  for (let i = turn.items.length - 1; i >= 0; i--) {
    const item = turn.items[i];
    if (item.kind !== "tool" || item.tool !== DESKTOP_TOOL) continue;
    const target = argsOf(item)?.kairos_target;
    if (target) return target;
  }
  return null;
}

/** An approval that can be answered now, for the sidebar's count. */
export function waitingApprovals(turns: Turn[]): number {
  return turns.reduce(
    (count, turn) =>
      count +
      turn.items.filter((item) => item.kind === "approval" && item.live && item.state === "waiting")
        .length,
    0,
  );
}
