// Pure map model and layout (D29-D32). No DOM.
import type { BundleData } from '../../shell';

export type Placement = 'declared-region' | 'outside' | 'neither';
export interface Link { block: string; state: 'declared' | 'not-declared' | 'not-linked'; tests: string[]; placement: Placement }
export interface Dependency {
  from: string; to: string; version: 'old' | 'new' | 'both'; via: 'reference' | 'value'; changed: boolean;
  sites: { file: string; line: number; endLine?: number }[];
}
export interface Group { id: string; rule: 'identical-edit' | 'moved' | 'rename' | 'formatting'; parts: string[]; count: number }

export const NODE_W = 232;
/** Closed node: name (up to two lines), numbers row, file row (shown on hover/focus), padding. */
export const NODE_H = 132;
export const OPEN_W = 400;
export const PART_H = 32;
export const COL_GAP = 72;
export const ROW_GAP = 24;
export const REGION_PAD = 16;
export const STUB = 120;
export const PAD = 16;

export interface Rect { x: number; y: number; w: number; h: number }
export interface Layout {
  nodes: Record<string, Rect>;
  columns: string[][];
  region: Rect | null;
  width: number;
  height: number;
}
export interface Sizes { heights?: Record<string, number>; widths?: Record<string, number>; vw?: number }

function list<T>(rec: unknown, field: string): T[] {
  const v = rec && (rec as Record<string, unknown>)[field];
  return Array.isArray(v) ? (v as T[]) : [];
}

export const linksOf = (d: BundleData) => list<Link>(d.records.links, 'links');
export const depsOf = (d: BundleData) => list<Dependency>(d.records.dependencies, 'dependencies');
export const groupsOf = (d: BundleData) => list<Group>(d.records.groups, 'groups');

/** Call-site groups per block: `dependencies.json` top-level `reach` (pipeline runs; absent in fixtures). */
export interface Reach { block: string; callSites: number; sites?: { file: string; line: number; endLine?: number }[] }
export const reachOf = (d: BundleData) => list<Reach>(d.records.dependencies, 'reach');

export function placementOf(d: BundleData, block: string): Placement {
  return linksOf(d).find((l) => l.block === block)?.placement ?? 'neither';
}

/** Columns that fit side by side in `vw` at scale 1. */
export function columnsFor(vw: number) {
  return Math.max(1, Math.floor((vw - 2 * PAD + COL_GAP) / (NODE_W + COL_GAP)));
}

/**
 * Columns: production blocks by dependency depth, then tests and docs. In each column declared blocks come first.
 * While there is room for more columns in `vw`, the tallest column is split in two, so a long change reads as a grid.
 */
export function columnsOf(d: BundleData, vw = 960): string[][] {
  const prod = d.blocks.filter((b) => b.category === 'production').map((b) => b.id);
  const rest = d.blocks.filter((b) => b.category !== 'production').map((b) => b.id);
  const deps = depsOf(d).filter((e) => e.from !== e.to && prod.includes(e.from) && prod.includes(e.to));
  const depth: Record<string, number> = Object.fromEntries(prod.map((id) => [id, 0]));
  for (let i = 0; i < prod.length; i++) {
    for (const e of deps) depth[e.to] = Math.max(depth[e.to], Math.min(depth[e.from] + 1, prod.length - 1));
  }
  const rank = (id: string) => ({ 'declared-region': 0, outside: 1, neither: 2 })[placementOf(d, id)];
  const cols: string[][] = [];
  for (const id of prod) (cols[depth[id]] ??= []).push(id);
  const out = cols.filter(Boolean).map((c) => c.map((id, i) => [id, i] as const)
    .sort((a, b) => rank(a[0]) - rank(b[0]) || a[1] - b[1]).map(([id]) => id));
  if (rest.length) out.push(rest);
  const max = columnsFor(vw);
  while (out.length < max) {
    let t = 0;
    out.forEach((c, i) => { if (c.length > out[t].length) t = i; });
    if (out[t].length < 4) break;
    const c = out[t];
    out.splice(t, 1, c.slice(0, Math.ceil(c.length / 2)), c.slice(Math.ceil(c.length / 2)));
  }
  return out;
}

/**
 * Grid at scale 1; `heights`/`widths` override a node's size (the opened block, from its measured content).
 * Declared blocks stack at the top of each column inside one region; every other block starts below it.
 * A column is as wide as its widest node, so growing a node moves later columns right and nodes below it down.
 */
export function layout(d: BundleData, s: Sizes = {}): Layout {
  const heights = s.heights ?? {}, widths = s.widths ?? {};
  const columns = columnsOf(d, s.vw);
  const inside = new Set(d.blocks.filter((b) => placementOf(d, b.id) === 'declared-region').map((b) => b.id));
  const nodes: Record<string, Rect> = {};
  const top = inside.size ? REGION_PAD + 32 : 0;
  const colW = columns.map((col) => Math.max(NODE_W, ...col.map((id) => widths[id] ?? NODE_W)));
  const colX: number[] = [];
  columns.forEach((_, c) => { colX[c] = c ? colX[c - 1] + colW[c - 1] + COL_GAP : REGION_PAD; });
  let regionBottom = 0;
  columns.forEach((col, c) => {
    let y = top;
    for (const id of col.filter((id) => inside.has(id))) {
      const h = heights[id] ?? NODE_H;
      nodes[id] = { x: colX[c], y, w: widths[id] ?? NODE_W, h };
      y += h + ROW_GAP;
      regionBottom = Math.max(regionBottom, y - ROW_GAP + REGION_PAD);
    }
  });
  let region: Rect | null = null;
  if (inside.size) {
    const xs = [...inside].map((id) => nodes[id]);
    const x0 = Math.min(...xs.map((r) => r.x)) - REGION_PAD;
    const x1 = Math.max(...xs.map((r) => r.x + r.w)) + REGION_PAD;
    region = { x: Math.max(0, x0), y: 0, w: x1 - Math.max(0, x0), h: regionBottom };
  }
  let height = region ? region.h : 0;
  let width = region ? region.x + region.w : 0;
  columns.forEach((col, c) => {
    let y = region ? region.h + ROW_GAP * 2 : 0;
    for (const id of col.filter((id) => !inside.has(id))) {
      const h = heights[id] ?? NODE_H;
      nodes[id] = { x: colX[c], y, w: widths[id] ?? NODE_W, h };
      y += h + ROW_GAP;
    }
  });
  for (const r of Object.values(nodes)) {
    height = Math.max(height, r.y + r.h);
    width = Math.max(width, r.x + r.w + REGION_PAD);
  }
  return { nodes, columns, region, width, height };
}

/** The opened block plus stubs of its direct neighbours, clamped to the graph. */
export function focusBox(l: Layout, id: string, neighbours: string[]): Rect {
  const r = l.nodes[id];
  let x0 = r.x - 8, x1 = r.x + r.w + 8;
  for (const n of neighbours) {
    const m = l.nodes[n];
    if (!m) continue;
    if (m.x < r.x) x0 = Math.min(x0, r.x - STUB);
    if (m.x > r.x) x1 = Math.max(x1, r.x + r.w + STUB);
  }
  x0 = Math.max(0, x0);
  return { x: x0, y: Math.max(0, r.y - 8), w: x1 - x0, h: r.h + 16 };
}

/** Horizontal scroll showing `box` (block plus stubs) centred when it fits in a `vw` wide view, else the block `node` itself. */
export function scrollFor(box: Rect, node: Rect, vw: number, k = 1): number {
  const inner = vw - 2 * PAD;
  const b = box.w * k <= inner ? box : node;
  const left = b.x * k, w = b.w * k;
  return Math.max(0, w <= inner ? left - (inner - w) / 2 : left);
}

export function neighboursOf(d: BundleData, id: string): string[] {
  return depsOf(d).flatMap((e) => (e.from === id ? [e.to] : e.to === id ? [e.from] : []));
}

export function overlaps(a: Rect, b: Rect): boolean {
  return a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
}
