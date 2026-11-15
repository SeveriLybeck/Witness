import { describe, expect, it } from 'vitest';
import { colors, diffTokens, fontSizes, hueTokens, motionMs, space, statusTokens, tokenCss } from '../src/shell/tokens';
import reviewCss from '../src/sections/review/review.css?raw';
import { allLabels, banned } from '../src/shell/labels';
import { shellCss } from '../src/shell/styles';
import { isBlue, isRed, oklch } from './color';

describe('tokens (D10, D11)', () => {
  it('four sizes, body at least 15 px', () => {
    const sizes = Object.values(fontSizes);
    expect(new Set(sizes).size).toBe(4);
    expect(Math.min(...sizes)).toBeGreaterThanOrEqual(15);
    expect(fontSizes.body).toBe(15);
  });

  it('spacing on the 8 px grid', () => {
    for (const v of Object.values(space)) expect(v % 8).toBe(0);
  });

  it('light and dark define the same tokens', () => {
    expect(Object.keys(colors.light).sort()).toEqual(Object.keys(colors.dark).sort());
  });

  it('neutral except the accent and diff tints; no red with blue', () => {
    for (const theme of ['light', 'dark'] as const) {
      const hued: { token: string; h: number }[] = [];
      for (const [token, value] of Object.entries(colors[theme])) {
        const { c, h } = oklch(value);
        if (!hueTokens.includes(token as never)) expect(c, `${theme} ${token}`).toBeLessThan(0.02);
        else if (c >= 0.02) hued.push({ token, h });
      }
      const outsideDiff = hued.filter((x) => !diffTokens.includes(x.token as never) && !statusTokens.includes(x.token as never));
      expect(outsideDiff.map((x) => x.token)).toEqual(['accent']);
      expect(outsideDiff.some((x) => isRed(x.h)) && outsideDiff.some((x) => isBlue(x.h))).toBe(false);
      expect(outsideDiff.some((x) => isRed(x.h))).toBe(false);
      // the three status hues are muted: neither loud nor neon on either theme
      for (const t of statusTokens) expect(oklch(colors[theme][t]).c, `${theme} ${t}`).toBeLessThan(0.13);
    }
  });

  it('the status hues colour only the status dot', () => {
    const css = [shellCss, reviewCss].join('\n').replace(/\/\*[\s\S]*?\*\//g, '');
    for (const rule of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
      if (/var\(--(ok|warn|stop)\)/.test(rule[2])) expect(rule[1].trim(), rule[0]).toMatch(/\.st-dot$/);
    }
  });

  it('motion 120-200 ms, 0 under reduced motion', () => {
    expect(motionMs).toBeGreaterThanOrEqual(120);
    expect(motionMs).toBeLessThanOrEqual(200);
    expect(tokenCss()).toMatch(/prefers-reduced-motion: reduce\)\{:root\{--motion:0ms\}/);
  });
});

describe('shell css uses tokens only', () => {
  const decls = [...shellCss.replace(/\/\*[\s\S]*?\*\//g, '').matchAll(/([a-z-]+)\s*:\s*([^;{}]+)/g)].map((m) => [m[1], m[2].trim()]);

  it('css is loaded', () => expect(decls.length).toBeGreaterThan(100));

  it('font sizes from the four tokens', () => {
    for (const [p, v] of decls) if (p === 'font-size') expect(v).toMatch(/^var\(--fs-(body|lead|title|display)\)$/);
  });

  it('margin, padding and gap on the 8 px grid', () => {
    for (const [p, v] of decls) {
      if (!/^(margin|padding|gap|row-gap|column-gap)(-|$)/.test(p)) continue;
      for (const px of v.matchAll(/(-?\d+(?:\.\d+)?)px/g)) expect(Number(px[1]) % 8, `${p}: ${v}`).toBe(0);
    }
  });

  it('no colour literals, gradients or glows', () => {
    for (const [p, v] of decls) {
      if (p.startsWith('--')) continue;
      expect(v, `${p}: ${v}`).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|hsl\(|oklch\(|gradient/i);
    }
  });
});

describe('words (D2, D9)', () => {
  const texts = [...allLabels(), tokenCss(), shellCss];

  it('no banned word in tokens or labels', () => {
    for (const t of texts) for (const w of banned) expect(t.toLowerCase(), w).not.toMatch(new RegExp(`\\b${w}`));
  });

  it('labels: at most three words, no sentence ending', () => {
    for (const l of allLabels()) {
      expect(l.split(/\s+/).length, l).toBeLessThanOrEqual(3);
      expect(l, l).not.toMatch(/[.?!]$/);
    }
  });
});
