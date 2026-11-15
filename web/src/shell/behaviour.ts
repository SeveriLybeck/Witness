// Grouping of differences by behaviour: the same change of outcome seen at several sites is one group.
import type { Difference } from './data';

export interface DiffGroup {
  /** Stable id: `g-` plus the first member's id. */
  id: string;
  kind: string;
  /** Count-led label, e.g. `3 crashes fixed`. */
  label: string;
  /** What went away and what came instead, e.g. `throws IndexOutOfRangeException → nothing reported`. */
  pattern: string;
  members: Difference[];
  /** Block ids of the members, in member order, unique. */
  blocks: string[];
}

const EXC = /\b[A-Z]\w*Exception\b/g;
const DIAG = /\b[A-Za-z]{2,}\d{3,}\b/g;
const NOTHING = /^(nothing reported|nothing|not present|absent|none)$/i;

/** One outcome string as its separate effects: `reports MsgPack005, throws X` → two effects. */
function effects(outcome: string): string[] {
  const out: string[] = [];
  for (const raw of String(outcome ?? '').split(/,\s+(?=[a-z])/)) {
    const e = raw.replace(/\s+/g, ' ').trim();
    if (e && !NOTHING.test(e) && !out.includes(e)) out.push(e);
  }
  return out;
}

/** Names that differ between sites of one behaviour: the exception type, the diagnostic id, the receiver of a call. */
const blur = (e: string) => e.replace(EXC, 'Exception').replace(DIAG, 'ID').replace(/\b[\w!?]+(?:\.[\w!?]+)*\.(\w+)\b/g, '_.$1');

/** The effects that differ between old and new; an effect present on both sides is not part of the behaviour. */
function delta(d: Difference): { old: string[]; new: string[] } {
  const o = effects(d.old), n = effects(d.new);
  return { old: o.filter((e) => !n.includes(e)), new: n.filter((e) => !o.includes(e)) };
}

/** Public API differences group by what was added or removed (`new diagnostic`, `new public type`), not by its name. */
function apiWhat(d: Difference): string {
  const c = blur(String(d.condition ?? '')).replace(/\bID\b/g, '').replace(/\s+/g, ' ').trim();
  return c || (NOTHING.test(String(d.old)) ? 'new public API' : NOTHING.test(String(d.new)) ? 'removed public API' : 'public API change');
}

/** Two differences with the same key are the same behaviour. */
export function behaviourKey(d: Difference): string {
  // The producer's own group id wins (`differences[].group`); the text-based key is the fallback.
  if (typeof d.group === 'string' && d.group) return `group|${d.group}`;
  if (d.kind === 'public-api') return `public-api|${apiWhat(d)}`;
  const x = delta(d);
  return `${d.kind}|${x.old.map(blur).join(', ')}|${x.new.map(blur).join(', ')}`;
}

const nouns: Record<string, [string, string]> = {
  'crash-fixed': ['crash fixed', 'crashes fixed'],
  crash: ['new exception', 'new exceptions'],
  'silent-failure': ['silent failure', 'silent failures'],
  'null-access': ['null access', 'null accesses'],
  'output-changed': ['output change', 'output changes'],
  'new-dependency': ['new dependency', 'new dependencies'],
  'data-loss': ['data loss', 'data losses'],
};

function noun(d: Difference, n: number): string {
  if (d.kind === 'public-api') {
    const w = apiWhat(d);
    return n === 1 ? w : w.endsWith('y') ? `${w.slice(0, -1)}ies` : w.endsWith('API') ? `${w} entries` : `${w}s`;
  }
  const x = delta(d);
  if (d.kind === 'silent-failure' && x.old.some((e) => e.startsWith('reports')) && !x.new.length) {
    return n === 1 ? 'diagnostic dropped' : 'diagnostics dropped';
  }
  const pair = nouns[d.kind];
  return pair ? pair[n === 1 ? 0 : 1] : d.kind.replace(/-/g, ' ');
}

const short = (s: string) => s.replace(/\s+/g, ' ').trim();
const tail = (name: string) => name.split('.').slice(-2).join('.');

function side(members: Difference[], which: 'old' | 'new'): string {
  const first = delta(members[0])[which];
  if (!first.length) {
    return members.some((m) => /nothing reported/i.test(String(m[which]))) ? 'nothing reported'
      : members[0].kind === 'public-api' ? short(String(members[0][which])) : 'nothing';
  }
  let text = first.join(', ');
  for (const re of [EXC, DIAG]) {
    const names = [...new Set(members.flatMap((m) => delta(m)[which].join(', ').match(re) ?? []))];
    if (names.length > 1) {
      // Diagnostic ids are short enough to list; of several exception types the first is named and the rest counted,
      // so the outcome after the arrow keeps its room. The members name each one.
      const keep = re === EXC ? 1 : 3;
      const shown = names.slice(0, keep).join(' / ') + (names.length > keep ? ` +${names.length - keep}` : '');
      let done = false;
      text = text.replace(re, (m) => (done ? m : ((done = true), shown)));
    }
  }
  return text;
}

function pattern(members: Difference[]): string {
  const d = members[0];
  if (d.kind === 'public-api') {
    const names = members.map((m) => tail(short(String(NOTHING.test(String(m.new)) ? m.old : m.new))));
    return names.slice(0, 3).join(', ') + (names.length > 3 ? ` +${names.length - 3}` : '');
  }
  return `${clip(side(members, 'old'))} → ${clip(side(members, 'new'))}`;
}

/** A side that is a block of generated text is cut: the whole text is in the difference's detail. */
const clip = (s: string, max = 72) => (s.length <= max ? s : `${s.slice(0, max).trimEnd()}…`);

/** `old → new` for some differences of one behaviour. */
export const behaviourPattern = (members: Difference[]) => (members.length ? pattern(members) : '');

/** Count-led words for some differences of one behaviour, e.g. `2 crashes fixed`. */
export const behaviourLabel = (members: Difference[]) => (members.length ? `${members.length} ${noun(members[0], members.length)}` : '');

/** Differences grouped by behaviour, groups in order of their first member's number, members by number. */
export function groupDifferences(differences: Difference[]): DiffGroup[] {
  const byKey = new Map<string, Difference[]>();
  for (const d of [...differences].sort((a, b) => a.number - b.number)) {
    const k = behaviourKey(d);
    const g = byKey.get(k);
    if (g) g.push(d); else byKey.set(k, [d]);
  }
  return [...byKey.values()].map((members) => ({
    id: `g-${members[0].id}`,
    kind: members[0].kind,
    label: `${members.length} ${noun(members[0], members.length)}`,
    pattern: pattern(members),
    members,
    blocks: [...new Set(members.map((m) => m.block))],
  }));
}

export function groupOf(groups: DiffGroup[], difference: string | null): DiffGroup | null {
  return difference ? groups.find((g) => g.members.some((m) => m.id === difference)) ?? null : null;
}
