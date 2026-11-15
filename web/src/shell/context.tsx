import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { blocksInFile, type BundleData, type DataSource } from './data';

export type Slot = 'review' | 'proof' | 'playground' | 'map' | 'decide' | 'signoff';
export const slots: Slot[] = ['review', 'proof', 'playground', 'map', 'decide', 'signoff'];

/** Shared selection (D8): a difference keeps its number in every view. */
export interface Selection {
  /** Difference id, e.g. `d2`. */
  difference: string | null;
  /** Block id with focus on the map, e.g. `b2`. Set from the difference when one is selected. */
  block: string | null;
}

export interface CodeTarget { file: string; line: number; side?: 'old' | 'new' }

export interface Command {
  id: string;
  label: string;
  /** Single key without modifiers, e.g. `f`; ignored while typing in a field. */
  key?: string;
  run: () => void;
}

export interface Shell {
  data: BundleData;
  source: DataSource;
  selection: Selection;
  selectDifference(id: string | null): void;
  focusBlock(id: string | null): void;
  /** File chosen in the files index and the blocks with a part in it. */
  highlight: { file: string | null; blocks: Set<string> };
  highlightFile(file: string | null): void;
  code: CodeTarget | null;
  openCode(target: CodeTarget): void;
  closeCode(): void;
  /** Folded sections: the three numbered sections and Decide start as one headline line each. */
  folded: Record<Slot, boolean>;
  setFolded(slot: Slot, folded: boolean): void;
  /** An item a citation asked to see, e.g. `test:<name>`, `tests:<category>`, `question:q3`; its owner unfolds what hides it. */
  reveal: string | null;
  setReveal(token: string | null): void;
  commands: Command[];
  /** Adds commands; returns a remover. Prefer the `useCommands` hook. */
  addCommands(list: Command[]): () => void;
  /** Part ids the code panel has shown, in order (input to D29 "opened"). */
  shownParts: string[];
  folderCounts: Record<string, number> | null;
}

/** Sections that start as one headline line; the review and Sign off start open. */
export const startsFolded: Slot[] = ['proof', 'playground', 'map', 'decide'];

const Ctx = createContext<Shell | null>(null);

export function ShellProvider({ data, source, children }: { data: BundleData; source: DataSource; children: ReactNode }) {
  const [selection, setSelection] = useState<Selection>({ difference: null, block: null });
  const [file, setFile] = useState<string | null>(null);
  const [code, setCode] = useState<CodeTarget | null>(null);
  const [folded, setFoldedState] = useState<Record<Slot, boolean>>(
    () => Object.fromEntries(slots.map((s) => [s, startsFolded.includes(s)])) as Record<Slot, boolean>,
  );
  const [reveal, setReveal] = useState<string | null>(null);
  const [commands, setCommands] = useState<Command[]>([]);
  const [shownParts, setShownParts] = useState<string[]>([]);
  const [folderCounts, setFolderCounts] = useState<Record<string, number> | null>(null);
  const seq = useRef(0);
  useEffect(() => {
    let live = true;
    source.folderCounts?.(data.key).then((c) => { if (live) setFolderCounts(c); }, () => undefined);
    return () => { live = false; };
  }, [source, data.key]);

  const selectDifference = useCallback((id: string | null) => {
    const d = id ? data.differences.find((x) => x.id === id) : null;
    setSelection((s) => ({ difference: id, block: d ? d.block : s.block }));
  }, [data]);
  const focusBlock = useCallback((id: string | null) => setSelection((s) => ({ ...s, block: id })), []);
  const highlight = useMemo(() => ({ file, blocks: file ? blocksInFile(data, file) : new Set<string>() }), [data, file]);
  const openCode = useCallback((t: CodeTarget) => {
    setCode(t);
    const side = t.side ?? 'new';
    const hit = data.parts.filter((p) => {
      const loc = p[side];
      return loc && loc.file === t.file && t.line >= loc.line && t.line <= loc.endLine;
    });
    if (hit.length) setShownParts((s) => [...s, ...hit.map((p) => p.id).filter((id) => !s.includes(id))]);
  }, [data]);
  const closeCode = useCallback(() => setCode(null), []);
  const setFolded = useCallback((slot: Slot, f: boolean) => setFoldedState((s) => ({ ...s, [slot]: f })), []);
  const addCommands = useCallback((list: Command[]) => {
    const tag = ++seq.current;
    const tagged = list.map((c) => Object.assign({}, c, { _tag: tag }));
    setCommands((cs) => [...cs.filter((c) => !list.some((n) => n.id === c.id)), ...tagged]);
    return () => setCommands((cs) => cs.filter((c) => (c as Command & { _tag?: number })._tag !== tag));
  }, []);

  const value: Shell = {
    data, source, selection, selectDifference, focusBlock, highlight, highlightFile: setFile,
    code, openCode, closeCode,
    folded, setFolded, reveal, setReveal,
    commands, addCommands, shownParts, folderCounts,
  };
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useShell(): Shell {
  const s = useContext(Ctx);
  if (!s) throw new Error('useShell outside ShellProvider');
  return s;
}

/** Registers commands for the palette and single-key shortcuts while the caller is mounted. */
export function useCommands(list: Command[], deps: unknown[] = []) {
  const { addCommands } = useShell();
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => addCommands(list), [addCommands, ...deps]);
}
