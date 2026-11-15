// Test harness for the shell and for sections (groups 15-18).
import { readFileSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { act, fireEvent, render } from '@testing-library/react';
import type { ReactNode } from 'react';
import { loadBundle, type BundleData, type DataSource, type FileText, type RecordName } from '../src/shell/data';
import { ShellProvider } from '../src/shell/context';
import { Page } from '../src/shell/Page';
import type { SectionModule } from '../src/shell/registry';
import type { Slot } from '../src/shell/context';

export type Change = '2283' | '2234' | '2297' | '2129';
export const changes: Change[] = ['2283', '2234', '2297', '2129'];
export const fixturesDir = resolve(__dirname, '../../contracts/fixtures');
/** Copies of the four prepared real bundles (2283 with the review the short-check prompt gave). */
export const realDir = resolve(__dirname, 'fixtures/real');
const fileTextDir = resolve(__dirname, 'fixtures');

export function readRecord(change: Change, name: RecordName, root = fixturesDir): unknown | null {
  const p = resolve(root, change, `${name}.json`);
  return existsSync(p) ? JSON.parse(readFileSync(p, 'utf8')) : null;
}

/** File texts kept under test/fixtures as gateway `fileText` (only 2283 TypeCollector.cs, old and new). */
function fileTexts(): FileText[] {
  return ['old', 'new'].map((v) => JSON.parse(readFileSync(resolve(fileTextDir, `TypeCollector.${v}.json`), 'utf8')));
}

/** A DataSource over `contracts/fixtures/<change>/` (or `root`). `request` answers via `onRequest` or throws 404. */
export function fixtureSource(change: Change, opts: {
  onRequest?: (method: string, path: string, body?: unknown, headers?: Record<string, string>) => unknown;
  folderCounts?: Record<string, number>;
  root?: string;
} = {}): DataSource & { key: string } {
  const b = readRecord(change, 'bundle', opts.root) as { oldCommit: string; newCommit: string };
  const key = `${b.oldCommit}-${b.newCommit}`;
  const texts = fileTexts();
  return {
    key,
    async bundles() {
      return [{ key, oldCommit: b.oldCommit, newCommit: b.newCommit, pullRequest: null, requirement: change, prepared: true, stale: false }];
    },
    async record(_k, name) { return readRecord(change, name, opts.root); },
    async fileText(commit, path) { return texts.find((t) => t.commit === commit && t.file === path) ?? null; },
    async request(method, path, body, headers) {
      if (opts.onRequest) return opts.onRequest(method, path, body, headers);
      throw Object.assign(new Error('gateway 404'), { status: 404 });
    },
    folderCounts: opts.folderCounts ? async () => opts.folderCounts! : undefined,
  };
}

/** A DataSource over the real bundle copy of one change. */
export const realSource = (change: Change) => fixtureSource(change, { root: realDir });

/** One DataSource listing all four real bundles, as the gateway does. */
export function realGateway(): DataSource {
  const each = changes.map((c) => ({ c, s: realSource(c) }));
  const of = (key: string) => each.find((e) => e.s.key === key)!.s;
  return {
    async bundles() {
      return each.map(({ c, s }, i) => {
        const [oldCommit, newCommit] = s.key.split('-');
        return { key: s.key, oldCommit, newCommit, pullRequest: i + 1, requirement: c, prepared: true, stale: false };
      });
    },
    record: (key, name) => of(key).record(key, name),
    fileText: async () => null,
    async request() { throw Object.assign(new Error('gateway 404'), { status: 404 }); },
  };
}

export async function fixtureData(change: Change): Promise<{ data: BundleData; source: DataSource }> {
  const source = fixtureSource(change);
  return { data: await loadBundle(source, source.key), source };
}

export async function realData(change: Change): Promise<BundleData> {
  const source = realSource(change);
  return loadBundle(source, source.key);
}

/** Renders `children` (e.g. one section component) inside the shell context for a fixture. */
export async function renderInShell(change: Change, children: (data: BundleData) => ReactNode, source?: DataSource) {
  const src = source ?? fixtureSource(change);
  const data = await loadBundle(src, (src as { key?: string }).key ?? '');
  return { data, ...render(<ShellProvider data={data} source={src}>{children(data)}</ShellProvider>) };
}

/** Unfolds every folded section of the rendered page. */
export function unfoldAll() {
  act(() => {
    document.querySelectorAll<HTMLElement>('.slot-toggle[aria-expanded="false"]').forEach((t) => fireEvent.click(t));
  });
}

/**
 * Renders the whole page for a fixture; `sections` defaults to everything under src/sections.
 * Sections 1–3 and Decide start folded on the page; tests of their content get them unfolded unless `asLoaded`.
 */
export async function renderPage(change: Change, opts: { sections?: Partial<Record<Slot, SectionModule>>; source?: DataSource; asLoaded?: boolean } = {}) {
  const r = await renderInShell(change, () => <Page sections={opts.sections} />, opts.source);
  if (!opts.asLoaded) unfoldAll();
  return r;
}
