// Pure views over the proof records (rows, differences, scope, lean, tests, links).
import type { BundleData, Difference, Location } from '../../shell';
import { checkLabels } from './labels';

export type LeanState = 'checking' | 'checked' | 'model-checked' | 'not-checked';
export interface LeanRow { row: string; state: LeanState; file?: string; seconds?: number }
export interface Scope {
  changedLines: Record<string, number>;
  modelledLines: number;
  assumptions: { contract: string; symbol: string; rows: string[] }[];
  notModelled: { location: Location; reason?: string }[];
}
export interface Test {
  name: string; location: Location; change: string; category: string; old: string | null; new: string | null;
  failureMessage: string | null; assertion: { before: string | null; after: string | null } | null;
}
export interface Link { block: string; state: string; tests: string[] }

const list = <T>(rec: unknown, field: string): T[] => {
  const v = rec && (rec as Record<string, unknown>)[field];
  return Array.isArray(v) ? (v as T[]) : [];
};

export const scopeOf = (d: BundleData) => (d.records.scope as Scope | null) ?? null;
export const leanRows = (d: BundleData) => list<LeanRow>(d.records.lean, 'rows');
export const testsOf = (d: BundleData) => list<Test>(d.records.tests, 'tests');
export const linksOf = (d: BundleData) => list<Link>(d.records.links, 'links');

export type Check = keyof typeof checkLabels;

/** How far one difference was checked (D22). Solver rows take their Lean state; a missing Lean row is not checked. */
export function checkOf(d: BundleData, diff: Difference): Check {
  if (diff.checker === 'file-comparison' || diff.checker === 'api-comparison') return diff.checker;
  const lean = leanRows(d);
  const states = (diff.rows ?? []).map((r) => lean.find((l) => l.row === r)?.state ?? 'not-checked');
  if (!states.length || states.includes('not-checked')) return 'not-checked';
  if (states.includes('checking')) return 'checking';
  if (states.includes('model-checked')) return 'model-checked';
  return 'checked';
}

export const leanChecked = (c: Check) => c === 'checked' || c === 'model-checked';

/** Count of differences Lean did not check (D22). */
export function notLeanCount(d: BundleData): number {
  return d.differences.filter((x) => !leanChecked(checkOf(d, x))).length;
}

export function testsFor(d: BundleData, diff: Difference): string[] {
  return linksOf(d).find((l) => l.block === diff.block)?.tests ?? [];
}

export const shortName = (name: string) => name.slice(name.lastIndexOf('.') + 1);

export function perIteration(d: BundleData, diff: Difference): boolean {
  const rows = list<{ id: string; perIteration?: boolean }>(d.records.rows, 'rows');
  const mine = rows.filter((r) => (diff.rows ?? []).includes(r.id));
  return mine.length > 0 && mine.every((r) => r.perIteration);
}

/** Solver syntax (SMT-LIB terms, mangled symbols) is never printed in a row (D2, D21). */
const solverSyntax = /\((not|and|or|=|<=|>=|<|>|ite)\s|\bw_[a-z]|_[0-9a-f]{4}\b/;
export function readableCondition(diff: Difference): string | null {
  const alt = diff as { conditionText?: unknown; readableCondition?: unknown };
  for (const v of [alt.readableCondition, alt.conditionText, diff.condition]) {
    if (typeof v === 'string' && v.trim() && !solverSyntax.test(v)) return v;
  }
  return null;
}

export interface Fact { key: string; text: string }
export interface UnitFacts { unit: string; facts: Fact[] }
export interface LoopResult { unit: string; perIteration: string; lifted: string; reason?: string }

/** Assumed facts per unit: `lean.json` `coverage[].facts` (the contract), or a top-level `facts` list in older bundles. */
export function leanFacts(d: BundleData): UnitFacts[] {
  type Raw = { unit: string; facts?: { key: string; text?: string; statement?: string }[] };
  const read = (u: Raw) => ({ unit: u.unit, facts: (u.facts ?? []).map((f) => ({ key: f.key, text: f.text ?? f.statement ?? f.key })) });
  const out = list<Raw>(d.records.lean, 'coverage').filter((u) => u.facts?.length).map(read);
  for (const u of list<Raw>(d.records.lean, 'facts').map(read)) {
    if (!out.some((x) => x.unit === u.unit)) out.push(u);
  }
  return out;
}
export const leanLoops = (d: BundleData) => list<LoopResult>(d.records.lean, 'loops');
export const leanCoverage = (d: BundleData) =>
  list<{ unit: string; state: LeanState; file?: string | null }>(d.records.lean, 'coverage');

/** Not-modelled ranges, one per file and line. */
export function notModelledOnce(s: Scope) {
  const seen = new Set<string>();
  return s.notModelled.filter((r) => {
    const k = `${r.location.file}:${r.location.line}`;
    if (seen.has(k)) return false;
    seen.add(k);
    return true;
  });
}

/** Before and after of one test (D26): declared → old failure message and assertion; edited → old and new assertion. */
/** Result words of a test on the old and new code, e.g. `fail → pass`. */
export function results(t: Test): string {
  const w = (s: string | null) => (s ?? 'not run').replace(/-/g, ' ');
  return `${w(t.old)} → ${w(t.new)}`;
}

/** First line of a failure message: what a test runner prints as the headline. */
export const firstLine = (s: string) => s.split(/\r?\n/).find((l) => l.trim())?.trim() ?? '';

/** A failing test on one line: the producer's `failureSummary` when present, else the first line of the message. */
export function failureLine(t: Test): string | null {
  const s = (t as Test & { failureSummary?: string | null }).failureSummary;
  if (typeof s === 'string' && s.trim()) return s.trim();
  return t.failureMessage ? firstLine(t.failureMessage) || null : null;
}

export function beforeAfter(t: Test): { before: string | null; after: string | null } {
  const a = t.assertion ?? { before: null, after: null };
  if (a.before != null) return { before: a.before, after: a.after };
  if (t.category === 'declared' || t.category === 'new-api') return { before: t.failureMessage, after: a.after };
  return { before: null, after: a.after };
}
