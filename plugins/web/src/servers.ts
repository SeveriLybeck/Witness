// Starts the old and new trees of the sample (`node server.js`, PORT=0) and the API proxy.
import { spawn, type ChildProcess } from 'node:child_process';
import { createServer, request as httpRequest, type Server } from 'node:http';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { join, dirname } from 'node:path';

export type Side = 'old' | 'new';

export interface App { side: Side; port: number; url: string; child: ChildProcess }

export function startApp(side: Side, dir: string): Promise<App> {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [join(dir, 'server.js')], { cwd: dir, env: { ...process.env, PORT: '0' }, stdio: ['ignore', 'pipe', 'ignore'] });
    let buf = '';
    child.stdout!.on('data', (c) => {
      buf += c;
      const m = buf.match(/listening (\d+)/);
      if (m) resolve({ side, port: Number(m[1]), url: `http://127.0.0.1:${m[1]}`, child });
    });
    child.on('exit', (code) => reject(new Error(`${side} app exited ${code}`)));
  });
}

export interface Reply { status: number; body: string; type: string }

export function send(base: string, method: string, path: string, body?: string): Promise<Reply> {
  return new Promise((resolve, reject) => {
    const u = new URL(path, base);
    const req = httpRequest(u, { method, headers: body ? { 'content-type': 'application/json' } : {} }, (res) => {
      let data = '';
      res.on('data', (c) => (data += c));
      res.on('end', () => resolve({ status: res.statusCode ?? 0, body: data, type: String(res.headers['content-type'] ?? '') }));
    });
    req.on('error', reject);
    if (body) req.write(body);
    req.end();
  });
}

export interface Exchange { method: string; path: string; body: string; old: Reply; new: Reply }


const scalarJs = () => readFileSync(join(dirname(fileURLToPath(import.meta.url)), '..', 'node_modules', '@scalar', 'api-reference', 'dist', 'browser', 'standalone.js'));

/** Proxy: `/` is the Scalar console, `/openapi.json` the new spec with this proxy as server,
 *  every other request goes to new (its answer is returned) and is replayed against old. */
export function startProxy(oldApp: App, newApp: App, log: Exchange[]): Promise<{ url: string; server: Server }> {
  const server = createServer(async (req, res) => {
    const path = req.url ?? '/';
    if (path === '/' || path === '/console') {
      res.writeHead(200, { 'content-type': 'text/html' });
      return res.end(`<!doctype html><html><head><meta charset="utf-8"><title>API console</title></head><body>
<script id="api-reference" data-url="/openapi.json"></script><script src="/scalar.js"></script></body></html>`);
    }
    if (path === '/scalar.js') { res.writeHead(200, { 'content-type': 'text/javascript' }); return res.end(scalarJs()); }
    if (path === '/openapi.json') {
      const spec = JSON.parse((await send(newApp.url, 'GET', '/openapi.json')).body);
      spec.servers = [{ url: `http://${req.headers.host}` }];
      res.writeHead(200, { 'content-type': 'application/json' });
      return res.end(JSON.stringify(spec));
    }
    let body = '';
    for await (const c of req) body += c;
    try {
      const n = await send(newApp.url, req.method ?? 'GET', path, body || undefined);
      const o = await send(oldApp.url, req.method ?? 'GET', path, body || undefined);
      log.push({ method: req.method ?? 'GET', path, body, old: o, new: n });
      res.writeHead(n.status, { 'content-type': n.type, 'access-control-allow-origin': '*' });
      res.end(n.body);
    } catch (e) { res.writeHead(502); res.end(String(e)); }
  });
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => {
    const a = server.address() as { port: number };
    resolve({ url: `http://127.0.0.1:${a.port}`, server });
  }));
}
