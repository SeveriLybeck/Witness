# map — status (group 17)

Works: grid of uniform nodes (name up to two lines, level glyph; numbers, `not linked` and `N call sites` on their own row; file on hover/focus), always at scale 1 or more; long changes split into as many columns as the width holds. The map is as tall as the graph and scrolls sideways: drag pans (page scrolls vertically), ctrl/cmd+wheel or pinch zooms 1–2x, plain wheel scrolls the page. Click opens a block in place, wider (up to 400 px, view width at 390) with wrapped rules and parts (`name · File.cs:line`, a part opens the code panel); its measured height and width re-lay the grid on both axes. Camera scrolls to the block plus 120 px neighbour stubs (the block alone when stubs do not fit); outside click, Esc or `Fit map` closes and returns. Dependency lines (old dashed, changed heavy); call-site chip from `dependencies.json` `reach`; mechanical groups as one quiet entry each; one "Declared" region, `running` while tests run. 17 tests.
Run: `nix-shell -p nodejs --run 'npm --prefix web ci && npm --prefix web test -- src/sections/map'`.

Interface
- `index.tsx` exports `sections = [{ slot: 'map', Component: MapSection, headline }]`; headline `6 blocks · 1 outside`.
- Reads `blocks parts levels differences links dependencies groups`; missing records draw nothing. Nodes carry `data-node`/`data-block`.
- Shell use: `selection.block` opens that block; opening calls `focusBlock`; `highlight.blocks` outlines nodes; palette `Fit map`.
- `layout.ts` is pure: `layout(data, {vw, heights, widths})`, `columnsOf`, `columnsFor`, `focusBox`, `scrollFor`, `overlaps`.
- CSS in `map.css`, injected once as `#witness-map-css`, tokens only. Rule strings `kind:rule` show the part after `:` as words.

Deviations and fallbacks
- Own absolute-positioned layout instead of `@xyflow/react`. Native horizontal scroll in place of a free camera so every head is reachable and Playwright-clickable; no fit below scale 1 (text under 15 px).
- Declared blocks stack at the top of each column inside the region; all others start below it.
- 2297 `groups.json` holds one group of 402 (not two variants): one entry `402 identical edits`. Edge `sites` are not drawn as chips; `reach` is absent in fixtures, so the test feeds that shape.
- Pass-through plumbing members are not drawn (no field on the edge).

Real browser (Chromium 1217 from nix, vite dev proxied to a gateway on 9490 serving copies of `replays/<change>/bundle`; probe opens blocks b1–b6, plus 2129 b22 b29 b36 and 2297 b3 b4, and trial-clicks every other head)
- All four changes at 1280×900 and 390×844: smallest node text 15 px, 0 clipped number chips, 0 opened-node clipping, 0 overlaps, 0 unclickable heads with any probed block open.
- Before on the same probe: 2129@1280 text 6.3 px, 13 clipped chips, 34 truncated names; b22/b29/b36 open left 16/21/33 heads unclickable; 2297@390 b3/b4 open left 2; opened 2283 b1/b2 clipped 5 elements each. After: 2129 still has 8 names longer than two lines (ellipsis, full name in the title).
