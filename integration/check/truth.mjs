// node integration/check/truth.mjs <change> <bundle dir>
// Compares a prepared bundle with that change's row of the ground-truth table in design.md and the facts the run
// established (RUN-LOG). Prints every disagreement on one line ("ground truth: a; b; ...") and exits 1.
// On stderr: a figures line (`figures: ...`) and one `extra: ...` line per difference no requirement claims.
import { existsSync, readFileSync } from 'node:fs';
const [change, dir] = process.argv.slice(2);
const rec = (n) => (existsSync(`${dir}/${n}.json`) ? JSON.parse(readFileSync(`${dir}/${n}.json`, 'utf8')) : null);
const bad = [];
const scope = rec('scope'), parts = rec('parts')?.parts ?? [], tests = rec('tests')?.tests ?? [];
const diffs = rec('differences')?.differences ?? [], groups = rec('groups')?.groups ?? [];
const links = rec('links')?.links ?? [], replays = rec('replays')?.replays ?? [], lean = rec('lean');
const review = rec('ai-review'), mode = rec('mode')?.mode;
const cat = (c) => tests.filter((t) => t.category === c).length;
const eq = (what, got, want) => { if (got !== want) bad.push(`${what} ${got}, expected ${want}`); };
const matched = new Set();
const has = (what, pred) => { const d = diffs.find(pred); if (d) matched.add(d.id); else bad.push(`no difference ${what}`); };
const claimAll = (pred) => diffs.filter(pred).forEach((d) => matched.add(d.id));
const declared = tests.filter((t) => t.category === 'declared' || t.category === 'new-api').map((t) => t.name);
const linkedDeclared = declared.filter((n) => links.some((l) => (l.tests ?? []).includes(n)));
const notLinked = links.filter((l) => l.state === 'not-linked').map((l) => l.block);
const outside = (scope?.notModelled ?? []).filter((n) => /outside compared units/.test(n.reason));
const nmLines = (scope?.notModelled ?? []).reduce((s, n) => s + (n.location.endLine - n.location.line + 1), 0);
const replayOf = (d) => replays.find((r) => r.difference === d.id);

// wording, every change: "reports X" names diagnostic IDs; no opaque model names
for (const d of diffs) for (const side of ['old', 'new']) {
  const t = String(d[side] ?? '');
  if (/^reports/.test(t) && !/\b(MsgPack\d{3}|CS\d{4}|AD\d{4})\b/.test(t)) bad.push(`${d.id} ${side} "${t.slice(0, 60)}" names no diagnostic ID`);
  if (/\b[a-z][A-Za-z]*_[0-9a-f]{4}\b/.test(t)) bad.push(`${d.id} ${side} "${t.slice(0, 60)}" shows a model name`);
}
const allDeclaredLinked = (n) => { eq('declared tests', declared.length, n); eq('declared tests linked to a block', linkedDeclared.length, n); };

if (change === '2283') {
  eq('production lines', scope?.changedLines?.production, 29);
  eq('test lines', scope?.changedLines?.test, 189);
  eq('docs lines', scope?.changedLines?.docs, 10);
  allDeclaredLinked(3);
  has('crash-fixed on the closed generic struct, seen live', (d) => d.kind === 'crash-fixed' && /IndexOutOfRange/.test(d.old) && replayOf(d)?.state === 'seen-live');
  has('data-loss: old reports MsgPack004, new nothing, round trip 42 -> 0', (d) => d.kind === 'data-loss' && /MsgPack004/.test(d.old)
    && /42/.test(JSON.stringify(replayOf(d)?.roundTrip?.sent)) && !/42/.test(JSON.stringify(replayOf(d)?.roundTrip?.received)));
} else if (change === '2234') {
  eq('production lines', scope?.changedLines?.production, 22);
  eq('production lines modelled', scope?.modelledLines, 22);
  eq('lines outside compared units', outside.length, 0);
  eq('declared tests', cat('declared') + cat('new-api'), 4);
  eq('hanging tests', cat('hanging'), 4);
  eq('not-run tests', cat('not-run'), 2);
  eq('declared tests linked to a block', linkedDeclared.length, 4);
  if (notLinked.length) bad.push(`blocks not-linked: ${notLinked.join(' ')}`);
  has('old reports MsgPack005 then throws, new reports MsgPack005 and the member is not added',
    (d) => /MsgPack005/.test(d.old) && /throws/.test(d.old) && /MsgPack005/.test(d.new) && !/throws/.test(d.new));
} else if (change === '2297') {
  eq('largest mechanical group', Math.max(0, ...groups.map((g) => (g.parts ?? []).length)), 402);
  has('output-changed recorded by snapshot GenericDataTypeWithPrivateMember',
    (d) => d.kind === 'output-changed' && /GenericDataTypeWithPrivateMember/.test(d.location?.file ?? ''));
  allDeclaredLinked(2);
} else if (change === '2129') {
  has('public-api: GeneratedMessagePackFormatterAttribute', (d) => d.kind === 'public-api' && /GeneratedMessagePackFormatterAttribute/.test(d.new));
  for (const id of ['MsgPack019', 'MsgPack020', 'MsgPack021', 'MsgPack022']) {
    has(`public-api: new diagnostic ${id}`, (d) => d.kind === 'public-api' && d.new.includes(id));
  }
  if (!diffs.some((d) => d.kind === 'public-api' && /member|\.\w+$/.test(d.new) && !/Attribute$/.test(d.new))) bad.push('no public-api member difference');
  claimAll((d) => d.kind === 'public-api');
  const solver = diffs.filter((d) => d.checker === 'solver');
  if (!solver.length) bad.push('no solver difference');
  solver.forEach((d) => matched.add(d.id));
}

// figures
const kinds = {};
for (const d of diffs) kinds[d.kind] = (kinds[d.kind] ?? 0) + 1;
const lr = lean?.rows ?? [];
const seen = replays.filter((r) => r.state === 'seen-live').length;
const reasons = {};
for (const r of replays.filter((x) => x.state !== 'seen-live')) { const v = r.reason?.kind ?? r.reason ?? 'none'; const k = typeof v === 'string' ? v : JSON.stringify(v); reasons[k] = (reasons[k] ?? 0) + 1; }
const lk = links.map((l) => `${l.block}:${l.state}${l.tests?.length ? `(${l.tests.length})` : ''}`).join(' ');
console.error(`figures: ${diffs.length} differences ${JSON.stringify(kinds)}; lines modelled ${scope?.modelledLines}/${scope?.changedLines?.production} (not modelled ${nmLines}, outside units ${outside.length}); `
  + `Lean ${lr.filter((r) => r.state === 'checked' || r.state === 'model-checked').length}/${lr.length}; replays seen-live ${seen}/${replays.length} ${JSON.stringify(reasons)}; `
  + `links ${lk}; review ${review?.statements?.length ?? 0} kept / ${review?.dropped?.length ?? 0} dropped; mode ${mode}`);
for (const d of diffs) if (!matched.has(d.id)) console.error(`extra: ${d.id} ${d.kind} ${d.location?.file?.split('/').pop()}:${d.location?.line} ${d.old} -> ${d.new}`);
if (bad.length) { console.log(`ground truth: ${bad.join('; ')}`); process.exit(1); }
