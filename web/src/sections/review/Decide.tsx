// Decisions (D40): one row per decision in the review's one list. Questions about the same behaviour, or about
// blocks of one file, are one decision whose answer goes to each of them; the single questions are one click further.
import { useEffect, useMemo, useState } from 'react';
import { DiffNumber, Loc, useShell } from '../../shell';
import { blockLoc, follow, questionGroups, questionsOf, testsOf, type Question, type QuestionGroup } from './cites';
import { L, questionText, ruleLabel } from './labels';
import { useSession } from './session';

type Value = 'accept' | 'return';

/** Accept or Return for one or several questions. Without a reviewer name it leads to the name field. */
export function Answer({ ids, name }: { ids: string[]; name: string }) {
  const shell = useShell();
  const { state, actions } = useSession(shell.data, shell.source);
  const answers = state.stale ? {} : state.session?.answers ?? {};
  const all = (v: Value) => ids.length > 0 && ids.every((id) => answers[id] === v);
  const waiting = !!state.reviewer && !state.session;
  async function set(v: Value) {
    if (!state.reviewer) {
      shell.setFolded('signoff', false);
      setTimeout(() => document.querySelector<HTMLElement>('#slot-signoff [data-reviewer]')?.focus(), 30);
      return;
    }
    for (const id of ids) await actions.answer(id, v);
  }
  const stale = state.stale && ids.some((id) => state.session?.answers[id]);
  return (
    <span className="rv-answer" role="group" aria-label={name}>
      <button type="button" className="ghost" aria-pressed={all('accept')} disabled={waiting} onClick={() => void set('accept')}>{L.accept}</button>
      <button type="button" className="ghost" aria-pressed={all('return')} disabled={waiting} onClick={() => void set('return')}>{L.return}</button>
      {stale && <span>{L.stale}</span>}
    </span>
  );
}

/** One question of a decision: its own words, difference number, location and answer. */
function MemberRow({ q }: { q: Question }) {
  const { data } = useShell();
  const b = q.block ? data.blocks.find((x) => x.id === q.block) : null;
  const loc = q.block ? blockLoc(data, q.block) : null;
  const diffs = q.differences.map((id) => data.differences.find((d) => d.id === id)).filter((d) => !!d).map((d) => ({ number: d!.number, kind: d!.kind }));
  const cats = [...new Set(testsOf(data).filter((t) => q.tests.includes(t.name)).map((t) => t.category))];
  const text = questionText(q.rule, { diffs, tests: q.tests.length, category: cats.length === 1 ? cats[0] : null, block: b?.name ?? null });
  return (
    <li className="rv-q rv-member" data-member={q.id}>
      <span className="rv-qtext">{text}</span>
      {q.differences.map((d) => <DiffNumber key={d} id={d} />)}
      {loc && <Loc file={loc.file} line={loc.line} side={loc.side} />}
      <span className="spacer" />
      <Answer ids={[q.id]} name={q.id} />
    </li>
  );
}

/** What `Evidence` opens for a decision: its first difference (with its replay), else its block, else its first test. */
export function evidenceOf(g: QuestionGroup): string | null {
  return g.differences[0] ?? g.blocks[0] ?? g.questions.flatMap((q) => q.tests)[0] ?? null;
}

export function DecisionRow({ g, line }: { g: QuestionGroup; line: string }) {
  const shell = useShell();
  const { data, reveal } = shell;
  const { state } = useSession(data, shell.source);
  const [open, setOpen] = useState(false);
  const ids = g.questions.map((q) => q.id);
  const many = ids.length > 1;
  useEffect(() => {
    if (many && reveal && reveal.startsWith('question:') && ids.includes(reveal.slice(9))) setOpen(true);
  }, [reveal, many, ids.join()]); // eslint-disable-line react-hooks/exhaustive-deps
  const answers = state.stale ? {} : state.session?.answers ?? {};
  const done = ids.filter((id) => answers[id]).length;
  const why = ruleLabel(g.rule);
  const evidence = evidenceOf(g);
  const body = <span className="rv-qtext" title={line}>{line}</span>;
  return (
    <>
      <li className="rv-q" data-question={ids[0]} data-questions={ids.join(' ')} data-group={g.id}>
        {many
          ? <button type="button" className="rv-qhead" aria-expanded={open} onClick={() => setOpen(!open)}><span className="chev" aria-hidden="true" />{body}</button>
          : <span className="rv-qhead">{body}</span>}
        <span className="rv-qev">
          {why && g.behaviour && <span className="rv-rule">{why}</span>}
          {g.loc && !line.endsWith(`:${g.loc.line}`) && <Loc file={g.loc.file} line={g.loc.line} side={g.loc.side} />}
        </span>
        <span className="spacer" />
        {many && done > 0 && done < ids.length && <span className="num">{L.answered(done, ids.length)}</span>}
        {evidence && <button type="button" className="ghost" data-evidence={evidence} onClick={() => follow(evidence, shell)}>{L.evidence}</button>}
        <Answer ids={ids} name={ids.join(' ')} />
      </li>
      {open && g.questions.map((q) => <MemberRow key={q.id} q={q} />)}
    </>
  );
}

/** Decisions with a question still unanswered (all of them when the head moved). */
export function openDecisions(groups: QuestionGroup[], answers: Record<string, string> | undefined, stale: boolean): number {
  return groups.filter((g) => stale || g.questions.some((q) => !answers?.[q.id])).length;
}

export function useDecisions() {
  const { data, source } = useShell();
  const { state } = useSession(data, source);
  const groups = useMemo(() => questionGroups(data), [data]);
  return { groups, open: openDecisions(groups, state.session?.answers, state.stale), questions: questionsOf(data).length };
}
