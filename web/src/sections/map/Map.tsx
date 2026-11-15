// The map section (D12, D14, D29-D33).
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type PointerEvent as RPointerEvent } from 'react';
import { DiffNumber, LevelGlyph, Loc, basename, levelOf, useCommands, useShell, type SectionProps, type Part } from '../../shell';
import mapCss from './map.css?raw';
import { mapLabels, ruleLabels } from './labels';
import {
  NODE_H, OPEN_W, PAD, PART_H, depsOf, focusBox, groupsOf, layout, linksOf, neighboursOf, reachOf, scrollFor,
  type Dependency, type Layout,
} from './layout';

/** Text never renders below scale 1 (no non-code text under 15 px); zoom only enlarges. */
export const MIN_K = 1;
export const MAX_K = 2;

function installMapCss() {
  if (typeof document === 'undefined' || document.getElementById('witness-map-css')) return;
  const el = document.createElement('style');
  el.id = 'witness-map-css';
  el.textContent = mapCss;
  document.head.appendChild(el);
}

/** First estimate of an opened block's height; the measured content height replaces it. */
export function openHeight(parts: number, rules: number) {
  return NODE_H + (rules ? 48 : 0) + parts * (PART_H + 24);
}
export const openWidth = (vw: number) => Math.max(200, Math.min(OPEN_W, vw - 2 * PAD));

const partLoc = (p: Part) => (p.new ? { ...p.new, side: 'new' as const } : p.old ? { ...p.old, side: 'old' as const } : null);
const partName = (p: Part) => p.member ?? p.kind;

function edgePath(l: Layout, e: Dependency) {
  const a = l.nodes[e.from], b = l.nodes[e.to];
  if (!a || !b) return null;
  const x1 = a.x + a.w, y1 = a.y + 20;
  const x2 = b.x, y2 = b.y + 20;
  const dx = Math.max(40, Math.abs(x2 - x1) / 2);
  return { d: `M${x1},${y1} C${x1 + dx},${y1} ${x2 - dx},${y2} ${x2},${y2}` };
}

export function MapSection({ data }: SectionProps) {
  const { selection, focusBlock, highlight } = useShell();
  const viewRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState<string | null>(null);
  const [vw, setVw] = useState(960);
  const [k, setK] = useState(1);
  const [sitesOpen, setSitesOpen] = useState<string | null>(null);
  const [groupOpen, setGroupOpen] = useState<string | null>(null);
  const [measured, setMeasured] = useState<{ id: string; h: number } | null>(null);
  const [dragging, setDragging] = useState(false);
  const pointers = useRef(new Map<number, { x: number; y: number }>());
  const moved = useRef(0);
  const pinch = useRef<number | null>(null);

  useEffect(installMapCss, []);
  useEffect(() => {
    const el = viewRef.current;
    if (!el) return;
    const measure = () => { if (el.clientWidth) setVw(el.clientWidth); };
    measure();
    const ro = typeof ResizeObserver !== 'undefined' ? new ResizeObserver(measure) : null;
    ro?.observe(el);
    return () => ro?.disconnect();
  }, []);
  useEffect(() => { if (selection.block && data.blocks.some((b) => b.id === selection.block)) setOpen(selection.block); }, [selection.block, data.blocks]);

  const partsById = useMemo(() => new Map(data.parts.map((p) => [p.id, p])), [data.parts]);
  const links = useMemo(() => linksOf(data), [data]);
  const deps = useMemo(() => depsOf(data), [data]);
  const reach = useMemo(() => reachOf(data), [data]);
  const groups = useMemo(() => groupsOf(data), [data]);
  const rulesOf = (id: string) => data.levels.find((l) => l.block === id)?.rules ?? [];
  const sizes = useMemo(() => {
    const b = open ? data.blocks.find((x) => x.id === open) : null;
    if (!b) return { vw };
    const est = openHeight(b.parts.length, data.levels.find((l) => l.block === b.id)?.rules.length ?? 0);
    const h = measured?.id === b.id ? measured.h : est;
    return { vw, heights: { [b.id]: h }, widths: { [b.id]: openWidth(vw) } };
  }, [open, data, measured, vw]);
  const l = useMemo(() => layout(data, sizes), [data, sizes]);

  // Measured content height of the opened block re-lays the grid, so it is neither clipped nor overlapping.
  useLayoutEffect(() => {
    if (!open) return;
    const el = viewRef.current?.querySelector<HTMLElement>(`[data-node="${open}"]`);
    const body = el?.querySelector<HTMLElement>('.mnode-body');
    if (!el || !body) return;
    const check = () => {
      const h = Math.ceil(body.scrollHeight) + 2;
      if (h > 10 && Math.abs(h - parseFloat(el.style.height)) > 1) setMeasured((m) => (m?.id === open && m.h === h ? m : { id: open, h }));
    };
    check();
    const ro = typeof ResizeObserver !== 'undefined' ? new ResizeObserver(check) : null;
    ro?.observe(body);
    return () => ro?.disconnect();
  });

  // Camera: show the opened block with stubs of its neighbours; the rest stays reachable by scrolling.
  useEffect(() => {
    const el = viewRef.current;
    if (!el) return;
    if (!open || !l.nodes[open]) return;
    el.scrollLeft = scrollFor(focusBox(l, open, neighboursOf(data, open)), l.nodes[open], vw, k);
    el.querySelector<HTMLElement>(`[data-node="${open}"]`)?.scrollIntoView?.({ block: 'nearest', inline: 'nearest' });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, l.nodes[open ?? '']?.x, l.nodes[open ?? '']?.h]);

  useEffect(() => {
    const el = viewRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      if (!(e.ctrlKey || e.metaKey)) return; // plain wheel scrolls the page (and the map sideways)
      e.preventDefault();
      setK((q) => Math.min(MAX_K, Math.max(MIN_K, q * Math.exp(-e.deltaY * 0.002))));
    };
    el.addEventListener('wheel', onWheel, { passive: false });
    return () => el.removeEventListener('wheel', onWheel);
  }, []);

  function onPointerDown(e: RPointerEvent) {
    if ((e.target as HTMLElement).closest('button, a, input')) return;
    pointers.current.set(e.pointerId, { x: e.clientX, y: e.clientY });
    moved.current = 0;
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    setDragging(true);
  }
  function onPointerMove(e: RPointerEvent) {
    const prev = pointers.current.get(e.pointerId);
    if (!prev) return;
    pointers.current.set(e.pointerId, { x: e.clientX, y: e.clientY });
    const pts = [...pointers.current.values()];
    if (pts.length === 2) {
      const dist = Math.hypot(pts[0].x - pts[1].x, pts[0].y - pts[1].y);
      if (pinch.current) { const f = dist / pinch.current; setK((q) => Math.min(MAX_K, Math.max(MIN_K, q * f))); }
      pinch.current = dist;
      moved.current += 10;
      return;
    }
    const dx = e.clientX - prev.x, dy = e.clientY - prev.y;
    moved.current += Math.abs(dx) + Math.abs(dy);
    const el = viewRef.current!;
    el.scrollLeft -= dx;
    if (e.pointerType !== 'touch') window.scrollBy?.(0, -dy);
  }
  function onPointerUp(e: RPointerEvent) {
    const was = pointers.current.has(e.pointerId);
    pointers.current.delete(e.pointerId);
    if (pointers.current.size < 2) pinch.current = null;
    if (!pointers.current.size) setDragging(false);
    // A click on the map outside any block closes the opened one.
    if (was && moved.current < 4 && !(e.target as HTMLElement).closest('[data-node], .msites') && open) close();
  }

  function close() {
    setOpen(null);
    focusBlock(null);
    setK(1);
    if (viewRef.current) viewRef.current.scrollLeft = 0;
  }
  function toggle(id: string) {
    if (open === id) return close();
    setMeasured(null);
    setOpen(id);
    focusBlock(id);
  }

  useCommands([{ id: 'map-fit', label: mapLabels.fit, run: () => close() }], []);

  const regionLabel = data.bundle.sections?.tests?.state === 'running' ? mapLabels.running : mapLabels.declared;
  const W = l.width * k + 2 * PAD, H = l.height * k + 2 * PAD;

  return (
    <div className="map">
      <div ref={viewRef} className={dragging ? 'map-view dragging' : 'map-view'} style={{ height: H }} data-testid="map-view"
        aria-label={mapLabels.map} role="group" tabIndex={-1}
        onKeyDown={(e) => { if (e.key === 'Escape' && open) { e.stopPropagation(); close(); } }}
        onPointerDown={onPointerDown} onPointerMove={onPointerMove} onPointerUp={onPointerUp} onPointerCancel={onPointerUp}>
        <div className="map-sizer" style={{ width: W, height: H }}>
          <div className="map-layer" data-testid="map-layer" data-k={k}
            style={{ transform: `translate(${PAD}px, ${PAD}px) scale(${k})`, width: l.width, height: l.height }}>
            <svg className="map-svg" width={l.width} height={l.height}>
              {l.region && <rect className="map-region" data-testid="map-region" x={l.region.x} y={l.region.y} width={l.region.w} height={l.region.h} rx={10} />}
              {deps.map((e, i) => {
                const p = edgePath(l, e);
                return p && <path key={i} className="map-edge" d={p.d} data-edge={`${e.from}-${e.to}`} data-version={e.version} data-changed={String(e.changed)} />;
              })}
            </svg>
            {l.region && <span className="map-region-label" style={{ left: l.region.x + 16, top: 8 }}>{regionLabel}</span>}
            {data.blocks.map((b) => {
              const r = l.nodes[b.id];
              if (!r) return null;
              const parts = b.parts.map((id) => partsById.get(id)).filter((p): p is Part => !!p);
              const file = parts.map(partLoc).find(Boolean)?.file;
              const link = links.find((x) => x.block === b.id);
              const diffs = data.differences.filter((d) => d.block === b.id);
              const isOpen = open === b.id;
              const rules = ruleLabels(rulesOf(b.id));
              const g = reach.find((x) => x.block === b.id && x.callSites > 0);
              const sites = g?.sites ?? [];
              const cls = ['mnode', isOpen && 'open', highlight.blocks.has(b.id) && 'hl'].filter(Boolean).join(' ');
              return (
                <div key={b.id} className={cls} data-block={b.id} data-node={b.id} data-level={levelOf(data, b.id)} data-placement={link?.placement ?? 'neither'}
                  aria-current={selection.block === b.id} style={{ left: r.x, top: r.y, width: r.w, height: r.h }}>
                  <div className="mnode-body">
                    <button type="button" className="mnode-head" aria-expanded={isOpen} title={b.name} onClick={() => toggle(b.id)}>
                      <LevelGlyph level={levelOf(data, b.id)} />
                      <span className="mnode-name">{b.name}</span>
                    </button>
                    <div className="mnode-meta">
                      {diffs.map((d) => <DiffNumber key={d.id} id={d.id} />)}
                      {link?.state === 'not-linked' && <span className="mnode-tag">{mapLabels.notLinked}</span>}
                      {g && (
                        <span className="msites" data-sites={b.id}>
                          <button type="button" className="ghost" aria-expanded={sitesOpen === b.id} disabled={!sites.length}
                            onClick={() => setSitesOpen(sitesOpen === b.id ? null : b.id)}>
                            {mapLabels.sites(g.callSites)}
                          </button>
                        </span>
                      )}
                    </div>
                    {!isOpen && file && <span className="mnode-file">{basename(file)}</span>}
                    {g && sitesOpen === b.id && (
                      <div className="msite-list" data-sites-list={b.id}>
                        {sites.map((s) => <Loc key={`${s.file}:${s.line}`} file={s.file} line={s.line} side="new" />)}
                      </div>
                    )}
                    {isOpen && rules.length > 0 && <div className="mnode-rules">{rules.join(' · ')}</div>}
                    {isOpen && parts.map((p) => {
                      const loc = partLoc(p);
                      return (
                        <div key={p.id} className="mpart" data-part={p.id}>
                          <span className="mpart-name">{partName(p)}</span>
                          {p.largeComment && <span className="mnode-tag">{mapLabels.largeComment}</span>}
                          {loc && <Loc file={loc.file} line={loc.line} side={loc.side} />}
                        </div>
                      );
                    })}
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </div>
      {groups.length > 0 && (
        <div className="map-groups">
          {groups.map((g) => (
            <div key={g.id} className="map-group" data-group={g.id}>
              <button type="button" className="ghost" aria-expanded={groupOpen === g.id} onClick={() => setGroupOpen(groupOpen === g.id ? null : g.id)}>
                {mapLabels.rule[g.rule](g.count)}
              </button>
              {groupOpen === g.id && (
                <div className="map-group-parts">
                  {g.parts.map((id) => {
                    const p = partsById.get(id);
                    const loc = p && partLoc(p);
                    return loc ? <Loc key={id} file={loc.file} line={loc.line} side={loc.side} /> : null;
                  })}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
