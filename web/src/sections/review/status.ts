// The page's status, like a pull request's checks: computed from the records by fixed rules, never by the AI.
// The AI's headings can raise a change to orange (claims without evidence, changes nobody asked for); only computed
// evidence can make it red, and green needs both to be quiet.
import { lostValue, type BundleData } from '../../shell';
import { questionGroups, readReview, type Review } from './cites';
import { statusLabels } from './labels';

export { statusLabels };
export type StatusLevel = keyof typeof statusLabels;
export interface Status {
  level: StatusLevel;
  label: string;
  /** Why the status is what it is, count-led (`1 data loss`, `4 to decide`); empty when green. */
  reasons: string[];
}

const n = (k: number, one: string, many = `${one}s`) => `${k} ${k === 1 ? one : many}`;

/** Computed facts the rule reads. Exposed for the explanation and the tests. */
export function statusInputs(data: BundleData, review: Review = readReview(data)) {
  const ds = data.differences;
  const lost = ds.filter((d) => d.kind === 'data-loss' || lostValue(data, d.id)).length;
  const silent = ds.filter((d) => d.kind === 'silent-failure');
  const dropped = silent.filter((d) => /\breports\b/.test(String(d.old)) && !/\breports\b/.test(String(d.new))).length;
  const thrown = ds.filter((d) => d.kind === 'crash' || d.kind === 'null-access').length;
  const scope = data.records.scope as { modelledLines?: number; changedLines?: { production?: number } } | null;
  const changed = scope?.changedLines?.production ?? 0;
  const compared = scope?.modelledLines ?? 0;
  const undeclared = new Set(questionGroups(data).filter((g) => g.rule === 'undeclared-difference').flatMap((g) => g.differences)).size;
  return {
    lost, dropped, silent: silent.length - dropped, thrown, changed, compared,
    // Changes beyond the description: the AI's count when there is a review, else the undeclared differences.
    unasked: review.recorded ? review.byHeading['not-in-description'].length : undeclared,
    unshown: review.recorded ? review.byHeading['described-not-found'].length : 0,
    recorded: review.recorded,
    decide: questionGroups(data).length,
  };
}

/**
 * Red: any lost value, dropped diagnostic or other silent failure, new exception, or less than half the changed
 * production lines compared. Orange: changes beyond the description, claims without evidence, decisions left, or no
 * AI summary. Green: none of these.
 */
export function statusOf(data: BundleData, review: Review = readReview(data)): Status {
  const x = statusInputs(data, review);
  const red: string[] = [];
  if (x.lost) red.push(n(x.lost, 'data loss', 'data losses'));
  if (x.dropped) red.push(n(x.dropped, 'diagnostic dropped', 'diagnostics dropped'));
  if (x.silent) red.push(n(x.silent, 'silent failure'));
  if (x.thrown) red.push(n(x.thrown, 'new exception'));
  if (x.changed && x.compared < x.changed / 2) red.push(`${x.compared} of ${x.changed} lines compared`);
  const orange: string[] = [];
  if (x.unasked) orange.push(`${x.unasked} not asked for`);
  if (x.unshown) orange.push(n(x.unshown, 'claim without evidence', 'claims without evidence'));
  if (x.decide) orange.push(`${x.decide} to decide`);
  if (!x.recorded) orange.push('no AI summary');
  const level: StatusLevel = red.length ? 'red' : orange.length ? 'orange' : 'green';
  return { level, label: statusLabels[level], reasons: [...red, ...orange] };
}

/** The two facts under the status: what was asked for, and what was not. */
export function factsOf(data: BundleData, review: Review = readReview(data)): { asked: string; unasked: string } {
  let asked = 'no AI summary';
  if (review.recorded) {
    const m = review.byHeading.matches.length;
    const all = m + review.byHeading['described-not-found'].length;
    asked = all ? `${m} of ${all} ${all === 1 ? 'claim has' : 'claims have'} evidence` : 'no claims found';
  }
  const x = statusInputs(data, review);
  const out: string[] = [];
  if (x.unasked) out.push(n(x.unasked, 'change'));
  const lost = data.differences.map((d) => lostValue(data, d.id)).filter(Boolean) as string[];
  if (lost.length) out.push(`${lost.length} ${lost.length === 1 ? 'loses' : 'lose'} data: ${lost[0]}`);
  if (x.thrown) out.push(n(x.thrown, 'new exception'));
  return { asked, unasked: out.length ? out.join(', ') : 'nothing' };
}
