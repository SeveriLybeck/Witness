// Web viewers on samples/tiny-shop over the real line protocol (bin/web, one process per viewer).
import { test, after } from 'node:test';
import assert from 'node:assert/strict';
import { spawn, spawnSync, execFileSync, type ChildProcess } from 'node:child_process';
import { createInterface } from 'node:readline';
import { readFileSync, readdirSync, existsSync, statSync, mkdirSync, writeFileSync, rmSync, symlinkSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const web = join(dirname(fileURLToPath(import.meta.url)), '..');
const root = join(web, '..', '..');
const shop = join(web, 'samples', 'tiny-shop');
const VIEWERS = ['site-preview', 'api-console', 'gallery', 'detect'];
const RESULT_TYPES = ['diagnostics', 'text-diff', 'table', 'facts', 'bytes', 'image', 'image-pair', 'page'];
const EXCLUDED = ['webcontainer', 'nodebox', 'cheerpj', '@apollo/explorer', '@apollo/sandbox', 'responsively'];

class Plugin {
  child: ChildProcess; next = 1; waiting: ((v: any) => void)[] = []; lines: string[] = [];
  constructor(viewer: string, cwd: string, env: Record<string, string> = { WEB_TREES: shop }, cmd = [join(web, 'bin', 'web'), '--plugin', viewer]) {
    this.child = spawn(cmd[0], cmd.slice(1), { cwd, env: { ...process.env, ...env }, stdio: ['pipe', 'pipe', 'inherit'] });
    createInterface({ input: this.child.stdout! }).on('line', (l) => { this.lines.push(l); this.waiting.shift()!(JSON.parse(l)); });
  }
  request(msg: any): Promise<any> {
    const id = this.next++;
    return new Promise((res) => { this.waiting.push(res); this.child.stdin!.write(JSON.stringify({ id, ...msg }) + '\n'); })
      .then((r: any) => { assert.equal(r.id, id); assert.equal(r.ok, true, r.error); return r; });
  }
  describe() { return this.request({ op: 'describe', bundle: { oldCommit: 'a'.repeat(40), newCommit: 'b'.repeat(40) }, workdir: join(web, 'work', 'test') }); }
  run(subject: string, inputs: any) { return this.request({ op: 'run', subject, inputs, sides: ['old', 'new'], preset: null }); }
  async stop() { await this.request({ op: 'shutdown' }); }
}
const plugins: Plugin[] = [];
const start = (v: string) => { mkdirSync(join(web, 'work', 'test'), { recursive: true }); const p = new Plugin(v, join(web, 'work', 'test')); plugins.push(p); return p; };
after(() => { for (const p of plugins) p.child.kill(); });

const pngSize = (url: string) => {
  assert.match(url, /^data:image\/png;base64,/);
  const b = Buffer.from(url.split(',')[1], 'base64'); return { width: b.readUInt32BE(16), height: b.readUInt32BE(20) }; };
const manifest = (v: string) => JSON.parse(readFileSync(join(web, v, 'plugin.json'), 'utf8'));

// file-exists with globs `*` per segment, as the plugin host matches them.
function exists(repo: string, glob: string): boolean {
  const walk = (dir: string, parts: string[]): boolean => {
    if (!parts.length) return true;
    const re = new RegExp('^' + parts[0].replace(/[.]/g, '\\.').replace(/\*/g, '.*') + '$');
    if (!existsSync(dir) || !statSync(dir).isDirectory()) return false;
    return readdirSync(dir).some((n) => re.test(n) && walk(join(dir, n), parts.slice(1)));
  };
  return walk(repo, glob.split('/'));
}

test('22.1 checks: contract hash, pinned playwright, no excluded components', () => {
  const hash = execFileSync('git', ['log', '-1', '--format=%H', '--', 'contracts/'], { cwd: root, encoding: 'utf8' }).trim();
  assert.equal(readFileSync(join(web, 'FALLBACKS.md'), 'utf8').split('\n')[0].trim(), hash);
  const lock = JSON.parse(readFileSync(join(web, 'package-lock.json'), 'utf8'));
  assert.equal(lock.packages['node_modules/playwright-core'].version, '1.59.1');
  const names = Object.keys(lock.packages).join('\n').toLowerCase();
  for (const x of EXCLUDED) assert.ok(!names.includes(x), x);
});

test('manifests: closed output list, schema keys, detection on tiny-shop', () => {
  const keys = ['$schema', 'name', 'version', 'executable', 'args', 'config', 'detect', 'inputs', 'outputs'];
  for (const v of VIEWERS) {
    const m = manifest(v);
    for (const k of Object.keys(m)) assert.ok(keys.includes(k), `${v}: ${k}`);
    for (const o of m.outputs) assert.ok(RESULT_TYPES.includes(o.type), `${v}: ${o.type}`);
    assert.equal(m.name, v, 'name equals folder, as the host resolves <plugins>/<name>/plugin.json');
    for (const side of ['old', 'new']) for (const d of m.detect) assert.ok(exists(join(shop, side), d.path), `${v} detects tiny-shop ${side}`);
    assert.ok(!m.detect.every((d: any) => exists(root, d.path)) || v === 'detect', `${v} does not detect this repository`);
    assert.ok(existsSync(join(web, v, m.executable)));
  }
  const lab = JSON.parse(readFileSync(join(root, 'plugins', 'generator-lab', 'plugin.json'), 'utf8'));
  assert.ok(!lab.detect.every((d: any) => exists(join(shop, 'new'), d.path)), 'generator lab does not detect tiny-shop');
  assert.ok(VIEWERS.length > 1, 'tabs');
});

test('22.2 site preview: device sizes, image pair, one process', async () => {
  const p = start('site-preview');
  const d = await p.describe();
  assert.deepEqual(d.subjects[0].presets.map((x: any) => x.label), ['phone', 'tablet', 'desktop']);
  const a = await p.run('site', { device: 'phone', path: '/' });
  const b = await p.run('site', { device: 'desktop', path: '/' });
  assert.equal(a.pid, b.pid);
  for (const l of p.lines) JSON.parse(l);
  const pa = a.results.find((r: any) => r.output === 'screens').value;
  assert.equal(pngSize(pa.old).width, 390);
  assert.equal(pngSize(pa.new).width, 390);
  assert.notEqual(pa.old, pa.new);
  assert.equal(pngSize(b.results[0].value.new).width, 1280);
  assert.ok(!JSON.stringify([a, b]).includes('127.0.0.1'), 'no namespace-local URLs');
  await p.stop();
});

test('22.2 API console: Scalar page, proxy replays against old', async () => {
  const p = start('api-console');
  await p.describe();
  const r = await p.run('api', { method: 'GET', path: '/api/products', body: '' });
  const diff = r.results.find((x: any) => x.output === 'response').value;
  assert.match(diff.old, /"price":1200/);
  assert.match(diff.new, /"price":1080/);
  await p.run('api', { method: 'GET', path: '/api/products/9', body: '' });
  const r3 = await p.run('api', { method: 'POST', path: '/api/cart', body: '[1,2]' });
  const rows = r3.results.find((x: any) => x.output === 'exchanges').value.rows;
  assert.deepEqual(rows.map((x: any[]) => x[4]), [false, true, false]);
  // Scalar rendered (the plugin waits for its /api/cart entry) and comes back as a data URL
  const c = r3.results.find((x: any) => x.output === 'console');
  assert.equal(c.type, 'image');
  assert.equal(pngSize(c.value.path).width, 1280);
  await p.stop();
});

test('22.3 component gallery from two static Storybook builds', async () => {
  const p = start('gallery');
  const d = await p.describe();
  assert.deepEqual(d.subjects[0].inputs[0].choices, ['badge--sale', 'button--primary', 'card--default']);
  const r = await p.run('stories', { story: 'card--default' });
  const rows = r.results.find((x: any) => x.output === 'stories').value.rows;
  assert.deepEqual(rows[0], ['badge--sale', false, true]);
  const pair = r.results.find((x: any) => x.output === 'story').value;
  assert.match(pair.old, /^data:image\/png;base64,/);
  assert.notEqual(pair.old, pair.new);
  await p.stop();
});

test('22.3 nixpacks detection on tiny-shop', async () => {
  const p = start('detect');
  await p.describe();
  const r = await p.run('plan', {});
  const facts = r.results.filter((x: any) => x.output === 'detected');
  assert.equal(facts.length, 2);
  for (const f of facts) { assert.equal(f.value.provider, 'node'); assert.match(f.value.nixPkgs, /nodejs/); }
  await p.stop();
});

const OLD = '1'.repeat(40), NEW = '2'.repeat(40);
function bundleDir() {
  const b = join(web, 'work', 'test', 'bundle'); mkdirSync(b, { recursive: true });
  writeFileSync(join(b, 'bundle.json'), JSON.stringify({ oldCommit: OLD, newCommit: NEW }));
  const wt = join(web, 'work', 'test', 'wt'); mkdirSync(wt, { recursive: true });
  for (const [c, s] of [[OLD, 'old'], [NEW, 'new']]) { rmSync(join(wt, c), { force: true, recursive: true }); symlinkSync(join(shop, s), join(wt, c)); }
  return { b, pattern: join(wt, '{commit}') };
}

test('trees from the bundle commits ($PLUGIN_BUNDLE_DIR)', async () => {
  const { b, pattern } = bundleDir();
  const p = new Plugin('gallery', join(web, 'work', 'test'), { WEB_TREES: '', PLUGIN_BUNDLE_DIR: b, WEB_TREE_PATTERN: pattern }); plugins.push(p);
  const d = await p.request({ op: 'describe', bundle: { oldCommit: OLD, newCommit: NEW }, workdir: join(web, 'work', 'test') });
  assert.deepEqual(d.subjects[0].inputs[0].choices, ['badge--sale', 'button--primary', 'card--default']);
  await p.stop();
});

test('through the real plugin host binary (skipped without cargo)', async (t) => {
  if (spawnSync('cargo', ['--version']).status !== 0) return t.skip('no cargo on PATH');
  const build = spawnSync('cargo', ['build', '-q', '-p', 'plugin-host', '--bin', 'plugin-host'], { cwd: root, stdio: 'inherit' });
  assert.equal(build.status, 0);
  const meta = JSON.parse(execFileSync('cargo', ['metadata', '-q', '--format-version', '1', '--no-deps'], { cwd: root, encoding: 'utf8' }));
  const host = join(meta.target_directory, 'debug', 'plugin-host');
  const { b, pattern } = bundleDir();
  const p = new Plugin('detect', root, { WEB_TREES: '', WEB_TREE_PATTERN: pattern },
    [host, '--plugins', web, '--work', join(web, 'work', 'host'), 'detect', b]); plugins.push(p);
  const d = await p.request({ op: 'describe' });
  assert.equal(d.subjects[0].id, 'plan');
  const r = await p.run('plan', {});
  assert.equal(r.results.filter((x: any) => x.output === 'detected').length, 2);
  await p.stop();
});

test('no sample server or browser left running (after shutdown or SIGTERM)', async () => {
  for (const p of plugins) if (p.child.exitCode === null) p.child.kill('SIGTERM');
  await new Promise((r) => setTimeout(r, 1000));
  const left = spawnSync('pgrep', ['-af', shop], { encoding: 'utf8' }).stdout.trim();
  assert.equal(left, '');
});
