// Copy deck for the AI review, Decide and Sign off (D9). Checked by review.test.tsx.

/** The status, like a pull request's checks (see status.ts for the rule). */
export const statusLabels = {
  green: 'All checks passed',
  orange: 'Possible problems',
  red: 'Needs human review',
} as const;

export const headingKeys = ['matches', 'not-in-description', 'described-not-found', 'left-to-you'] as const;
export type HeadingKey = (typeof headingKeys)[number];

export const headings: Record<HeadingKey, string> = {
  matches: 'Matches the description',
  'not-in-description': 'Not in the description',
  'described-not-found': 'Described, not found',
  'left-to-you': 'Left to you',
};

export const glyphs: Record<HeadingKey, string> = {
  matches: '=', 'not-in-description': '+', 'described-not-found': '−', 'left-to-you': '?',
};

/** Why a decision is asked, in a C# developer's words. Empty where the decision's own name says it (`1 data loss`, `4 hanging tests`). */
export const ruleLabels: Record<string, string> = {
  'undeclared-difference': 'not asked for',
  'declared-severe-difference': '',
  'level-3-block': 'read this block',
  'test-group': '',
};

/** Terse label for a rule name, including `kind:detail` forms such as `architectural:changed-wiring`. */
export function ruleLabel(rule: string): string {
  if (rule in ruleLabels) return ruleLabels[rule];
  const tail = rule.includes(':') ? rule.slice(rule.indexOf(':') + 1) : rule;
  const s = tail.replace(/[-_]+/g, ' ').trim();
  return s ? s[0].toUpperCase() + s.slice(1) : rule;
}

const kindWords: Record<string, string> = {
  crash: 'the crash', 'silent-failure': 'the silent failure', 'data-loss': 'the data loss',
};

/** The decide question for a rule (D40); questions are one of the places sentences are allowed (D9). */
export function questionText(rule: string, o: { diffs: { number: number; kind: string }[]; tests: number; category: string | null; block: string | null }): string {
  const nums = o.diffs.map((d) => d.number).join(', ');
  switch (rule) {
    case 'undeclared-difference': return `Accept undeclared difference ${nums}?`;
    case 'declared-severe-difference': {
      const k = o.diffs.map((d) => kindWords[d.kind] ?? d.kind.replace(/-/g, ' '))[0] ?? 'the change';
      return `Accept ${k} in difference ${nums}?`;
    }
    case 'level-3-block': return `Accept level 3 block${o.block ? ` ${o.block}` : ''}?`;
    case 'test-group': return `Accept ${o.tests} ${o.category ? o.category.replace(/-/g, ' ') + ' ' : ''}tests?`;
    default: return `Accept ${ruleLabel(rule).toLowerCase()}?`;
  }
}

export const gateLabels: Record<string, string> = {
  'section-running': 'Section running',
  'question-unanswered': 'Open questions',
  'spot-check': 'Spot-check',
  'level-3-unopened': 'Level 3 unopened',
  'head-moved': 'Head moved',
  'self-approval': 'Self-approval',
};

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

export const L = {
  noReview: 'no AI review',
  aiReview: 'AI review',
  read: 'Read',
  didNotRead: 'Did not read',
  code: 'code',
  dropped: (n: number) => `${plural(n, 'statement')} dropped`,
  differences: (n: number) => plural(n, 'difference'),
  tests: (n: number) => plural(n, 'test'),
  rules: (n: number) => plural(n, 'rule'),
  groups: (n: number) => plural(n, 'group'),
  scope: 'scope',
  description: 'description',
  requirement: 'requirement',
  mode: (m: string) => (m === 'light' ? 'Light review' : 'Full review'),
  category: (n: number, c: string) => `${n} ${c} tests`,
  preset: (n: number | string) => `Preset ${n}`,
  reproduction: 'Reported bug',
  question: (id: string | number) => `Question ${String(id).replace(/^q/, "")}`,
  open: (n: number) => `${n} open`,
  lost: (v: string) => `value lost: ${v}`,
  evidence: 'Evidence',
  claimsMatch: (n: number) => `${n} ${n === 1 ? 'claim matches' : 'claims match'}`,
  showAll: (n: number) => `All ${n}`,
  answerItems: (n: number) => `Answer ${n} ${n === 1 ? 'item' : 'items'}`,
  waitFor: 'Still running',
  claims: 'Claims',
  aiSummary: 'AI summary',
  askedFor: 'Asked for',
  notAskedFor: 'Not asked for',
  matchesGroup: 'Matches',
  needsYou: 'Needs you',
  status: 'Status',
  descriptionTitle: 'Description',
  fullText: 'Full text',
  more: (n: number) => `+${n}`,
  answered: (a: number, n: number) => `${a} / ${n}`,
  noDescription: 'no description',
  accept: 'Accept',
  return: 'Return',
  stale: 'stale',
  reviewer: 'Reviewer',
  token: 'Token',
  risk: 'Risk',
  approve: 'Approve',
  requestChanges: 'Request changes',
  spotCheck: 'Spot-check',
  opened: 'Opened',
  notOpened: 'Not opened',
  refused: 'Refused',
  submitted: 'Submitted',
  notSubmitted: 'Not submitted',
  noToken: 'no token',
  failed: 'failed',
  done: 'done',
} as const;

export const reviewBanned = ['proved', 'safe', 'verified', 'trusted', 'correct'];

export function allLabels(): string[] {
  const out: string[] = [...Object.values(headings), ...Object.values(ruleLabels), ...Object.values(gateLabels), ...Object.values(statusLabels)];
  for (const v of Object.values(L)) out.push(typeof v === 'function' ? (v as (a: number, b: string) => string)(2, 'hanging') : v);
  return out;
}
