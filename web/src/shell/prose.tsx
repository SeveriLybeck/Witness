// Issue Markdown shown as plain prose: headings, paragraphs, lists, code blocks. No HTML is interpreted.
import type { ReactNode } from 'react';

function inline(s: string): ReactNode[] {
  return s.split(/(`[^`]+`)/g).map((t, i) =>
    t.startsWith('`') && t.endsWith('`') && t.length > 1
      ? <code key={i}>{t.slice(1, -1)}</code>
      : t.replace(/\*\*([^*]+)\*\*|__([^_]+)__/g, '$1$2').replace(/\[([^\]]+)\]\([^)]*\)/g, '$1'));
}

export function Prose({ text }: { text: string }) {
  const out: ReactNode[] = [];
  const lines = text.replace(/\r\n/g, '\n').split('\n');
  let para: string[] = [];
  let list: { ordered: boolean; items: string[] } | null = null;
  const flush = () => {
    if (para.length) out.push(<p key={out.length}>{inline(para.join(' '))}</p>);
    para = [];
    if (list) {
      const items = list.items.map((it, i) => <li key={i}>{inline(it)}</li>);
      out.push(list.ordered ? <ol key={out.length}>{items}</ol> : <ul key={out.length}>{items}</ul>);
    }
    list = null;
  };
  for (let i = 0; i < lines.length; i++) {
    const l = lines[i];
    const fence = /^\s*(```|~~~)/.exec(l);
    if (fence) {
      flush();
      const body: string[] = [];
      for (i++; i < lines.length && !lines[i].trim().startsWith(fence[1]); i++) body.push(lines[i]);
      out.push(<pre key={out.length}>{body.join('\n')}</pre>);
      continue;
    }
    const h = /^\s*#{1,6}\s+(.*?)\s*#*\s*$/.exec(l);
    if (h) { flush(); out.push(<h3 key={out.length}>{inline(h[1])}</h3>); continue; }
    const li = /^\s*(?:([-*+])|(\d+)[.)])\s+(.*)$/.exec(l);
    if (li) {
      if (para.length) { const p = para; para = []; out.push(<p key={out.length}>{inline(p.join(' '))}</p>); }
      const ordered = !!li[2];
      if (list && list.ordered !== ordered) flush();
      list = list ?? { ordered, items: [] };
      list.items.push(li[3]);
      continue;
    }
    if (!l.trim()) { flush(); continue; }
    if (list && /^\s{2,}\S/.test(l)) { list.items[list.items.length - 1] += ' ' + l.trim(); continue; }
    if (list) flush();
    para.push(l.trim().replace(/^>\s?/, ''));
  }
  flush();
  return <>{out}</>;
}
