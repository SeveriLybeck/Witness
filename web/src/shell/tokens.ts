// Single source of design tokens (D10, D11). `tokenCss()` turns them into CSS custom properties.

/** The four font sizes in px. Body is the first. */
export const fontSizes = { body: 15, lead: 20, title: 24, display: 32 } as const;

/** Spacing scale on the 8 px grid, in px. */
export const space = { 0: 0, 1: 8, 2: 16, 3: 24, 4: 32, 6: 48, 8: 64 } as const;

export const radius = { control: 6, panel: 10 } as const;

/** Motion for state changes only; 0 under reduced motion. */
export const motionMs = 160;

export type Theme = 'dark' | 'light';
export type ColorToken =
  | 'bg' | 'surface' | 'raise' | 'line' | 'line-2' | 'fg' | 'fg-dim' | 'accent' | 'add' | 'del' | 'shadow'
  | 'ok' | 'warn' | 'stop';

export const colors: Record<Theme, Record<ColorToken, string>> = {
  dark: {
    bg: '#0e0e10', surface: '#161618', raise: '#1e1e21', line: '#26262a', 'line-2': '#3a3a40',
    fg: '#e8e7e3', 'fg-dim': '#8b8b90', accent: '#7c84e6',
    add: 'rgb(140 170 140 / 0.10)', del: 'rgb(180 130 130 / 0.11)', shadow: 'rgb(0 0 0 / 0.5)',
    ok: '#6fa883', warn: '#c9a15a', stop: '#c9746c',
  },
  light: {
    bg: '#f6f6f5', surface: '#ffffff', raise: '#efefed', line: '#e4e4e1', 'line-2': '#cfcfcb',
    fg: '#1c1c1e', 'fg-dim': '#76767a', accent: '#5e6ad2',
    add: 'rgb(140 170 140 / 0.14)', del: 'rgb(180 130 130 / 0.14)', shadow: 'rgb(0 0 0 / 0.12)',
    ok: '#4b8a5f', warn: '#a87f33', stop: '#b2564e',
  },
};

/** Tokens allowed to carry hue: the accent, the code diff tints and the three status dots. */
export const hueTokens: ColorToken[] = ['accent', 'add', 'del', 'ok', 'warn', 'stop'];
export const diffTokens: ColorToken[] = ['add', 'del'];
/** Green / orange / red of the status, used only for its dot (`.st-dot`). */
export const statusTokens: ColorToken[] = ['ok', 'warn', 'stop'];

export const fonts = {
  sans: 'Inter, ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif',
  mono: '"JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, monospace',
};

/** Below this width the code panel becomes a full-screen sheet. */
export const phoneMax = 719;

function vars(theme: Theme): string {
  return Object.entries(colors[theme]).map(([k, v]) => `--${k}:${v};`).join('');
}

export function tokenCss(): string {
  const fs = Object.entries(fontSizes).map(([k, v]) => `--fs-${k}:${v}px;`).join('');
  const sp = Object.entries(space).map(([k, v]) => `--s-${k}:${v}px;`).join('');
  return [
    `:root{${fs}${sp}--r-control:${radius.control}px;--r-panel:${radius.panel}px;--motion:${motionMs}ms;` +
      `--sans:${fonts.sans};--mono:${fonts.mono};color-scheme:dark;${vars('dark')}}`,
    `@media (prefers-color-scheme: light){:root:not([data-theme="dark"]){color-scheme:light;${vars('light')}}}`,
    `:root[data-theme="light"]{color-scheme:light;${vars('light')}}`,
    `:root[data-theme="dark"]{color-scheme:dark;${vars('dark')}}`,
    `@media (prefers-reduced-motion: reduce){:root{--motion:0ms}}`,
  ].join('\n');
}
