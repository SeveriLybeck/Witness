// Length budgets of the page as it loads, on copies of the four real bundles: the review is a short check,
// everything else is one headline line until opened. Pixel heights are measured in a browser (web/STATUS.md).
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { changes, realData, realGateway, realSource, renderPage, type Change } from './harness';
import { App } from '../src/shell/App';
import { howItems, howLead, howStatus } from '../src/shell/howText';
import { banned } from '../src/shell/labels';
import { LINE_WORDS, questionGroups, wordCount } from '../src/sections/review/cites';
import { resetSessions } from '../src/sections/review/session';

const slot = (s: string) => document.getElementById(`slot-${s}`)!;
const words = (el: Element | null) => wordCount(el?.textContent?.replace(/([a-z0-9])([A-Z+−=?])/g, '$1 $2') ?? '');

beforeEach(() => { resetSessions(); localStorage.clear(); });
afterEach(() => { cleanup(); window.location.hash = ''; });

/** Per change, as loaded: the status and the two facts, the list (at most four lines), and the evidence doors. */
const first: Record<Change, { status: string; facts: [string, string]; list: string[]; total: number; doors: string[] }> = {
  '2283': {
    status: 'Needs human review', facts: ['2 of 2 claims have evidence', '2 changes, 1 loses data: 42 → 0'],
    list: ['A value of 42 comes back as 0', 'Analyzer diagnostic MsgPack016 no longer shown (reportDiagnostic not called)', 'Analyzer diagnostic MsgPack005 no longer shown (reportDiagnostic not called)', 'IndexOutOfRangeException, InvalidCastException no longer thrown (2 places)'],
    total: 4,
    doors: ['Behaviour changes7 · 28 of 29 lines compared', 'Tests3', 'Try it4 of 7 replayed · reported bug replayed', 'Code map6 blocks · 1 outside'],
  },
  '2234': {
    status: 'Possible problems', facts: ['1 of 1 claim has evidence', '2 changes'],
    list: ['Members with missing or duplicate keys are now silently dropped', '4 new tests already pass on the old code', '2 tests did not run'],
    total: 3,
    doors: ['Behaviour changes12 · 22 of 22 lines compared', 'Tests10', 'Try it3 of 12 replayed · reported bug not replayed', 'Code map6 blocks'],
  },
  '2297': {
    status: 'Needs human review', facts: ['1 of 2 claims have evidence', '1 change'],
    list: ['Only one formatter snapshot shows the attribute; other generated types not evidenced', '', 'Read CompositeResolverTemplate.tt:7'],
    total: 3,
    doors: ['Behaviour changes1 · 0 of 29 lines compared · 1 not re-checked', 'Tests2', 'Try it0 of 1 replayed · reported bug not replayed', 'Code map4 blocks · 1 outside · 402 identical edits +5'],
  },
  '2129': {
    status: 'Needs human review', facts: ['2 of 2 claims have evidence', '4 changes, 1 new exception'],
    list: ['Can now throw ArgumentOutOfRangeException', '4 new analyzer diagnostics: MsgPack019, MsgPack020, MsgPack021 +1', '1 new public type: CodeAnalysis.GeneratedFormatterRequest', '1 new public member: ObjectSerializationInfo.IsUserDeclaredFormatter'],
    total: 15,
    doors: ['Behaviour changes11 · 83 of 450 lines compared · 7 not re-checked', 'Tests13', 'Try it0 of 11 replayed · reported bug not replayed', 'Code map43 blocks · 26 outside · 2 identical edits'],
  },
};

describe('the first screen, on the four real bundles', () => {
  for (const c of changes) {
    it(`${c}: a status, two facts, two groups, a row of doors`, async () => {
      const { data } = await renderPage(c, { source: realSource(c), asLoaded: true });
      const box = slot('review').querySelector('[data-review-box]')!;
      const f = first[c];
      expect(slot('review').querySelector('.slot-title .st-label')!.textContent).toBe(f.status);
      expect([...box.querySelectorAll('[data-fact] [data-value]')].map((e) => e.textContent)).toEqual(f.facts);
      expect(box.querySelector('legend')!.textContent).toBe('AI summary');
      expect(document.querySelector('[data-doors-label]')!.textContent).toBe('Evidencecomputed, no AI');
      const lines = [...box.querySelectorAll('#slot-decide > li .rv-qtext')].map((e) => e.textContent!);
      expect(lines.map((l, i) => (f.list[i] === '' ? '' : l))).toEqual(f.list);
      expect(lines.length).toBeLessThanOrEqual(4);
      // every question is in the list (all of it one click away), and nothing is shown twice
      expect(questionGroups(data).length + lines.length - box.querySelectorAll('[data-question]').length).toBeLessThanOrEqual(f.total + 0);
      const all = box.querySelector<HTMLElement>('[data-all]');
      expect(all ? all.textContent : null).toBe(f.total > 4 ? `All ${f.total}` : null);
      expect(document.querySelectorAll('[data-question]')).toHaveLength(box.querySelectorAll('[data-question]').length);
      // the first layer: no heading glyphs, counters or bare numbers; lines of at most twelve words where the page writes them
      expect(box.querySelectorAll('.rv-glyph, [data-chip], .diffno, [data-more-cites]')).toHaveLength(0);
      for (const l of [...box.querySelectorAll('[data-statement] .rv-qtext, [data-summary]')].map((e) => e.textContent!)) expect(wordCount(l), l).toBeLessThanOrEqual(LINE_WORDS);
      expect(box.querySelectorAll('h3, li').length).toBeLessThanOrEqual(6);
      // the evidence is a row of doors, each with its values; nothing else is open but Sign off
      const doors = [...document.querySelectorAll('[data-doors] > :not([data-doors-label])')].map((d) => d.textContent);
      expect(doors).toEqual(f.doors);
      for (const s of ['proof', 'playground', 'map']) expect(slot(s).querySelector('.slot-body'), s).toBeNull();
      expect(document.querySelectorAll('main pre')).toHaveLength(0);
      expect(document.body.textContent).not.toMatch(/\/home\/|   at [A-Z]/);
      expect(words(document.querySelector('main'))).toBeLessThanOrEqual(200);
      expect(document.querySelector('main')!.textContent).not.toMatch(/Logical proof|Level 3|Spot-check|Full review|Playground/);
      for (const w of banned) expect(document.body.textContent!.toLowerCase(), w).not.toContain(w);
    });
  }

  it('2283: the lost value leads the facts and the list, without the restated outcome; the header carries the mark, not the title again', async () => {
    await renderPage('2283', { source: realSource('2283'), asLoaded: true });
    const box = slot('review').querySelector('[data-review-box]')!;
    expect(box.querySelector('.rv-claimtext')!.textContent).toBe('Generator and analyzer crash with IndexOutOfRangeException on a closed generic struct from a referenced assembly');
    expect(document.querySelector('.pagehead .req-title')).toBeNull();
    const home = document.querySelector('.pagehead a.home')!;
    expect(home.getAttribute('aria-label')).toBe('Witness');
    expect(home.querySelector('img')!.getAttribute('src')).toBe('/witness-icon-flat-indigo.svg');
    const loss = box.querySelector('[data-question="q5"]')!;
    expect(loss).toBe(box.querySelector('[data-question]'));
    expect(box.querySelector('.rv-pattern')).toBeNull();
    expect(box.textContent).not.toContain('nothing reported');
    expect(loss.querySelector('.loc')).toBeTruthy();
    expect(loss.querySelector('[data-evidence]')!.textContent).toBe('Evidence');
    expect(box.querySelector('[data-question="q4"] .rv-rule')!.textContent).toBe('not asked for');
  });

  it('2129: fifteen decisions for twenty-seven questions, four shown and all of them in place on demand', async () => {
    const data = await realData('2129');
    const groups = questionGroups(data);
    expect(groups.reduce((n, g) => n + g.questions.length, 0)).toBe(27);
    await renderPage('2129', { source: realSource('2129'), asLoaded: true });
    // the pipeline's 18 filler rows for questions the model did not mention are not lines on the page
    fireEvent.click(slot('review').querySelector('[data-all]')!);
    expect(document.querySelectorAll('#slot-decide > [data-question]')).toHaveLength(15);
    expect(document.getElementById('slot-decide')!.textContent).not.toMatch(/level-3-block|undeclared-difference/);
  });

  it('the Tests door opens the behaviour changes with the tests list shown', async () => {
    await renderPage('2234', { source: realSource('2234'), asLoaded: true });
    fireEvent.click(document.querySelector('[data-door="tests"]')!);
    await waitFor(() => expect(slot('proof').querySelectorAll('.proof-test')).toHaveLength(10));
  });
});

describe('start page and explanation', () => {
  it('lists the four changes with claim, unasked-for, not found, decisions, lines compared and status', async () => {
    render(<App source={realGateway()} />);
    await waitFor(() => expect(document.querySelectorAll('.start-table [data-extra]:not(:empty)')).toHaveLength(4));
    const rows = [...document.querySelectorAll('.start-table tbody tr')].map((r) => [...r.querySelectorAll('td')].map((d) => d.textContent!.trim()));
    expect(rows.map((r) => r[0])).toEqual(['2283', '2234', '2297', '2129']);
    expect(rows[0][1]).toBe('Generator and analyzer crash with IndexOutOfRangeException on a closed generic struct from a referenced assembly');
    expect(rows[1][1]).toBe('Implementation for #2234 The analyzer does not work if SuppressSourceGeneration = true');
    expect(rows.map((r) => r.slice(2))).toEqual([
      ['2', '0', '4', '28 / 29', 'Needs human review'],
      ['2', '0', '2', '22 / 22', 'Possible problems'],
      ['1', '1', '2', '0 / 29', 'Needs human review'],
      ['4', '0', '15', '83 / 450', 'Needs human review'],
    ]);
    expect([...document.querySelectorAll('.start-table [data-status]')].map((e) => e.getAttribute('data-status'))).toEqual(['red', 'orange', 'red', 'red']);
    expect(document.querySelectorAll('.start-table .st-dot')).toHaveLength(4);
    expect(document.querySelector('.start-table thead')!.textContent).not.toContain('Mode');
    expect(document.querySelector('.start-table a')!.getAttribute('href')).toMatch(/^#\/[0-9a-f]{40}-[0-9a-f]{40}$/);
  });

  it('How it works opens from the start page: six short items, each with what it does not show, no tool names up front', async () => {
    render(<App source={realGateway()} />);
    fireEvent.click(await screen.findByRole('button', { name: 'How it works' }));
    const how = screen.getByTestId('how');
    expect(how.querySelectorAll('section')).toHaveLength(6);
    expect(how.querySelectorAll('section .how-k')).toHaveLength(6);
    const first = [howLead, howStatus.text, ...howItems.flatMap((i) => [i.title, i.shows, i.limit])].join(' ');
    expect(wordCount(first)).toBeLessThanOrEqual(260);
    expect(first).not.toMatch(/Lean|SMT|solver|Z3/);
    for (const i of howItems) {
      expect(wordCount(i.shows), i.id).toBeLessThanOrEqual(45);
      expect(wordCount(i.limit), i.id).toBeLessThanOrEqual(22);
    }
    for (const w of banned) expect(how.textContent!.toLowerCase(), w).not.toContain(w);
    expect(first).toContain('for all inputs');
    expect(first).toContain('reads none');
    expect(first).toContain('approves nothing');
  });

  it('How it works opens from the header and from an open section heading, at that section', async () => {
    await renderPage('2283', { source: realSource('2283'), asLoaded: true });
    expect(slot('proof').querySelector('[data-how-open]')).toBeNull();
    fireEvent.click(document.querySelector('.pagehead [data-how-open]')!);
    expect(screen.getByTestId('how').querySelector('[aria-current="true"]')).toBeNull();
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });
    fireEvent.click(slot('proof').querySelector('.slot-toggle')!);
    fireEvent.click(slot('proof').querySelector('[data-how-open="compare"]')!);
    expect(screen.getByTestId('how').querySelector('[aria-current="true"]')!.getAttribute('data-how')).toBe('compare');
  });
});
