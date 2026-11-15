// The AI summary: the claim, what was asked for and what was not, then two groups, what matches and what needs the
// reader (most serious first, with Accept / Return and one way to the evidence). The status beside the title is
// computed by fixed rules (status.ts). The AI's statements in full, their citations, what was read and what was
// dropped are one click away in `Full text`.
import { useEffect, useMemo, useRef, useState } from 'react';
import { LevelGlyph, Loc, Prose, Sheet, basename, claimOf, levelOf, useCommands, useShell, type BundleData, type SectionProps } from '../../shell';
import { blockLoc, follow, readReview, resolve, reviewItems, shortLine, target, testsOf, type Review, type Statement } from './cites';
import { factsOf } from './status';
import { headingKeys, headings, L } from './labels';
import { DecisionRow, useDecisions } from './Decide';

/** Lines of the list shown as loaded; the rest opens in place. */
export const SHOWN_ITEMS = 4;

function Chip({ cite, data, onFollow }: { cite: string; data: BundleData; onFollow?: () => void }) {
  const shell = useShell();
  const c = resolve(cite, data);
  if (!c) return <span className="rv-chip" data-failed>{cite}</span>;
  let body: React.ReactNode;
  let title: string | undefined;
  switch (c.kind) {
    case 'difference': body = <span className="num">{c.number}</span>; break;
    case 'block': {
      const b = data.blocks.find((x) => x.id === c.id)!;
      body = <><LevelGlyph level={levelOf(data, c.id)} /> {b.name}</>;
      title = b.name;
      break;
    }
    case 'question': body = L.question(c.id); break;
    case 'part': {
      const p = data.parts.find((x) => x.id === c.id)!;
      const loc = p.new ?? p.old;
      body = loc ? `${basename(loc.file)}:${loc.line}` : c.id;
      break;
    }
    case 'test': body = <span className="rv-test">{c.name.slice(c.name.lastIndexOf('.') + 1)}</span>; title = c.name; break;
    case 'category': body = L.category(c.count, c.category); break;
    case 'preset': body = `▸ ${L.preset(c.number)}`; break;
    case 'reproduction': body = L.reproduction; break;
    case 'location': body = `${basename(c.file)}:${c.line}`; break;
  }
  return (
    <button type="button" className={`rv-chip rv-${c.kind}`} data-chip={cite} title={title}
      onClick={(e) => { e.stopPropagation(); onFollow?.(); follow(cite, shell); }}><span className="rv-chiptext">{body}</span></button>
  );
}

/** Where a statement's first cited difference or block is in the code, as a location chip. */
function StatementLoc({ data, cites }: { data: BundleData; cites: string[] }) {
  for (const c of cites) {
    const d = data.differences.find((x) => x.id === c);
    if (d?.location) return <span className="rv-qev"><Loc file={d.location.file} line={d.location.line} side="new" /></span>;
    if (/^b\d+$/.test(c)) { const l = blockLoc(data, c); if (l) return <span className="rv-qev"><Loc file={l.file} line={l.line} side={l.side} /></span>; }
  }
  return null;
}

/** Marks every cited evidence element in the other sections (D46). */
function useCitedMarks(cites: string[], data: BundleData) {
  useEffect(() => {
    const sel = cites.map((c) => resolve(c, data)).filter((c) => c && target(c).slot)
      .map((c) => `#slot-${target(c!).slot} :is(${target(c!).selector})`);
    if (!sel.length || typeof MutationObserver === 'undefined') return;
    const all = sel.join(', ');
    const mark = () => document.querySelectorAll<HTMLElement>(all).forEach((el) => {
      if (!el.closest('#slot-review') && !el.hasAttribute('data-ai-cited')) el.setAttribute('data-ai-cited', '');
    });
    mark();
    const ob = new MutationObserver(mark);
    ob.observe(document.body, { childList: true, subtree: true });
    return () => ob.disconnect();
  }, [cites.join(), data]);
}

function readCounts(data: BundleData): string[] {
  const rules = new Set(data.levels.flatMap((l) => l.rules));
  const groups = ((data.records.groups as { groups?: unknown[] } | null)?.groups ?? []).length;
  const out = [L.differences(data.differences.length), L.tests(testsOf(data).length), L.rules(rules.size), L.groups(groups), L.scope];
  if (data.bundle.description) out.push(L.description);
  if (data.bundle.requirement) out.push(L.requirement);
  return out;
}

/** A fallback row the pipeline adds for a question the model did not mention: `<rule> <block>`. */
const filler = (s: Statement) => !s.text || /^[a-z0-9-]+( b\d+)?$/.test(s.text);

/** The whole review as the model wrote it, under its four headings, what it read, and what the page dropped. */
function FullText({ data, review, onClose }: { data: BundleData; review: Review; onClose: () => void }) {
  return (
    <Sheet title={L.fullText} onClose={onClose} wide>
      <div className="rv-full" data-testid="review-full">
        {headingKeys.map((h) => {
          const items = review.byHeading[h].filter((s) => !filler(s));
          if (!items.length) return null;
          return (
            <div key={h} className="rv-group" data-heading={h}>
              <h3>{headings[h]}</h3>
              <ul>
                {items.map((s, i) => (
                  <li key={i} className="rv-fullst">
                    <p>{s.detail ?? s.text}</p>
                    <span className="rv-chips">{s.cites.map((c) => <Chip key={c} cite={c} data={data} onFollow={onClose} />)}</span>
                  </li>
                ))}
              </ul>
            </div>
          );
        })}
        <div className="rv-prov">
          <span><span className="rv-k">{L.read}</span> {readCounts(data).join(' · ')}</span>
          <span><span className="rv-k">{L.didNotRead}</span> {L.code}</span>
          <span className="num" data-dropped>{L.dropped(review.dropped.length)}</span>
        </div>
        {review.dropped.length > 0 && (
          <ul className="rv-dropped">
            {review.dropped.map((d, i) => (
              <li key={i}><s>{d.text}</s> <span className="rv-chips">{d.cites.map((c) => <Chip key={c} cite={c} data={data} onFollow={onClose} />)}</span></li>
            ))}
          </ul>
        )}
      </div>
    </Sheet>
  );
}

let asked = 0;
/** `Full text` on the review's title row: every statement in its long wording, what was read, what was dropped. */
export function FullTextButton({ data }: SectionProps) {
  const { setReveal } = useShell();
  const recorded = !!(data.records['ai-review'] as { recording?: string | null } | null)?.recording;
  if (!recorded) return null;
  return <button type="button" className="ghost" data-fulltext aria-haspopup="dialog" onClick={() => setReveal(`review:full:${++asked}`)}>{L.fullText}</button>;
}

export function AiReview({ data }: SectionProps) {
  const shell = useShell();
  const review = useMemo(() => readReview(data), [data]);
  const { groups } = useDecisions();
  const items = useMemo(() => reviewItems(data, review, groups), [data, review, groups]);
  const facts = useMemo(() => factsOf(data, review), [data, review]);
  const [sheet, setSheet] = useState<null | 'full' | 'description'>(null);
  const [matchesOpen, setMatchesOpen] = useState(false);
  const [all, setAll] = useState(false);
  const box = useRef<HTMLFieldSetElement>(null);
  const claim = claimOf(data);
  const { reveal } = shell;
  useEffect(() => {
    if (reveal?.startsWith('review:full')) setSheet('full');
    if (reveal?.startsWith('question:')) setAll(true);
  }, [reveal]);

  useCitedMarks(headingKeys.flatMap((h) => review.byHeading[h]).flatMap((s) => s.cites), data);

  useCommands([{
    id: 'review.first', label: L.aiReview, key: 'a',
    run: () => box.current?.querySelector<HTMLElement>('[data-statement], .rv-qhead, .rv-head')?.focus(),
  }], []);

  function onKey(e: React.KeyboardEvent) {
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
    const rows = [...(box.current?.querySelectorAll<HTMLElement>('[data-statement], button.rv-qhead, .rv-head') ?? [])];
    const i = rows.indexOf(document.activeElement as HTMLElement);
    if (i < 0) return;
    e.preventDefault();
    rows[Math.max(0, Math.min(rows.length - 1, i + (e.key === 'ArrowDown' ? 1 : -1)))].focus();
  }

  const matches = review.byHeading.matches;
  const summary = review.summaries.matches ?? (matches[0] ? shortLine(matches[0].text, 10) : '');
  const shown = all ? items : items.slice(0, SHOWN_ITEMS);

  // The AI's reading of the evidence against the description; the evidence itself is the row of doors below.
  return (
    <fieldset className="rv-box" data-review-box ref={box} onKeyDown={onKey} aria-label={L.aiSummary}>
      <legend className="rv-cap" data-ai>{L.aiSummary}</legend>
      {/* What was asked for is the description's claim; beside it, how many of its claims the evidence shows. */}
      <div className="rv-row" data-fact="asked">
        <span className="rv-k">{L.askedFor}</span>
        <span className="rv-cell rv-asked">
          <button type="button" className="rv-claim" data-claim aria-haspopup="dialog" title={claim} onClick={() => setSheet('description')}>
            <span className="rv-claimtext">{claim || L.noDescription}</span>
          </button>
          <span className="rv-v num" data-value>{facts.asked}</span>
        </span>
      </div>
      <div className="rv-row" data-fact="unasked"><span className="rv-k">{L.notAskedFor}</span><span className="rv-v num" data-value>{facts.unasked}</span></div>
      {!review.recorded && <p className="rv-none">{L.noReview}</p>}
      {matches.length > 0 && (
        <div className="rv-row rv-group" data-heading="matches" data-open={matchesOpen || undefined}>
          <span className="rv-k">{L.matchesGroup}</span>
          <div className="rv-cell">
            <button type="button" className="rv-head" aria-expanded={matchesOpen} onClick={() => setMatchesOpen(!matchesOpen)}>
              <span className="rv-sum" data-summary>{summary || L.claimsMatch(matches.length)}</span>
              <span className="chev" aria-hidden="true" />
            </button>
            {matchesOpen && (
              <ul>
                {matches.map((s, i) => (
                  <li key={i} className="rv-st">
                    <button type="button" className="rv-text" data-statement title={s.detail ?? s.text} onClick={() => follow(s.cites[0], shell)}>{shortLine(s.text)}</button>
                    <span className="spacer" />
                    <button type="button" className="ghost" data-evidence={s.cites[0]} onClick={() => follow(s.cites[0], shell)}>{L.evidence}</button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </div>
      )}
      {/* The one list. It is also where a question citation lands, so it keeps the Decide slot's id. */}
      {items.length > 0 && (
        <div className="rv-row rv-group" data-heading="needs-you">
          <span className="rv-k rv-kcol">
            <span>{L.needsYou}</span>
            {!all && items.length > SHOWN_ITEMS && (
              <button type="button" className="ghost rv-all" data-all onClick={() => setAll(true)}>{L.showAll(items.length)}</button>
            )}
          </span>
        <ul className="rv-list" id="slot-decide" data-list>
          {shown.map((it) => (it.kind === 'decision'
            ? <DecisionRow key={it.key} g={it.group} line={it.line} />
            : (
              <li key={it.key} className="rv-q rv-item" data-item={it.statement.heading}>
                <button type="button" className="rv-qhead rv-text" data-statement title={it.statement.detail ?? it.statement.text}
                  onClick={() => follow(it.statement.cites[0], shell)}>
                  <span className="rv-qtext">{it.line}</span>
                </button>
                <StatementLoc data={data} cites={it.statement.cites} />
                <span className="spacer" />
                <button type="button" className="ghost" data-evidence={it.statement.cites[0]} onClick={() => follow(it.statement.cites[0], shell)}>{L.evidence}</button>
              </li>
            )))}
        </ul>
        </div>
      )}
      {sheet === 'full' && <FullText data={data} review={review} onClose={() => setSheet(null)} />}
      {sheet === 'description' && (
        <Sheet title={L.descriptionTitle} onClose={() => setSheet(null)} wide>
          <div className="prose"><Prose text={data.bundle.description || L.noDescription} /></div>
        </Sheet>
      )}
    </fieldset>
  );
}
