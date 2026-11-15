import { afterEach, describe, expect, it, vi } from 'vitest';
import { httpSource } from '../src/shell/data';
import { fixtureSource } from './harness';

afterEach(() => vi.unstubAllGlobals());

describe('source.request headers', () => {
  it('httpSource sends extra headers with the JSON body', async () => {
    const fetch = vi.fn(async () => new Response('{"ok":true}', { status: 200 }));
    vi.stubGlobal('fetch', fetch);
    const r = await httpSource().request('POST', '/api/sessions/k/a/review', { event: 'APPROVE' }, { Authorization: 'Bearer t' });
    expect(r).toEqual({ ok: true });
    const init = (fetch.mock.calls[0] as unknown[])[1] as RequestInit;
    expect(init.headers).toEqual({ 'content-type': 'application/json', Authorization: 'Bearer t' });
  });

  it('fixtureSource passes headers to onRequest', async () => {
    const seen: unknown[] = [];
    const s = fixtureSource('2283', { onRequest: (...a) => { seen.push(a); return null; } });
    await s.request('POST', '/x', 1, { Authorization: 'Bearer t' });
    expect(seen).toEqual([['POST', '/x', 1, { Authorization: 'Bearer t' }]]);
  });
});
