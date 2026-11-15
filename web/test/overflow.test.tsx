// Horizontal overflow, as far as jsdom can tell (no layout engine: no box widths, no scrollWidth).
// Checked here: no inline or attribute width above the viewport, every <pre>/code view scrolls inside
// its own box, long unbreakable text is allowed to wrap or clip. A real browser is needed for
// `document.documentElement.scrollWidth <= innerWidth` itself (group 19, Playwright).
import { afterEach, describe, expect, it } from 'vitest';
import { act, fireEvent, screen, waitFor } from '@testing-library/react';
import { renderPage, type Change } from './harness';
import { installStyles } from '../src/shell/styles';
import { Prose } from '../src/shell/prose';
import { render } from '@testing-library/react';

function setWidth(w: number) { Object.defineProperty(window, 'innerWidth', { configurable: true, value: w }); }
afterEach(() => setWidth(1440));

function scrolls(el: Element | null): boolean {
  for (let e = el; e; e = e.parentElement) {
    const o = getComputedStyle(e).overflowX || getComputedStyle(e).overflow;
    if (['auto', 'scroll', 'hidden'].includes(o)) return true;
  }
  return false;
}

const changes: Change[] = ['2283', '2234', '2297', '2129'];

describe('no horizontal overflow (D14)', () => {
  for (const change of changes) for (const width of [390, 1280]) {
    it(`${change} at ${width} px, all sections`, async () => {
      setWidth(width);
      installStyles(document);
      const { container } = await renderPage(change);
      await act(async () => { await new Promise((r) => setTimeout(r, 20)); });
      for (const el of container.querySelectorAll<HTMLElement>('*')) {
        for (const prop of ['width', 'minWidth'] as const) {
          const m = /^(\d+(?:\.\d+)?)px$/.exec(el.style[prop]);
          // A wide layer is fine inside a box that clips or scrolls (e.g. the map's pan layer).
          if (m && Number(m[1]) > width) expect(scrolls(el.parentElement), `${el.tagName}.${el.className} ${prop} ${m[1]}px unclipped`).toBe(true);
        }
        const attr = el.getAttribute('width');
        if (attr && /^\d+$/.test(attr) && Number(attr) > width) expect(scrolls(el.parentElement), `${el.tagName} width=${attr} unclipped`).toBe(true);
      }
      for (const pre of container.querySelectorAll('pre')) expect(scrolls(pre), 'pre scrolls in its box').toBe(true);
    });
  }

  it('2283 code panel breadcrumb keeps class and method; crumbs wrap', async () => {
    setWidth(390);
    installStyles(document);
    await renderPage('2283', { sections: {} });
    fireEvent.click(screen.getByRole('button', { name: '4 files' }));
    fireEvent.click(screen.getByRole('button', { name: 'src/MessagePack.SourceGenerator/CodeAnalysis/TypeCollector.cs' }));
    await waitFor(() => expect(document.querySelector('.row.target')).toBeTruthy());
    const crumbs = [...document.querySelectorAll('.crumb')];
    for (const c of crumbs.filter((x) => !x.classList.contains('folder'))) {
      const s = getComputedStyle(c);
      expect(s.textOverflow).not.toBe('ellipsis');
      expect(s.flexShrink, c.textContent!).toBe('0');
    }
    expect(getComputedStyle(document.querySelector('.crumbs')!).flexWrap).toBe('wrap');
  });
});

describe('requirement prose', () => {
  it('renders Markdown as headings, lists and code text', () => {
    const { container } = render(<Prose text={'#### Bug description\n\nBoth `A` and **B** fail.\n\n- one\n- two\n\n```csharp\nvar x = 1;\n```'} />);
    expect(container.querySelector('h3')!.textContent).toBe('Bug description');
    expect(container.querySelector('p')!.textContent).toBe('Both A and B fail.');
    expect([...container.querySelectorAll('li')].map((l) => l.textContent)).toEqual(['one', 'two']);
    expect(container.querySelector('pre')!.textContent).toBe('var x = 1;');
    expect(container.textContent).not.toContain('#');
  });
});
