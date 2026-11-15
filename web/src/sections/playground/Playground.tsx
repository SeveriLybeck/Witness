// Playground (D34-D38): every control and result comes from the plugin's describe reply.
import { useEffect, useMemo, useState } from 'react';
import { Loc, repoPaths, useShell } from '../../shell';
import { call, presetsFor, props, shownResults, stripPaths, viewersOf, type Preset, type Viewers, type Reply, type Result, type Side, type Slot, type Subject } from './protocol';
import { ResultView } from './Results';
import { pl } from './labels';

const css = `
.pg { display: grid; gap: var(--s-2); font-size: var(--fs-2); min-width: 0; max-width: 100%; }
.pg * { min-width: 0; box-sizing: border-box; }
.pg-row { display: flex; flex-wrap: wrap; gap: var(--s-1); align-items: center; }
.pg-slot { display: grid; gap: var(--s-1); }
.pg-slot textarea { font-family: var(--mono, monospace); font-size: var(--fs-1); min-height: calc(var(--s-4) * 4); width: 100%; }
.pg-side { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 16rem), 1fr)); gap: var(--s-2); }
.pg-pair { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 12rem), 1fr)); gap: var(--s-1); }
.pg pre { overflow: auto; font-size: var(--fs-1); margin: 0; max-width: 100%; max-height: calc(var(--s-4) * 12); }
.pg dd, .pg li { overflow-wrap: anywhere; }
.pg select { max-width: 100%; }
.pg-facts div { display: flex; gap: var(--s-1); } .pg-facts dd { margin: 0; }
.pg-page { width: 100%; min-height: calc(var(--s-4) * 10); border: 0; }
.pg [aria-pressed="true"] { outline: 2px solid var(--accent); }
`;

function fieldFor(slot: Slot, value: unknown, set: (v: unknown) => void) {
  const id = `pg-${slot.name}`;
  const label = <label htmlFor={id}>{slot.role} <code>{slot.type}</code></label>;
  if (!slot.constructible || !slot.field) {
    return <div className="pg-slot" key={slot.name} data-slot={slot.name} data-constructible="false">
      <span>{slot.role} <code>{slot.type}</code></span><span>{pl.notConstructible}</span></div>;
  }
  let input;
  if (slot.name === 'buildProperties' && (slot.field === 'text' || slot.field === 'string' || slot.field === 'code')) {
    input = <textarea id={id} spellCheck={false} value={props.show(value)} onChange={(e) => set(props.parse(e.target.value))} />;
    return <div className="pg-slot" key={slot.name} data-slot={slot.name} data-field={slot.field}>{label}{input}</div>;
  }
  switch (slot.field) {
    case 'code': case 'text':
      input = <textarea id={id} spellCheck={false} value={String(value ?? '')} onChange={(e) => set(e.target.value)} />; break;
    case 'bool':
      input = <input id={id} type="checkbox" checked={Boolean(value)} onChange={(e) => set(e.target.checked)} />; break;
    case 'int': case 'number':
      input = <input id={id} type="number" step={slot.field === 'int' ? 1 : 'any'} value={value == null ? '' : String(value)}
        onChange={(e) => set(e.target.value === '' ? null : Number(e.target.value))} />; break;
    case 'choice':
      input = <select id={id} value={String(value ?? '')} onChange={(e) => set(e.target.value)}>
        {(slot.choices ?? []).map((c) => <option key={c}>{c}</option>)}</select>; break;
    default:
      input = <input id={id} type="text" value={String(value ?? '')} onChange={(e) => set(e.target.value)} />;
  }
  return <div className="pg-slot" key={slot.name} data-slot={slot.name} data-field={slot.field}>{label}{input}</div>;
}

const sideLabel: Record<Side, string> = { old: pl.old, new: pl.new, reverted: pl.reverted };

// Reads everything through useShell; the section props are not needed.
export function Playground() {
  const { data, source, selection } = useShell();
  const [list, setList] = useState<Viewers | null>(null);
  const [viewer, setViewer] = useState<string | null>(null);
  const [subjects, setSubjects] = useState<Subject[] | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [subjectId, setSubjectId] = useState<string | null>(null);
  const [preset, setPreset] = useState<Preset | null>(null);
  const [inputs, setInputs] = useState<Record<string, unknown>>({});
  const [results, setResults] = useState<Result[] | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let live = true;
    setList(null); setViewer(null);
    viewersOf(source, data.key).then((v) => {
      if (!live) return;
      setList(v); setViewer(v.viewers[0]?.name ?? null);
    }, (e: Error) => live && setFailed(repoPaths(e.message, data)));
    return () => { live = false; };
  }, [source, data.key]);

  useEffect(() => {
    if (!viewer) return;
    let live = true;
    setSubjects(null); setFailed(null); setPreset(null); setInputs({}); setResults(null);
    call(source, data.key, viewer, { op: 'describe' }).then((r: Reply) => {
      if (!live) return;
      if (!r.ok) { setFailed(repoPaths(r.error ?? pl.error, data)); return; }
      const subs = stripPaths(r.subjects ?? [], data);
      setSubjects(subs); setSubjectId(subs[0]?.id ?? null);
    }, (e: Error) => live && setFailed(repoPaths(e.message, data)));
    return () => { live = false; };
  }, [source, data.key, viewer]);

  const subject = subjects?.find((s) => s.id === subjectId) ?? null;
  const presets = useMemo(() => (subject ? presetsFor(subject, data) : []), [subject, data]);

  // A revert reruns the current input against the reverted build, so it keeps what is loaded.
  const choose = (p: Preset | null) => {
    setPreset(p);
    setInputs((cur) => (!p ? {} : p.kind === 'revert' ? { ...cur, ...p.inputs } : { ...p.inputs }));
    setResults(null);
  };

  // Selecting a difference anywhere loads its replay input (D8).
  useEffect(() => {
    const p = presets.find((x) => x.kind === 'difference' && x.difference === selection.difference);
    if (p) choose(p);
  }, [selection.difference, presets]);

  const run = async () => {
    if (!subject || !viewer) return;
    setBusy(true); setFailed(null);
    try {
      const pre = preset ? { kind: preset.kind, ...(preset.difference ? { difference: preset.difference } : {}),
        ...(preset.block ? { block: preset.block } : {}) } : { kind: 'free' };
      const r = await call(source, data.key, viewer, { op: 'run', subject: subject.id, sides: ['old', 'new'], preset: pre, inputs });
      if (r.ok) setResults(shownResults(r.results ?? [], data)); else setFailed(repoPaths(r.error ?? pl.error, data));
    } catch (e) { setFailed(repoPaths((e as Error).message, data)); } finally { setBusy(false); }
  };

  const sides = useMemo(() => {
    const s: Side[] = ['old', 'new'];
    if (results?.some((r) => r.side === 'reverted')) s.push('reverted');
    return s;
  }, [results]);

  return (
    <div className="pg" data-playground>
      <style>{css}</style>
      {list?.tabs && (
        <div role="tablist" aria-label={pl.viewer} className="pg-row">
          {list.viewers.map((v) => <button key={v.name} type="button" role="tab" aria-selected={v.name === viewer}
            onClick={() => setViewer(v.name)}>{v.title}</button>)}
        </div>)}
      {failed && <div role="alert">{pl.error}: {failed}</div>}
      {list && !list.viewers.length && <div data-state="unavailable">{pl.unavailable}</div>}
      {subjects && !subjects.length && <div data-state="unavailable">{pl.unavailable}</div>}
      {subjects && subjects.length > 0 && (
        <label className="pg-row">{pl.subject}
          <select aria-label={pl.subject} value={subjectId ?? ''} onChange={(e) => { setSubjectId(e.target.value); choose(null); }}>
            {subjects.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
          </select>
        </label>)}
      {subject && <>
        <section aria-label={pl.definition} data-area="definition">
          <h3>{pl.definition}</h3>
          <p>{subject.definition.text}</p>
          <div className="pg-row">{subject.definition.locations.map((l) => <Loc key={`${l.file}:${l.line}`} file={l.file} line={l.line} side="new" />)}</div>
        </section>
        <div role="group" aria-label={pl.presets} className="pg-row">
          {presets.map((p) => (
            <button key={`${p.kind}:${p.label}`} type="button" data-kind={p.kind} data-preset={p.difference ?? (p.kind === 'issue' ? 'reproduction' : p.kind)}
              data-cite={p.difference ? `preset:${p.difference}` : p.kind === 'issue' ? 'reproduction' : undefined}
              data-source={p.source} data-reason={p.reason} title={p.detail}
              disabled={Boolean(p.reason)} aria-pressed={preset === p} onClick={() => choose(p)}>
              {p.label}{p.source && <small> {p.source}</small>}{p.reason && <small> {p.reason}</small>}
            </button>))}
        </div>
        <section aria-label={pl.inputs} data-area="inputs">
          <h3>{pl.inputs}</h3>
          {subject.inputs.map((s) => fieldFor(s, inputs[s.name], (v) => setInputs((o) => ({ ...o, [s.name]: v }))))}
          <button type="button" onClick={run} disabled={busy}>{busy ? pl.running : pl.run}</button>
        </section>
        <section aria-label={pl.outputs} data-area="outputs">
          <h3>{pl.outputs}</h3>
          <div className="pg-side">
            {sides.map((side) => (
              <div key={side} data-side={side}>
                <h4>{sideLabel[side]}</h4>
                {subject.outputs.map((o) => {
                  const r = results?.find((x) => x.output === o.name && x.side === side);
                  return (
                    <div key={o.name} data-output={o.name} data-type={o.type}>
                      <h5>{o.name} <code>{o.type}</code></h5>
                      {r ? <ResultView type={o.type} value={r.value} bundle={data.key} plugin={viewer ?? ''} /> : <span>{pl.none}</span>}
                    </div>);
                })}
              </div>))}
          </div>
        </section>
      </>}
    </div>
  );
}
