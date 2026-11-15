// node integration/check/sections.mjs <bundle dir>: prints the first section that is not ready (review may be
// unavailable when no recording exists, D46) and exits 0 either way; prints nothing when all are ready.
import { readFileSync } from 'node:fs';
const b = JSON.parse(readFileSync(`${process.argv[2]}/bundle.json`, 'utf8'));
for (const s of ['map', 'proof', 'playground', 'tests', 'rules', 'review']) {
  const st = b.sections?.[s] ?? { state: 'absent' };
  if (st.state === 'ready' || (s === 'review' && st.state === 'unavailable')) continue;
  console.log(`section ${s} ${st.state}${st.detail ? ` (${st.detail.slice(0, 160)})` : ''}`);
  break;
}
