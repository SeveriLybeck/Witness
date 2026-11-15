import { useEffect, useMemo, useRef, useState } from 'react';
import { useShell } from './context';
import { basename, changedLines, type FileText } from './data';
import { commonIndent, enclosing, hunkWindow, rowOf, splitRows, unifiedRows, type DiffRow } from './code';
import { labels } from './labels';

/** Lines added above or below by one expand. */
const more = 20;
/** Lines of context around the changed hunk as opened. */
const context = 6;
/** Side by side is offered from this viewport width; the docked panel then takes 60 % of it. */
const splitMin = 1024;
const cache = new Map<string, Promise<FileText | null>>();

function useFile(commit: string, path: string): FileText | null | undefined {
  const { source } = useShell();
  const [text, setText] = useState<FileText | null | undefined>(undefined);
  useEffect(() => {
    let live = true;
    const k = `${commit}:${path}`;
    if (!cache.has(k)) cache.set(k, source.fileText(commit, path).catch(() => null));
    setText(undefined);
    cache.get(k)!.then((t) => { if (live) setText(t); });
    return () => { live = false; };
  }, [source, commit, path]);
  return text;
}

function useWide(): boolean {
  const q = `(min-width: ${splitMin}px)`;
  const get = () => (typeof window.matchMedia === 'function' ? window.matchMedia(q).matches : window.innerWidth >= splitMin);
  const [wide, setWide] = useState(get);
  useEffect(() => {
    if (typeof window.matchMedia !== 'function') return;
    const m = window.matchMedia(q);
    const on = () => setWide(m.matches);
    m.addEventListener?.('change', on);
    return () => m.removeEventListener?.('change', on);
  }, [q]);
  return wide;
}

const mark = { ctx: ' ', del: '−', add: '+' } as const;

function Row({ r, target, cols }: { r: DiffRow; target: boolean; cols: 'both' | 'old' | 'new' }) {
  const cls = ['row', r.kind === 'ctx' ? '' : r.kind, target ? 'target' : ''].join(' ').replace(/\s+/g, ' ').trim();
  return (
    <div className={cls} role="row" data-old={r.old ?? undefined} data-new={r.new ?? undefined} data-line={(cols === 'old' ? r.old : r.new ?? r.old) ?? undefined}
      data-changed={r.kind !== 'ctx' || undefined}>
      {cols !== 'new' && <span className="ln" role="cell">{r.old ?? ''}</span>}
      {cols !== 'old' && <span className="ln" role="cell">{r.new ?? ''}</span>}
      <span className="mk" aria-hidden="true">{mark[r.kind]}</span>
      <span className="tx" role="cell">{r.text || ' '}</span>
    </div>
  );
}

const Blank = () => <div className="row blank" aria-hidden="true"><span className="ln" /><span className="mk" /><span className="tx"> </span></div>;

/** Docked, non-modal code panel (D13) with one diff view: unified, or side by side when the page is wide. */
export function CodePanel({ sheet }: { sheet: boolean }) {
  const { data, code, closeCode } = useShell();
  const target = code!;
  const side = target.side ?? 'new';
  const oldText = useFile(data.bundle.oldCommit, target.file);
  const newText = useFile(data.bundle.newCommit, target.file);
  const wide = useWide() && !sheet;
  const [view, setView] = useState<'unified' | 'split'>('unified');
  const [copied, setCopied] = useState(false);
  const bodyRef = useRef<HTMLDivElement>(null);

  const { added, deleted } = useMemo(() => changedLines(data, target.file), [data, target.file]);
  const loading = oldText === undefined || newText === undefined;
  const rows = useMemo(
    () => (loading ? [] : unifiedRows(oldText?.lines ?? [], newText?.lines ?? [], deleted, added)),
    [loading, oldText, newText, added, deleted],
  );
  const at = rows.length ? rowOf(rows, side, target.line) : -1;
  const initial = useMemo(() => hunkWindow(rows, Math.max(0, at), context), [rows, at]);
  const [range, setRange] = useState(initial);
  useEffect(() => { setRange(initial); setCopied(false); }, [initial]);

  const lines = (side === 'new' ? newText : oldText)?.lines ?? (newText ?? oldText)?.lines ?? [];
  const where = lines.length ? enclosing(lines, target.line) : { types: [], member: null };
  const slash = target.file.lastIndexOf('/');
  const folder = slash > 0 ? target.file.slice(0, slash + 1) : '';
  const split = wide && view === 'split';

  useEffect(() => {
    bodyRef.current?.querySelector('.target')?.scrollIntoView?.({ block: 'center' });
  }, [rows, at, split]);

  // The indentation every shown line shares is left out, so deeply nested code starts at the line numbers.
  const cut = useMemo(() => commonIndent(rows.slice(range.from, range.to + 1).map((r) => r.text)), [rows, range]);
  const trim = (r: DiffRow): DiffRow => (cut ? { ...r, text: r.text.slice(cut) } : r);
  const shown = rows.slice(range.from, range.to + 1).map(trim);
  const pairs = split ? splitRows(rows).filter((p) => p.index >= range.from && p.index <= range.to)
    .map((p) => ({ ...p, left: p.left && trim(p.left), right: p.right && trim(p.right), at: p.left === rows[at] || p.right === rows[at] })) : [];
  const neither = !loading && oldText === null && newText === null;
  return (
    <aside className={sheet ? 'codepanel sheet-mode' : 'codepanel'} aria-label={labels.code} data-layout={sheet ? 'sheet' : 'dock'}
      data-view={split ? 'split' : 'unified'}
      onKeyDown={(e) => { if (e.key === 'Escape') { e.stopPropagation(); closeCode(); } }}>
      <div className="cp-head">
        <nav className="crumbs" aria-label="Breadcrumb">
          {folder && <span className="crumb folder" title={folder}>{folder}</span>}
          <span className="crumb">{basename(target.file)}</span>
          {where.types.map((t) => <span key={t} className="crumb">{t}</span>)}
          {where.member && <span className="crumb">{where.member}</span>}
        </nav>
        <div className="cp-tools">
          {wide && (
            <div className="seg" role="group" aria-label={labels.diffView}>
              <button type="button" aria-pressed={!split} onClick={() => setView('unified')}>{labels.unified}</button>
              <button type="button" aria-pressed={split} onClick={() => setView('split')}>{labels.split}</button>
            </div>
          )}
          {!loading && oldText === null && newText !== null && <span>{labels.notInOld}</span>}
          {!loading && newText === null && oldText !== null && <span>{labels.notInNew}</span>}
          <button type="button" className="ghost" onClick={() => {
            void navigator.clipboard?.writeText(target.file).catch(() => undefined);
            setCopied(true);
          }}>{copied ? labels.copied : labels.copyPath}</button>
          <button type="button" className="ghost" aria-label={labels.closeCode} onClick={closeCode}>×</button>
        </div>
      </div>
      <div className="cp-body" ref={bodyRef}>
        {loading && <div className="cp-note">{labels.loading}</div>}
        {neither && <div className="cp-note">{labels.noFile}</div>}
        {rows.length > 0 && (
          <>
            {range.from > 0 && <button type="button" className="ghost expand" onClick={() => setRange((r) => ({ ...r, from: Math.max(0, r.from - more) }))}>{labels.expandUp}</button>}
            {split ? (
              <div className="code split" role="table" aria-label={`${basename(target.file)} diff`}>
                {(['left', 'right'] as const).map((s) => (
                  <div key={s} className="code-side" data-side={s === 'left' ? 'old' : 'new'}>
                    <div className="code-rows">
                      {pairs.map((p) => {
                        const r = p[s];
                        const own = s === 'left' ? rows[at]?.kind !== 'add' : rows[at]?.kind !== 'del';
                        return r ? <Row key={p.index + s} r={r} target={p.at && own} cols={s === 'left' ? 'old' : 'new'} /> : <Blank key={p.index + s} />;
                      })}
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <div className="code unified" role="table" aria-label={`${basename(target.file)} diff`}>
                <div className="code-rows">
                  {shown.map((r, k) => <Row key={range.from + k} r={r} target={range.from + k === at} cols="both" />)}
                </div>
              </div>
            )}
            {range.to < rows.length - 1 && <button type="button" className="ghost expand" onClick={() => setRange((r) => ({ ...r, to: Math.min(rows.length - 1, r.to + more) }))}>{labels.expandDown}</button>}
          </>
        )}
      </div>
    </aside>
  );
}
