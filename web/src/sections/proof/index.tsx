import { Fragment, useEffect, useMemo, useState, type ReactNode } from 'react';
import {
  useShell, Loc, DiffNumber, Sheet, basename, groupDifferences, groupOf, repoPaths, lostValue,
  type SectionModule, type SectionProps, type BundleData, type Difference, type DiffGroup,
} from '../../shell';
import css from './proof.css?raw';
import { kindLabels, checkLabels, categoryLabels, categoryOrder, proofLabels as L } from './labels';
import {
  scopeOf, leanRows, testsOf, testsFor, shortName, checkOf, notLeanCount, perIteration, beforeAfter, readableCondition,
  leanFacts, leanLoops, leanCoverage, notModelledOnce, leanChecked, results, failureLine, type Test, type LeanState,
} from './model';

export function headline(data: BundleData): string {
  // The section's title is "Behaviour changes": the headline leads with their count.
  const items = [String(data.differences.length)];
  // How much of the change was compared at all: never left for the reader to find.
  const s = scopeOf(data);
  if (s) items.push(L.modelled(s.modelledLines, s.changedLines.production ?? 0));
  const off = notLeanCount(data);
  if (off) items.push(L.notChecked(off));
  return items.join(' · ');
}

/** `old → new` of one difference on one line, with the value a replay lost when there is one. */
function changeOf(data: BundleData, d: Difference): string {
  const lost = lostValue(data, d.id);
  return `${lost ? `${L.lost(lost)} · ` : ''}${oneLine(d.old)} → ${oneLine(d.new)}`;
}

const leanWord = (s: string) => checkLabels[s as LeanState] ?? s;
const oneLine = (s: unknown) => String(s ?? '').replace(/\s+/g, ' ').trim();

/** How each difference was checked, the assumptions made, and the assumed facts per unit. One click from the scope line. */
export function LeanSheet({ data, onClose }: { data: BundleData; onClose: () => void }) {
  const facts = leanFacts(data);
  const loops = leanLoops(data);
  const lean = leanRows(data);
  const scope = scopeOf(data);
  const diffs = [...data.differences].sort((a, b) => a.number - b.number).filter((d) => d.checker === 'solver' || !d.checker);
  return (
    <Sheet title={L.checks} onClose={onClose} wide>
      <table className="proof-table proof-lean" data-testid="proof-lean">
        <tbody>
          {diffs.map((d) => {
            const files = [...new Set((d.rows ?? []).map((r) => lean.find((l) => l.row === r)?.file).filter(Boolean))] as string[];
            return (
              <tr key={d.id} data-lean-diff={d.id}>
                <td><DiffNumber id={d.id} /></td>
                <td>{checkLabels[checkOf(data, d)]}</td>
                <td className="loc">{files.map(basename).join(' ')}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
      {scope && scope.assumptions.length > 0 && (
        <ul className="proof-assume" data-testid="proof-assumptions">
          {scope.assumptions.map((a, i) => <li key={i}><span>{a.contract.replace(/-/g, ' ')}</span><span className="mono">{a.symbol}</span></li>)}
        </ul>
      )}
      {leanCoverage(data).map((c) => {
        const f = facts.find((x) => x.unit === c.unit)?.facts ?? [];
        const loop = loops.find((x) => x.unit === c.unit);
        return (
          <div key={c.unit} className="proof-unit" data-unit={c.unit}>
            <div className="proof-test-head">
              <span>{c.unit.replace(/#\d+$/, '')}</span>
              <span>{c.state === 'checked' || c.state === 'model-checked' ? L.cover : leanWord(c.state)}</span>
            </div>
            {loop && (
              <div className="proof-ba">
                <span className="k">{L.perIteration}</span><span>{leanWord(loop.perIteration)}</span>
                <span className="k">{L.anyLength}</span><span>{leanWord(loop.lifted)}</span>
              </div>
            )}
            {f.length > 0 && (
              <details>
                <summary>{L.facts(f.length)}</summary>
                <ul>{f.map((x) => <li key={x.key}>{x.text}</li>)}</ul>
              </details>
            )}
          </div>
        );
      })}
    </Sheet>
  );
}

export function ScopeLine({ data }: { data: BundleData }) {
  const s = scopeOf(data);
  const [sheet, setSheet] = useState<null | 'checks' | 'ranges'>(null);
  if (!s) return null;
  const changed = s.changedLines.production ?? 0;
  const ranges = notModelledOnce(s);
  const items: ReactNode[] = [
    <span key="m" className="num">{L.modelled(s.modelledLines, changed)}</span>,
    <span key="a" className="num">{L.assumptions(s.assumptions.length)}</span>,
  ];
  if (ranges.length > 0 && ranges.length <= 2) {
    items.push(...ranges.map((r) => (
      <span key={`${r.location.file}:${r.location.line}`} title={r.reason}>
        {L.notModelled} <Loc file={r.location.file} line={r.location.line} />
      </span>
    )));
  } else if (ranges.length > 2) {
    items.push(<button key="r" type="button" className="ghost num" data-ranges onClick={() => setSheet('ranges')}>{L.notModelledN(ranges.length)}</button>);
  }
  if (data.records.lean != null) {
    items.push(<button key="l" type="button" className="ghost" data-checks onClick={() => setSheet('checks')}>{L.checks}</button>);
  }
  return (
    <div className="proof-scope" data-testid="proof-scope">
      {items.map((it, i) => <Fragment key={i}>{i > 0 && <span className="sep">·</span>}{it}</Fragment>)}
      {sheet === 'checks' && <LeanSheet data={data} onClose={() => setSheet(null)} />}
      {sheet === 'ranges' && (
        <Sheet title={L.notModelledN(ranges.length)} onClose={() => setSheet(null)}>
          <ul className="proof-ranges">
            {ranges.map((r) => (
              <li key={`${r.location.file}:${r.location.line}`}><Loc file={r.location.file} line={r.location.line} /><span>{r.reason}</span></li>
            ))}
          </ul>
        </Sheet>
      )}
    </div>
  );
}

/** Everything about one difference; the row keeps to one line. */
function DifferenceSheet({ data, d, onClose }: { data: BundleData; d: Difference; onClose: () => void }) {
  const tests = testsFor(data, d);
  const block = data.blocks.find((b) => b.id === d.block);
  const when = readableCondition(d) ?? d.condition;
  return (
    <Sheet title={L.difference(d.number)} onClose={onClose} wide>
      <dl className="proof-dl" data-testid="proof-detail">
        <dt>{L.kind}</dt><dd>{kindLabels[d.kind] ?? d.kind}</dd>
        <dt>{L.location}</dt><dd><Loc file={d.location.file} line={d.location.line} side="new" /></dd>
        {block && <><dt>{L.block}</dt><dd>{block.name}</dd></>}
        <dt>{L.when}</dt><dd><pre>{when}{perIteration(data, d) ? ` · ${L.perIteration}` : ''}</pre></dd>
        <dt>{L.old}</dt><dd><pre>{repoPaths(d.old, data)}</pre></dd>
        <dt>{L.new}</dt><dd><pre>{repoPaths(d.new, data)}</pre></dd>
        <dt>{L.tests}</dt><dd>{tests.length ? <ul>{tests.map((t) => <li key={t}>{shortName(t)}</li>)}</ul> : L.noTest}</dd>
        <dt>{L.check}</dt><dd>{checkLabels[checkOf(data, d)]}</dd>
      </dl>
    </Sheet>
  );
}

function MemberRow({ data, d, lead, onDetail }: { data: BundleData; d: Difference; lead: 'single' | 'member'; onDetail: (d: Difference) => void }) {
  const { selection, selectDifference } = useShell();
  const tests = testsFor(data, d);
  const check = checkOf(data, d);
  const when = oneLine(readableCondition(d) ?? '');
  const change = changeOf(data, d);
  return (
    <tr className={lead === 'member' ? 'proof-member' : 'proof-single'} data-diff={d.id} data-row={d.id} data-cite={`diff:${d.number}`}
      aria-selected={selection.difference === d.id}
      onClick={(e) => { if (!(e.target as HTMLElement).closest('button')) selectDifference(d.id); }}>
      <td className="c-lead"><DiffNumber id={d.id} />{lead === 'single' && <span className="proof-label">{kindLabels[d.kind] ?? d.kind}</span>}</td>
      <td className="proof-change clip" title={when ? `${change}\n${L.when} ${when}` : change}>{change}</td>
      <td className="c-loc"><Loc file={d.location.file} line={d.location.line} side="new" /></td>
      <td className="c-test clip" data-check={check} title={tests.map(shortName).join('\n')}>
        {!leanChecked(check) && check === 'not-checked' ? L.unchecked : tests.length ? L.testsN(tests.length) : L.noTest}
      </td>
      <td className="c-more"><button type="button" className="ghost" data-detail={d.id} onClick={() => onDetail(d)}>{L.details}</button></td>
    </tr>
  );
}

const SHOWN_NUMBERS = 5;

function GroupRow({ data, g, open, onToggle }: { data: BundleData; g: DiffGroup; open: boolean; onToggle: () => void }) {
  const { selection } = useShell();
  const locs = [...new Map(g.members.map((m) => [`${m.location.file}:${m.location.line}`, m.location])).values()];
  const tests = new Set(g.members.flatMap((m) => testsFor(data, m)));
  const off = g.members.filter((m) => checkOf(data, m) === 'not-checked').length;
  const lost = g.members.map((m) => lostValue(data, m.id)).find(Boolean);
  return (
    <tr className="proof-group" data-group={g.id} data-diff={g.members[0].id} data-members={g.members.map((m) => m.id).join(' ')}
      aria-selected={g.members.some((m) => m.id === selection.difference)}
      onClick={(e) => { if (!(e.target as HTMLElement).closest('button')) onToggle(); }}>
      <td className="c-lead">
        <button type="button" className="proof-toggle" aria-expanded={open} onClick={onToggle}>
          <span className="chev" aria-hidden="true" /><span className="proof-label num">{g.label}</span>
        </button>
      </td>
      <td className="proof-change clip" title={g.pattern}>{lost ? `${L.lost(lost)} · ` : ''}{g.pattern}</td>
      <td className="c-loc" data-locs={locs.length} title={locs.map((l) => `${basename(l.file)}:${l.line}`).join('\n')}><Loc file={locs[0].file} line={locs[0].line} side="new" /></td>
      <td className="proof-nums" colSpan={2} title={`${tests.size ? L.testsN(tests.size) : L.noTest}${off ? ` · ${L.notChecked(off)}` : ''}`}>
        {g.members.slice(0, SHOWN_NUMBERS).map((m) => <DiffNumber key={m.id} id={m.id} />)}
        {g.members.length > SHOWN_NUMBERS && <span className="num">{L.more(g.members.length - SHOWN_NUMBERS)}</span>}
      </td>
    </tr>
  );
}

/** Differences as behaviours: one row per group, members on demand (also when one of them is selected). */
export function DifferenceRows({ data }: { data: BundleData }) {
  const { selection, selectDifference } = useShell();
  const groups = useMemo(() => groupDifferences(data.differences), [data]);
  const [open, setOpen] = useState<Set<string>>(new Set());
  const [detail, setDetail] = useState<Difference | null>(null);
  const selected = groupOf(groups, selection.difference)?.id;
  useEffect(() => {
    if (selected) setOpen((s) => (s.has(selected) ? s : new Set(s).add(selected)));
  }, [selected, selection.difference]);
  if (!data.differences.length) return <div>{L.identical}</div>;
  function toggle(g: DiffGroup) {
    const was = open.has(g.id);
    setOpen((s) => { const n = new Set(s); if (was) n.delete(g.id); else n.add(g.id); return n; });
    if (!was && !g.members.some((m) => m.id === selection.difference)) selectDifference(g.members[0].id);
  }
  return (
    <>
      <table className="proof-table" data-testid="proof-differences">
        <colgroup>
          <col className="c-lead" /><col /><col className="c-loc" /><col className="c-test" /><col className="c-more" />
        </colgroup>
        <tbody>
          {groups.map((g) => (g.members.length === 1
            ? <MemberRow key={g.id} data={data} d={g.members[0]} lead="single" onDetail={setDetail} />
            : (
              <Fragment key={g.id}>
                <GroupRow data={data} g={g} open={open.has(g.id)} onToggle={() => toggle(g)} />
                {open.has(g.id) && g.members.map((m) => <MemberRow key={m.id} data={data} d={m} lead="member" onDetail={setDetail} />)}
              </Fragment>
            )))}
        </tbody>
      </table>
      {detail && <DifferenceSheet data={data} d={detail} onClose={() => setDetail(null)} />}
    </>
  );
}

const clean = (s: string, data: BundleData) => repoPaths(s.replace(/\r/g, ''), data);

function TestSheet({ data, t, onClose }: { data: BundleData; t: Test; onClose: () => void }) {
  const { before, after } = beforeAfter(t);
  const line = failureLine(t);
  return (
    <Sheet title={shortName(t.name)} onClose={onClose} wide>
      {line && <p className="proof-summary" data-testid="proof-test-summary">{clean(line, data)}</p>}
      <div className="proof-ba" data-testid="proof-test-output">
        {before != null && <><span className="k">{L.before}</span><pre>{clean(before, data)}</pre></>}
        {after != null && <><span className="k">{L.after}</span><pre>{clean(after, data)}</pre></>}
      </div>
    </Sheet>
  );
}

function TestEntry({ t, onOutput }: { t: Test; onOutput: (t: Test) => void }) {
  const { before, after } = beforeAfter(t);
  return (
    <div className="proof-test" data-test={t.name} data-cite={`test:${t.name}`}>
      <span className="proof-test-name clip" title={t.name}>{shortName(t.name)}</span>
      <span className="proof-result" title={failureLine(t) ?? undefined}>{results(t)}</span>
      <Loc file={t.location.file} line={t.location.line} side="new" />
      {(before != null || after != null)
        ? <button type="button" className="ghost" data-output onClick={() => onOutput(t)}>{L.output}</button> : <span />}
    </div>
  );
}

/** Tests: a line of counts per category; the list has one line per test, its output one click further. */
export function TestsList({ data }: { data: BundleData }) {
  const { reveal } = useShell();
  const tests = testsOf(data);
  const st = data.bundle.sections?.tests?.state;
  const [open, setOpen] = useState(false);
  const [shown, setShown] = useState<Test | null>(null);
  useEffect(() => { if (reveal && /^tests?:/.test(reveal)) setOpen(true); }, [reveal]);
  if (!tests.length && (!st || st === 'ready')) return null;
  const cats = [...categoryOrder, ...new Set(tests.map((t) => t.category).filter((c) => !categoryOrder.includes(c)))]
    .filter((c) => tests.some((t) => t.category === c));
  return (
    <div className="proof-tests" data-testid="proof-tests">
      <button type="button" className="proof-toggle proof-tests-line" aria-expanded={open} onClick={() => setOpen(!open)}>
        <span className="chev" aria-hidden="true" />
        <span className="proof-label">{L.tests}</span>
        <span className="num">{cats.map((c) => `${tests.filter((t) => t.category === c).length} ${(categoryLabels[c] ?? c).toLowerCase()}`).join(' · ')}</span>
        {st && st !== 'ready' && <span>{L.state[st] ?? st}</span>}
      </button>
      {open && cats.map((c) => {
        const group = tests.filter((t) => t.category === c);
        return (
          <section key={c} data-category={c} data-cite={`tests:${c}`}>
            <h3>{categoryLabels[c] ?? c} <span className="num">{group.length}</span></h3>
            {group.map((t) => <TestEntry key={t.name} t={t} onOutput={setShown} />)}
          </section>
        );
      })}
      {shown && <TestSheet data={data} t={shown} onClose={() => setShown(null)} />}
    </div>
  );
}

function useProofStyles() {
  useEffect(() => {
    if (document.getElementById('witness-proof-css')) return;
    const el = document.createElement('style');
    el.id = 'witness-proof-css';
    el.textContent = css;
    document.head.appendChild(el);
  }, []);
}

export function Proof({ data }: SectionProps) {
  useProofStyles();
  return (
    <div className="proof">
      <ScopeLine data={data} />
      <DifferenceRows data={data} />
      <TestsList data={data} />
    </div>
  );
}

export const sections: SectionModule[] = [{ slot: 'proof', Component: Proof, headline }];
