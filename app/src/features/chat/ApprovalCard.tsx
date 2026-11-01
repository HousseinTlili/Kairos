import type { ApprovalChoice } from "../../lib/tauri";
import { ShieldIcon } from "../shell/icons";
import { isDesktopApproval, offeredChoices } from "./desktop";
import type { ApprovalRow } from "./turn";

// Kairos asks its Master's leave (docs/design/BRAND.md, Words); the answer keeps the same verb.
const CHOICE_LABEL: Record<ApprovalChoice, string> = {
  once: "Grant once",
  session: "Grant for this conversation",
  always: "Always grant",
  deny: "Refuse",
};

const ANSWERED_LABEL: Record<ApprovalChoice, string> = {
  once: "Granted once",
  session: "Granted for this conversation",
  always: "Always granted",
  deny: "Refused",
};

interface Props {
  row: ApprovalRow;
  onAnswer: (choice: ApprovalChoice) => void;
  /** A desktop action in words, when the plugin could tell (M20): "Click Select in Notepad". */
  desktop?: string;
}

export function ApprovalCard({ row, onAnswer, desktop }: Props) {
  const answerable = row.live && row.state === "waiting";
  // Offer Hermes's choices, in a fixed order, with Deny kept apart from the rest.
  const offered = offeredChoices(row);
  const allow = (["once", "session", "always"] as const).filter((c) => offered.includes(c));
  const onDesktop = isDesktopApproval(row);

  if (!answerable) {
    const answered = row.choice ? ANSWERED_LABEL[row.choice] : "Answered";
    return (
      <div className={`approval-note ${row.choice === "deny" ? "denied" : ""}`}>
        <ShieldIcon />
        <span>
          {row.state === "answering" ? "Sending your answer…" : answered}
          {row.rule ? ` (Kairos rule ${row.rule})` : ""}: {row.description}
        </span>
        {row.command && <code>{row.command}</code>}
      </div>
    );
  }

  return (
    <div className="approval-card" role="alertdialog" aria-labelledby={`approval-${row.promptId}`}>
      <div className="approval-head">
        <ShieldIcon />
        <h3 id={`approval-${row.promptId}`}>
          {row.source === "kairos" || onDesktop
            ? "Kairos asks your leave"
            : "Hermes asks your leave for this command"}
        </h3>
      </div>
      {row.will && <p className="approval-will">It will {row.will}.</p>}
      {desktop && <p className="approval-will">{desktop}</p>}
      <p className="approval-why">
        {onDesktop && row.source === "hermes" ? (
          <>
            The button&apos;s name comes from the app, and the window may have changed since Kairos
            looked. Refuse if it looks wrong.
          </>
        ) : row.source === "kairos" ? (
          <>
            Rule <code>{row.rule ?? "unknown"}</code>: {row.description}
          </>
        ) : (
          <>Hermes flagged it as: {row.description}</>
        )}
      </p>
      {/* M1 §9 rule 3: exactly what will run, not the model's description of it. */}
      {row.command && (
        <pre className="approval-command">
          <code>{row.command}</code>
        </pre>
      )}
      <div className="approval-actions">
        {row.choices.includes("deny") && (
          <button className="deny" onClick={() => onAnswer("deny")}>
            {CHOICE_LABEL.deny}
          </button>
        )}
        <span className="spacer" />
        {allow.map((choice) => (
          <button
            key={choice}
            className={choice === "once" ? "primary" : undefined}
            onClick={() => onAnswer(choice)}
          >
            {CHOICE_LABEL[choice]}
          </button>
        ))}
      </div>
    </div>
  );
}
