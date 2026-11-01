/**
 * A skill's `SKILL.md`, as plain text. It is untrusted: never Markdown, never HTML, and so nothing
 * in it can fetch a picture or hide behind a link. Don't pass it through the answer renderer.
 */
export function SkillText({ text }: { text: string }) {
  return <pre className="skill-text">{text}</pre>;
}
