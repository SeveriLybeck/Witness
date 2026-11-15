// Web viewers (D39): one line-protocol process per viewer. `node src/plugin.ts --plugin <viewer>`.
// Viewers: site-preview, api-console, gallery, detect. One JSON message per line on stdin/stdout.
import { createInterface } from 'node:readline';
import { readFileSync, existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium, type Browser } from 'playwright-core';
import { startApp, startProxy, send, type App, type Exchange, type Side } from './servers.ts';

const here = dirname(fileURLToPath(import.meta.url));
export const DEVICES: Record<string, { width: number; height: number }> = {
  phone: { width: 390, height: 844 }, tablet: { width: 820, height: 1180 }, desktop: { width: 1280, height: 800 },
};

const repoRoot = resolve(here, '..', '..', '..');

/** Old and new trees. `$WEB_TREES` (a folder holding `old/` and `new/`, as the sample is laid out) overrides.
 *  Otherwise the bundle's commits (`describe`'s bundle, else `$PLUGIN_BUNDLE_DIR/bundle.json`) fill the first
 *  existing pattern of `$WEB_TREE_PATTERN` or `web.json` `trees` (`…/{commit}`, relative to the repo root). */
export function trees(): Record<Side, string> {
  if (process.env.WEB_TREES) return { old: join(process.env.WEB_TREES, 'old'), new: join(process.env.WEB_TREES, 'new') };
  const b = st.bundle ?? (process.env.PLUGIN_BUNDLE_DIR && existsSync(join(process.env.PLUGIN_BUNDLE_DIR, 'bundle.json'))
    ? JSON.parse(readFileSync(join(process.env.PLUGIN_BUNDLE_DIR, 'bundle.json'), 'utf8')) : null);
  if (!b?.oldCommit || !b?.newCommit) throw new Error('no bundle commits and no WEB_TREES');
  const cfg = JSON.parse(readFileSync(join(here, '..', 'web.json'), 'utf8'));
  const patterns: string[] = process.env.WEB_TREE_PATTERN ? [process.env.WEB_TREE_PATTERN] : cfg.trees;
  const find = (commit: string) => {
    for (const p of patterns) { const d = resolve(repoRoot, p.replace('{commit}', commit)); if (existsSync(d)) return d; }
    throw new Error(`no tree for ${commit}`);
  };
  return { old: find(b.oldCommit), new: find(b.newCommit) };
}

type Json = any;
interface State { apps?: Record<Side, App>; proxy?: { url: string; server: any }; log: Exchange[]; browser?: Browser; workdir: string; bundle?: Json }
const st: State = { log: [], workdir: process.cwd() };

async function apps() {
  if (!st.apps) { const t = trees(); st.apps = { old: await startApp('old', t.old), new: await startApp('new', t.new) }; }
  return st.apps;
}
async function browser() {
  st.browser ??= await chromium.launch({ executablePath: process.env.WITNESS_CHROMIUM || undefined, chromiumSandbox: false });
  return st.browser;
}
/** Screenshot as a `data:image/png;base64,…` URL (the page shows it in `<img src>`). */
async function shot(url: string, size: { width: number; height: number }, ready = 'body[data-ready="1"]'): Promise<string> {
  const page = await (await browser()).newPage({ viewport: size });
  try {
    await page.goto(url);
    const wait = page.waitForSelector(ready, { timeout: 30_000 });
    // the sample marks readiness with data-ready; other sites may not, so only an explicit selector must appear
    await (ready.startsWith('body[data-ready') ? wait.catch(() => {}) : wait);
    return 'data:image/png;base64,' + (await page.screenshot({ fullPage: true })).toString('base64');
  } finally { await page.close(); }
}

const slot = (name: string, role: string, type: string, field: string) => ({ name, role, type, constructible: true, field });
const free = [{ kind: 'free', label: 'free', inputs: {} }];

function stories(side: Side): Record<string, Json> {
  const f = join(trees()[side], 'storybook-static', 'index.json');
  return existsSync(f) ? JSON.parse(readFileSync(f, 'utf8')).entries : {};
}

/** Plan from `nixpacks plan <tree>` when nixpacks is on PATH, else the recorded `nixpacks-plan.<side>.json`. */
export function plan(side: Side): { plan: Json; source: string } {
  const tree = trees()[side];
  const r = spawnSync('nixpacks', ['plan', tree], { encoding: 'utf8', timeout: 60_000 });
  if (r.status === 0) return { plan: JSON.parse(r.stdout), source: 'nixpacks' };
  const rec = join(dirname(tree), `nixpacks-plan.${side}.json`);
  if (existsSync(rec)) return { plan: JSON.parse(readFileSync(rec, 'utf8')), source: 'recorded' };
  return { plan: {}, source: 'unavailable' };
}
const planFacts = (p: Json, source: string) => ({
  provider: p.variables?.NIXPACKS_METADATA ?? null, start: p.start?.cmd ?? null,
  install: (p.phases?.install?.cmds ?? []).join('; '), nixPkgs: (p.phases?.setup?.nixPkgs ?? []).join(', '), source,
});

export const VIEWERS: Record<string, { subjects: () => Json[]; run: (inputs: Json, sides: Side[]) => Promise<Json[]> }> = {
  'site-preview': {
    subjects: () => [{
      id: 'site', name: 'Site', definition: { text: 'public/index.html served by server.js', locations: [{ file: 'public/index.html', line: 1, endLine: 1 }] },
      inputs: [{ ...slot('device', 'device size', 'choice', 'choice'), choices: Object.keys(DEVICES) }, slot('path', 'page path', 'string', 'string')],
      outputs: [{ name: 'screens', type: 'image-pair' }],
      presets: Object.keys(DEVICES).map((d) => ({ kind: 'free', label: d, inputs: { device: d, path: '/' } })),
    }],
    run: async (inputs) => {
      const a = await apps(); const size = DEVICES[inputs.device ?? 'desktop'] ?? DEVICES.desktop; const path = inputs.path || '/';
      const img = { old: await shot(a.old.url + path, size), new: await shot(a.new.url + path, size) };
      return [{ output: 'screens', side: 'new', type: 'image-pair', value: img }];
    },
  },
  'api-console': {
    subjects: () => [{
      id: 'api', name: 'API', definition: { text: 'openapi.json', locations: [{ file: 'openapi.json', line: 1, endLine: 1 }] },
      inputs: [{ ...slot('method', 'method', 'choice', 'choice'), choices: ['GET', 'POST'] }, slot('path', 'request path', 'string', 'string'), slot('body', 'request body', 'text', 'text')],
      outputs: [{ name: 'console', type: 'image' }, { name: 'response', type: 'text-diff' }, { name: 'exchanges', type: 'table' }],
      presets: [{ kind: 'free', label: 'products', inputs: { method: 'GET', path: '/api/products', body: '' } },
        { kind: 'free', label: 'cart', inputs: { method: 'POST', path: '/api/cart', body: '[1,2]' } }],
    }],
    run: async (inputs) => {
      const a = await apps();
      st.proxy ??= await startProxy(a.old, a.new, st.log);
      const out: Json[] = [];
      if (inputs.path) {
        await send(st.proxy.url, inputs.method ?? 'GET', inputs.path, inputs.body || undefined);
        const x = st.log[st.log.length - 1];
        out.push({ output: 'response', side: 'new', type: 'text-diff', value: { old: `${x.old.status}\n${x.old.body}`, new: `${x.new.status}\n${x.new.body}` } });
      }
      // The Scalar console's URL lives in the plugin's network namespace, so the page gets a screenshot.
      out.unshift({ output: 'console', side: 'new', type: 'image', value: { path: await shot(st.proxy.url + '/', DEVICES.desktop, ':text("/api/cart")') } });
      out.push({ output: 'exchanges', side: 'new', type: 'table', value: {
        columns: ['method', 'path', 'old', 'new', 'same'],
        rows: st.log.map((x) => [x.method, x.path, `${x.old.status} ${x.old.body}`, `${x.new.status} ${x.new.body}`, x.old.status === x.new.status && x.old.body === x.new.body]),
      } });
      return out;
    },
  },
  gallery: {
    subjects: () => {
      const o = stories('old'), n = stories('new'); const ids = [...new Set([...Object.keys(o), ...Object.keys(n)])].sort();
      return [{
        id: 'stories', name: 'Components', definition: { text: 'storybook-static/index.json', locations: [{ file: 'storybook-static/index.json', line: 1, endLine: 1 }] },
        inputs: [{ ...slot('story', 'story', 'choice', 'choice'), choices: ids }],
        outputs: [{ name: 'stories', type: 'table' }, { name: 'story', type: 'image-pair' }],
        presets: ids.map((id) => ({ kind: 'free', label: id, inputs: { story: id } })),
      }];
    },
    run: async (inputs) => {
      const o = stories('old'), n = stories('new'); const ids = [...new Set([...Object.keys(o), ...Object.keys(n)])].sort();
      const out: Json[] = [{ output: 'stories', side: 'new', type: 'table', value: { columns: ['story', 'old', 'new'], rows: ids.map((id) => [id, id in o, id in n]) } }];
      if (inputs.story) {
        const t = trees(); const url = (s: Side) => `file://${join(t[s], 'storybook-static', 'iframe.html')}?id=${encodeURIComponent(inputs.story)}`;
        out.push({ output: 'story', side: 'new', type: 'image-pair', value: { old: await shot(url('old'), { width: 400, height: 300 }), new: await shot(url('new'), { width: 400, height: 300 }) } });
      }
      return out;
    },
  },
  detect: {
    subjects: () => [{
      id: 'plan', name: 'Build plan', definition: { text: 'nixpacks plan', locations: [{ file: 'package.json', line: 1, endLine: 1 }] },
      inputs: [], outputs: [{ name: 'plan', type: 'text-diff' }, { name: 'detected', type: 'facts' }], presets: free,
    }],
    run: async (_inputs, sides) => {
      const o = plan('old'), n = plan('new');
      return [{ output: 'plan', side: 'new', type: 'text-diff', value: { old: JSON.stringify(o.plan, null, 2), new: JSON.stringify(n.plan, null, 2) } },
        ...sides.filter((s) => s !== 'reverted').map((s) => { const p = s === 'old' ? o : n; return { output: 'detected', side: s, type: 'facts', value: planFacts(p.plan, p.source) }; })];
    },
  },
};

export async function handle(viewer: string, msg: Json): Promise<Json> {
  const v = VIEWERS[viewer];
  const base = { id: msg.id, op: msg.op, pid: process.pid };
  try {
    if (msg.op === 'describe') { if (msg.workdir) st.workdir = msg.workdir; if (msg.bundle) st.bundle = msg.bundle; return { ...base, ok: true, subjects: v.subjects() }; }
    if (msg.op === 'run') return { ...base, ok: true, results: await v.run(msg.inputs ?? {}, msg.sides ?? ['old', 'new']) };
    if (msg.op === 'shutdown') return { ...base, ok: true };
    return { ...base, ok: false, error: `unknown op ${msg.op}` };
  } catch (e) { return { ...base, ok: false, error: String((e as Error).message ?? e) }; }
}

export async function close() {
  await st.browser?.close(); st.browser = undefined;
  st.proxy?.server.close(); st.proxy = undefined;
  for (const a of Object.values(st.apps ?? {})) a.child.kill();
  st.apps = undefined;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const i = process.argv.indexOf('--plugin'); const viewer = process.argv[i + 1];
  if (i < 0 || !VIEWERS[viewer]) { console.error(`usage: plugin.ts --plugin <${Object.keys(VIEWERS).join('|')}>`); process.exit(2); }
  const rl = createInterface({ input: process.stdin });
  let chain = Promise.resolve();
  rl.on('line', (line) => {
    chain = chain.then(async () => {
      let msg: Json;
      try { msg = JSON.parse(line); } catch { return; }
      const reply = await handle(viewer, msg);
      process.stdout.write(JSON.stringify(reply) + '\n');
      if (msg.op === 'shutdown') { await close(); process.exit(0); }
    });
  });
  rl.on('close', () => chain.then(close).then(() => process.exit(0)));
  process.on('exit', () => { for (const a of Object.values(st.apps ?? {})) a.child.kill(); });
  for (const s of ['SIGTERM', 'SIGINT', 'SIGHUP'] as const) process.on(s, () => { for (const a of Object.values(st.apps ?? {})) a.child.kill(); process.exit(0); });
}
