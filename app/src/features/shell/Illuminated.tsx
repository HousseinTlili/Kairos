/** Kairos's line with an illuminated initial (docs/design/BRAND.md, Ornament 2). */
export function Illuminated({
  text,
  className,
  level = 1,
}: {
  text: string;
  className?: string;
  level?: 1 | 2;
}) {
  const Heading = level === 1 ? "h1" : "h2";
  return (
    <Heading className={`illuminated ${className ?? ""}`} aria-label={text}>
      <span className="initial" aria-hidden="true">
        {text.slice(0, 1)}
      </span>
      <span aria-hidden="true">{text.slice(1)}</span>
    </Heading>
  );
}
