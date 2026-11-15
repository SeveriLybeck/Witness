// One reviewer's session on one bundle, kept by the gateway (D41, D48). The page only shows it.
import { useCallback, useEffect, useSyncExternalStore } from 'react';
import type { BundleData, DataSource } from '../../shell';

export interface Session {
  oldCommit: string; newCommit: string; reviewer: string;
  opened: string[]; shown: string[];
  runs: { difference: string | null; input: string }[];
  answers: Record<string, 'accept' | 'return'>;
  spotCheck: string | null;
}
export interface Review { event: string; body: string; result?: { status: number; url: string | null } | null }
export type Outcome =
  | { kind: 'submitted'; review: Review }
  | { kind: 'refused'; status: number; gate: string | null; error: string; items: string[] };

export interface State {
  reviewer: string;
  session: Session | null;
  stale: boolean;
  busy: boolean;
  outcome: Outcome | null;
}

const reviewerKey = 'witness.reviewer';
function storedReviewer(): string {
  try { return localStorage.getItem(reviewerKey) ?? ''; } catch { return ''; }
}

interface Store { state: State; listeners: Set<() => void>; loading: string | null }
const stores = new Map<string, Store>();

function store(key: string): Store {
  let s = stores.get(key);
  if (!s) {
    s = { state: { reviewer: storedReviewer(), session: null, stale: false, busy: false, outcome: null }, listeners: new Set(), loading: null };
    stores.set(key, s);
  }
  return s;
}
function set(key: string, patch: Partial<State>) {
  const s = store(key);
  s.state = { ...s.state, ...patch };
  s.listeners.forEach((l) => l());
}

/** Forget every store (tests). */
export function resetSessions() { stores.clear(); picks.clear(); }

const sessionPath = (key: string, reviewer: string) => `/api/sessions/${key}/${encodeURIComponent(reviewer)}`;

/** POST the review with the reviewer's token in `Authorization` (gateway-api.md). Refusals come back as `{status, data}`. */
async function postReview(source: DataSource, path: string, body: unknown, token: string): Promise<{ status: number; data: unknown }> {
  try {
    const data = await source.request('POST', path, body, token ? { authorization: `Bearer ${token}` } : {});
    return { status: 200, data };
  } catch (e) {
    const err = e as { status?: number; data?: unknown };
    if (typeof err.status !== 'number') throw e;
    return { status: err.status, data: err.data ?? null };
  }
}

/**
 * The random level-1 pick shown before a session exists (D48): kept in this browser per bundle until a
 * reviewer's session stores it; the session's own pick wins once there is one.
 */
const picks = new Map<string, string>();
export function pendingSpot(key: string, level1: string[]): string | null {
  if (!level1.length) return null;
  const k = `witness.spot.${key}`;
  let p = picks.get(key) ?? null;
  if (!p) { try { p = localStorage.getItem(k); } catch { p = null; } }
  if (!p || !level1.includes(p)) {
    p = level1[Math.floor(Math.random() * level1.length)];
    try { localStorage.setItem(k, p); } catch { /* per-viewer convenience only */ }
  }
  picks.set(key, p);
  return p;
}

export function useSession(data: BundleData, source: DataSource) {
  const key = data.key;
  const s = store(key);
  const state = useSyncExternalStore(
    (l) => { s.listeners.add(l); return () => { s.listeners.delete(l); }; },
    () => s.state,
  );

  const level1 = data.levels.filter((l) => l.level === 1).map((l) => l.block);

  const load = useCallback(async () => {
    const r = store(key).state.reviewer;
    if (!r || store(key).loading === r) return;
    store(key).loading = r;
    try {
      let sess = (await source.request('GET', sessionPath(key, r))) as Session;
      if (!sess.spotCheck && level1.length) {
        const pick = pendingSpot(key, level1)!;
        sess =(await source.request('POST', sessionPath(key, r), { spotCheck: pick })) as Session;
      }
      if (store(key).state.reviewer === r) set(key, { session: sess });
    } catch { /* gateway unreachable: no session shown */ } finally {
      store(key).loading = null;
    }
  }, [key, source, level1.join()]);

  useEffect(() => { if (!state.session) void load(); }, [load, state.session, state.reviewer]);

  const update = useCallback(async (body: Record<string, unknown>) => {
    const r = store(key).state.reviewer;
    if (!r) return;
    try {
      const sess = (await source.request('POST', sessionPath(key, r), body)) as Session;
      set(key, { session: sess });
    } catch { /* kept as it was */ }
  }, [key, source]);

  const actions = {
    setReviewer(name: string) {
      try { localStorage.setItem(reviewerKey, name); } catch { /* per-viewer convenience only */ }
      set(key, { reviewer: name, session: null, outcome: null });
    },
    async refresh() {
      const r = store(key).state.reviewer;
      if (!r || !store(key).state.session) return;
      try { set(key, { session: (await source.request('GET', sessionPath(key, r))) as Session }); } catch { /* kept */ }
    },
    answer: (question: string, value: 'accept' | 'return') => update({ answer: { question, value } }),
    open: (block: string) => update({ opened: [block] }),
    show: (parts: string[]) => update({ shown: parts }),
    async submit(risk: string, token: string) {
      const r = store(key).state.reviewer;
      if (!r) return;
      set(key, { busy: true, outcome: null });
      try {
        const res = await postReview(source, `${sessionPath(key, r)}/review`, { risk }, token);
        const d = (res.data ?? {}) as { review?: Review; error?: string; gate?: string | null; items?: string[] };
        if (res.status < 400 && d.review) set(key, { outcome: { kind: 'submitted', review: d.review } });
        else {
          set(key, {
            outcome: { kind: 'refused', status: res.status, gate: d.gate ?? null, error: d.error ?? '', items: d.items ?? [] },
            stale: d.gate === 'head-moved' || store(key).state.stale,
          });
        }
      } catch {
        set(key, { outcome: { kind: 'refused', status: 0, gate: null, error: '', items: [] } });
      } finally {
        set(key, { busy: false });
      }
    },
  };
  return { state, actions };
}
