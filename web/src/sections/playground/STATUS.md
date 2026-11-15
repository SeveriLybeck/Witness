# playground — status (group 16)

Works: viewers from `GET /api/bundles/<key>/viewers` (tabs only when `tabs`; empty list → `unavailable`), describe,
subject selector, Definition with code-panel locations, role-named input slots (`constructible: false`/`field: null`
marked "not constructible", no field), Old/New/Reverted columns per declared output, renderers for the eight result
types only, presets (one per difference in `replays.json` with its source; `not-replayed` ones disabled with their
reason, detail as tooltip; issue; revert; free), shared difference selection loads its replay. 10 tests.
Run: `nix-shell -p nodejs --run 'npm --prefix web ci && npm --prefix web test -- src/sections/playground'`.
Checked in Chromium at 1280 and 390 against the real gateway, plugin host and generator lab on 2283.

Public interface
- `index.tsx` exports `sections = [{ slot: 'playground', Component: Playground }]`; no headline.
- Calls via `source.request`: `GET /api/bundles/<key>/viewers`; `POST /api/bundles/<key>/playground` with
  `{plugin, request}`, `request` = `{id, op: 'describe'}` or `{id, op: 'run', subject, sides: ['old','new'], preset: {kind, difference?|block?}, inputs}`.
- Preset buttons: `data-preset="dN|reproduction|revert|free"`, `data-cite="preset:dN|reproduction"`, `data-kind`.
- `page` results: relative URL → `/api/bundles/<key>/pages/<plugin>/<path>` in `<iframe sandbox="allow-scripts">`;
  absolute URLs are not framed (`blocked`). Images: `data:image/` or relative via the same route.
- `protocol.ts`: types, `viewersOf`, `call`, `presetsFor`, `splitReplay`, `pageSrc`, `imageSrc`, `props`.

Deviations and fallbacks
- Replay text split at `// lib` / `// app`; unmarked text is `app`. Plugin difference preset inputs win over the
  replay split; replay `input.buildProperties` fills `buildProperties` (text field, one `Key=Value` per line).
- Revert keeps the loaded input and reruns it (the plugin's revert preset has no input of its own).
- Null facts render empty; object facts as JSON.
- Runs are not recorded to the session (`sessionUpdate.run`); left to the review/sign-off owner.
- Styles: inline `<style>` in the section, shell tokens only.
