import { useEffect, useRef } from 'react';
import { useShell } from './context';
import { basename, firstChange, type FileEntry } from './data';

function folderOf(path: string) {
  const i = path.lastIndexOf('/');
  return i < 0 ? '' : path.slice(0, i + 1);
}

/** Changed files grouped by folder, `+a −d` each (D13). `counts`: optional per-folder count (D44). */
export function FilesIndex({ onClose, counts }: { onClose: () => void; counts?: Record<string, number> | null }) {
  const { data, openCode, highlightFile, highlight } = useShell();
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => { ref.current?.querySelector<HTMLButtonElement>('button.file')?.focus(); }, []);
  const groups = new Map<string, FileEntry[]>();
  for (const f of [...data.files].sort((a, b) => a.file.localeCompare(b.file))) {
    const k = folderOf(f.file);
    groups.set(k, [...(groups.get(k) ?? []), f]);
  }
  // D44: `files[].history` from the gateway; `counts` (DataSource.folderCounts) wins where both exist.
  const merged: Record<string, number> = {};
  for (const f of data.files) {
    if (!f.history) continue;
    const k = f.history.folder.endsWith('/') || !f.history.folder ? f.history.folder : `${f.history.folder}/`;
    merged[k] = Math.max(merged[k] ?? 0, f.history.undeclared);
  }
  Object.assign(merged, counts ?? {});
  function choose(f: FileEntry) {
    const c = firstChange(data, f.file);
    highlightFile(f.file);
    openCode({ file: f.file, line: c.line, side: c.side });
    onClose();
  }
  return (
    <div className="popover files" ref={ref} role="dialog" aria-label="Files"
      onKeyDown={(e) => {
        if (e.key === 'Escape') { e.stopPropagation(); onClose(); }
        if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
          const all = [...(ref.current?.querySelectorAll<HTMLButtonElement>('button.file') ?? [])];
          const i = all.indexOf(document.activeElement as HTMLButtonElement);
          all[(i + (e.key === 'ArrowDown' ? 1 : all.length - 1)) % all.length]?.focus();
          e.preventDefault();
        }
      }}>
      {[...groups].map(([folder, files]) => (
        <div key={folder} className="folder">
          {folder && (
            <div className="folder-row" title={folder}>
              <span className="folder-name">{folder}</span>
              {(merged[folder] ?? 0) > 0 && (
                <span className="num folder-count" data-folder-count aria-label={`${merged[folder]} undeclared before`}>{merged[folder]}</span>
              )}
            </div>
          )}
          {files.map((f) => (
            <button key={f.file} type="button" className="file" aria-label={f.file} aria-pressed={highlight.file === f.file}
              onClick={() => choose(f)}>
              <span className="file-name">{basename(f.file)}</span>
              <span className="num dim">+{f.added} −{f.deleted}</span>
            </button>
          ))}
        </div>
      ))}
    </div>
  );
}
