import { describe, expect, it } from 'vitest';
import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { fixtureSource, renderPage } from './harness';
import { Loc, useShell, type SectionModule } from '../src/shell';
import { alignLines, commonIndent, enclosing, hunkWindow, mapLine, rowOf, splitRows, unifiedRows } from '../src/shell/code';

const tc = 'src/MessagePack.SourceGenerator/CodeAnalysis/TypeCollector.cs';

function Locs() {
  return <div><Loc file={tc} line={1077} /><Loc file={tc} line={239} /></div>;
}
function MapStub() {
  const { data, highlight } = useShell();
  return <div>{data.blocks.map((b) => <span key={b.id} data-node={b.id} data-lit={highlight.blocks.has(b.id) || undefined} />)}</div>;
}
const sections: Partial<Record<SectionModule['slot'], SectionModule>> = {
  proof: { slot: 'proof', Component: Locs },
  map: { slot: 'map', Component: MapStub },
};

const panel = () => screen.getByRole('complementary', { name: 'Code' });
const target = () => panel().querySelector('.row.target') as HTMLElement;

describe('code panel (D13)', () => {
  it('opens TypeCollector.cs:1077 as one unified diff: removed and added lines together, both line numbers, the hunk in view', async () => {
    await renderPage('2283', { sections });
    const loc = screen.getByRole('button', { name: `${tc}:1077` });
    expect(loc.textContent).toMatch(/^[^/]+\.[A-Za-z]+:\d+$/);
    act(() => loc.click());
    await waitFor(() => expect(target()).toBeTruthy());
    expect(target().dataset.new).toBe('1077');
    expect(target().dataset.changed).toBe('true');
    expect(target().classList.contains('add')).toBe(true);
    expect(target().textContent).toContain('MsgPack00xMessagePackAnalyzer.MemberNeedsKey');
    const crumbs = [...panel().querySelectorAll('.crumb')].map((c) => c.textContent);
    expect(crumbs.slice(0, 3)).toEqual(['src/MessagePack.SourceGenerator/CodeAnalysis/', 'TypeCollector.cs', 'TypeCollector']);
    expect(crumbs[crumbs.length - 1]).toBe('GetObjectInfo');
    // no switching between old and new: one view holds both
    expect(within(panel()).queryByRole('button', { name: 'Old' })).toBeNull();
    const rows = () => [...panel().querySelectorAll<HTMLElement>('.row')];
    expect(rows().some((r) => r.classList.contains('del'))).toBe(true);
    expect(rows().some((r) => r.classList.contains('add'))).toBe(true);
    for (const r of rows()) expect(r.querySelectorAll('.ln')).toHaveLength(2);
    const ctx = rows().find((r) => !r.dataset.changed)!;
    expect(ctx.dataset.old && ctx.dataset.new).toBeTruthy();
    // a removed line has only its old number, an added one only its new number
    const del = rows().find((r) => r.classList.contains('del'))!;
    expect([del.dataset.old != null, del.dataset.new == null]).toEqual([true, true]);
    // a few lines of context around the hunk, the rest of the file one expand away
    expect(rows()[0].dataset.changed).toBeUndefined();
    expect(rows()[rows().length - 1].dataset.changed).toBeUndefined();
    const n = rows().length;
    expect(n).toBeLessThan(60);
    fireEvent.click(within(panel()).getByRole('button', { name: 'Expand up' }));
    expect(rows().length).toBe(n + 20);

    // side by side on a wide page: old left, new right, the target on its side
    fireEvent.click(within(panel()).getByRole('button', { name: 'Split' }));
    expect(panel().dataset.view).toBe('split');
    const sides = panel().querySelectorAll('.code-side');
    expect([...sides].map((s) => s.getAttribute('data-side'))).toEqual(['old', 'new']);
    expect(sides[0].querySelectorAll('.row').length).toBe(sides[1].querySelectorAll('.row').length);
    expect(sides[1].querySelector('.row.target')!.getAttribute('data-new')).toBe('1077');
    fireEvent.click(within(panel()).getByRole('button', { name: 'Unified' }));
    expect(panel().dataset.view).toBe('unified');
  });

  it('stays open and docks beside the page while another location is opened', async () => {
    const { container } = await renderPage('2283', { sections });
    act(() => screen.getByRole('button', { name: `${tc}:1077` }).click());
    await waitFor(() => expect(target()).toBeTruthy());
    expect(container.querySelector('.layout')!.classList.contains('docked')).toBe(true);
    expect(panel().dataset.layout).toBe('dock');
    act(() => screen.getByRole('button', { name: `${tc}:239` }).click());
    await waitFor(() => expect(target().dataset.line).toBe('239'));
    expect(screen.getByRole('button', { name: `${tc}:1077` })).toBeTruthy();
    const crumbs = [...panel().querySelectorAll('.crumb')].map((c) => c.textContent);
    expect(crumbs).toContain('TypeCollector');
  });

  it('files index opens the first change and highlights that file’s blocks', async () => {
    const source = fixtureSource('2283', { folderCounts: { 'src/MessagePack.SourceGenerator/CodeAnalysis/': 2 } });
    const { container } = await renderPage('2283', { sections, source });
    fireEvent.click(screen.getByRole('button', { name: '4 files' }));
    const index = screen.getByRole('dialog', { name: 'Files' });
    expect(index.querySelector('[data-folder-count]')!.textContent).toBe('2');
    expect(within(index).getByRole('button', { name: tc }).textContent).toBe('TypeCollector.cs+26 −3');
    fireEvent.click(within(index).getByRole('button', { name: tc }));
    await waitFor(() => expect(target().dataset.line).toBe('239'));
    const lit = [...container.querySelectorAll('[data-lit]')].map((n) => n.getAttribute('data-node'));
    expect(lit).toEqual(['b1', 'b2', 'b3']);
    expect(screen.getByRole('button', { name: `${tc} ×` })).toBeTruthy();
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.queryByRole('complementary', { name: 'Code' })).toBeNull();
  });
});

describe('code helpers', () => {
  it('maps lines through the diff', () => {
    const m = alignLines(5, 6, new Set([2]), new Set([2, 3]));
    expect(m.oldToNew.slice(1)).toEqual([1, null, 4, 5, 6]);
    expect(mapLine(m.newToOld, 3)).toEqual({ line: 2, exact: false });
    expect(mapLine(m.newToOld, 4)).toEqual({ line: 3, exact: true });
  });

  it('builds one unified diff, its first window and the side-by-side pairs', () => {
    const old = ['a', 'b', 'c', 'd', 'e'];
    const nu = ['a', 'B1', 'B2', 'c', 'd', 'e'];
    const rows = unifiedRows(old, nu, new Set([2]), new Set([2, 3]));
    expect(rows.map((r) => `${r.kind}:${r.old ?? ''}:${r.new ?? ''}:${r.text}`)).toEqual([
      'ctx:1:1:a', 'del:2::b', 'add::2:B1', 'add::3:B2', 'ctx:3:4:c', 'ctx:4:5:d', 'ctx:5:6:e',
    ]);
    expect(rowOf(rows, 'new', 3)).toBe(3);
    expect(rowOf(rows, 'old', 2)).toBe(1);
    expect(hunkWindow(rows, 3, 1)).toEqual({ from: 0, to: 4 });
    expect(hunkWindow(rows, 6, 1)).toEqual({ from: 5, to: 6 });
    expect(splitRows(rows).map((p) => [p.left?.text ?? null, p.right?.text ?? null])).toEqual([
      ['a', 'a'], ['b', 'B1'], [null, 'B2'], ['c', 'c'], ['d', 'd'], ['e', 'e'],
    ]);
    expect(commonIndent(['        if (x)', '', '            y();'])).toBe(8);
    // a new file: every line added
    expect(unifiedRows([], ['x', 'y'], new Set(), new Set([1, 2])).map((r) => r.kind)).toEqual(['add', 'add']);
  });

  it('finds the enclosing class and method', () => {
    const src = ['namespace N', '{', '  public class A', '  {', '    private int F(int x)', '    {', '      return x;', '    }', '  }', '}'];
    expect(enclosing(src, 7)).toEqual({ types: ['A'], member: 'F' });
    expect(enclosing(src, 8)).toEqual({ types: ['A'], member: null });
  });
});
