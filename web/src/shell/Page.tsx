import { useEffect, useMemo, useState } from 'react';
import { useShell, useCommands, type Command, type Slot } from './context';
import { registered, slotState, type SectionModule } from './registry';
import { labels, slotTitles } from './labels';
import { basename, claimHasTitle, type SectionState } from './data';
import { CodePanel } from './CodePanel';
import { FilesIndex } from './FilesIndex';
import { Sheet } from './parts';
import { Prose } from './prose';
import { phoneMax } from './tokens';
import { HowSheet, howTitle, type HowTopic } from './How';

const sectionKeys: Partial<Record<Slot, string>> = { proof: 'j', playground: 't', map: 'm' };

export type ThemeChoice = 'light' | 'dark' | 'system';
export function applyTheme(t: ThemeChoice) {
  if (t === 'system') delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = t;
}

function useNarrow(): boolean {
  const q = `(max-width: ${phoneMax}px)`;
  const get = () => typeof window.matchMedia === 'function' ? window.matchMedia(q).matches : window.innerWidth <= phoneMax;
  const [narrow, setNarrow] = useState(get);
  useEffect(() => {
    if (typeof window.matchMedia !== 'function') return;
    const m = window.matchMedia(q);
    const on = () => setNarrow(m.matches);
    m.addEventListener?.('change', on);
    return () => m.removeEventListener?.('change', on);
  }, [q]);
  return narrow;
}

function typing(t: EventTarget | null) {
  const el = t as HTMLElement | null;
  return !!el && (el.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(el.tagName));
}

const howTopics: Record<Slot, HowTopic> = { review: 'ai', proof: 'compare', playground: 'replay', map: 'map', decide: 'signoff', signoff: 'signoff' };

function SectionSlot({ slot, mod, onHow }: { slot: Slot; mod?: SectionModule; onHow: (t: HowTopic) => void }) {
  const { data, folded, setFolded } = useShell();
  const state: SectionState = data.bundle.sections?.[slotState[slot]] ?? { state: 'not-run' };
  const isFolded = folded[slot];
  const headline = mod?.headline?.(data);
  const id = `slot-${slot}`;
  return (
    <section id={id} className={`slot slot-${slot}`} aria-labelledby={`${id}-title`} data-slot={slot} data-folded={isFolded || undefined}>
      <h2 className="slot-title" id={`${id}-title`}>
        <button type="button" className="slot-toggle" aria-expanded={!isFolded} onClick={() => setFolded(slot, !isFolded)}>
          <span>{slotTitles[slot]}</span>
          <span className="chev" aria-hidden="true" />
        </button>
        {mod?.Headline ? <span className="slot-headline num"><mod.Headline data={data} state={state} /></span>
          : headline && <span className="slot-headline num">{headline}</span>}
        {state.state !== 'ready' && <span className="slot-state">{state.state.replace('-', ' ')}</span>}
        {!isFolded && mod?.Tools && <span className="slot-tools"><mod.Tools data={data} state={state} /></span>}
        {!isFolded && slot !== 'signoff' && slot !== 'review' && (
          <button type="button" className="ghost slot-how" data-how-open={howTopics[slot]} aria-haspopup="dialog" onClick={() => onHow(howTopics[slot])}>{howTitle}</button>
        )}
      </h2>
      {mod?.Always && <mod.Always data={data} state={state} />}
      {!isFolded && mod && <div className="slot-body"><mod.Component data={data} state={state} /></div>}
    </section>
  );
}

/** One scrolling page (D7), docked code panel (D13), palette and keys (D14). */
export function Page({ sections = registered }: { sections?: Partial<Record<Slot, SectionModule>> }) {
  const shell = useShell();
  const { data, code, closeCode, highlight, highlightFile, setFolded, commands } = shell;
  const narrow = useNarrow();
  const [overlay, setOverlay] = useState<null | 'palette' | 'keys' | 'requirement' | 'files' | 'how'>(null);
  const [topic, setTopic] = useState<HowTopic | null>(null);
  const how = (t: HowTopic | null) => { setTopic(t); setOverlay('how'); };
  const req = data.bundle.requirement;
  const tests = ((data.records.tests as { tests?: unknown[] } | null)?.tests ?? []).length;

  const base: Command[] = useMemo(() => [
    { id: 'shell.files', label: labels.openFiles, key: 'f', run: () => setOverlay('files') },
    { id: 'shell.requirement', label: labels.requirement, key: 'r', run: () => setOverlay('requirement') },
    { id: 'shell.keys', label: labels.keys, key: '?', run: () => setOverlay('keys') },
    { id: 'shell.how', label: howTitle, key: 'h', run: () => { setTopic(null); setOverlay('how'); } },
    { id: 'shell.theme.light', label: labels.themeLight, run: () => applyTheme('light') },
    { id: 'shell.theme.dark', label: labels.themeDark, run: () => applyTheme('dark') },
    { id: 'shell.theme.system', label: labels.themeSystem, run: () => applyTheme('system') },
    ...(['proof', 'playground', 'map'] as Slot[]).map((s) => ({
      id: `shell.go.${s}`, label: slotTitles[s], key: sectionKeys[s], run: () => {
        setFolded(s, false);
        const el = document.getElementById(`slot-${s}`);
        el?.scrollIntoView?.({ block: 'start' });
        el?.querySelector<HTMLElement>('.slot-toggle')?.focus();
      },
    })),
  ], [setFolded]);
  useCommands(base, [base]);

  // A section may ask for the explanation at one of its topics (`how:status:<n>`).
  useEffect(() => {
    const m = /^how:([a-z]+)/.exec(shell.reveal ?? '');
    if (m) how(m[1] as HowTopic);
  }, [shell.reveal]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') { e.preventDefault(); setOverlay((o) => o === 'palette' ? null : 'palette'); return; }
      if (e.key === 'Escape') {
        if (overlay) setOverlay(null);
        else if (code) closeCode();
        else if (highlight.file) highlightFile(null);
        return;
      }
      if (overlay || e.ctrlKey || e.metaKey || e.altKey || typing(e.target)) return;
      const c = commands.find((x) => x.key === e.key);
      if (c) { e.preventDefault(); c.run(); }
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [commands, overlay, code, closeCode, highlight.file, highlightFile]);

  return (
    <div className={code && !narrow ? 'layout docked' : 'layout'}>
      <div className="main">
        <header className="pagehead">
          <a className="home" href="#/" aria-label={labels.product} title={labels.bundles}>
            <img src="/witness-icon-flat-indigo.svg" alt="" width={24} height={24} />
          </a>
          {req ? (
            <button type="button" className="req" onClick={() => setOverlay('requirement')} aria-haspopup="dialog">{req.id}</button>
          ) : null}
          {req?.title && !claimHasTitle(data) && <span className="req-title">{req.title}</span>}
          <span className="spacer" />
          {highlight.file && (
            <button type="button" className="ghost chip" onClick={() => highlightFile(null)} aria-label={`${highlight.file} ×`}>
              {basename(highlight.file)} ×
            </button>
          )}
          <button type="button" className="ghost" data-how-open="all" aria-haspopup="dialog" onClick={() => how(null)}>{howTitle}</button>
          <div className="files-anchor">
            <button type="button" className="ghost" aria-haspopup="dialog" aria-expanded={overlay === 'files'}
              onClick={() => setOverlay(overlay === 'files' ? null : 'files')}>{labels.files(data.files.length)}</button>
            {overlay === 'files' && <FilesIndex onClose={() => setOverlay(null)} counts={shell.folderCounts} />}
          </div>
        </header>
        <main>
          <SectionSlot slot="review" mod={sections.review} onHow={how} />
          {/* The evidence as one row of plainly named doors; an opened door takes the full width below the row. */}
          <div className="doors" data-doors>
            <span className="doors-k" data-doors-label>
              <span className="door-title">{labels.evidence}</span><span>{labels.computed}</span>
            </span>
            <SectionSlot slot="proof" mod={sections.proof} onHow={how} />
            {tests > 0 && (
              <button type="button" className="door" data-door="tests"
                onClick={() => { setFolded('proof', false); shell.setReveal(`tests:all:${Date.now()}`); setTimeout(() => document.querySelector('[data-testid="proof-tests"]')?.scrollIntoView?.({ block: 'center' }), 60); }}>
                <span className="door-title">{labels.tests}</span><span className="num">{tests}</span>
              </button>
            )}
            <SectionSlot slot="playground" mod={sections.playground} onHow={how} />
            <SectionSlot slot="map" mod={sections.map} onHow={how} />
          </div>
          <SectionSlot slot="signoff" mod={sections.signoff} onHow={how} />
        </main>
      </div>
      {code && <CodePanel sheet={narrow} />}
      {overlay === 'requirement' && req && (
        <Sheet title={req.id} onClose={() => setOverlay(null)} wide>
          <div className="prose">{req.title && <p className="prose-title">{req.title}</p>}<Prose text={req.text} /></div>
        </Sheet>
      )}
      {overlay === 'how' && <HowSheet topic={topic} onClose={() => setOverlay(null)} />}
      {overlay === 'keys' && (
        <Sheet title={labels.keys} onClose={() => setOverlay(null)}>
          <table className="keys"><tbody>
            <tr><td className="kbd">Ctrl K</td><td>{labels.palette}</td></tr>
            {commands.filter((c) => c.key).map((c) => <tr key={c.id}><td className="kbd">{c.key}</td><td>{c.label}</td></tr>)}
            <tr><td className="kbd">Esc</td><td>{labels.close}</td></tr>
          </tbody></table>
        </Sheet>
      )}
      {overlay === 'palette' && <Palette onClose={() => setOverlay(null)} commands={commands} />}
    </div>
  );
}

function Palette({ commands, onClose }: { commands: Command[]; onClose: () => void }) {
  const [q, setQ] = useState('');
  const [i, setI] = useState(0);
  const hits = commands.filter((c) => c.label.toLowerCase().includes(q.toLowerCase()));
  const run = (c?: Command) => { if (!c) return; onClose(); c.run(); };
  return (
    <Sheet title={labels.palette} onClose={onClose}>
      <input className="palette-input" autoFocus aria-label={labels.palette} value={q}
        onChange={(e) => { setQ(e.target.value); setI(0); }}
        onKeyDown={(e) => {
          if (e.key === 'ArrowDown') { setI((x) => Math.min(x + 1, hits.length - 1)); e.preventDefault(); }
          if (e.key === 'ArrowUp') { setI((x) => Math.max(x - 1, 0)); e.preventDefault(); }
          if (e.key === 'Enter') run(hits[i]);
        }} />
      <ul className="palette-list" role="listbox" aria-label={labels.palette}>
        {hits.map((c, k) => (
          <li key={c.id} role="option" aria-selected={k === i} className={k === i ? 'on' : ''}
            onMouseDown={(e) => { e.preventDefault(); run(c); }}>
            <span>{c.label}</span>{c.key && <span className="kbd">{c.key}</span>}
          </li>
        ))}
      </ul>
    </Sheet>
  );
}
