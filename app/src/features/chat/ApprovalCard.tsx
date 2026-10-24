import type { ApprovalChoice } from "../../lib/tauri";
import { ShieldIcon } from "../shell/icons";
import type { ApprovalRow } from "./turn";

const CHOICE_LABEL: Record<ApprovalChoice, string> = {
  once: "Allow once",
  session: "Allow for this conversation",
  always: "Always allow",
  deny: "Deny",
};

const ANSWERED_LABEL: Record<ApprovalChoice, string> = {
  once: "Allowed once",
  session: "Allowed for this conversation",
  always: "Always allowed",
  deny: "Denied",
};

interface Props {
  row: ApprovalRow;
  onAnswer: (choice: ApprovalChoice) => void;
}

export function ApprovalCard({ row, onAnswer }: Props) {
  const answerable = row.live && row.state === "waiting";
  // Offer Hermes's choices, in a fixed order, with Deny kept apart from the rest.
  const allow = (["once", "session", "always"] as const).filter((c) => row.choices.includes(c));

  if (!answerable) {
    const answered = row.choice ? ANSWERED_LABEL[row.choice] : "Answered";
    return (
      <div className={`approval-note ${row.choice === "deny" ? "denied" : ""}`}>
        <ShieldIcon />
        <span>
          {row.state === "answering" ? "Sending your answer…" : answered}: {row.description}
        </span>
        <code>{row.command}</code>
      </div>
    );
  }

  return (
    <div className="approval-card" role="alertdialog" aria-labelledby={`approval-${row.promptId}`}>
      <div className="approval-head">
        <ShieldIcon />
        <h3 id={`approval-${row.promptId}`}>Kairos wants to run a command that needs your OK</h3>
      </div>
      <p className="approval-why">Hermes flagged it as: {row.description}</p>
      {/* M1 §9 rule 3: exactly what will run, not the model's description of it. */}
      <pre className="approval-command">
        <code>{row.command}</code>
      </pre>
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
