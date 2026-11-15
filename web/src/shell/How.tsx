// "How it works" as a sheet; opened from the start page, the header, and each open section's heading.
import { useEffect, useRef } from 'react';
import { Sheet } from './parts';
import { howItems, howLead, howLimit, howStatus, howTitle, howTools, type HowTopic } from './howText';

export { howTitle, howItems, howLead, howStatus };
export type { HowTopic };

export function HowSheet({ topic, onClose }: { topic?: HowTopic | null; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (topic) ref.current?.querySelector(`[data-how="${topic}"]`)?.scrollIntoView?.({ block: 'start' });
  }, [topic]);
  return (
    <Sheet title={howTitle} onClose={onClose} wide>
      <div className="how prose" ref={ref} data-testid="how">
        <img className="how-mark" src="/witness-icon-flat-indigo.svg" alt="" width={24} height={24} />
        <p className="how-lead">{howLead}</p>
        <div className="how-status" data-how="status" aria-current={topic === 'status' || undefined}>
          <h3>{howStatus.title}</h3>
          <p>{howStatus.text}</p>
        </div>
        {howItems.map((it) => (
          <section key={it.id} data-how={it.id} aria-current={topic === it.id || undefined}>
            <h3>{it.title}</h3>
            <p>{it.shows}</p>
            <p><span className="how-k">{howLimit}</span> {it.limit}</p>
            {it.tools && <details><summary>{howTools}</summary><p>{it.tools}</p></details>}
          </section>
        ))}
      </div>
    </Sheet>
  );
}
