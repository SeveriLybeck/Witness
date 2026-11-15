// tiny-shop (new): static page plus a JSON API. PORT=0 picks a free port; prints "listening <port>".
import { createServer } from 'node:http';
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
const here = dirname(fileURLToPath(import.meta.url));
const products = [{ id: 1, name: 'Mug', price: 1200 }, { id: 2, name: 'Shirt', price: 2500 }, { id: 3, name: 'Cap', price: 1800 }];
const price = (p) => ({ ...p, price: Math.round(p.price * 0.9) });
const send = (res, code, body, type = 'application/json') => { res.writeHead(code, { 'content-type': type }); res.end(typeof body === 'string' ? body : JSON.stringify(body)); };
const server = createServer((req, res) => {
  const url = new URL(req.url, 'http://x');
  if (url.pathname === '/api/products') return send(res, 200, products.map(price));
  const m = url.pathname.match(/^\/api\/products\/(\d+)$/);
  if (m) { const p = products.find((x) => x.id === Number(m[1])); return p ? send(res, 200, price(p)) : send(res, 404, { error: 'not found' }); }
  if (url.pathname === '/api/cart' && req.method === 'POST') {
    let body = ''; req.on('data', (c) => (body += c));
    return req.on('end', () => { const ids = JSON.parse(body || '[]'); const items = products.filter((p) => ids.includes(p.id)).map(price); send(res, 200, { items, total: items.reduce((s, p) => s + p.price, 0) }); });
  }
  if (url.pathname === '/openapi.json') return send(res, 200, readFileSync(join(here, 'openapi.json'), 'utf8'));
  if (url.pathname === '/' || url.pathname === '/index.html') return send(res, 200, readFileSync(join(here, 'public/index.html'), 'utf8'), 'text/html');
  send(res, 404, { error: 'not found' });
});
server.listen(Number(process.env.PORT ?? 3000), '127.0.0.1', () => console.log('listening ' + server.address().port));
