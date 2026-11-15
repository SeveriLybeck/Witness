import { afterEach, describe, expect, it } from 'vitest';
import { act, fireEvent, screen, waitFor } from '@testing-library/react';
import { renderPage } from './harness';
import { Loc } from '../src/shell';
import { shellCss } from '../src/shell/styles';

const tc = 'src/MessagePack.SourceGenerator/CodeAnalysis/TypeCollector.cs';
const sections = { proof: { slot: 'proof' as const, Component: () => <Loc file={tc} line={1077} /> } };

function setWidth(w: number) {
  Object.defineProperty(window, 'innerWidth', { configurable: true, value: w });
}
afterEach(() => { setWidth(1440); delete document.documentElement.dataset.theme; });

describe('craft (D14)', () => {
  it('Ctrl+K opens the palette; Theme light switches the theme', async () => {
    await renderPage('2283', { sections: {} });
    fireEvent.keyDown(window, { key: 'k', ctrlKey: true });
    const input = screen.getByRole('textbox', { name: 'Commands' });
    fireEvent.change(input, { target: { value: 'theme li' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(document.documentElement.dataset.theme).toBe('light');
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('? opens the keyboard map; f opens files; Esc closes', async () => {
    await renderPage('2283', { sections: {} });
    fireEvent.keyDown(window, { key: '?' });
    const keys = screen.getByRole('dialog', { name: 'Keyboard' });
    expect(keys.textContent).toContain('Ctrl K');
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.queryByRole('dialog')).toBeNull();
    fireEvent.keyDown(window, { key: 'f' });
    expect(screen.getByRole('dialog', { name: 'Files' })).toBeTruthy();
  });

  it('every control is a focusable button with the accent focus outline', async () => {
    const { container } = await renderPage('2283', { sections });
    for (const b of container.querySelectorAll('button')) expect(b.tabIndex).toBe(0);
    expect(shellCss).toMatch(/:focus-visible\s*\{\s*outline:\s*2px solid var\(--accent\)/);
  });

  it('header file chip shrinks with an ellipsis (long names at 390 px)', () => {
    expect(shellCss).toMatch(/\.pagehead \.chip \{[^}]*min-width: 0;[^}]*text-overflow: ellipsis/);
    expect(shellCss).toMatch(/\.files-anchor \{[^}]*flex: 0 0 auto/);
  });

  it('transitions use the motion token only', () => {
    for (const m of shellCss.matchAll(/transition[^:]*:\s*([^;]+)/g)) {
      for (const d of m[1].matchAll(/(\d+)m?s\b/g)) expect(d[0], m[0]).toBe('0ms');
      expect(m[1].includes('var(--motion)') || /0ms/.test(m[1]), m[0]).toBe(true);
    }
  });

  it('390 px: code panel is a full-screen sheet, no fixed width over 390 px', async () => {
    setWidth(390);
    const { container } = await renderPage('2283', { sections });
    act(() => screen.getByRole('button', { name: `${tc}:1077` }).click());
    await waitFor(() => expect(screen.getByRole('complementary', { name: 'Code' }).dataset.layout).toBe('sheet'));
    expect(container.querySelector('.layout')!.classList.contains('docked')).toBe(false);
    const css = shellCss.replace(/\/\*[\s\S]*?\*\//g, '');
    for (const rule of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
      if (rule[1].includes('.layout.docked')) continue; // only applied above the phone width
      for (const m of rule[2].matchAll(/(?:^|;)\s*((?:min-)?width)\s*:\s*([^;]+)/g)) {
        for (const px of m[2].matchAll(/(\d+)px/g)) {
          if (m[2].includes('min(')) continue;
          expect(Number(px[1]), `${rule[1].trim()} ${m[1]}`).toBeLessThanOrEqual(390);
        }
      }
    }
  });
});
