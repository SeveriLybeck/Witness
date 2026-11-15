// Every label the map renders (D9).
export const mapLabels = {
  declared: 'Declared',
  running: 'running',
  notLinked: 'not linked',
  outside: 'outside',
  largeComment: 'large comment',
  sites: (n: number) => (n === 1 ? "1 call site" : `${n} call sites`),
  blocks: (n: number) => `${n} blocks`,
  rule: {
    'identical-edit': (n: number) => `${n} identical edits`,
    moved: (n: number) => `${n} moved`,
    rename: (n: number) => `${n} renames`,
    formatting: (n: number) => `${n} formatting`,
  },
  map: 'Map',
  fit: 'Fit map',
} as const;

/** Rule names are free strings like `severe-difference` or `architectural:changed-wiring`; shown as terse words, the kind suffix in place of its prefix. */
/** A rule may also arrive as `{rule, fact}` (levels.json); the fact is the readable part: `{architectural, changed-wiring}` → `changed wiring`. */
export type RuleLike = string | { rule?: string; fact?: string | null } | null | undefined;
export function ruleLabel(r: RuleLike): string {
  const s = typeof r === "string" ? r : r ? (r.fact || r.rule || "") : "";
  return (s.includes(":") ? s.slice(s.indexOf(":") + 1) : s).replace(/[-_]/g, " ");
}
export const ruleLabels = (rs: RuleLike[]) => [...new Set(rs.map(ruleLabel).filter(Boolean))];
