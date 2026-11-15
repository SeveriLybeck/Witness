// Small shared components for sections.
import { useEffect, useRef, type ReactNode } from 'react';
import { useShell } from './context';
import { basename } from './data';
import { labels } from './labels';

/** `File.cs:line`; full path in the accessible name; opens the code panel (D13). */
export function Loc({ file, line, side }: { file: string; line: number; side?: 'old' | 'new' }) {
  const { openCode } = useShell();
  return (
    <button type="button" className="loc" data-loc={`${file}:${line}`} aria-label={`${file}:${line}`} title={file}
      onClick={() => openCode({ file, line, side })}>
      <span className="loc-text">{basename(file)}:{line}</span>
    </button>
  );
}

/** Monochrome three-bar importance glyph (D12). */
export function LevelGlyph({ level }: { level: 1 | 2 | 3 }) {
  return (
    <span className="level" role="img" aria-label={`level ${level}`} data-level={level}>
      {[1, 2, 3].map((b) => <span key={b} className={b <= level ? 'bar on' : 'bar'} />)}
    </span>
  );
}

/** Outlined difference number; selecting it selects the difference everywhere (D8). */
export function DiffNumber({ id }: { id: string }) {
  const { data, selection, selectDifference } = useShell();
  const d = data.differences.find((x) => x.id === id);
  if (!d) return null;
  return (
    <button type="button" className="diffno" data-cite={`diff:${d.number}`} aria-pressed={selection.difference === id}
      onClick={() => selectDifference(id)}>
      {d.number}
    </button>
  );
}

/** Modal sheet with only a close control. Esc closes. */
export function Sheet({ title, onClose, children, wide }: { title: string; onClose: () => void; children: ReactNode; wide?: boolean }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const prev = document.activeElement as HTMLElement | null;
    ref.current?.focus();
    return () => prev?.focus?.();
  }, []);
  return (
    <div className="scrim" onMouseDown={(e) => { if (e.target === e.currentTarget) onClose(); }}>
      <div ref={ref} className={wide ? 'sheet wide' : 'sheet'} role="dialog" aria-modal="true" aria-label={title} tabIndex={-1}
        onKeyDown={(e) => { if (e.key === 'Escape') { e.stopPropagation(); onClose(); } }}>
        <div className="sheet-head">
          <h2>{title}</h2>
          <button type="button" className="ghost" onClick={onClose}>{labels.close}</button>
        </div>
        <div className="sheet-body">{children}</div>
      </div>
    </div>
  );
}
