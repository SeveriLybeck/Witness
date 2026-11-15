import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { fixtureSource, readRecord, renderPage, type Change } from '../../../test/harness';
import { DiffNumber, useShell, type SectionModule, type SectionProps } from '../../shell';
import { sections } from './index';
import { LINE_WORDS, plainLine, questionGroups, readReview, resolve, reviewItems, shortLine, wordCount } from './cites';
import { factsOf, statusOf } from './status';
import { allLabels, headings, reviewBanned, ruleLabel } from './labels';
import reviewCss from './review.css?raw';
import { resetSessions, type Session } from './session';
import { loadBundle } from '../../shell/data';

function ProofStub({ data }: SectionProps) {
  const { selection } = useShell();
  return <ul>{data.differences.map((d) => (
    <li key={d.id} data-row={d.id} aria-selected={selection.difference === d.id}><DiffNumber id={d.id} /></li>
  ))}</ul>;
}
function MapStub({ data }: SectionProps) {
  const { selection } = useShell();
  return <div>{data.blocks.map((b) => <div key={b.id} data-node={b.id} data-focus={selection.block === b.id || undefined}>{b.name}</div>)}</div>;
}
const mods: Partial<Record<SectionModule['slot'], SectionModule>> = {
  proof: { slot: 'proof', Component: ProofStub },
  map: { slot: 'map', Component: MapStub },
  playground: { slot: 'playground', Component: () => <div data-playground /> },
  ...Object.fromEntries(sections.map((s) => [s.slot, s])),
};

/** An in-memory gateway session endpoint; records every request. */
function gateway(change: Change, patch: Record<string, unknown> = {}, init: Partial<Session> = {}) {
  const calls: { method: string; path: string; body?: unknown; headers?: Record<string, string> }[] = [];
  const reviews: { status: number; data: unknown }[] = [];
  const b = readRecord(change, 'bundle') as { oldCommit: string; newCommit: string };
  const sess: Session = { oldCommit: b.oldCommit, newCommit: b.newCommit, reviewer: 'reviewer-a', opened: [], shown: [], runs: [], answers: {}, spotCheck: null, ...init };
  const base = fixtureSource(change, {
    onRequest(method, path, body, headers) {
      calls.push({ method, path, body, headers });
      if (!path.startsWith('/api/sessions/')) throw Object.assign(new Error('404'), { status: 404 });
      if (path.endsWith('/review')) {
        const r = reviews.shift()!;
        if (r.status >= 400) throw Object.assign(new Error(`gateway ${r.status}`), r);
        return r.data;
      }
      if (method === 'POST') {
        const u = body as { answer?: { question: string; value: 'accept' | 'return' }; spotCheck?: string; opened?: string[]; shown?: string[] };
        if (u.answer) sess.answers = { ...sess.answers, [u.answer.question]: u.answer.value };
        if (u.spotCheck && !sess.spotCheck) sess.spotCheck = u.spotCheck;
        if (u.opened) sess.opened = [...sess.opened, ...u.opened];
        if (u.shown) sess.shown = [...sess.shown, ...u.shown];
      }
      return structuredClone(sess);
    },
  });
  const source = {
    ...base,
    async record(k: string, name: Parameters<typeof base.record>[1]) {
      return name in patch ? patch[name] : base.record(k, name);
    },
  };
  return { source, calls, sess, reviews };
}

const slot = (s: string) => document.getElementById(`slot-${s}`)!;

beforeEach(() => { resetSessions(); localStorage.setItem('witness.reviewer', 'reviewer-a'); });
afterEach(() => { vi.restoreAllMocks(); localStorage.clear(); });

const openFull = () => fireEvent.click(slot('review').querySelector('[data-fulltext]')!);
const full = () => document.querySelector<HTMLElement>('[data-testid="review-full"]')!;
const box = () => slot('review').querySelector<HTMLElement>('[data-review-box]')!;
const list = () => document.getElementById('slot-decide')!;

describe('the review as a summary (D46)', () => {
  it('2283: status by the title, the box labelled AI summary with two facts, a matches group and a needs-you group', async () => {
    const { source } = gateway('2283');
    const { data } = await renderPage('2283', { sections: mods, source, asLoaded: true });
    const review = readReview(data);
    const groups = questionGroups(data);
    const status = slot('review').querySelector<HTMLElement>('.slot-title [data-status]')!;
    expect(status.getAttribute('data-status')).toBe(statusOf(data).level);
    expect(status.querySelector('.st-label')!.textContent).toBe(statusOf(data).label);
    expect(status.querySelector('[role="tooltip"]')!.textContent).toBe(statusOf(data).reasons.join(' · '));
    expect(box().querySelector('legend')!.textContent).toBe('AI summary');
    const fact = (k: string) => box().querySelector(`[data-fact="${k}"]`)!;
    expect(fact('asked').querySelector('.rv-k')!.textContent).toBe('Asked for');
    expect(fact('asked').querySelector('[data-value]')!.textContent).toBe(factsOf(data, review).asked);
    expect(fact('unasked').querySelector('.rv-k')!.textContent).toBe('Not asked for');
    expect(fact('unasked').querySelector('[data-value]')!.textContent).toBe(factsOf(data, review).unasked);
    expect(box().textContent).not.toMatch(/Does what it says|nobody asked for/);
    expect(groups.length).toBe(2);
    // no heading glyphs, counters or bare numbered chips on the first layer
    expect(box().querySelectorAll('.rv-glyph, [data-chip], .diffno, [data-more-cites]')).toHaveLength(0);
    expect(box().textContent).not.toMatch(/Not in the description|Left to you|Described, not found/);
    expect([...box().querySelectorAll(':scope > .rv-group > .rv-k')].map((k) => k.textContent)).toEqual(['Matches', 'Needs you']);
    // what matches is one quiet line that expands
    const matches = box().querySelector('[data-heading="matches"]')!;
    expect(matches.querySelector('[data-summary]')!.textContent).toBe('Fixes the generator crash on a closed generic struct from a referenced…');
    expect(matches.querySelectorAll('[data-statement]')).toHaveLength(0);
    fireEvent.click(matches.querySelector('.rv-head')!);
    expect(matches.querySelectorAll('[data-statement]')).toHaveLength(1);
    // the list: every question, as decisions, the gravest first; each with one way to its evidence
    const rows = [...list().querySelectorAll('[data-question]')];
    expect(rows.flatMap((r) => r.getAttribute('data-questions')!.split(' ')).sort()).toEqual(['q1', 'q2']);
    expect(rows.map((r) => r.getAttribute('data-question'))).toEqual(['q1', 'q2']);
    expect(rows.map((r) => r.querySelector('.rv-qtext')!.textContent)).toEqual(reviewItems(data, review, groups).map((i) => i.line));
    expect(rows[1].querySelector('.rv-qtext')!.textContent).toBe('IndexOutOfRangeException no longer thrown');
    for (const r of rows) expect(r.querySelector('[data-evidence]')!.textContent).toBe('Evidence');
    for (const l of rows.map((r) => r.querySelector('.rv-qtext')!.textContent!)) expect(wordCount(l)).toBeLessThanOrEqual(LINE_WORDS);
  });

  it('plain lines: lost value, dropped diagnostic, crash fixed, blocks, tests', async () => {
    const { data } = await renderPage('2283', { sections: mods, source: gateway('2283').source });
    const g = questionGroups(data);
    const fake = (over: object) => ({ ...g[0], behaviour: null, differences: [], blocks: [], loc: null, ...over }) as (typeof g)[number];
    expect(plainLine(data, fake({ blocks: ['b1'] })).line).toBe('Read ReportIfLocatedInCompilation');
    expect(plainLine(data, fake({ label: '4 hanging tests', questions: [{ ...g[0].questions[0], tests: ['a', 'b', 'c', 'd'] }] })).line).toBe('4 new tests already pass on the old code');
    expect(plainLine(data, fake({ label: '2 not run tests', questions: [{ ...g[0].questions[0], tests: ['a', 'b'] }] })).line).toBe('2 tests did not run');
    expect(shortLine('one two three four five six seven eight nine ten eleven twelve thirteen')).toBe('one two three four five six seven eight nine ten eleven twelve…');
  });

  it('described-but-not-found and uncovered unasked-for statements join the list as plain lines; the rest is one click away', async () => {
    const st = (heading: string, text: string, cites: string[]) => ({ heading, text, cites });
    const rec = { recording: 'k', dropped: [], summaries: { matches: 'Crash fixed.', 'not-in-description': 'This is safe.' }, statements: [
      st('matches', 'Fixes it.', ['d3']),
      st('not-in-description', 'Covered by a decision.', ['d1']),
      st('not-in-description', 'README changed.', ['b5']),
      st('described-not-found', 'Description names the analyzer; no evidence of it.', ['b4']),
      ...[1, 2, 3, 4].map((i) => st('not-in-description', `Extra file ${i} changed.`, ['b6'])),
    ] };
    const { source } = gateway('2283', { 'ai-review': rec });
    await renderPage('2283', { sections: mods, source, asLoaded: true });
    expect(box().querySelector('[data-fact="asked"] [data-value]')!.textContent).toBe('1 of 2 claims have evidence');
    expect(box().querySelector('[data-heading="matches"] [data-summary]')!.textContent).toBe('Crash fixed');
    const lines = () => [...list().querySelectorAll(':scope > li .rv-qtext')].map((e) => e.textContent);
    expect(lines()).toHaveLength(4);
    expect(lines()[1]).toBe('Description names the analyzer; no evidence of it');
    expect(lines()).toContain('README changed');
    expect(lines()).not.toContain('Covered by a decision');
    expect(box().querySelector('[data-all]')!.textContent).toBe('All 8');
    fireEvent.click(box().querySelector('[data-all]')!);
    expect(lines()).toHaveLength(8);
    // a plain line leads to its evidence in one click
    fireEvent.click(list().querySelector<HTMLElement>('[data-item="described-not-found"] [data-evidence]')!);
    await waitFor(() => expect(slot('map').querySelector('[data-node="b4"]')!.getAttribute('data-focus')).toBe('true'));
  });

  it('full text: every statement under its heading with citations, Read 3 differences, Did not read code, dropped struck', async () => {
    const rec = readRecord('2283', 'ai-review') as { statements: object[] };
    const patched = { ...rec, dropped: [{ text: 'Matches the issue reproduction step 4.', cites: ['d9'] }], statements: [...rec.statements,
      { heading: 'matches', text: 'Also touches difference 4.', cites: ['d4'] },
      { heading: 'matches', text: 'The fix is safe', cites: ['d1'] }] };
    const { source } = gateway('2283', { 'ai-review': patched });
    await renderPage('2283', { sections: mods, source, asLoaded: true });
    expect(box().textContent).not.toContain('The fix is safe');
    openFull();
    expect([...full().querySelectorAll('h3')].map((h) => h.textContent)).toEqual([headings.matches, headings['not-in-description'], headings['left-to-you']]);
    expect(full().textContent).toContain('Diagnostics located outside the compilation are dropped; replay sends 42 and receives 0.');
    expect([...full().querySelectorAll('[data-chip]')].map((c) => c.getAttribute('data-chip'))).toEqual(expect.arrayContaining(['d1', 'd2', 'b1', 'b2', 'preset:d1', 'q1']));
    const prov = full().querySelector('.rv-prov')!;
    expect(prov.textContent).toContain('Read 3 differences');
    expect(prov.textContent).toContain('Did not read code');
    expect(prov.querySelector('[data-dropped]')!.textContent).toBe('3 statements dropped');
    expect([...full().querySelectorAll('.rv-dropped s')].map((s) => s.textContent)).toEqual(['Matches the issue reproduction step 4.', 'Also touches difference 4.', 'The fix is safe']);
    expect(full().querySelector('.rv-dropped [data-failed]')!.textContent).toBe('d9');
    // a citation in the full text closes the sheet and lands on its target
    fireEvent.click(full().querySelector<HTMLElement>('[data-chip="d2"]')!);
    await waitFor(() => expect(document.activeElement).toBe(slot('proof').querySelector('[data-row="d2"]')));
    expect(document.querySelector('[data-testid="review-full"]')).toBeNull();
  });

  it('every citation spelling resolves (crates/model); an unresolved one is dropped with the cite that failed', async () => {
    const { source } = gateway('2283');
    const data = await loadBundle(source, source.key);
    for (const c of ['d1', 'b1', 'q1', 'p1', 'preset:d1', 'GenericsFormatterTests.ClosedGenericStruct_FromMetadataReference', 'declared', 'TypeCollector.cs:239', 'reproduction'])
      expect(resolve(c, data), c).not.toBeNull();
    for (const c of ['d4', 'b99', 'Nope.cs:1', 'preset:d9']) expect(resolve(c, data), c).toBeNull();
    const rec = data.records['ai-review'] as { statements: object[] };
    const own = { ...data, records: { ...data.records, 'ai-review': { ...rec, statements: [...rec.statements, { heading: 'matches', text: 'Cites nothing here.', cites: ['d1', 'd9'] }] } } };
    expect(readReview(own).dropped).toEqual([{ text: 'Cites nothing here.', cites: ['d1', 'd9'], failed: 'd9' }]);
  });

  it('no recording: "no AI review", the decisions and the evidence still render (2234)', async () => {
    const { source } = gateway('2234', { 'ai-review': { recording: null, statements: [], dropped: [] } });
    await renderPage('2234', { sections: mods, source });
    expect(box().textContent).toContain('no AI review');
    expect(slot('review').querySelector('[data-fulltext]')).toBeNull();
    expect(list().querySelectorAll('[data-question]').length).toBeGreaterThan(0);
    expect(box().querySelector('[data-fact="asked"] [data-value]')!.textContent).toBe('no AI summary');
    expect(slot('review').querySelector('.slot-title [data-status]')!.getAttribute('data-status')).not.toBe('green');
    expect(slot('proof').querySelectorAll('[data-row]').length).toBeGreaterThan(0);
    expect(slot('signoff').querySelector('[data-submit]')).toBeTruthy();
  });

  it('evidence sections start as doors; Evidence on a decision opens its difference and lands on it', async () => {
    const { source } = gateway('2283');
    await renderPage('2283', { sections: mods, source, asLoaded: true });
    for (const s of ['proof', 'playground', 'map']) {
      expect(slot(s).hasAttribute('data-folded'), s).toBe(true);
      expect(slot(s).querySelector('.slot-body'), s).toBeNull();
      expect(slot(s).parentElement!.hasAttribute('data-doors'), s).toBe(true);
    }
    expect(document.querySelectorAll('section#slot-decide')).toHaveLength(0);
    fireEvent.click(list().querySelector<HTMLElement>('[data-question="q2"] [data-evidence]')!);
    await waitFor(() => expect(document.activeElement).toBe(slot('proof').querySelector('[data-row="d2"]')));
    expect(slot('proof').querySelector('[data-row="d2"]')!.getAttribute('aria-selected')).toBe('true');
    expect(slot('map').hasAttribute('data-folded')).toBe(true);
    await waitFor(() => expect(slot('proof').querySelector('[data-row="d2"]')!.hasAttribute('data-ai-cited')).toBe(true));
  });

  it('a question citation shows the whole list and lands on its decision', async () => {
    const { source } = gateway('2283');
    await renderPage('2283', { sections: mods, source, asLoaded: true });
    openFull();
    fireEvent.click(full().querySelector<HTMLElement>('[data-chip="q2"]')!);
    await waitFor(() => expect(document.activeElement).toBe(list().querySelector('[data-question="q2"]')));
  });

  it('the claim: a description that only points at the issue gives way to the requirement title; it opens the description', async () => {
    const { source } = gateway('2283');
    await renderPage('2283', { sections: mods, source, asLoaded: true });
    const claim = slot('review').querySelector<HTMLElement>('[data-claim]')!;
    const title = (readRecord('2283', 'bundle') as { requirement: { title: string } }).requirement.title;
    expect(claim.querySelector('.rv-claimtext')!.textContent).toBe(title);
    fireEvent.click(claim);
    expect(screen.getByRole('dialog', { name: 'Description' }).textContent).toContain('implementation for #2283');
  });
});

describe('decisions (D40)', () => {
  it('answer Return posts to the gateway; Sign off stops asking for that item', async () => {
    const { source, calls } = gateway('2283');
    await renderPage('2283', { sections: mods, source, asLoaded: true });
    const row = list().querySelector<HTMLElement>('[data-question="q1"]')!;
    await waitFor(() => expect(within(row).getByRole('button', { name: 'Return' }).hasAttribute('disabled')).toBe(false));
    expect(slot('signoff').querySelector('[data-gate="question-unanswered"]')!.textContent).toBe('Answer 2 items');
    await act(async () => { fireEvent.click(within(row).getByRole('button', { name: 'Return' })); });
    expect(calls).toContainEqual({ method: 'POST', path: `/api/sessions/${source.key}/reviewer-a`, body: { answer: { question: 'q1', value: 'return' } } });
    await waitFor(() => expect(slot('signoff').querySelector('[data-submit]')!.textContent).toBe('Request changes'));
    expect(slot('signoff').querySelector('[data-blocking]')).toBeNull();
  });

  it('questions about one behaviour are one decision: one answer goes to each, members on demand', async () => {
    const questions = { questions: [
      { id: 'q1', rule: 'undeclared-difference', block: 'b2', differences: ['d2'], tests: [] },
      { id: 'q2', rule: 'undeclared-difference', block: 'b3', differences: ['d3'], tests: [] },
      { id: 'q3', rule: 'declared-severe-difference', block: 'b1', differences: ['d1'], tests: [] },
    ] };
    const { source, calls, sess } = gateway('2283', { questions });
    const { data } = await renderPage('2283', { sections: mods, source });
    expect(questionGroups(data).map((g) => [g.label, g.questions.map((q) => q.id).join(' ')]))
      .toEqual([['1 data loss', 'q3'], ['2 crashes fixed', 'q1 q2']]);
    const rows = [...list().querySelectorAll<HTMLElement>('[data-question]')];
    expect(rows).toHaveLength(2);
    const group = rows[1];
    expect(group.getAttribute('data-questions')).toBe('q1 q2');
    expect(group.querySelector('.rv-qtext')!.textContent).toBe('IndexOutOfRangeException no longer thrown (2 places)');
    expect(group.querySelector('.rv-rule')!.textContent).toBe('not asked for');
    expect(rows[0].querySelector('.rv-rule')).toBeNull();
    await waitFor(() => expect(within(group).getByRole('button', { name: 'Accept' }).hasAttribute('disabled')).toBe(false));
    await act(async () => { fireEvent.click(within(group).getByRole('button', { name: 'Accept' })); });
    await waitFor(() => expect(sess.answers).toEqual({ q1: 'accept', q2: 'accept' }));
    expect(calls.filter((c) => (c.body as { answer?: unknown } | undefined)?.answer)).toHaveLength(2);
    await waitFor(() => expect(within(group).getByRole('button', { name: 'Accept' }).getAttribute('aria-pressed')).toBe('true'));
    await waitFor(() => expect(slot('signoff').querySelector('[data-gate="question-unanswered"]')!.textContent).toBe('Answer 1 item'));
    expect(list().querySelectorAll('[data-member]')).toHaveLength(0);
    fireEvent.click(group.querySelector('.rv-qhead')!);
    const members = list().querySelectorAll<HTMLElement>('[data-member]');
    expect([...members].map((m) => m.getAttribute('data-member'))).toEqual(['q1', 'q2']);
    expect(members[0].querySelector('.rv-qtext')!.textContent).toBe('Accept undeclared difference 2?');
    await act(async () => { fireEvent.click(within(members[1]).getByRole('button', { name: 'Return' })); });
    await waitFor(() => expect(sess.answers.q2).toBe('return'));
  });

  it('without a reviewer name an answer leads to the name field', async () => {
    localStorage.clear();
    const { source, calls } = gateway('2283');
    await renderPage('2283', { sections: mods, source });
    fireEvent.click(within(list().querySelector<HTMLElement>('[data-question="q1"]')!).getByRole('button', { name: 'Accept' }));
    await waitFor(() => expect(document.activeElement).toBe(slot('signoff').querySelector('[data-reviewer]')));
    expect(calls.filter((c) => c.method === 'POST')).toHaveLength(0);
  });

  it('nothing to ask: no list, 0 to decide; a rule may be a name or {rule, fact}', async () => {
    const levels = readRecord('2283', 'levels') as { levels: { block: string; rules: unknown[] }[] };
    const patched = { ...levels, levels: levels.levels.map((l, i) => (i ? l : { ...l, rules: ['architectural', { rule: 'architectural', fact: 'changed-wiring' }, 'reach'] })) };
    const rec = readRecord('2283', 'ai-review') as { statements: { heading: string }[] };
    const { source } = gateway('2283', { questions: { questions: [] }, levels: patched, 'ai-review': { ...rec, statements: rec.statements.filter((s) => s.heading === 'matches') } });
    const { data } = await renderPage('2283', { sections: mods, source });
    expect(document.getElementById('slot-decide')).toBeNull();
    expect(box().querySelector('[data-fact="unasked"] [data-value]')!.textContent).not.toMatch(/change/);
    expect(statusOf(data).reasons).not.toContain('0 to decide');
    expect(data.levels[0].rules).toEqual(['architectural:changed-wiring', 'reach']);
    expect(ruleLabel('architectural:changed-wiring')).toBe('Changed wiring');
  });
});

describe('Sign off (D41, D47, D48)', () => {
  it('what blocks is shown as plain lines that say what to do, and only while it blocks', async () => {
    const g = gateway('2283');
    await renderPage('2283', { sections: mods, source: g.source, asLoaded: true });
    const so = slot('signoff');
    await waitFor(() => expect(g.sess.spotCheck).toBeTruthy());
    const lines = () => [...so.querySelectorAll('[data-blocking] > li')].map((l) => l.textContent);
    await waitFor(() => expect(lines()).toHaveLength(3));
    expect(lines()[0]).toBe('Answer 2 items');
    expect(lines()[1]).toMatch(/^Open one randomly chosen file: [\w.]+$/);
    expect(lines()[2]).toMatch(/^Open 2 changed blocks first: /);
    expect(so.textContent).not.toMatch(/Spot-check|Level 3|Full review|\d+ open/);
    expect(so.querySelector('[data-mode="full"]')).toBeTruthy();
    // the spot-check line opens the file and goes away; the same block is kept on a later load
    const first = so.querySelector('[data-spotcheck]')!.getAttribute('data-spotcheck')!;
    expect(['b4', 'b5', 'b6']).toContain(first);
    await act(async () => { fireEvent.click(so.querySelector<HTMLElement>('[data-spotcheck] button')!); });
    await waitFor(() => expect(g.sess.opened).toContain(first));
    await waitFor(() => expect(so.querySelector('[data-spotcheck]')).toBeNull());
    expect(g.calls.filter((c) => (c.body as { spotCheck?: string } | undefined)?.spotCheck)).toHaveLength(1);
    // the blocks line opens the next changed block on the map
    await act(async () => { fireEvent.click(so.querySelector<HTMLElement>('[data-gate="level-3-unopened"] button')!); });
    await waitFor(() => expect(lines().some((l) => /^Open 1 changed block first/.test(l!))).toBe(true));
  });

  it('nothing blocks: only the fields and the button', async () => {
    const { source } = gateway('2283', {}, { answers: { q1: 'accept', q2: 'accept' }, spotCheck: 'b5', opened: ['b1', 'b2', 'b3', 'b5'] });
    await renderPage('2283', { sections: mods, source, asLoaded: true });
    await waitFor(() => expect(slot('signoff').querySelector('[data-submit]')!.hasAttribute('disabled')).toBe(false));
    expect(slot('signoff').querySelector('[data-blocking]')).toBeNull();
  });

  it('approve is disabled while a section runs, and says which', async () => {
    const b = readRecord('2283', 'bundle') as { sections: Record<string, object> };
    const { source } = gateway('2283', { bundle: { ...b, sections: { ...b.sections, tests: { state: 'running' } } } });
    await renderPage('2283', { sections: mods, source });
    await waitFor(() => expect(slot('signoff').querySelector('[data-submit]')!.hasAttribute('disabled')).toBe(true));
    expect(slot('signoff').querySelector('[data-gate="section-running"]')!.textContent).toBe('Still running: tests');
  });

  it('the token is asked on first use; the gateway refusal is plain lines; then the body the gateway composed', async () => {
    const { source, calls, reviews } = gateway('2283', {}, { spotCheck: 'b5' });
    reviews.push(
      { status: 409, data: { error: 'unmet', gate: 'question-unanswered', items: ['q1', 'q2', 'b5', 'b1'] } },
      { status: 200, data: { review: { event: 'APPROVE', body: 'Requirement: #2283\nRisk: none seen.', result: { status: 200, url: null } } } },
    );
    await renderPage('2283', { sections: mods, source });
    const so = slot('signoff');
    await waitFor(() => expect(so.querySelector('[data-submit]')!.hasAttribute('disabled')).toBe(false));
    fireEvent.change(so.querySelector('textarea')!, { target: { value: 'none seen.' } });
    fireEvent.change(so.querySelector('input[type="password"]')!, { target: { value: 'TOK' } });
    await act(async () => { fireEvent.click(so.querySelector('[data-submit]')!); });
    const sub = calls.find((c) => c.path.endsWith('/review'))!;
    expect(sub.path).toBe(`/api/sessions/${source.key}/reviewer-a/review`);
    expect(sub.headers?.authorization).toBe('Bearer TOK');
    expect(sub.body).toEqual({ risk: 'none seen.' });
    const out = await waitFor(() => so.querySelector('[data-outcome="refused"]')!);
    expect([...out.querySelectorAll('[data-gate]')].map((x) => x.getAttribute('data-gate'))).toEqual(['question-unanswered', 'spot-check', 'level-3-unopened']);
    expect(out.textContent).toContain('ReportIfLocatedInCompilation');
    expect(out.textContent).not.toMatch(/\bb1\b|\bq1\b/);
    await act(async () => { fireEvent.click(so.querySelector('[data-submit]')!); });
    await waitFor(() => expect(so.querySelector('.rv-body')!.textContent).toBe('Requirement: #2283\nRisk: none seen.'));
    // after a submitted review the token field is not shown again in this browser session
    await waitFor(() => expect(so.querySelector('input[type="password"]')).toBeNull());
    sessionStorage.clear();
  });

  it('a block opened and parts shown are posted to the session, also while Sign off is folded', async () => {
    const { source, calls, sess } = gateway('2283');
    await renderPage('2283', { sections: mods, source, asLoaded: true });
    await waitFor(() => expect(sess.spotCheck).toBeTruthy());
    fireEvent.click(slot('signoff').querySelector('.slot-toggle')!);
    expect(slot('signoff').querySelector('.slot-body')).toBeNull();
    openFull();
    fireEvent.click(full().querySelector<HTMLElement>('[data-chip="b1"]')!);
    await waitFor(() => expect(sess.opened).toContain('b1'));
    expect(calls).toContainEqual(expect.objectContaining({ method: 'POST', body: { opened: ['b1'] } }));
    fireEvent.click(list().querySelector<HTMLElement>('.loc')!);
    await waitFor(() => expect(sess.shown.length).toBeGreaterThan(0));
  });

  it('the spot-check line shows before a reviewer is named', async () => {
    localStorage.clear();
    const { source } = gateway('2283');
    await renderPage('2283', { sections: mods, source });
    expect(slot('signoff').querySelector('[data-spotcheck]')).toBeTruthy();
  });
});

describe('layout and wording (D2, D9, D14)', () => {
  it('css keeps every row to one line inside a 390 px slot', () => {
    for (const sel of ['.rv-box', '.rv-text', '.rv-st']) expect(reviewCss).toMatch(new RegExp(`${sel.replace('.', '\\.')}[^{]*\\{[^}]*min-width: 0`));
    expect(reviewCss).toMatch(/\.rv-text \{[^}]*text-overflow: ellipsis/);
    expect(reviewCss).toContain('overflow-wrap: anywhere');
  });

  it('labels are terse and avoid banned words', () => {
    for (const l of allLabels()) {
      if (!Object.values(headings).includes(l as never)) expect(l.split(/\s+/).length, l).toBeLessThanOrEqual(3);
      for (const w of [...reviewBanned, 'reproduced']) expect(l.toLowerCase(), l).not.toContain(w);
    }
  });
});

