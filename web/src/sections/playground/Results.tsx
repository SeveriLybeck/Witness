// Renderers for the eight declared result types (D36). Nothing else is drawn.
import { imageSrc, pageSrc, type ResultType } from './protocol';
import { pl } from './labels';

type Diag = { id: string; severity: string; message: string };

function Img({ path, alt, bundle, plugin }: { path: string; alt: string; bundle: string; plugin: string }) {
  const src = imageSrc(path, bundle, plugin);
  return src ? <img src={src} alt={alt} /> : <span data-blocked>{pl.blocked}</span>;
}

export function ResultView({ type, value, bundle = '', plugin = '' }: { type: ResultType; value: unknown; bundle?: string; plugin?: string }) {
  switch (type) {
    case 'diagnostics': {
      const list = (value as Diag[]) ?? [];
      if (!list.length) return <span className="pg-none">{pl.none}</span>;
      return <ul className="pg-diags">{list.map((d, i) => <li key={i}><code>{d.id}</code> {d.severity} {d.message}</li>)}</ul>;
    }
    case 'text-diff': {
      const v = value as { old?: string; new?: string };
      return <div className="pg-pair"><pre>{v?.old ?? ''}</pre><pre>{v?.new ?? ''}</pre></div>;
    }
    case 'table': {
      const v = value as { columns: string[]; rows: unknown[][] };
      return (
        <table className="pg-table"><thead><tr>{v.columns.map((c) => <th key={c}>{c}</th>)}</tr></thead>
          <tbody>{v.rows.map((r, i) => <tr key={i}>{r.map((c, j) => <td key={j}>{c == null ? '' : String(c)}</td>)}</tr>)}</tbody></table>
      );
    }
    case 'facts':
      return <dl className="pg-facts">{Object.entries((value as Record<string, unknown>) ?? {}).map(([k, v]) =>
        <div key={k}><dt>{k}</dt><dd>{v == null ? '' : typeof v === 'object' ? JSON.stringify(v) : String(v)}</dd></div>)}</dl>;
    case 'bytes':
      return <pre className="pg-bytes">{String(value).replace(/(..)/g, '$1 ').trim()}</pre>;
    case 'image':
      return <Img path={(value as { path: string }).path} alt="" bundle={bundle} plugin={plugin} />;
    case 'image-pair': {
      const v = value as { old: string; new: string };
      return <div className="pg-pair"><Img path={v.old} alt={pl.old} bundle={bundle} plugin={plugin} />
        <Img path={v.new} alt={pl.new} bundle={bundle} plugin={plugin} /></div>;
    }
    case 'page': {
      const src = pageSrc((value as { url: string }).url, bundle, plugin);
      return src ? <iframe className="pg-page" src={src} title={pl.outputs} sandbox="allow-scripts" />
        : <span data-blocked>{pl.blocked}</span>;
    }
  }
}
