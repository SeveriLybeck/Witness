import { useEffect, useState } from 'react';
import { claimHasTitle, claimOf, loadBundle, type BundleData, type BundleListEntry, type DataSource } from './data';
import { ShellProvider } from './context';
import { Page } from './Page';
import { labels } from './labels';
import { HowSheet, howTitle } from './How';
// The start page shows the same counts as each review: the review's own readers give them.
import { questionGroups, readReview } from '../sections/review/cites';
import { statusOf, type Status } from '../sections/review/status';
import { StatusMark } from '../sections/review/StatusMark';

function keyFromHash() {
  const m = /^#\/([0-9a-f]{40}-[0-9a-f]{40})/.exec(window.location.hash);
  return m ? m[1] : null;
}

interface Row {
  key: string; id: string; title: string; claim: string;
  extra: number | null; missing: number | null; decisions: number; compared: string; status: Status;
}

function rowOf(entry: BundleListEntry, data: BundleData): Row {
  const review = readReview(data);
  const scope = data.records.scope as { modelledLines?: number; changedLines?: { production?: number } } | null;
  return {
    key: entry.key,
    id: data.bundle.requirement?.id ?? entry.requirement ?? (entry.pullRequest != null ? `#${entry.pullRequest}` : entry.key.slice(0, 8)),
    title: claimHasTitle(data) ? '' : data.bundle.requirement?.title ?? '',
    claim: claimOf(data),
    extra: review.recorded ? review.byHeading['not-in-description'].length : null,
    missing: review.recorded ? review.byHeading['described-not-found'].length : null,
    decisions: questionGroups(data).length,
    compared: scope ? `${scope.modelledLines ?? 0} / ${scope.changedLines?.production ?? 0}` : '',
    status: statusOf(data, review),
  };
}

/** Start page: the changes to review, each with its claim and the values that matter (D7); one bundle opens directly. */
export function App({ source }: { source: DataSource }) {
  const [key, setKey] = useState(keyFromHash);
  const [list, setList] = useState<BundleListEntry[] | null>(null);
  const [rows, setRows] = useState<Record<string, Row>>({});
  const [data, setData] = useState<BundleData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [how, setHow] = useState(false);

  useEffect(() => {
    const on = () => setKey(keyFromHash());
    window.addEventListener('hashchange', on);
    return () => window.removeEventListener('hashchange', on);
  }, []);
  useEffect(() => {
    if (key) return;
    let live = true;
    source.bundles().then((b) => {
      if (!live) return;
      if (b.length === 1) { setKey(b[0].key); return; }
      setList(b);
      for (const e of b) {
        loadBundle(source, e.key).then((d) => { if (live) setRows((r) => ({ ...r, [e.key]: rowOf(e, d) })); }, () => undefined);
      }
    }, (e) => setError(String(e)));
    return () => { live = false; };
  }, [source, key]);
  useEffect(() => {
    if (!key) return;
    setData(null);
    loadBundle(source, key).then(setData, (e) => setError(String(e)));
  }, [source, key]);

  const shownId = key && data ? data.bundle.requirement?.id ?? null : null;
  useEffect(() => { document.title = shownId ? `${shownId} · ${labels.product}` : labels.product; }, [shownId]);

  if (error) return <div className="start"><span>{error}</span></div>;
  if (key && data) return <ShellProvider data={data} source={source}><Page /></ShellProvider>;
  if (key || !list) return <div className="start">{labels.loading}</div>;
  const sorted = [...list].sort((a, b) => (a.pullRequest ?? 0) - (b.pullRequest ?? 0));
  return (
    <nav className="start" aria-label={labels.bundles}>
      <div className="start-head">
        <h1 className="start-logo" aria-label={labels.product}>
          <img className="logo-light" src="/witness-lockup-on-light.svg" alt="" height={32} />
          <img className="logo-dark" src="/witness-lockup-on-dark.svg" alt="" height={32} />
        </h1>
        <button type="button" className="ghost" data-how-open aria-haspopup="dialog" onClick={() => setHow(true)}>{howTitle}</button>
      </div>
      <table className="start-table">
        <thead>
          <tr>
            <th>{labels.change}</th><th>{labels.claims}</th><th className="n">{labels.notAsked}</th><th className="n">{labels.notFound}</th>
            <th className="n">{labels.decisions}</th><th className="n">{labels.compared}</th><th>{labels.status}</th>
          </tr>
        </thead>
        <tbody>
          {sorted.map((b) => {
            const r = rows[b.key];
            const id = r?.id ?? b.requirement ?? (b.pullRequest != null ? `#${b.pullRequest}` : b.key.slice(0, 8));
            return (
              <tr key={b.key} data-change={id} onClick={() => { window.location.hash = `#/${b.key}`; }}>
                <td className="start-id num"><a href={`#/${b.key}`}>{id}</a></td>
                <td className="start-claim" title={r?.claim}>
                  {r && <><span className="start-title">{r.claim || labels.noDescription}</span> <span className="start-said">{r.title}</span></>}
                </td>
                <td className="n num" data-extra>{r?.extra ?? ''}</td>
                <td className="n num" data-missing>{r?.missing ?? ''}</td>
                <td className="n num" data-decisions>{r?.decisions ?? ''}</td>
                <td className="n num" data-compared>{r?.compared ?? ''}</td>
                <td data-status-cell>{r && <StatusMark status={r.status} />}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
      {how && <HowSheet onClose={() => setHow(false)} />}
    </nav>
  );
}
