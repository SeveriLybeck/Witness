// Sign off (D41, D47, D48): one button with the reviewer and risk fields. What still blocks approval is shown only
// while it blocks, each as a plain line that says what to do and does it when activated. The gateway enforces.
import { useEffect, useState } from 'react';
import { useShell, type BundleData, type SectionProps } from '../../shell';
import { basename } from '../../shell';
import { blockLoc, follow, modeOf, questionGroups, questionsOf } from './cites';
import { gateLabels, L } from './labels';
import { pendingSpot, useSession } from './session';

const gateOrder = ['section-running', 'question-unanswered', 'spot-check', 'level-3-unopened'];
const tokenKey = 'witness.token.used';

/** Sorts the gateway's unmet items under the gates in the spec's order (the gateway lists all, names the first). */
function groupRefusal(items: string[], spot: string | null): Record<string, string[]> {
  const g: Record<string, string[]> = {};
  const add = (k: string, v: string) => { (g[k] ??= []).push(v); };
  for (const i of items) {
    if (/^q\d+$/.test(i)) add('question-unanswered', i);
    else if (/^b\d+$/.test(i)) add(i === spot ? 'spot-check' : 'level-3-unopened', i);
    else add('section-running', i);
  }
  return g;
}

/** Keeps the session in step with what the reviewer opened, whether or not Sign off is unfolded. */
export function SessionEffects({ data }: SectionProps) {
  const shell = useShell();
  const { state, actions } = useSession(data, shell.source);
  const s = state.session;
  // A block focused on the map counts as opened; parts the code panel showed go to `shown` (session.json).
  const block = shell.selection.block;
  const hasSession = !!s;
  useEffect(() => {
    if (block && hasSession && !s!.opened.includes(block)) void actions.open(block);
  }, [block, hasSession]); // eslint-disable-line react-hooks/exhaustive-deps
  const newShown = shell.shownParts.filter((p) => s && !s.shown.includes(p));
  useEffect(() => {
    if (newShown.length) void actions.show(newShown);
  }, [newShown.join()]); // eslint-disable-line react-hooks/exhaustive-deps
  return null;
}

/** The lines that block approval, in order, for a set of unmet items per gate. */
function BlockLines({ data, unmet, spotId }: { data: BundleData; unmet: Record<string, string[]>; spotId: string | null }) {
  const shell = useShell();
  const { actions } = useSession(data, shell.source);
  const name = (id: string) => data.blocks.find((b) => b.id === id)?.name ?? id;
  const running = unmet['section-running'] ?? [];
  const qs = unmet['question-unanswered'] ?? [];
  const decisions = questionGroups(data).filter((g) => g.questions.some((q) => qs.includes(q.id)));
  const blocks = unmet['level-3-unopened'] ?? [];
  const spot = (unmet['spot-check'] ?? [])[0] ?? null;
  function openBlock(id: string) {
    shell.setFolded('map', false);
    shell.focusBlock(id);
    const loc = blockLoc(data, id);
    if (loc) shell.openCode(loc);
    void actions.open(id);
  }
  return (
    <ul className="rv-blocks" data-blocking>
      {running.length > 0 && (
        <li className="rv-q" data-gate="section-running"><span>{L.waitFor}: {running.join(', ')}</span></li>
      )}
      {decisions.length > 0 && (
        <li className="rv-q" data-gate="question-unanswered">
          <button type="button" className="ghost" onClick={() => follow(decisions[0].questions.find((q) => qs.includes(q.id))!.id, shell)}>{L.answerItems(decisions.length)}</button>
        </li>
      )}
      {spot && (
        <li className="rv-q" data-gate="spot-check" data-spotcheck={spot}>
          <button type="button" className="ghost" onClick={() => openBlock(spot)}>Open one randomly chosen file: {basename(blockLoc(data, spot)?.file ?? name(spot))}</button>
        </li>
      )}
      {blocks.length > 0 && (
        <li className="rv-q" data-gate="level-3-unopened" title={blocks.map(name).join('\n')}>
          <button type="button" className="ghost" onClick={() => openBlock(blocks[0])}>Open {blocks.length} changed {blocks.length === 1 ? 'block' : 'blocks'} first: {name(blocks[0])}{blocks.length > 1 ? ', …' : ''}</button>
        </li>
      )}
      {spotId && !spot && null}
    </ul>
  );
}

export function SignOff({ data }: SectionProps) {
  const shell = useShell();
  const { state, actions } = useSession(data, shell.source);
  const [risk, setRisk] = useState('');
  const [token, setToken] = useState('');
  const [name, setName] = useState(state.reviewer);
  const s = state.session;
  const mode = modeOf(data);
  // The token is asked for on first use in this browser session, and again only when the gateway refuses it.
  const [tokenUsed, setTokenUsed] = useState(() => { try { return sessionStorage.getItem(tokenKey) === '1'; } catch { return false; } });

  const running = Object.entries(data.bundle.sections ?? {}).filter(([, v]) => v.state === 'running').map(([k]) => k);
  const opened = new Set(s?.opened ?? []);
  const level1 = data.levels.filter((l) => l.level === 1).map((l) => l.block);
  const spotId = s?.spotCheck ?? pendingSpot(data.key, level1);
  const anyReturn = !state.stale && Object.values(s?.answers ?? {}).includes('return');
  // Shown, not enforced: the gateway decides (D41).
  const local: Record<string, string[]> = {
    'section-running': running,
    'question-unanswered': questionsOf(data).filter((q) => state.stale || s?.answers[q.id] !== 'accept').map((q) => q.id),
    'spot-check': spotId && !opened.has(spotId) ? [spotId] : [],
    'level-3-unopened': mode === 'full' ? data.levels.filter((l) => l.level === 3 && !opened.has(l.block)).map((l) => l.block) : [],
  };
  const blocked = !anyReturn && gateOrder.some((g) => local[g].length);

  const o = state.outcome;
  useEffect(() => {
    if (o?.kind === 'submitted') { try { sessionStorage.setItem(tokenKey, '1'); } catch { /* per-viewer convenience only */ } setTokenUsed(true); }
    if (o?.kind === 'refused' && o.status === 401) setTokenUsed(false);
  }, [o]);
  const gateRefusal = o?.kind === 'refused' && o.items.length > 0 && !!o.gate && gateOrder.includes(o.gate);
  return (
    <div className="rv-signoff" data-mode={mode ?? undefined}>
      {blocked && <BlockLines data={data} unmet={local} spotId={spotId} />}
      <div className="rv-line rv-form">
        <label className="rv-field">{L.reviewer}
          <input data-reviewer value={name} onChange={(e) => setName(e.target.value)}
            onBlur={() => { if (name.trim() !== state.reviewer) actions.setReviewer(name.trim()); }} />
        </label>
        {!tokenUsed && (
          <label className="rv-field">{L.token}
            <input type="password" autoComplete="off" value={token} onChange={(e) => setToken(e.target.value)} />
          </label>
        )}
        <label className="rv-field rv-risk">{L.risk}
          <textarea rows={1} value={risk} onChange={(e) => setRisk(e.target.value)} />
        </label>
        <button type="button" className="rv-submit" data-submit
          disabled={running.length > 0 || state.busy || !state.reviewer}
          onClick={() => void actions.submit(risk, token)}>
          {anyReturn ? L.requestChanges : L.approve}
        </button>
      </div>
      {o?.kind === 'refused' && (
        <div className="rv-outcome" data-outcome="refused">
          <div className="rv-line">
            <span className="rv-k">{L.refused}</span>
            {!gateRefusal && <span>{o.gate ? gateLabels[o.gate] ?? o.gate : o.status === 401 ? L.noToken : o.error || L.failed}</span>}
            {o.gate === 'self-approval' && <span>{L.notSubmitted}</span>}
          </div>
          {gateRefusal && <BlockLines data={data} unmet={groupRefusal(o.items, spotId)} spotId={spotId} />}
        </div>
      )}
      {o?.kind === 'submitted' && (
        <div className="rv-outcome" data-outcome="submitted">
          <div className="rv-line">
            <span className="rv-k">{L.submitted}</span>
            <span>{o.review.event === 'APPROVE' ? L.approve : L.requestChanges}</span>
            {o.review.result?.url && <a href={o.review.result.url}>{o.review.result.url}</a>}
          </div>
          <pre className="rv-body">{o.review.body}</pre>
        </div>
      )}
    </div>
  );
}
