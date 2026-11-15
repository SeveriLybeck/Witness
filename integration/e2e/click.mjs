// node integration/e2e/click.mjs <change> <bundle key> <gateway url> <gateway data dir> <bundle dir>
// Click-through in a real browser (D8, D41, D46, review-page spec). Prints `ok` or the first failing step; exit 1.
// Every change: start page lists the four changes with values; first screen within the length budget and the review
// box within 400 px; no horizontal overflow at 1280x900 and 390x844 as loaded, all unfolded, and with a map block
// open; no absolute host path in any shown text; one citation of each kind present lands on its target; the Checks
// sheet lists lean.json's assumed facts; a playground preset runs through the gateway's viewers endpoint; sign-off
// to an approval in the mode the rules give. 2283 also: `42 → 0` visible on load, a group row opens, the data-loss
// preset shows the lost value.
import { chromium } from 'playwright-core';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const [change, key, base, data, bundleDir] = process.argv.slice(2);
const T = 30_000;
const BUDGET = { 2283: 1800, 2234: 1800, 2297: 1350, 2129: 2700 }[change] ?? 2700;
let step = 'launch';
const rec = (n) => (bundleDir && existsSync(join(bundleDir, `${n}.json`)) ? JSON.parse(readFileSync(join(bundleDir, `${n}.json`), 'utf8')) : null);
const lean = rec('lean'), diffs = rec('differences')?.differences ?? [];
const browser = await chromium.launch({ executablePath: process.env.WITNESS_CHROMIUM || undefined });
const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
const log = [];
page.on('pageerror', (e) => { log.push(String(e)); console.error('pageerror', e.stack); });
const viewerCalls = [];
page.on('request', (r) => { if (/\/api\/bundles\/[^/]+\/viewers/.test(r.url())) viewerCalls.push(r.url().replace(/^https?:\/\/[^/]+/, '')); });
const fail = (m) => { throw new Error(m); };

async function overflow(when) {
  for (const [w, h] of [[1280, 900], [390, 844]]) {
    await page.setViewportSize({ width: w, height: h });
    await page.waitForTimeout(300);
    const m = await page.evaluate(() => {
      const sw = document.documentElement.scrollWidth, iw = window.innerWidth;
      let worst = null, right = iw;
      for (const el of document.querySelectorAll('body *')) { const r = el.getBoundingClientRect(); if (r.right > right + 0.5) { right = r.right; worst = el; } }
      const sel = worst ? `${worst.tagName.toLowerCase()}${typeof worst.className === 'string' && worst.className.trim() ? '.' + worst.className.trim().split(/\s+/).join('.') : ''}` : '';
      return { sw, iw, sel, slot: worst?.closest('section[id^="slot-"]')?.id ?? '', right: Math.round(right) };
    });
    if (m.sw > m.iw) fail(`NEEDS web: #${m.slot} ${m.sel}, scrollWidth ${m.sw} > ${m.iw} (right ${m.right}), ${change}, ${w}x${h}, ${when}`);
  }
  await page.setViewportSize({ width: 1280, height: 900 });
}
async function hostPaths(when) {
  const t = await page.evaluate(() => document.body.innerText);
  const m = t.match(/[^\s"'`]*(\/home\/|\/tmp\/|\/nix\/store)[^\s"'`]*/);
  if (m) fail(`NEEDS web: host path "${m[0].slice(0, 90)}" shown, ${change}, ${when}`);
}
async function unfold(slot) {
  const t = page.locator(`#slot-${slot} .slot-toggle`);
  if (await t.count() && (await t.getAttribute('aria-expanded')) === 'false') await t.click();
}
async function landed(slot, kind, c) {
  const sel = `#slot-${slot} [data-flash], #slot-${slot} [aria-selected="true"], #slot-${slot} [aria-expanded="true"][data-node], #slot-${slot} .mnode-head[aria-expanded="true"], #slot-${slot} [aria-current]`;
  try { await page.locator(sel).first().waitFor({ timeout: 3_000 }); } catch { fail(`NEEDS web: cite ${c} (${kind}) reached no target in #slot-${slot}, ${change}, 1280x900`); }
}

try {
  step = 'start page';
  await page.goto(`${base}/`);
  const rows = page.locator('tr[data-change]');
  await rows.first().waitFor({ timeout: T });
  if ((await rows.count()) !== 4) fail(`start page lists ${await rows.count()} changes, expected 4`);
  for (let i = 0; i < 4; i++) {
    const vals = await rows.nth(i).locator('td.n').allInnerTexts();
    if (!vals.some((v) => v.trim() !== '')) fail(`start page row ${await rows.nth(i).getAttribute('data-change')} shows no values`);
  }

  step = 'first screen';
  await page.goto(`${base}/#/${key}`);
  await page.locator('#slot-review').waitFor({ timeout: T });
  await page.waitForTimeout(1000);
  const sh = await page.evaluate(() => document.documentElement.scrollHeight);
  if (sh > BUDGET) fail(`NEEDS web: document scrollHeight ${sh} > ${BUDGET}, ${change}, 1280x900 as loaded`);
  const rb = await page.locator('#slot-review [data-review-box]').first().boundingBox();
  if (rb && rb.height > 400) fail(`NEEDS web: #slot-review [data-review-box] height ${Math.round(rb.height)} > 400, ${change}, 1280x900`);
  if (change === '2283') {
    const vis = await page.evaluate(() => {
      for (const el of document.querySelectorAll('body *')) {
        if (el.children.length || !/42\s*→\s*0/.test(el.textContent ?? '')) continue;
        const r = el.getBoundingClientRect();
        if (r.width && r.top >= 0 && r.bottom <= window.innerHeight) return true;
      }
      return false;
    });
    if (!vis) fail('NEEDS web: text "42 → 0" not visible without scrolling, 2283, 1280x900');
  }
  await hostPaths('as loaded');
  await overflow('as loaded');
  console.error(`first screen ${sh}px, review ${Math.round(rb?.height ?? 0)}px`);

  step = 'citations';
  const kindOf = (c) => /^d\d+$/.test(c) ? 'difference' : /^b\d+$/.test(c) ? 'block' : /^q\d+$/.test(c) ? 'question'
    : /^preset:d\d+$/.test(c) ? 'preset' : /^[\w.]+\.\w+$/.test(c) && !/\.(cs|tt|md)$/.test(c) ? 'test' : null;
  const slotOf = { difference: 'proof', block: 'map', question: 'decide', preset: 'playground', test: 'proof' };
  const openFull = async () => {
    await page.locator('#slot-review [data-fulltext]').first().click();
    const full = page.locator('[data-testid="review-full"]');
    await full.waitFor({ timeout: T });
    while (await full.locator('[data-more-cites]').count()) { await full.locator('[data-more-cites]').first().click(); await page.waitForTimeout(100); }
    return full;
  };
  const followed = [];
  if (await page.locator('#slot-review [data-fulltext]').count()) {
    let full = await openFull();
    const chips = await full.locator('[data-chip]').evaluateAll((els) => els.map((e) => e.getAttribute('data-chip')));
    await page.keyboard.press('Escape');
    for (const kind of ['difference', 'block', 'question', 'test', 'preset']) {
      const c = chips.find((x) => kindOf(x ?? '') === kind);
      if (!c) continue;
      full = await openFull();
      await full.locator(`[data-chip="${c}"]`).first().click();
      await page.waitForTimeout(200);
      await landed(slotOf[kind], kind, c);
      followed.push(`${kind}:${c}`);
      if (await page.locator('[data-testid="review-full"]').count()) await page.keyboard.press('Escape');
    }
    console.error(`cites followed ${followed.join(' ') || 'none'} (of ${chips.length} chips)`);
  } else if (!(await page.getByText('no AI review').count())) fail('review has neither full text nor "no AI review"');

  step = 'everything unfolded';
  for (const s of ['review', 'proof', 'playground', 'map', 'signoff']) await unfold(s);
  await page.waitForTimeout(800);
  await overflow('all unfolded');
  await hostPaths('all unfolded');

  step = 'Checks sheet facts';
  const facts = (lean?.coverage ?? []).reduce((n, c) => n + (c.facts?.length ?? 0), 0);
  await page.locator('#slot-proof [data-checks]').first().click();
  if (facts > 0) {
    const sum = page.locator('summary', { hasText: /\d+ facts?/ });
    await sum.first().waitFor({ timeout: T });
    const shown = (await sum.allInnerTexts()).reduce((n, t) => n + Number(t.match(/(\d+)/)[1]), 0);
    if (shown !== facts) fail(`NEEDS web: Checks sheet lists ${shown} facts, lean.json ${facts}, ${change}`);
  }
  await hostPaths('Checks sheet');
  await page.keyboard.press('Escape');
  console.error(`Checks facts ${facts}`);

  step = 'proof group row and test output';
  {
    const row = page.locator('#slot-proof tr[data-diff]').first();
    if (await row.count()) {
      await row.click();
      await page.locator('#slot-proof tr[aria-selected="true"]').first().waitFor({ timeout: T });
    }
    const line = page.locator('#slot-proof .proof-tests-line').first();
    if (await line.count()) { await line.click(); await page.waitForTimeout(300); }
    await hostPaths('test output opened');
  }

  step = 'playground preset through viewers';
  {
    const pg = page.locator('#slot-playground [data-playground]');
    await pg.waitFor({ timeout: T });
    if (!viewerCalls.length) fail('page never called /api/bundles/<key>/viewers');
    const loss = diffs.find((d) => d.kind === 'data-loss');
    const p = change === '2283' && loss ? pg.locator(`[data-preset="${loss.id}"]`) : pg.locator('[data-kind="difference"]:not([disabled]), [data-kind="issue"]:not([disabled])');
    if (change === '2283' && !(await p.count())) fail(`no data-loss preset ${loss?.id}`);
    await (await p.count() ? p.first() : pg.locator('[data-kind="free"]').first()).click();
    await pg.locator('[data-area="inputs"] button', { hasText: /run/i }).first().click();
    await pg.locator('[data-side="new"] [data-output]').first().waitFor({ timeout: 120_000 });
    await pg.locator('[data-area="inputs"] button', { hasText: /running/i }).waitFor({ state: 'detached', timeout: 120_000 });
    if (change === '2283') {
      await page.waitForTimeout(500);
      const t = await pg.innerText();
      if (!/42\s*(->|→)\s*0/.test(t)) fail(`NEEDS web: data-loss preset ${loss.id} shows no lost value (42 → 0), 2283`);
    }
    await hostPaths('playground output');
    console.error(`viewers ${viewerCalls[0]}`);
  }

  step = 'map block open';
  {
    const head = page.locator('#slot-map [data-node] .mnode-head').first();
    if ((await head.getAttribute('aria-expanded')) !== 'true') await head.click();
    await page.waitForTimeout(400);
    await overflow('map block open');
  }

  step = 'sign-off: reviewer';
  const so = page.locator('#slot-signoff');
  const name = so.locator('[data-reviewer]');
  await name.fill('reviewer-a');
  await name.blur();
  await page.waitForTimeout(300);

  step = 'sign-off: decisions';
  if (await page.locator('#slot-decide [data-all], #slot-review [data-all]').count()) { await page.locator('#slot-decide [data-all], #slot-review [data-all]').first().click(); await page.waitForTimeout(300); }
  const qs = page.locator('#slot-decide li[data-question]');
  for (let i = 0; i < await qs.count(); i++) {
    const q = qs.nth(i);
    await q.getByRole('button', { name: 'Accept' }).first().click();
    await q.getByRole('button', { name: 'Accept', pressed: true }).first().waitFor({ timeout: T });
  }

  step = 'sign-off: spot-check';
  const spot = so.locator('[data-spotcheck] button').first();
  if (await spot.count()) await spot.click();

  step = 'sign-off: level-3 blocks';
  if (await so.locator('[data-mode="full"]').count() || await page.locator('[data-mode="full"]').count()) {
    const l3 = page.locator('#slot-map [data-node][data-level="3"]');
    for (let i = 0; i < await l3.count(); i++) {
      const open = page.locator('#slot-map .mnode-head[aria-expanded="true"]');
      while (await open.count()) { await open.first().click(); await page.waitForTimeout(250); }
      const head = l3.nth(i).locator('.mnode-head');
      await head.scrollIntoViewIfNeeded();
      await head.click({ timeout: 5_000 });
      await page.waitForTimeout(250);
    }
  }

  step = 'sign-off: submit';
  await so.locator('textarea').fill('Library types with unkeyed members now build silently.');
  if (await so.locator('input[type="password"]').count()) await so.locator('input[type="password"]').fill('reviewer-token');
  await page.waitForTimeout(500);
  await so.locator('[data-submit]').click();
  const out = so.locator('[data-outcome]');
  await out.waitFor({ timeout: T });
  if ((await out.getAttribute('data-outcome')) !== 'submitted') fail(`refused: ${await out.innerText()}`);

  step = 'stand-in recorded review';
  const f = join(data, 'sessions', key, 'reviewer-a', 'review.json');
  if (!existsSync(f)) fail('no review.json');
  const r = JSON.parse(readFileSync(f, 'utf8'));
  console.error(`review ${r.event ?? r.state ?? ''}`);
  console.log('ok');
} catch (e) {
  console.log(`${step}: ${String(e.message ?? e).split('\n')[0]}${log.length ? ` (page: ${log[0]})` : ''}`);
  process.exitCode = 1;
} finally {
  await browser.close();
}
