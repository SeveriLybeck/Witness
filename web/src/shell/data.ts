// Records and the gateway (contracts/messages/gateway-api.md).

export const recordNames = [
  'bundle', 'files', 'parts', 'blocks', 'groups', 'dependencies', 'rows', 'differences', 'scope', 'lean',
  'replays', 'tests', 'links', 'levels', 'questions', 'mode', 'ai-review',
] as const;
export type RecordName = (typeof recordNames)[number];

export type StateName = 'ready' | 'running' | 'not-run' | 'unavailable';
export interface SectionState { state: StateName; detail?: string }

export interface Location { file: string; line: number; endLine?: number }

export interface Bundle {
  repository: string;
  pullRequest: number | null;
  oldCommit: string;
  newCommit: string;
  description?: string;
  requirement: { id: string; title?: string; text: string } | null;
  sections: Record<string, SectionState>;
  [k: string]: unknown;
}
export interface FolderHistory { folder: string; undeclared: number; reviews: number }
export interface FileEntry { file: string; category: string; added: number; deleted: number; history?: FolderHistory }
export interface Part {
  id: string; kind: string; category: string; member: string | null; largeComment: boolean;
  addedLines: number[]; deletedLines: number[];
  old: (Location & { endLine: number }) | null; new: (Location & { endLine: number }) | null;
  references?: string[];
}
export interface Block { id: string; name: string; category: string; parts: string[] }
export interface Difference {
  id: string; number: number; kind: string; block: string; condition: string; old: string; new: string;
  location: Location; checker?: string; rows?: string[]; [k: string]: unknown;
}
/** A rule that fired: a name, or `{rule, fact}` in newer bundles. `loadBundle` flattens both to `rule` or `rule:fact`. */
export type RuleRef = string | { rule?: string; fact?: string | null } | null;
export const ruleName = (r: RuleRef): string =>
  typeof r === 'string' ? r : r && r.rule ? (r.fact ? `${r.rule}:${r.fact}` : r.rule) : '';
/** Rule names of one level entry: unique, and a bare `kind` dropped when a `kind:fact` says more. */
export function ruleNames(rules: unknown): string[] {
  const all = [...new Set((Array.isArray(rules) ? (rules as RuleRef[]) : []).map(ruleName).filter(Boolean))];
  return all.filter((r) => r.includes(':') || !all.some((o) => o.startsWith(`${r}:`)));
}
export interface Level { block: string; level: 1 | 2 | 3; rules: string[] }

/** Raw record files by name; `null` when the gateway has none (404). */
export type Records = { [K in RecordName]: unknown | null };

export interface FileText { commit: string; file: string; lines: string[] }

/** Everything loaded for one bundle, with typed views of the records the shell itself reads. */
export interface BundleData {
  key: string;
  bundle: Bundle;
  records: Records;
  files: FileEntry[];
  parts: Part[];
  blocks: Block[];
  differences: Difference[];
  levels: Level[];
}

export interface BundleListEntry {
  key: string; oldCommit: string; newCommit: string; pullRequest: number | null;
  requirement: string | null; prepared: boolean; stale: boolean;
}

/** Where the page gets its data. `httpSource` in the app, `fixtureSource` (test/harness) in tests. */
export interface DataSource {
  bundles(): Promise<BundleListEntry[]>;
  record(key: string, name: RecordName): Promise<unknown | null>;
  fileText(commit: string, path: string): Promise<FileText | null>;
  /** Any other gateway call, e.g. sessions and playground: JSON in, JSON out. */
  request(method: 'GET' | 'POST', path: string, body?: unknown, headers?: Record<string, string>): Promise<unknown>;
  /** Optional per-folder count for the files index (D44), keyed by folder path ending in `/`. */
  folderCounts?(key: string): Promise<Record<string, number> | null>;
}

export function httpSource(base = ''): DataSource {
  async function json(method: string, path: string, body?: unknown, extra?: Record<string, string>): Promise<{ status: number; data: unknown }> {
    const res = await fetch(base + path, {
      method,
      headers: { ...(body === undefined ? {} : { 'content-type': 'application/json' }), ...extra },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    const text = await res.text();
    return { status: res.status, data: text ? JSON.parse(text) : null };
  }
  return {
    async bundles() {
      const r = await json('GET', '/api/bundles');
      return (r.data as { bundles: BundleListEntry[] }).bundles;
    },
    async record(key, name) {
      const r = await json('GET', `/api/bundles/${key}/${name}.json`);
      return r.status === 404 ? null : r.data;
    },
    async fileText(commit, path) {
      const r = await json('GET', `/api/file?commit=${commit}&path=${encodeURIComponent(path)}`);
      return r.status >= 400 ? null : (r.data as FileText);
    },
    async request(method, path, body, headers) {
      const r = await json(method, path, body, headers);
      if (r.status >= 400) throw Object.assign(new Error(`gateway ${r.status}`), { status: r.status, data: r.data });
      return r.data;
    },
  };
}

function list<T>(rec: unknown, field: string): T[] {
  const v = rec && (rec as Record<string, unknown>)[field];
  return Array.isArray(v) ? (v as T[]) : [];
}

export async function loadBundle(source: DataSource, key: string): Promise<BundleData> {
  const entries = await Promise.all(
    recordNames.map(async (n) => [n, await source.record(key, n).catch(() => null)] as const),
  );
  const records = Object.fromEntries(entries) as Records;
  const bundle = records.bundle as Bundle;
  if (!bundle) throw new Error(`no bundle ${key}`);
  return {
    key,
    bundle,
    records,
    files: list(records.files, 'files'),
    parts: list(records.parts, 'parts'),
    blocks: list(records.blocks, 'blocks'),
    differences: list(records.differences, 'differences'),
    levels: list<Level>(records.levels, 'levels').map((l) => ({ ...l, rules: ruleNames(l.rules) })),
  };
}

/**
 * Text from records with absolute host paths cut down to repository-relative ones: a path is cut at its first
 * segment that is a top-level folder of a changed file; other absolute paths keep their last two segments.
 */
export function repoPaths(text: string, data: Pick<BundleData, 'files'>): string {
  const tops = new Set(data.files.map((f) => f.file.split('/')[0]).filter((s) => s && !s.includes('.')));
  return text.replace(/(?<![\w.:/-])\/(?:[\w.@+~-]+\/)+[\w.@+~`-]*/g, (path) => {
    const seg = path.split('/').filter(Boolean);
    if (seg.length < 3) return path;
    const at = seg.map((s, k) => (k > 0 && tops.has(s) ? k : -1)).filter((k) => k > 0);
    if (!at.length) return `…/${seg.slice(-2).join('/')}`;
    const known = at.find((k) => data.files.some((f) => f.file === seg.slice(k).join('/')));
    return seg.slice(known ?? at[at.length - 1]).join('/');
  });
}

/** Words that only point at the issue: a description made of these says nothing about what the change does. */
const POINTER = /^(agentic|implementation|implements?|implemented|fix(es|ed)?|close[sd]?|resolve[sd]?|for|of|the|issue|pr|pull|request|#?\d+|[:.\-–—])$/i;

/**
 * What the pull request says it does, on one line: the first line of the description. When that line only
 * points at the issue (`implementation for #2283`), the requirement's title is the claim.
 */
export function claimOf(data: Pick<BundleData, 'bundle'>): string {
  const line = (data.bundle.description ?? '').split(/\r?\n/).map((l) => l.replace(/^[#>*\s-]+(?=\D)/, '').trim()).find(Boolean) ?? '';
  const says = line.split(/\s+/).filter((w) => w && !POINTER.test(w));
  const title = data.bundle.requirement?.title?.trim() ?? '';
  return says.length < 3 && title ? title : line;
}

/** Whether the claim line already carries the requirement's title, so the header need not repeat it. */
export function claimHasTitle(data: Pick<BundleData, 'bundle'>): boolean {
  const norm = (s: string) => s.toLowerCase().replace(/[^a-z0-9]+/g, ' ').trim();
  const title = norm(data.bundle.requirement?.title ?? '');
  return !!title && norm(claimOf(data)).includes(title);
}

interface ReplayEntry { difference: string | null; state?: string; input?: { source?: string } | null; roundTrip?: { sent?: unknown; received?: unknown } | null }
const replayList = (data: Pick<BundleData, 'records'>) => list<ReplayEntry>(data.records.replays, 'replays');

/** The value a replay sent and got back when they differ, e.g. `42 → 0`: what a data loss looks like to a caller. */
export function lostValue(data: Pick<BundleData, 'records'>, difference: string): string | null {
  const rt = replayList(data).find((r) => r.difference === difference)?.roundTrip;
  if (!rt || rt.sent == null || rt.received == null || String(rt.sent) === String(rt.received)) return null;
  return `${String(rt.sent)} → ${String(rt.received)}`;
}

/** How many differences were replayed live, of how many, and whether the issue's own reproduction is among the inputs. */
export function replayCounts(data: Pick<BundleData, 'records'>): { replayed: number; total: number; issue: boolean } {
  const all = replayList(data).filter((r) => r.difference);
  return {
    replayed: all.filter((r) => r.input && r.state !== 'not-replayed').length,
    total: all.length,
    issue: replayList(data).some((r) => r.input?.source === 'issue'),
  };
}

export const basename = (path: string) => path.slice(path.lastIndexOf('/') + 1);

/** Block ids with a part in `file` (old or new side). */
export function blocksInFile(data: BundleData, file: string): Set<string> {
  const parts = new Set(data.parts.filter((p) => p.new?.file === file || p.old?.file === file).map((p) => p.id));
  return new Set(data.blocks.filter((b) => b.parts.some((id) => parts.has(id))).map((b) => b.id));
}

/** Changed lines of one file: added on the new side, deleted on the old side. */
export function changedLines(data: BundleData, file: string): { added: Set<number>; deleted: Set<number> } {
  const added = new Set<number>();
  const deleted = new Set<number>();
  for (const p of data.parts) {
    if (p.new?.file === file) p.addedLines.forEach((l) => added.add(l));
    if (p.old?.file === file) p.deletedLines.forEach((l) => deleted.add(l));
  }
  return { added, deleted };
}

export function firstChange(data: BundleData, file: string): { line: number; side: 'old' | 'new' } {
  const { added, deleted } = changedLines(data, file);
  if (added.size) return { line: Math.min(...added), side: 'new' };
  if (deleted.size) return { line: Math.min(...deleted), side: 'old' };
  return { line: 1, side: 'new' };
}

export function levelOf(data: BundleData, block: string): 1 | 2 | 3 {
  return data.levels.find((l) => l.block === block)?.level ?? 1;
}
