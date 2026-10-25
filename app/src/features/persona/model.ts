// The Persona page's small rules (M10 §4). Pure, for the tests.
import type { PersonaKind } from "../../lib/tauri";

const KIND_LABEL: Record<PersonaKind, string> = {
  kairos: "Kairos persona",
  hermes_default: "Hermes's default",
  custom: "Your own text",
};

/** What the saved file is, and whether the editor holds unsaved changes. */
export function personaLabel(kind: PersonaKind, saved: string, draft: string): string {
  const label = KIND_LABEL[kind];
  return sameText(saved, draft) ? label : `${label} (unsaved changes)`;
}

/** Line ends and trailing space don't count as a change. */
export function sameText(a: string, b: string): boolean {
  const tidy = (t: string) => t.replace(/\r\n/g, "\n").trimEnd();
  return tidy(a) === tidy(b);
}
