import { describe, expect, it } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { ResultView } from './Results';
import { shownResults, splitReplay, stripPaths } from './protocol';

describe('playground helpers', () => {
  it('unmarked replay text goes to the app', () => {
    expect(splitReplay('class A {}')).toEqual({ library: '', app: 'class A {}' });
    expect(splitReplay('// lib\nL\n// app\nA')).toEqual({ library: 'L', app: 'A' });
  });
  it('no absolute host path in any text the playground shows', () => {
    const data = { files: [{ file: 'src/MessagePack.SourceGenerator/CodeAnalysis/TypeCollector.cs' }] } as never;
    const msg = "Generator 'MessagePackGenerator' failed: IndexOutOfRangeException at TypeCollector.GetObjectInfo() in /home/u/work/sb/wt/abc/src/MessagePack.SourceGenerator/CodeAnalysis/TypeCollector.cs:line 985";
    const results = shownResults([
      { output: 'diagnostics', side: 'old', type: 'diagnostics', value: [{ id: 'CS8785', severity: 'warning', message: msg }] },
      { output: 'text', side: 'old', type: 'text-diff', value: { old: 'at /tmp/build-7/obj/Gen.g.cs', new: 'see /nix/store/abc-dotnet/sdk/x.targets' } },
      { output: 'facts', side: 'new', type: 'facts', value: { path: '/home/u/x/y/z.cs', n: 1 } },
      { output: 'page', side: 'new', type: 'page', value: { url: 'a/b/index.html' } },
    ], data);
    const text = JSON.stringify(results);
    expect(text).not.toMatch(/\/home\/|\/tmp\/|\/nix\/store\//);
    expect(text).toContain('src/MessagePack.SourceGenerator/CodeAnalysis/TypeCollector.cs:line 985');
    expect(stripPaths({ m: 'in /home/a/b/c/d.cs' }, data)).toEqual({ m: 'in …/c/d.cs' });
    const { container } = render(<ResultView type="diagnostics" value={results[0].value} />);
    expect(container.textContent).not.toContain('/home/');
  });
  it('null facts render empty', () => {
    const { container } = render(<ResultView type="facts" value={{ error: null, sent: '42' }} />);
    expect(container.querySelectorAll('dd')[0].textContent).toBe('');
    expect(container.textContent).not.toContain('null');
  });
});
import { fixtureSource, renderInShell, readRecord } from '../../../test/harness';
import { DiffNumber, banned } from '../../shell';
import { Playground } from './Playground';
import { pl } from './labels';
import { resultTypes, type Reply, type Subject } from './protocol';

import describeJson from '../../../../contracts/messages/examples/plugin-reply.describe.json';
import runJson from '../../../../contracts/messages/examples/plugin-reply.run.json';
const describeReply = describeJson as unknown as Reply;
const runReply = runJson as unknown as Reply;

// A subject with a simple signature (D35): int and string get fields, a type without a public constructor does not.
const sig: Subject = {
  id: 'sig', name: 'Shift(int, string, Options)',
  definition: { text: 'static string Shift(int n, string s, Options o)', locations: [{ file: 'src/A.cs', line: 3 }] },
  inputs: [
    { name: 'n', role: 'n', type: 'int', constructible: true, field: 'int' },
    { name: 's', role: 's', type: 'string', constructible: true, field: 'string' },
    { name: 'o', role: 'o', type: 'Options', constructible: false, field: null }],
  outputs: [{ name: 'value', type: 'facts' }], presets: [],
};

const lab = { name: 'generator-lab', title: 'Generator lab', outputs: ['diagnostics', 'text-diff', 'facts'] };

function setup(subjects: Subject[] = describeReply.subjects!, list: { viewers: typeof lab[]; tabs: boolean } = { viewers: [lab], tabs: false },
  change: '2283' | '2234' = '2283', run: Reply = runReply) {
  const calls: { body: { plugin: string; request: Record<string, unknown> } }[] = [];
  const source = fixtureSource(change, {
    onRequest: (m, path, body) => {
      if (path.endsWith('/viewers')) { expect(m).toBe('GET'); return list; }
      expect(path).toMatch(/^\/api\/bundles\/[0-9a-f]{40}-[0-9a-f]{40}\/playground$/);
      const b = body as { plugin: string; request: Record<string, unknown> };
      calls.push({ body: b });
      return b.request.op === 'describe' ? { ...describeReply, subjects } : run;
    },
  });
  return { source, calls };
}

describe('playground', () => {
  it('copy deck has no banned words', () => {
    const text = Object.values(pl).join(' ').toLowerCase();
    for (const w of banned) expect(text).not.toContain(w);
  });

  it('draws controls and results only from declarations (D35, D36)', async () => {
    const { source, calls } = setup();
    await renderInShell('2283', () => <Playground />, source);
    await screen.findByRole('combobox', { name: pl.subject });
    expect(calls[0].body).toMatchObject({ plugin: 'generator-lab', request: { op: 'describe' } });
    expect(screen.getByText(describeReply.subjects![0].definition.text)).toBeTruthy();
    expect(document.querySelector('[data-loc$="TypeCollector.cs:985"]')).toBeTruthy();
    expect(screen.getByLabelText(/library code/)).toBeTruthy();
    expect(screen.getByLabelText(/app code/)).toBeTruthy();
    expect(document.querySelector('[data-slot="options"][data-constructible="false"]')?.textContent).toContain(pl.notConstructible);
    expect(screen.queryByRole('tablist')).toBeNull();
    for (const s of ['old', 'new']) {
      const outs = [...document.querySelectorAll(`[data-side="${s}"] [data-output]`)].map((e) => e.getAttribute('data-type'));
      expect(outs).toEqual(['diagnostics', 'text-diff', 'facts']);
      for (const t of outs) expect(resultTypes).toContain(t);
    }
    fireEvent.click(screen.getByRole('button', { name: pl.run }));
    await waitFor(() => expect(document.querySelector('[data-side="old"] [data-output="diagnostics"]')?.textContent).toContain('MsgPack004'));
    expect(calls[1].body.request).toMatchObject({ op: 'run', subject: 'consumer', sides: ['old', 'new'] });
    expect(document.querySelector('[data-side="new"] [data-output="roundTrip"]')?.textContent).toContain('0');
  });

  it('one preset per replayed difference plus issue, revert, free; selecting difference 2 loads its input (D8, D34)', async () => {
    const { source, calls } = setup();
    await renderInShell('2283', () => <><DiffNumber id="d2" /><Playground /></>, source);
    await screen.findByRole('group', { name: pl.presets });
    const replays = (readRecord('2283', 'replays') as { replays: { input: unknown }[] }).replays.filter((r) => r.input);
    const diffs = document.querySelectorAll('[data-kind="difference"]');
    expect([...diffs].map((e) => e.firstChild?.textContent)).toEqual(['1', '2', '3'].slice(0, replays.length));
    expect(document.querySelector('[data-preset="reproduction"][data-cite~="reproduction"]')).toBeTruthy();
    expect(document.querySelector('[data-preset="d2"][data-cite~="preset:d2"]')).toBeTruthy();
    expect(document.querySelector('[data-kind][data-cite^="diff:"]')).toBeNull();
    expect(document.querySelector('[data-preset="revert"]')).toBeTruthy();
    expect(document.querySelector('[data-preset="free"]')).toBeTruthy();

    fireEvent.click(document.querySelector('button.diffno')!);
    await waitFor(() => expect((screen.getByLabelText(/library code/) as HTMLTextAreaElement).value).toContain('[Key(null)]'));
    expect((screen.getByLabelText(/app code/) as HTMLTextAreaElement).value).toContain('Item<int> Member');
    expect(document.querySelector('[data-kind="difference"][aria-pressed="true"]')?.getAttribute('data-preset')).toBe('d2');
    fireEvent.click(screen.getByRole('button', { name: pl.run }));
    await waitFor(() => expect(calls.length).toBe(2));
    expect(calls[1].body.request.preset).toEqual({ kind: 'difference', difference: 'd2' });
  });

  it('revert preset sends its block', async () => {
    const { source, calls } = setup();
    await renderInShell('2283', () => <Playground />, source);
    fireEvent.click(await screen.findByRole('button', { name: 'revert b1' }));
    fireEvent.click(screen.getByRole('button', { name: pl.run }));
    await waitFor(() => expect(calls.length).toBe(2));
    expect(calls[1].body.request.preset).toEqual({ kind: 'revert', block: 'b1' });
  });

  it('generates a form from a simple signature and marks the untyped parameter (D35)', async () => {
    const { source } = setup([sig]);
    await renderInShell('2283', () => <Playground />, source);
    await screen.findByText(sig.definition.text);
    expect(document.querySelector('[data-slot="n"] input[type="number"]')).toBeTruthy();
    expect(document.querySelector('[data-slot="s"] input[type="text"]')).toBeTruthy();
    const o = document.querySelector('[data-slot="o"]')!;
    expect(o.getAttribute('data-constructible')).toBe('false');
    expect(o.querySelector('input, textarea, select')).toBeNull();
  });

  it('tabs only when the gateway says so; empty viewer list is unavailable (D38)', async () => {
    const two = { viewers: [lab, { ...lab, name: 'other', title: 'Other' }], tabs: true };
    const a = setup(undefined, two);
    const r = await renderInShell('2283', () => <Playground />, a.source);
    expect((await screen.findAllByRole('tab')).map((t) => t.textContent)).toEqual(['Generator lab', 'Other']);
    r.unmount();
    const b = setup(undefined, { viewers: [], tabs: false });
    await renderInShell('2283', () => <Playground />, b.source);
    await waitFor(() => expect(document.querySelector('[data-state="unavailable"]')?.textContent).toBe(pl.unavailable));
    expect(screen.queryByRole('tab')).toBeNull();
    expect(b.calls.length).toBe(0);
  });

  it('2234 presets carry source and buildProperties; unmarked text is the app', async () => {
    const withProps = { ...describeReply.subjects![0], inputs: [...describeReply.subjects![0].inputs,
      { name: 'buildProperties', role: 'build properties', type: 'properties', constructible: true, field: 'text' as const }] };
    const { source, calls } = setup([withProps], undefined, '2234');
    await renderInShell('2234', () => <Playground />, source);
    const first = (await screen.findAllByRole('button', { name: /^1/ })).find((e) => e.getAttribute('data-kind') === 'difference')!;
    expect(first.getAttribute('data-source')).toBe('model');
    fireEvent.click(first);
    expect((screen.getByLabelText(/build properties/) as HTMLTextAreaElement).value).toBe('NoWarn=MsgPack005');
    expect((screen.getByLabelText(/library code/) as HTMLTextAreaElement).value).toBe('');
    fireEvent.click(screen.getByRole('button', { name: pl.run }));
    await waitFor(() => expect(calls.length).toBe(2));
    expect(calls[1].body.request.inputs).toMatchObject({ buildProperties: { NoWarn: 'MsgPack005' } });
    expect(String((calls[1].body.request.inputs as Record<string, unknown>).app)).toContain('[Key(0)]');
  });
});

describe('result sources', () => {
  it('pages go through the pages route; absolute URLs are not framed; data: images allowed', () => {
    const k = 'a-b';
    const { container: c1 } = render(<ResultView type="page" value={{ url: 'site/index.html' }} bundle={k} plugin="web" />);
    expect(c1.querySelector('iframe')?.getAttribute('src')).toBe('/api/bundles/a-b/pages/web/site/index.html');
    expect(c1.querySelector('iframe')?.getAttribute('sandbox')).toBe('allow-scripts');
    const { container: c2 } = render(<ResultView type="page" value={{ url: 'http://127.0.0.1:9999/' }} bundle={k} plugin="web" />);
    expect(c2.querySelector('iframe')).toBeNull();
    const png = 'data:image/png;base64,iVBORw0KGgo=';
    const { container: c3 } = render(<ResultView type="image-pair" value={{ old: png, new: 'shots/new.png' }} bundle={k} plugin="web" />);
    expect([...c3.querySelectorAll('img')].map((i) => i.getAttribute('src'))).toEqual([png, '/api/bundles/a-b/pages/web/shots/new.png']);
  });
});
