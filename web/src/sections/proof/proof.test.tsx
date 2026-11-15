import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fireEvent, within } from '@testing-library/react';
import { renderInShell, fixtureData, fixtureSource, readRecord, realData, realSource, type Change } from '../../../test/harness';
import { banned, groupDifferences, repoPaths, type BundleData } from '../../shell';
import { Proof, headline, sections } from './index';
import { allProofLabels } from './labels';
import { leanFacts, notLeanCount } from './model';

const renderProof = (change: Change) =>
  renderInShell(change, (data) => <Proof data={data} state={{ state: 'ready' }} />);
const renderReal = (change: Change) =>
  renderInShell(change, (data) => <Proof data={data} state={{ state: 'ready' }} />, realSource(change));

describe('proof section', () => {
  it('registers the proof slot', () => {
    expect(sections.map((s) => s.slot)).toEqual(['proof']);
  });

  it('2283 scope line shows 29 changed lines', async () => {
    const r = await renderProof('2283');
    expect(r.getByTestId('proof-scope').textContent).toContain('29 of 29 lines compared');
    expect(r.getByTestId('proof-scope').textContent).toContain('2 assumptions');
  });

  it('2283 headline: 3 differences and how much was compared; rows are behaviours on one line each', async () => {
    const { data } = await fixtureData('2283');
    expect(headline(data)).toBe('3 · 29 of 29 lines compared');
    const r = await renderProof('2283');
    const rows = within(r.getByTestId('proof-differences')).getAllByRole('row');
    expect(rows).toHaveLength(2);
    expect(rows[0].textContent).toContain('data loss');
    expect(rows[0].textContent).toContain('reports MsgPack004 → nothing reported');
    expect(rows[0].textContent).toContain('TypeCollector.cs:239');
    expect(rows[0].textContent).toContain('1 test');
    expect(rows[1].getAttribute('data-group')).toBe('g-d2');
    expect(rows[1].textContent).toContain('2 crashes fixed');
    expect(rows[1].textContent).toContain('throws IndexOutOfRangeException → nothing reported');
    // members on demand, one row each with its own fields
    fireEvent.click(rows[1].querySelector('.proof-toggle')!);
    const members = r.container.querySelectorAll('tr.proof-member');
    expect([...members].map((m) => m.getAttribute('data-row'))).toEqual(['d2', 'd3']);
    expect(members[0].textContent).toContain('no test');
    expect(members[0].textContent).toContain('TypeCollector.cs:985');
    // the rest of a difference is one click further
    fireEvent.click(rows[0].querySelector('[data-detail]')!);
    const detail = r.getByTestId('proof-detail');
    expect(detail.textContent).toContain('ClosedGenericStruct_FromMetadataReference_UnkeyedMember');
    expect(detail.textContent).toContain('re-checked by a second checker');
  });

  it('a difference the second checker did not confirm is marked on its row and counted in the headline', async () => {
    const lean = readRecord('2283', 'lean') as { rows: { row: string; state: string }[] };
    const changed = { ...lean, rows: lean.rows.map((r) => (r.row === 'r2' ? { ...r, state: 'not-checked' } : r)) };
    const src = fixtureSource('2283');
    const base = src.record.bind(src);
    src.record = async (k, n) => (n === 'lean' ? changed : base(k, n));
    const r = await renderInShell('2283', (data: BundleData) => {
      expect(notLeanCount(data)).toBe(1);
      expect(headline(data)).toBe('3 · 29 of 29 lines compared · 1 not re-checked');
      return <Proof data={data} state={{ state: 'ready' }} />;
    }, src);
    const group = r.container.querySelector('tr[data-group="g-d2"]')!;
    expect(group.querySelector('.proof-nums')!.getAttribute('title')).toContain('1 not re-checked');
    fireEvent.click(group.querySelector('.proof-toggle')!);
    expect(r.container.querySelector('[data-row="d2"] [data-check]')!.textContent).toBe('not re-checked');
    const text = r.container.textContent!.toLowerCase();
    for (const w of [...banned, 'lean', 'solver']) expect(text).not.toContain(w);
  });

  it('selecting a difference opens its group and selects its row', async () => {
    const r = await renderProof('2283');
    const group = r.container.querySelector('tr[data-group="g-d2"]')!;
    expect(r.container.querySelector('[data-row="d3"]')).toBeNull();
    fireEvent.click(group.querySelector('[data-cite="diff:3"]')!);
    const row = r.container.querySelector('[data-row="d3"]')!;
    expect(row.getAttribute('aria-selected')).toBe('true');
    expect(group.getAttribute('aria-selected')).toBe('true');
    fireEvent.click(r.container.querySelector('[data-row="d1"] td:nth-child(2)')!);
    expect(r.container.querySelector('[data-row="d1"]')!.getAttribute('aria-selected')).toBe('true');
    expect(document.querySelectorAll('#witness-proof-css')).toHaveLength(1);
  });

  it('2234 tests: a line of counts; the list has one line per test; output verbatim one click further', async () => {
    const r = await renderProof('2234');
    const list = r.getByTestId('proof-tests');
    expect(list.textContent).toContain('4 declared · 4 hanging · 2 not run');
    expect(list.querySelectorAll('.proof-test')).toHaveLength(0);
    fireEvent.click(list.querySelector('.proof-tests-line')!);
    expect(list.querySelectorAll('[data-category="declared"] .proof-test')).toHaveLength(4);
    expect(list.querySelectorAll('[data-category="hanging"] .proof-test')).toHaveLength(4);
    expect(list.querySelectorAll('[data-category="not-run"] .proof-test')).toHaveLength(2);
    expect(list.querySelector('[data-category="declared"] .proof-test')!.textContent).toContain('fail → pass');
    expect(list.querySelectorAll('pre')).toHaveLength(0);
    const tests = (readRecord('2234', 'tests') as { tests: { category: string; failureMessage: string | null }[] }).tests;
    const msg = tests.find((t) => t.category === 'declared' && t.failureMessage)!.failureMessage!;
    fireEvent.click(list.querySelector('[data-category="declared"] [data-output]')!);
    const pres = [...r.getByTestId('proof-test-output').querySelectorAll('pre')].map((p) => p.textContent);
    expect(pres).toContain(msg.replace(/\r/g, ''));
  });

  it('table is fixed layout and every cell stays on one line', () => {
    const css = readFileSync(resolve(__dirname, 'proof.css'), 'utf8');
    expect(css).toMatch(/\.proof-table \{[^}]*table-layout: fixed/);
    expect(css).toMatch(/\.clip \{[^}]*text-overflow: ellipsis; white-space: nowrap/);
  });

  it('renders every fixture without banned words', async () => {
    for (const c of ['2283', '2234', '2297', '2129'] as const) {
      const r = await renderProof(c);
      const text = r.container.textContent!.toLowerCase();
      for (const w of banned) expect(text, `${c}: ${w}`).not.toContain(w);
      r.unmount();
    }
    for (const l of allProofLabels()) for (const w of [...banned, 'lean', 'solver']) expect(l.toLowerCase()).not.toContain(w);
  });
});

describe('proof on the four real bundles', () => {
  const labels = async (c: Change) => groupDifferences((await realData(c)).differences).map((g) => `${g.label} [${g.members.map((m) => m.number).join(' ')}]`);

  it('groups differences by behaviour', async () => {
    expect(await labels('2283')).toEqual([
      '2 diagnostics dropped [1 4]', '3 crashes fixed [2 3 5]', '1 crash fixed [6]', '1 data loss [7]',
    ]);
    expect(await labels('2234')).toEqual([
      '2 crashes fixed [1 2]', '2 output changes [3 4]', '1 crash fixed [5]', '2 output changes [6 8]', '1 crash fixed [7]', '4 crashes fixed [9 10 11 12]',
    ]);
    expect(await labels('2297')).toEqual(['1 output change [1]']);
    expect(await labels('2129')).toEqual([
      '2 new public types [1 7]', '4 new diagnostics [2 3 4 5]', '1 new exception [6]', '1 output change [8]',
      '1 new public member [9]', '1 output change [10]', '1 output change [11]',
    ]);
  });

  it('a behaviour seen with and without a reported diagnostic is one group', async () => {
    const g = groupDifferences((await realData('2234')).differences);
    expect(g[0].pattern).toBe('throws InvalidOperationException → skips the rest of the iteration');
    expect(g[5].pattern).toBe('throws ArgumentException → without intMembers.Add');
  });

  it('headlines say how much was compared and re-checked', async () => {
    expect(headline(await realData('2283'))).toBe('7 · 28 of 29 lines compared');
    expect(headline(await realData('2297'))).toBe('1 · 0 of 29 lines compared · 1 not re-checked');
    expect(headline(await realData('2129'))).toMatch(/^11 · 83 of 450 lines compared · \d+ not re-checked$/);
  });

  it('collapsed: one row per behaviour, a tests line, no output, no host paths', async () => {
    for (const [c, rows] of [['2283', 4], ['2234', 6], ['2297', 1], ['2129', 7]] as const) {
      const r = await renderReal(c);
      expect(r.container.querySelectorAll('[data-testid="proof-differences"] tr'), c).toHaveLength(rows);
      expect(r.container.querySelectorAll('pre'), c).toHaveLength(0);
      expect(r.container.querySelectorAll('.proof-test'), c).toHaveLength(0);
      expect(r.container.textContent, c).not.toMatch(/\/home\/|w_null|\(not /);
      expect(r.container.textContent!.split(/\s+/).length, c).toBeLessThan(140);
      r.unmount();
    }
  });

  it('2283: the data loss row leads with the value a replay lost', async () => {
    const r = await renderReal('2283');
    expect(r.container.querySelector('[data-row="d7"] .proof-change')!.textContent).toBe('value lost: 42 → 0 · reports MsgPack004 → nothing reported');
  });

  it('rows give the room to the outcome: no condition column, the file column is the one that gives way', () => {
    const css = readFileSync(resolve(__dirname, 'proof.css'), 'utf8');
    const w = (c: string) => Number(new RegExp(`col\\.${c} \\{ width: (\\d+)px`).exec(css)![1]);
    // at 1152 px of table the old → new cell keeps at least 480 px
    expect(1152 - w('c-lead') - w('c-loc') - w('c-test') - w('c-more')).toBeGreaterThanOrEqual(480);
    expect(css).not.toContain('c-when');
  });

  it('test output is shown with repository-relative paths', async () => {
    const r = await renderReal('2283');
    fireEvent.click(r.getByTestId('proof-tests').querySelector('.proof-tests-line')!);
    fireEvent.click(r.container.querySelector('[data-output]')!);
    const out = r.getByTestId('proof-test-output').textContent!;
    expect(out).toContain('System.InvalidOperationException');
    expect(out).toContain(' in src/MessagePack.SourceGenerator/CodeAnalysis/TypeCollector.cs:line');
    expect(out).not.toContain('/home/');
    expect(out).not.toContain('work/sandbox');
    const data = await realData('2283');
    expect(repoPaths('at X in /home/u/w/sandbox/tc/mix/a-b/src/MessagePack.SourceGenerator/CodeAnalysis/TypeCollector.cs:line 5', data))
      .toBe('at X in src/MessagePack.SourceGenerator/CodeAnalysis/TypeCollector.cs:line 5');
    expect(repoPaths('// /0/Test0.cs(10,5): error', data)).toBe('// /0/Test0.cs(10,5): error');
  });

  it('a producer group id on differences decides the grouping; without one the outcome text does', async () => {
    const data = await realData('2283');
    const own = data.differences.map((d) => ({ ...d, group: d.number <= 2 ? 'ga' : d.number === 7 ? null : 'gb' }));
    expect(groupDifferences(own).map((g) => g.members.map((m) => m.number).join(' '))).toEqual(['1 2', '3 4 5 6', '7']);
    expect(groupDifferences(data.differences.map((d) => ({ ...d, group: null }))).map((g) => g.members.length)).toEqual([2, 3, 1, 1]);
  });

  it('a failing test on one line: failureSummary when present, else the first line of the message', async () => {
    const src = realSource('2283');
    const base = src.record.bind(src);
    src.record = async (k, n) => {
      const v = await base(k, n);
      if (n !== 'tests') return v;
      const t = (v as { tests: object[] }).tests;
      return { ...(v as object), tests: [{ ...t[0], failureSummary: 'expected 0 diagnostics, got 2' }, ...t.slice(1)] };
    };
    const r = await renderInShell('2283', (data: BundleData) => <Proof data={data} state={{ state: 'ready' }} />, src);
    fireEvent.click(r.getByTestId('proof-tests').querySelector('.proof-tests-line')!);
    const rows = r.container.querySelectorAll('.proof-test');
    expect(rows[0].querySelector('.proof-result')!.getAttribute('title')).toBe('expected 0 diagnostics, got 2');
    expect(rows[1].querySelector('.proof-result')!.getAttribute('title')).toMatch(/^System\.InvalidOperationException : Mismatch/);
    fireEvent.click(rows[0].querySelector('[data-output]')!);
    expect(r.getByTestId('proof-test-summary').textContent).toBe('expected 0 diagnostics, got 2');
  });

  it('assumed facts come from lean.json coverage and are listed in the Checks sheet', async () => {
    const data = await realData('2234');
    const total = leanFacts(data).reduce((n, u) => n + u.facts.length, 0);
    const cov = (data.records.lean as { coverage: { facts?: unknown[] }[] }).coverage;
    expect(total).toBe(cov.reduce((n, c) => n + (c.facts?.length ?? 0), 0));
    expect(total).toBeGreaterThan(0);
    const r = await renderReal('2234');
    fireEvent.click(r.getByTestId('proof-scope').querySelector('[data-checks]')!);
    const shown = [...document.querySelectorAll('summary')].map((s) => Number(/(\d+) facts?/.exec(s.textContent!)![1]));
    expect(shown.reduce((a, b) => a + b, 0)).toBe(total);
    expect(r.getByTestId('proof-lean').querySelector('[data-cite="diff:1"]')).not.toBeNull();
    expect(r.getByTestId('proof-assumptions').textContent).toContain('nullable value null');
  });

  it('many ranges that were not compared are one value that opens the list', async () => {
    const r = await renderReal('2129');
    const b = r.getByTestId('proof-scope').querySelector('[data-ranges]')!;
    expect(b.textContent).toMatch(/^\d+ ranges not compared$/);
    expect(r.getByTestId('proof-scope').querySelectorAll('[data-loc]')).toHaveLength(0);
    fireEvent.click(b);
    expect(document.querySelectorAll('.proof-ranges [data-loc]').length).toBeGreaterThan(2);
  });
});
