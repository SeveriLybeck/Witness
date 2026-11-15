// The status as a small coloured dot and a short label, like a pull request's checks. Why it is what it is shows on
// hover and focus; activating it opens How it works at the rule.
import { useId, useMemo } from 'react';
import { useShell, type SectionProps } from '../../shell';
import { statusOf, type Status } from './status';

const quiet = 'every claim has evidence · nothing beyond the description · nothing to decide';
export const whyText = (s: Status) => (s.reasons.length ? s.reasons.join(' · ') : quiet);

export function StatusMark({ status, onWhy }: { status: Status; onWhy?: () => void }) {
  const id = useId();
  const why = whyText(status);
  if (!onWhy) {
    return (
      <span className="st" data-status={status.level} title={why}>
        <span className="st-dot" aria-hidden="true" /><span className="st-label">{status.label}</span>
      </span>
    );
  }
  return (
    <span className="st" data-status={status.level}>
      <button type="button" className="st-mark" aria-describedby={id} aria-haspopup="dialog" onClick={onWhy}>
        <span className="st-dot" aria-hidden="true" /><span className="st-label">{status.label}</span>
      </button>
      <span role="tooltip" id={id} className="st-why" data-why>{why}</span>
    </span>
  );
}

let asked = 0;
/** The review's headline: the status beside the section title. */
export function ReviewStatus({ data }: SectionProps) {
  const { setReveal } = useShell();
  const status = useMemo(() => statusOf(data), [data]);
  return <StatusMark status={status} onWhy={() => setReveal(`how:status:${++asked}`)} />;
}
