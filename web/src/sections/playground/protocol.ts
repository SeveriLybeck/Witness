// Plugin reply shapes (contracts/messages/plugin-reply.schema.json) and the gateway calls.
import { repoPaths, type BundleData, type DataSource } from '../../shell';

export type Field = 'int' | 'string' | 'bool' | 'number' | 'code' | 'text' | 'choice' | null;
export interface Slot { name: string; role: string; type: string; constructible: boolean; field?: Field; choices?: string[] }
export type ResultType = 'diagnostics' | 'text-diff' | 'table' | 'facts' | 'bytes' | 'image' | 'image-pair' | 'page';
export const resultTypes: ResultType[] = ['diagnostics', 'text-diff', 'table', 'facts', 'bytes', 'image', 'image-pair', 'page'];
export interface Preset {
  kind: 'difference' | 'issue' | 'revert' | 'free'; label: string; difference?: string; block?: string;
  inputs: Record<string, unknown>;
  /** From replays.json: where the input came from, and why a difference has none. */
  source?: string; reason?: string; detail?: string;
}
export interface Subject {
  id: string; name: string;
  definition: { text: string; locations: { file: string; line: number; endLine?: number }[] };
  inputs: Slot[]; outputs: { name: string; type: ResultType }[]; presets: Preset[];
}
export type Side = 'old' | 'new' | 'reverted';
export interface Result { output: string; side: Side; type: ResultType; value: unknown }
export interface Reply { id: number; op: string; ok: boolean; error?: string; subjects?: Subject[]; results?: Result[]; pid?: number }
/** Every string inside a value with absolute host paths shortened (`repoPaths`); keys are kept. */
export function stripPaths<T>(v: T, data: Pick<BundleData, 'files'>): T {
  if (typeof v === 'string') return repoPaths(v, data) as T;
  if (Array.isArray(v)) return v.map((x) => stripPaths(x, data)) as T;
  if (v && typeof v === 'object') return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, stripPaths(x, data)])) as T;
  return v;
}

/** What the playground shows of a run: text results without host paths; images and pages keep their own paths. */
export function shownResults(results: Result[], data: Pick<BundleData, 'files'>): Result[] {
  return results.map((r) => (r.type === 'image' || r.type === 'image-pair' || r.type === 'page' ? r : { ...r, value: stripPaths(r.value, data) }));
}

export interface Viewer { name: string; title: string; outputs: string[] }
export interface Viewers { viewers: Viewer[]; tabs: boolean }

export async function viewersOf(source: DataSource, key: string): Promise<Viewers> {
  return (await source.request('GET', `/api/bundles/${key}/viewers`)) as Viewers;
}

let nextId = 1;
export async function call(source: DataSource, key: string, plugin: string, request: Record<string, unknown>): Promise<Reply> {
  const body = { plugin, request: { id: nextId++, ...request } };
  return (await source.request('POST', `/api/bundles/${key}/playground`, body)) as Reply;
}

/** A `page` result's URL as an iframe src: relative URLs go through the gateway's pages route; any absolute
 *  URL (e.g. a plugin's own `http://127.0.0.1` server) is refused and gives null. */
export function pageSrc(url: string, key: string, plugin: string): string | null {
  if (/^[a-z][a-z0-9+.-]*:/i.test(url) || url.startsWith('//')) return null;
  return `/api/bundles/${key}/pages/${plugin}/${url.replace(/^\.?\/+/, '')}`;
}

/** Image src: `data:image/...` URLs and relative paths (through the pages route); other absolute URLs refused. */
export function imageSrc(path: string, key: string, plugin: string): string | null {
  if (/^data:image\//i.test(path)) return path;
  return pageSrc(path, key, plugin);
}

interface Replay {
  difference: string; state?: string; reason?: string; detail?: string; roundTrip?: unknown;
  input: { source: string; text: string; buildProperties?: Record<string, string> } | null;
}

/** Splits a replay text with `// lib` and `// app` markers into the library and app inputs; unmarked text is app. */
export function splitReplay(text: string): Record<string, string> {
  const m = /^\/\/ lib\n([\s\S]*?)\n?\/\/ app\n([\s\S]*)$/.exec(text);
  return m ? { library: m[1], app: m[2] } : { library: '', app: text };
}

/** One preset per difference in replays.json (D34), labelled with its number, then the plugin's own other
 *  presets. A difference without an input keeps its preset, disabled, with its reason. A plugin difference
 *  preset's own inputs win over the replay split. */
export function presetsFor(subject: Subject, data: BundleData): Preset[] {
  const replays = (data.records.replays as { replays?: Replay[] } | null)?.replays ?? [];
  const fromPlugin = subject.presets.filter((p) => p.kind === 'difference');
  const out: Preset[] = [];
  for (const d of [...data.differences].sort((a, b) => a.number - b.number)) {
    const r = replays.find((x) => x.difference === d.id);
    if (!r) continue;
    const extra = fromPlugin.find((p) => p.difference === d.id)?.inputs ?? {};
    const text = r.input?.text;
    out.push({ kind: 'difference', label: String(d.number), difference: d.id,
      source: r.input?.source, reason: text ? undefined : (r.reason ?? r.state), detail: r.detail,
      inputs: text ? {
        ...splitReplay(text), ...(r.roundTrip ? { roundTrip: true } : {}),
        ...(r.input?.buildProperties ? { buildProperties: r.input.buildProperties } : {}), ...extra,
      } : {} });
  }
  return [...out, ...subject.presets.filter((p) => p.kind !== 'difference')];
}

/** `buildProperties` object <-> one `Key=Value` per line, for a text field. */
export const props = {
  show: (v: unknown) => (v && typeof v === 'object' ? Object.entries(v).map(([k, x]) => `${k}=${x}`).join('\n') : String(v ?? '')),
  parse: (s: string) => Object.fromEntries(s.split('\n').map((l) => l.trim()).filter(Boolean)
    .map((l) => { const i = l.indexOf('='); return i < 0 ? [l, ''] : [l.slice(0, i).trim(), l.slice(i + 1).trim()]; })),
};
