// Citations of the AI review (D46): which item a cite names, whether it exists, and where it leads.
import { basename, behaviourLabel, behaviourPattern, groupDifferences, groupOf, lostValue, ruleName, type BundleData, type DiffGroup, type Shell, type Slot } from '../../shell';
import type { RuleRef } from '../../shell/data';
import { headingKeys, reviewBanned, type HeadingKey } from './labels';

/** `text` is the short line; `detail` the longer wording when the model gave one (shown on demand). */
export interface Statement { heading: HeadingKey; text: string; cites: string[]; detail?: string }
export interface Dropped { text: string; cites: string[]; failed: string | null }
export interface Question { id: string; rule: string; block: string | null; differences: string[]; tests: string[] }
interface Test { name: string; category: string; location?: { file: string; line: number } }
interface Replay { difference: string | null; input: { source: string; text: string } | null }

export type Cite =
  | { kind: 'difference'; id: string; number: number }
  | { kind: 'block'; id: string }
  | { kind: 'question'; id: string }
  | { kind: 'part'; id: string }
  | { kind: 'test'; name: string }
  | { kind: 'category'; category: string; count: number }
  | { kind: 'preset'; difference: string; number: number }
  | { kind: 'reproduction' }
  | { kind: 'location'; file: string; line: number };

function list<T>(rec: unknown, field: string): T[] {
  const v = rec && (rec as Record<string, unknown>)[field];
  return Array.isArray(v) ? (v as T[]) : [];
}

export const questionsOf = (data: BundleData) =>
  list<Question & { rule: RuleRef }>(data.records.questions, 'questions').map((q) => ({ ...q, rule: ruleName(q.rule) || 'question' })) as Question[];
export const testsOf = (data: BundleData) => list<Test>(data.records.tests, 'tests');
const replaysOf = (data: BundleData) => list<Replay>(data.records.replays, 'replays');
export const modeOf = (data: BundleData): string | null =>
  ((data.records.mode as { mode?: string } | null)?.mode) ?? null;

/** The item a cite names, or null when it does not resolve in this bundle. */
export function resolve(cite: string, data: BundleData): Cite | null {
  const id = /^([dbqp])[1-9][0-9]*$/.exec(cite);
  if (id) {
    const k = id[1];
    if (k === 'd') { const d = data.differences.find((x) => x.id === cite); return d ? { kind: 'difference', id: cite, number: d.number } : null; }
    if (k === 'b') return data.blocks.some((x) => x.id === cite) ? { kind: 'block', id: cite } : null;
    if (k === 'q') return questionsOf(data).some((x) => x.id === cite) ? { kind: 'question', id: cite } : null;
    return data.parts.some((x) => x.id === cite) ? { kind: 'part', id: cite } : null;
  }
  const preset = /^preset:(d[1-9][0-9]*)$/.exec(cite);
  if (preset) {
    const d = data.differences.find((x) => x.id === preset[1]);
    const ok = d && replaysOf(data).some((r) => r.difference === d.id && r.input);
    return ok ? { kind: 'preset', difference: d.id, number: d.number } : null;
  }
  if (cite === 'reproduction') return replaysOf(data).some((r) => r.input?.source === 'issue') ? { kind: 'reproduction' } : null;
  const tests = testsOf(data);
  if (tests.some((t) => t.name === cite)) return { kind: 'test', name: cite };
  const inCat = tests.filter((t) => t.category === cite);
  if (inCat.length) return { kind: 'category', category: cite, count: inCat.length };
  const loc = /^(.+):([1-9][0-9]*)$/.exec(cite);
  if (loc) {
    const f = data.files.find((x) => x.file === loc[1]) ?? data.files.find((x) => basename(x.file) === loc[1]);
    if (f) return { kind: 'location', file: f.file, line: Number(loc[2]) };
  }
  return null;
}

export const hasBanned = (text: string) => reviewBanned.some((w) => text.toLowerCase().includes(w));

export interface Review {
  recorded: boolean;
  byHeading: Record<HeadingKey, Statement[]>;
  dropped: Dropped[];
  /** The model's one line per heading, where it gave one that passes the word check. */
  summaries: Partial<Record<HeadingKey, string>>;
}

/** Longest a line of the short check may be, in words. */
export const LINE_WORDS = 12;

/** A statement as one short line: its first clause, cut at `max` words. */
export function shortLine(text: string, max = LINE_WORDS): string {
  const whole = text.replace(/\s+/g, ' ').trim().replace(/[.;]$/, '');
  if (whole.split(' ').length <= max) return whole;
  const clause = text.replace(/\s+/g, ' ').trim().split(/(?<=[^.]{12})(?:; |\. (?=[A-Z])| — )/)[0].replace(/[.;]$/, '');
  const words = clause.split(' ');
  return words.length <= max ? clause : `${words.slice(0, max).join(' ').replace(/[,;:]$/, '')}…`;
}

export const wordCount = (text: string) => text.split(/\s+/).filter(Boolean).length;

/** Checks `ai-review.json` again on the page: unresolved cites and banned words are dropped (D46). */
export function readReview(data: BundleData): Review {
  const rec = data.records['ai-review'] as {
    recording: string | null; statements?: Statement[]; dropped?: { text: string; cites: string[] }[];
    summaries?: Record<string, unknown>;
  } | null;
  const byHeading = Object.fromEntries(headingKeys.map((h) => [h, [] as Statement[]])) as Record<HeadingKey, Statement[]>;
  if (!rec || !rec.recording) return { recorded: false, byHeading, dropped: [], summaries: {} };
  const dropped: Dropped[] = (rec.dropped ?? []).map((d) => ({ ...d, failed: d.cites.find((c) => !resolve(c, data)) ?? null }));
  for (const s of rec.statements ?? []) {
    const failed = s.cites.find((c) => !resolve(c, data));
    if (!headingKeys.includes(s.heading) || !s.cites.length || failed !== undefined || hasBanned(s.text) || hasBanned(s.detail ?? '')) {
      dropped.push({ text: s.text, cites: s.cites, failed: failed ?? null });
    } else byHeading[s.heading].push(s);
  }
  const summaries: Partial<Record<HeadingKey, string>> = {};
  for (const h of headingKeys) {
    const v = rec.summaries?.[h];
    if (typeof v === 'string' && v.trim() && !hasBanned(v)) summaries[h] = shortLine(v);
  }
  // Left to you lists every question (D40, D46).
  const asked = new Set(byHeading['left-to-you'].flatMap((s) => s.cites));
  for (const q of questionsOf(data)) {
    if (!asked.has(q.id)) byHeading['left-to-you'].push({ heading: 'left-to-you', text: '', cites: [q.id] });
  }
  return { recorded: true, byHeading, dropped, summaries };
}

/** Questions that are one decision: the same rule on the same behaviour, on blocks of one file, or one test group. */
export interface QuestionGroup {
  /** `qg-` plus the first question's id. */
  id: string;
  rule: string;
  questions: Question[];
  /** Count-led words for what is decided, e.g. `3 crashes fixed`, `6 blocks`, `4 hanging tests`. */
  label: string;
  /** Behaviour group of the differences, when the questions are about differences. */
  behaviour: DiffGroup | null;
  differences: string[];
  blocks: string[];
  loc: { file: string; line: number; side: 'old' | 'new' } | null;
}

export function questionGroups(data: BundleData): QuestionGroup[] {
  const behaviours = groupDifferences(data.differences);
  const tests = testsOf(data);
  const byKey = new Map<string, { behaviour: DiffGroup | null; questions: Question[] }>();
  for (const q of questionsOf(data)) {
    const behaviour = groupOf(behaviours, q.differences[0] ?? null);
    const file = q.block ? blockLoc(data, q.block)?.file ?? q.block : '';
    const key = behaviour ? `${q.rule}|${behaviour.id}` : q.tests.length && !q.block ? `${q.rule}|${q.id}` : `${q.rule}|${file}`;
    const g = byKey.get(key);
    if (g) g.questions.push(q); else byKey.set(key, { behaviour, questions: [q] });
  }
  // The gravest decisions lead, so the few shown on the first screen are the ones that matter most:
  // lost data, new exceptions, lost diagnostics, then other differences, blocks to read, tests.
  const gravity = ['data-loss', 'crash', 'silent-failure', 'null-access', 'public-api', 'new-dependency', 'output-changed', 'crash-fixed'];
  const rank = (g: { behaviour: DiffGroup | null; questions: Question[] }) => {
    const kind = data.differences.find((d) => d.id === g.questions[0].differences[0])?.kind;
    if (kind) { const i = gravity.indexOf(kind); return i < 0 ? gravity.length : i; }
    return g.questions[0].block ? gravity.length + 1 : gravity.length + 2;
  };
  const ordered = [...byKey.values()].map((g, i) => ({ g, i })).sort((a, b) => rank(a.g) - rank(b.g) || a.i - b.i).map((x) => x.g);
  return ordered.map(({ behaviour, questions }) => {
    const q0 = questions[0];
    const differences = [...new Set(questions.flatMap((q) => q.differences))];
    const blocks = [...new Set(questions.map((q) => q.block).filter((b): b is string => !!b))];
    const names = [...new Set(questions.flatMap((q) => q.tests))];
    const cats = [...new Set(tests.filter((t) => names.includes(t.name)).map((t) => t.category))];
    const loc = q0.block ? blockLoc(data, q0.block) : null;
    let label: string;
    if (behaviour) {
      // The words of the behaviour, counted over the differences these questions ask about.
      label = behaviourLabel(behaviour.members.filter((m) => differences.includes(m.id))) || behaviour.label;
    } else if (blocks.length) {
      const b = data.blocks.find((x) => x.id === blocks[0]);
      label = blocks.length === 1 ? b?.name ?? blocks[0] : `${blocks.length} blocks · ${loc ? basename(loc.file) : b?.name ?? ''}`;
    } else {
      label = `${names.length} ${cats.length === 1 ? `${cats[0].replace(/-/g, ' ')} ` : ''}${names.length === 1 ? 'test' : 'tests'}`;
    }
    return { id: `qg-${q0.id}`, rule: q0.rule, questions, label, behaviour, differences, blocks, loc };
  });
}

const EXC = /\b[A-Z]\w*Exception\b/g;
const DIAG = /\b[A-Za-z]{2,}\d{3,}\b/g;
const uniq = (xs: string[]) => [...new Set(xs)];

/**
 * Severity of each analyzer diagnostic as a record states it. Only the replays carry one, as compiler output text
 * (`errors MsgPack004; warnings CS8785, AD0001: …`); a diagnostic no replay produced has none.
 */
export function diagnosticSeverities(data: BundleData): Map<string, 'error' | 'warning'> {
  const out = new Map<string, 'error' | 'warning'>();
  for (const r of list<{ old?: unknown; new?: unknown }>(data.records.replays, 'replays')) {
    for (const side of [r.old, r.new]) {
      for (const m of String(side ?? '').matchAll(/\b(errors|warnings)\s+([^;:]+)/g)) {
        for (const id of m[2].match(DIAG) ?? []) if (!out.has(id)) out.set(id, m[1] === 'errors' ? 'error' : 'warning');
      }
    }
  }
  return out;
}

/** The delegate the code reports diagnostics through, as the difference's condition names it (`reportDiagnostic`). */
function reporter(ms: { condition?: unknown }[]): string | null {
  for (const m of ms) { const r = /\b(report\w*Diagnostic\w*)\b/i.exec(String(m.condition ?? '')); if (r) return r[1]; }
  return null;
}

/** Diagnostic ids as the library's user sees them: `Analyzer error MsgPack005`, `3 analyzer diagnostics`. */
function diagnosticNoun(ids: string[], sev: Map<string, 'error' | 'warning'>): string {
  const kinds = new Set(ids.map((id) => sev.get(id) ?? 'diagnostic'));
  const word = kinds.size === 1 ? [...kinds][0] : 'diagnostic';
  return ids.length === 1 ? `analyzer ${word}` : `${ids.length} analyzer ${word}s`;
}

const cap = (s: string) => s[0].toUpperCase() + s.slice(1);
const listIds = (ids: string[], max = 3) => ids.slice(0, max).join(', ') + (ids.length > max ? ` +${ids.length - max}` : '');

/** A decision as one plain line: what the library's user sees first, then the name in the code. */
export function plainLine(data: BundleData, g: QuestionGroup): { line: string } {
  if (g.behaviour) {
    const ms = g.behaviour.members.filter((m) => g.differences.includes(m.id));
    const d = ms[0] ?? g.behaviour.members[0];
    const pat = behaviourPattern(ms.length ? ms : g.behaviour.members);
    const lost = g.differences.map((id) => lostValue(data, id)).find(Boolean);
    const olds = ms.map((m) => String(m.old)).join(' ');
    const news = ms.map((m) => String(m.new)).join(' ');
    const places = ms.length > 1 ? ` (${ms.length} places)` : '';
    if (lost) { const [sent, got] = lost.split(' → '); return { line: `A value of ${sent} comes back as ${got}` }; }
    const ids = uniq(olds.match(DIAG) ?? []).filter((id) => !news.includes(id));
    if ((d.kind === 'silent-failure' || d.kind === 'data-loss') && ids.length) {
      const sev = diagnosticSeverities(data);
      const call = reporter(ms);
      if (ids.length === 1) return { line: `${cap(diagnosticNoun(ids, sev))} ${ids[0]} no longer shown${call ? ` (${call} not called)` : ''}${places}` };
      return { line: `${diagnosticNoun(ids, sev)} no longer shown: ${listIds(ids)}${places}` };
    }
    if (d.kind === 'public-api') {
      const added = ms.map((m) => String(m.new)).filter((v) => new RegExp(`^${DIAG.source}$`).test(v));
      if (added.length && added.length === ms.length) {
        const noun = diagnosticNoun(added, diagnosticSeverities(data));
        return { line: added.length === 1 ? `New ${noun} ${added[0]} can be shown` : `${added.length} new ${noun.replace(/^\d+ /, '')}: ${listIds(added)}` };
      }
    }
    if (d.kind === 'crash-fixed') { const e = uniq(olds.match(EXC) ?? []); if (e.length) return { line: `${e.join(', ')} no longer thrown${places}` }; }
    if (d.kind === 'crash') { const e = uniq(news.match(EXC) ?? []); if (e.length) return { line: `Can now throw ${e.join(', ')}` }; }
    // a generated text or long value is not a line: the count and the location say where, the evidence shows it
    return { line: pat.length > 64 ? g.label : `${g.label}: ${pat}` };
  }
  if (g.blocks.length) {
    const b = data.blocks.find((x) => x.id === g.blocks[0]);
    return { line: g.blocks.length === 1 ? `Read ${b?.name ?? g.blocks[0]}` : `Read ${g.blocks.length} blocks in ${g.loc ? basename(g.loc.file) : b?.name ?? ''}` };
  }
  const n = g.questions.reduce((k, q) => k + q.tests.length, 0);
  if (/hanging/.test(g.label)) return { line: `${n} new ${n === 1 ? 'test already passes' : 'tests already pass'} on the old code` };
  if (/not run/.test(g.label)) return { line: `${n} ${n === 1 ? 'test' : 'tests'} did not run` };
  return { line: g.label };
}

export type ReviewItem =
  | { kind: 'decision'; key: string; group: QuestionGroup; line: string; grave: boolean }
  | { kind: 'statement'; key: string; statement: Statement; line: string };

/**
 * The one list of the first screen: what needs the reader, most serious first. Decisions carry Accept / Return;
 * what the review says was described but not found, and unasked-for changes no decision covers, are plain lines.
 */
export function reviewItems(data: BundleData, review: Review, groups: QuestionGroup[]): ReviewItem[] {
  const decided = new Set(groups.flatMap((g) => g.differences));
  const kindOf = (g: QuestionGroup) => data.differences.find((d) => d.id === g.differences[0])?.kind ?? '';
  const decisions = groups.map((g): ReviewItem => ({ kind: 'decision', key: g.id, group: g, ...plainLine(data, g), grave: ['data-loss', 'crash', 'silent-failure', 'null-access'].includes(kindOf(g)) }));
  const st = (s: Statement, i: number): ReviewItem => ({ kind: 'statement', key: `${s.heading}-${i}`, statement: s, line: shortLine(s.text) });
  // Several statements of one heading that no decision covers are said once, in the AI's own line for that heading:
  // it tells what happens to the library's user, the single statements name the code (Full text, Behaviour changes).
  const merged = (heading: HeadingKey, items: Statement[]): ReviewItem[] => {
    const sum = review.summaries[heading];
    if (items.length < 2 || !sum) return items.map(st);
    const statement: Statement = { heading, text: sum, cites: uniq(items.flatMap((s) => s.cites)), detail: items.map((s) => s.text).join(' · ') };
    return [{ kind: 'statement', key: `${heading}-all`, statement, line: shortLine(sum) }];
  };
  const missing = merged('described-not-found', review.byHeading['described-not-found']);
  const extra = merged('not-in-description', review.byHeading['not-in-description'].filter((s) => {
    const ds = s.cites.filter((c) => /^d\d+$/.test(c));
    return !ds.length || !ds.every((d) => decided.has(d));
  }));
  return [...decisions.filter((d) => d.kind === 'decision' && d.grave), ...missing, ...extra, ...decisions.filter((d) => d.kind === 'decision' && !d.grave)];
}

/** Sections fold when the review found nothing unexpected and the mode is light (D46, D47). */
export function quiet(data: BundleData, r: Review): boolean {
  return r.recorded && modeOf(data) === 'light' && !r.byHeading['not-in-description'].length && !r.byHeading['described-not-found'].length;
}

const esc = (s: string) => s.replace(/["\\]/g, "\\$&");

/** Selectors for the evidence element a cite leads to, and the slot it lives in. */
export function target(c: Cite): { slot: Slot | null; selector: string } {
  switch (c.kind) {
    // The member row first; the group row that holds it while its group is closed.
    case 'difference': return { slot: 'proof', selector: `[data-row="${c.id}"], tr[data-diff="${c.id}"], [data-members~="${c.id}"], [data-difference="${c.id}"], [data-cite~="diff:${c.number}"]` };
    case 'block': return { slot: 'map', selector: `[data-node="${c.id}"], [data-cite~="block:${c.id}"]` };
    case 'question': return { slot: 'decide', selector: `[data-member="${c.id}"], [data-question="${c.id}"], [data-questions~="${c.id}"]` };
    case 'test': return { slot: 'proof', selector: `[data-test="${esc(c.name)}"], [data-cite~="test:${esc(c.name)}"]` };
    case 'category': return { slot: 'proof', selector: `[data-cite~="tests:${c.category}"], [data-category="${c.category}"]` };
    case 'preset': return { slot: 'playground', selector: `[data-cite~="preset:${c.difference}"], [data-preset="${c.difference}"]` };
    case 'reproduction': return { slot: 'playground', selector: '[data-cite~="reproduction"], [data-preset="reproduction"]' };
    default: return { slot: null, selector: '' };
  }
}

function find(c: Cite): HTMLElement | null {
  const t = target(c);
  if (!t.slot) return null;
  // Selectors in order of preference, not document order.
  for (const sel of t.selector.split(', ')) {
    const el = document.querySelector<HTMLElement>(`#slot-${t.slot} ${sel}`);
    if (el) return el;
  }
  return null;
}

function flash(el: HTMLElement) {
  if (!el.matches('button, a, input, [tabindex]')) el.tabIndex = -1;
  el.focus({ preventScroll: true });
  // Scroll after the unfolded section has laid out, and again once late content settles.
  // A target taller than the window (a test row with its assertion) is aligned at its top.
  const scroll = () => el.scrollIntoView?.({ block: el.getBoundingClientRect().height > window.innerHeight * 0.8 ? 'start' : 'center' });
  scroll();
  if (typeof requestAnimationFrame === 'function') requestAnimationFrame(() => requestAnimationFrame(scroll));
  setTimeout(scroll, 250);
  el.dataset.flash = '';
  setTimeout(() => { delete el.dataset.flash; }, 1400);
}

/** Opens a cite's evidence in one action (D46, D8). */
export function follow(cite: string, shell: Shell): void {
  const { data } = shell;
  const c = resolve(cite, data);
  if (!c) return;
  const t = target(c);
  if (t.slot) shell.setFolded(t.slot, false);
  if (c.kind === 'difference' || c.kind === 'preset') shell.selectDifference(c.kind === 'preset' ? c.difference : c.id);
  if (c.kind === 'block') shell.focusBlock(c.id);
  // What hides the target unfolds: the tests list, a question's group.
  if (c.kind === 'test') shell.setReveal(`test:${c.name}`);
  if (c.kind === 'category') shell.setReveal(`tests:${c.category}`);
  if (c.kind === 'question') shell.setReveal(`question:${c.id}`);
  if (c.kind === 'part') {
    const p = data.parts.find((x) => x.id === c.id)!;
    const loc = p.new ?? p.old;
    if (loc) shell.openCode({ file: loc.file, line: loc.line, side: p.new ? 'new' : 'old' });
    return;
  }
  if (c.kind === 'location') { shell.openCode({ file: c.file, line: c.line }); return; }
  let tries = 0;
  // A difference or question inside a closed group: give the group a moment to open before settling for its row.
  const own = c.kind === 'difference' ? `#slot-proof [data-row="${c.id}"]` : c.kind === 'question' ? `#slot-decide [data-member="${c.id}"]` : null;
  const go = () => {
    if (own && tries < 5 && !document.querySelector(own)) { tries++; setTimeout(go, 30); return; }
    const el = find(c);
    if (el) {
      flash(el);
      if (c.kind === 'reproduction' && el.tagName === 'BUTTON') el.click();
      return;
    }
    if (++tries < 12) { setTimeout(go, 30); return; }
    if (c.kind === 'test') {
      const loc = testsOf(data).find((x) => x.name === c.name)?.location;
      if (loc) shell.openCode({ file: loc.file, line: loc.line });
      return;
    }
    if (t.slot) document.getElementById(`slot-${t.slot}`)?.scrollIntoView?.({ block: 'start' });
  };
  setTimeout(go, 0);
}

/** First location of a block: its first part with a location. */
export function blockLoc(data: BundleData, id: string): { file: string; line: number; side: 'old' | 'new' } | null {
  const b = data.blocks.find((x) => x.id === id);
  for (const pid of b?.parts ?? []) {
    const p = data.parts.find((x) => x.id === pid);
    if (p?.new) return { file: p.new.file, line: p.new.line, side: 'new' };
    if (p?.old) return { file: p.old.file, line: p.old.line, side: 'old' };
  }
  return null;
}
