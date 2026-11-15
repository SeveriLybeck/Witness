import { describe, expect, it } from 'vitest';
import { act, fireEvent, screen, within } from '@testing-library/react';
import { renderPage } from './harness';
import { DiffNumber, LevelGlyph, useShell, type SectionModule, type SectionProps } from '../src/shell';
import { collect } from '../src/shell/registry';

function ProofStub({ data }: SectionProps) {
  const { selection } = useShell();
  return <ul>{data.differences.map((d) => (
    <li key={d.id} data-row={d.id} aria-selected={selection.difference === d.id}><DiffNumber id={d.id} /></li>
  ))}</ul>;
}
function MapStub({ data }: SectionProps) {
  const { selection, highlight } = useShell();
  return <div>{data.blocks.map((b) => (
    <div key={b.id} data-node={b.id} data-focus={selection.block === b.id || undefined} data-lit={highlight.blocks.has(b.id) || undefined}>
      <LevelGlyph level={data.levels.find((l) => l.block === b.id)?.level ?? 1} />
      {data.differences.filter((d) => d.block === b.id).map((d) => <DiffNumber key={d.id} id={d.id} />)}
    </div>
  ))}</div>;
}
const stubs: Partial<Record<SectionModule['slot'], SectionModule>> = {
  proof: { slot: 'proof', Component: ProofStub, headline: (d) => `${d.differences.length} differences` },
  map: { slot: 'map', Component: MapStub },
};

describe('page order (D7)', () => {
  it('2283: header, AI review, 1-3, Decide, Sign off', async () => {
    const { container } = await renderPage('2283', { sections: {} });
    const header = container.querySelector('header')!;
    const sections = [...container.querySelectorAll('section')];
    expect(header.compareDocumentPosition(sections[0]) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(sections.map((s) => s.querySelector('h2 .slot-toggle')!.textContent)).toEqual([
      'Review', 'Behaviour changes', 'Try it', 'Code map', 'Sign off',
    ]);
    expect(screen.getAllByRole('region').map((r) => r.getAttribute('aria-labelledby'))).toHaveLength(5);
  });

  it('requirement ID opens the requirement text with only a close control', async () => {
    await renderPage('2283', { sections: {} });
    fireEvent.click(screen.getByRole('button', { name: '2283' }));
    const sheet = screen.getByRole('dialog', { name: '2283' });
    expect(sheet.textContent).toContain('closed generic value type defined in a referenced assembly');
    expect(within(sheet).getAllByRole('button').map((b) => b.textContent)).toEqual(['Close']);
    fireEvent.click(within(sheet).getByRole('button'));
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('headline from the section module', async () => {
    await renderPage('2283', { sections: stubs });
    expect(document.querySelector('#slot-proof .slot-headline')!.textContent).toBe('3 differences');
  });

  it('registry refuses one slot twice', () => {
    const s: SectionModule = { slot: 'map', Component: () => null };
    expect(() => collect({ a: { sections: [s] }, b: { sections: [s] } })).toThrow(/twice/);
  });
});

describe('one object, three views (D8)', () => {
  it('2283: selecting difference 2 on its map node selects proof row 2 and focuses its block', async () => {
    const { container } = await renderPage('2283', { sections: stubs });
    const node = container.querySelector('[data-node="b2"]')!;
    const chip = within(node as HTMLElement).getByRole('button', { name: '2' });
    act(() => chip.click());
    expect(container.querySelector('[data-row="d2"]')!.getAttribute('aria-selected')).toBe('true');
    expect(container.querySelector('[data-row="d1"]')!.getAttribute('aria-selected')).toBe('false');
    expect(container.querySelector('[data-focus]')!.getAttribute('data-node')).toBe('b2');
    const twos = [...container.querySelectorAll('[data-cite="diff:2"]')];
    expect(twos.length).toBe(2);
    for (const t of twos) expect(t.textContent).toBe('2');
  });

  it('level glyph bars equal the level', async () => {
    const { container } = await renderPage('2283', { sections: stubs });
    const glyph = container.querySelector('[data-node="b1"] .level')!;
    expect(glyph.querySelectorAll('.bar.on').length).toBe(3);
    expect(container.querySelector('[data-node="b4"] .level')!.querySelectorAll('.bar.on').length).toBe(1);
  });
});
