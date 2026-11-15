import { describe, expect, it } from 'vitest';
import { fireEvent, screen } from '@testing-library/react';
import { fixtureData, renderInShell, renderPage, type Change } from '../../../test/harness';
import { banned } from '../../shell';
import { MIN_K, MapSection, openHeight, openWidth } from './Map';
import { sections } from './index';
import { mapLabels, ruleLabels } from './labels';
import { NODE_H, NODE_W, columnsFor, focusBox, layout, neighboursOf, overlaps, scrollFor } from './layout';

const changes: Change[] = ['2283', '2234', '2297', '2129'];
const ready = { state: 'ready' as const };

describe('map (D29)', () => {
  it('registers the map slot', () => {
    expect(sections.map((s) => s.slot)).toEqual(['map']);
  });

  it('uniform nodes with name and difference numbers', async () => {
    const { container } = await renderInShell('2283', (d) => <MapSection data={d} state={ready} />);
    const nodes = container.querySelectorAll<HTMLElement>('.mnode');
    expect(nodes.length).toBe(6);
    expect(new Set([...nodes].map((n) => `${n.style.width}x${n.style.height}`)).size).toBe(1);
    const b2 = container.querySelector('[data-block="b2"]')!;
    expect(b2.textContent).toContain('GetPropertyIdentifierLocation');
    expect(b2.querySelector('[data-cite="diff:2"]')).toBeTruthy();
    expect(container.querySelector('[data-node="b2"]')).toBe(b2);
  });

  it('ctrl+wheel zooms (never below 1), plain wheel is left to the page, drag pans', async () => {
    await renderInShell('2283', (d) => <MapSection data={d} state={ready} />);
    const view = screen.getByTestId('map-view');
    const k0 = Number(screen.getByTestId('map-layer').dataset.k);
    expect(k0).toBe(MIN_K);
    const plain = new WheelEvent('wheel', { deltaY: -100, bubbles: true, cancelable: true });
    view.dispatchEvent(plain);
    expect(plain.defaultPrevented).toBe(false);
    const out = new WheelEvent('wheel', { deltaY: 400, ctrlKey: true, bubbles: true, cancelable: true });
    fireEvent(view, out);
    expect(out.defaultPrevented).toBe(true);
    expect(Number(screen.getByTestId('map-layer').dataset.k)).toBe(MIN_K);
    fireEvent(view, new WheelEvent('wheel', { deltaY: -100, ctrlKey: true, bubbles: true, cancelable: true }));
    expect(Number(screen.getByTestId('map-layer').dataset.k)).toBeGreaterThan(k0);
    view.scrollLeft = 100;
    fireEvent.pointerDown(view, { pointerId: 1, clientX: 60, clientY: 10 });
    fireEvent.pointerMove(view, { pointerId: 1, clientX: 10, clientY: 10 });
    fireEvent.pointerUp(view, { pointerId: 1 });
    expect(view.scrollLeft).toBe(150);
  });

  it('long changes lay out as a grid of columns that fits the width, not one column', async () => {
    const { data } = await fixtureData('2129');
    const l = layout(data, { vw: 1280 });
    expect(l.columns.length).toBe(columnsFor(1280));
    expect(l.width).toBeLessThanOrEqual(1280);
  });

  it('closed nodes hold name, numbers and file row; not linked sits off the name row', async () => {
    const { data } = await fixtureData('2283');
    const links = { links: [{ block: 'b2', state: 'not-linked', tests: [], placement: 'outside' }] };
    const d2 = { ...data, records: { ...data.records, links } };
    const { container } = await renderInShell('2283', () => <MapSection data={d2} state={ready} />);
    const b2 = container.querySelector('[data-node="b2"]')!;
    expect(b2.querySelector('.mnode-head')!.textContent).toBe('GetPropertyIdentifierLocation');
    expect(b2.querySelector('.mnode-meta')!.textContent).toContain(mapLabels.notLinked);
    expect(b2.querySelector('.mnode-meta [data-cite="diff:2"]')).toBeTruthy();
    expect(NODE_H).toBeGreaterThanOrEqual(8 + 44 + 8 + 24 + 8 + 20 + 8 + 2);
  });
});

describe('opening a block (D14, D29)', () => {
  it('opens in place into parts with locations; outside click and Esc close it', async () => {
    const { container } = await renderInShell('2283', (d) => <MapSection data={d} state={ready} />);
    fireEvent.click(container.querySelector('[data-block="b1"] .mnode-head') as HTMLElement);
    const b1 = container.querySelector('[data-block="b1"]') as HTMLElement;
    expect(b1.querySelectorAll('[data-part]').length).toBe(2);
    expect(b1.querySelector('[data-loc$=":239"]')).toBeTruthy();
    expect(b1.textContent).toContain('severe difference');
    expect(parseFloat(b1.style.width)).toBeGreaterThan(NODE_W);
    const view = screen.getByTestId('map-view');
    fireEvent.pointerDown(view, { pointerId: 2, clientX: 5, clientY: 5 });
    fireEvent.pointerUp(view, { pointerId: 2, clientX: 5, clientY: 5 });
    expect(container.querySelectorAll('[data-part]').length).toBe(0);
    fireEvent.click(container.querySelector('[data-block="b2"] .mnode-head') as HTMLElement);
    fireEvent.keyDown(view, { key: 'Escape' });
    expect(container.querySelectorAll('[data-part]').length).toBe(0);
  });

  it('no opened block overlaps another node, at any measured size, on every fixture and width', async () => {
    for (const c of changes) {
      const { data } = await fixtureData(c);
      for (const vw of [390, 1280]) {
        for (const b of data.blocks) {
          for (const h of [openHeight(b.parts.length, 2), 600]) {
            const l = layout(data, { vw, heights: { [b.id]: h }, widths: { [b.id]: openWidth(vw) } });
            const ids = Object.keys(l.nodes);
            for (const a of ids) for (const z of ids) if (a < z) expect(overlaps(l.nodes[a], l.nodes[z]), `${c}@${vw} ${b.id}: ${a}/${z}`).toBe(false);
            // the opened block fits the view width, and the camera scroll shows all of it
            expect(l.nodes[b.id].w).toBeLessThanOrEqual(vw - 32);
            const x = scrollFor(focusBox(l, b.id, neighboursOf(data, b.id)), l.nodes[b.id], vw);
            expect(l.nodes[b.id].x - x).toBeGreaterThanOrEqual(-0.5);
            expect(l.nodes[b.id].x + l.nodes[b.id].w - x).toBeLessThanOrEqual(vw - 32 + 0.5);
          }
        }
      }
    }
  });

  it('copy: call site singular, rule kind suffix as words', () => {
    expect(mapLabels.sites(1)).toBe('1 call site');
    expect(mapLabels.sites(28)).toBe('28 call sites');
    expect(ruleLabels(['architectural:changed-wiring', 'severe-difference'])).toEqual(['changed wiring', 'severe difference']);
  });
});

describe('dependencies and groups (D30, D31)', () => {
  it('2283: changed edges in both versions from the filter block', async () => {
    const { data } = await fixtureData('2283');
    const { container } = await renderInShell('2283', () => <MapSection data={data} state={ready} />);
    for (const to of ['b2', 'b3']) {
      const e = container.querySelector(`[data-edge="b1-${to}"]`)!;
      expect(e.getAttribute('data-changed')).toBe('true');
      expect(e.getAttribute('data-version')).toBe('both');
    }
    const deps = (data.records.dependencies as { dependencies: { to: string; sites: { line: number }[] }[] }).dependencies;
    expect(deps.find((d) => d.to === 'b3')!.sites[0].line).toBe(1080);
  });

  it('call sites are one group node expanding to one location per site', async () => {
    const { data } = await fixtureData('2283');
    const sites = Array.from({ length: 28 }, (_, i) => ({ file: 'src/A.cs', line: 100 + i, endLine: 100 + i }));
    const deps = { ...(data.records.dependencies as object), reach: [{ block: 'b1', callSites: 28, sites }] };
    const d2 = { ...data, records: { ...data.records, dependencies: deps } };
    const { container } = await renderInShell('2283', () => <MapSection data={d2} state={ready} />);
    const chip = screen.getByRole('button', { name: mapLabels.sites(28) });
    expect(container.querySelectorAll('[data-sites]').length).toBe(1);
    fireEvent.click(chip);
    expect(container.querySelectorAll('[data-sites-list] [data-loc]').length).toBe(28);
  });

  it('2283: b1 shows 28 call sites, b2 and b3 no chip', async () => {
    const { container } = await renderInShell('2283', (d) => <MapSection data={d} state={ready} />);
    expect(container.querySelector('[data-sites="b1"]')!.textContent).toContain('28 call sites');
    expect(container.querySelector('[data-sites="b2"]')).toBeNull();
    expect(container.querySelector('[data-sites="b3"]')).toBeNull();
  });

  it('no reach, no call-site chip', async () => {
    const { data } = await fixtureData('2283');
    const { reach: _reach, ...deps } = data.records.dependencies as Record<string, unknown>;
    const d2 = { ...data, records: { ...data.records, dependencies: deps } };
    const { container } = await renderInShell('2283', () => <MapSection data={d2} state={ready} />);
    expect(container.querySelectorAll('[data-sites]').length).toBe(0);
  });

  it("2297's snapshot repeats show as one entry", async () => {
    const { container } = await renderInShell('2297', (d) => <MapSection data={d} state={ready} />);
    const groups = container.querySelectorAll('[data-group]');
    expect(groups.length).toBe(1);
    expect(groups[0].textContent).toBe('402 identical edits');
    expect(container.querySelectorAll('.mnode').length).toBe(12);
  });
});

describe('boundary and levels (D12, D32, D33)', () => {
  it("2283's filter block is inside Declared, b2 outside", async () => {
    const { container } = await renderInShell('2283', (d) => <MapSection data={d} state={ready} />);
    const region = screen.getByTestId('map-region');
    const box = { x: +region.getAttribute('x')!, y: +region.getAttribute('y')!, w: +region.getAttribute('width')!, h: +region.getAttribute('height')! };
    const at = (id: string) => {
      const s = (container.querySelector(`[data-block="${id}"]`) as HTMLElement).style;
      return { x: parseFloat(s.left), y: parseFloat(s.top), w: parseFloat(s.width), h: parseFloat(s.height) };
    };
    const inside = (r: ReturnType<typeof at>) => r.x >= box.x && r.y >= box.y && r.x + r.w <= box.x + box.w && r.y + r.h <= box.y + box.h;
    expect(inside(at('b1'))).toBe(true);
    expect(inside(at('b3'))).toBe(true);
    expect(overlaps(at('b2'), box)).toBe(false);
    expect(container.textContent).toContain(mapLabels.declared);
  });

  it('boundary reads running while the test contract runs', async () => {
    const { data } = await fixtureData('2283');
    const d2 = { ...data, bundle: { ...data.bundle, sections: { ...data.bundle.sections, tests: { state: 'running' as const } } } };
    const { container } = await renderInShell('2283', () => <MapSection data={d2} state={ready} />);
    expect(container.querySelector('.map-region-label')!.textContent).toBe(mapLabels.running);
  });

  it('not-linked blocks are labelled', async () => {
    const { data } = await fixtureData('2283');
    const links = { links: [{ block: 'b2', state: 'not-linked', tests: [], placement: 'outside' }] };
    const d2 = { ...data, records: { ...data.records, links } };
    const { container } = await renderInShell('2283', () => <MapSection data={d2} state={ready} />);
    expect(container.querySelector('[data-block="b2"]')!.textContent).toContain(mapLabels.notLinked);
    expect(screen.queryByTestId('map-region')).toBeNull();
  });

  it('level glyph without hue; css uses tokens only', async () => {
    const { container } = await renderInShell('2283', (d) => <MapSection data={d} state={ready} />);
    expect(container.querySelector('[data-block="b1"] [data-level="3"]')).toBeTruthy();
    const css = (await import('./map.css?raw')).default as string;
    expect(css).not.toMatch(/#[0-9a-f]{3,6}\b|rgb|hsl/i);
  });

  it('page renders with the map and no banned words', async () => {
    const { container } = await renderPage('2283', { sections: { map: sections[0] } });
    const text = container.textContent!.toLowerCase();
    for (const w of banned) expect(text).not.toContain(w);
    expect(text).toContain('6 blocks · 1 outside');
  });
});
