# web — status (group 22)

Works: four web viewers over the plugin line protocol, tested on `samples/tiny-shop`: site preview at three device sizes (Playwright), API console (Scalar 1.78.1) behind a proxy that replays each request against old, component gallery from two static Storybook builds, repository detection from a nixpacks plan. Tests drive the real `bin/web` process, and the real `plugin-host` binary when cargo is on PATH.
Run: `nix-shell -p nodejs playwright-driver.browsers --run 'npm --prefix plugins/web ci && npm --prefix plugins/web test'` (9 tests, about 9 s; the host test is skipped without cargo; add `cargo rustc gcc pkg-config` to run it). `test/run.sh` sets `PLAYWRIGHT_BROWSERS_PATH` from `nix-build '<nixpkgs>' -A playwright-driver.browsers` when unset.

Interface
- Plugins `site-preview`, `api-console`, `gallery`, `detect`: `plugins/web/<name>/plugin.json`, name = folder. Host: `plugin-host --plugins plugins/web <name> <bundle dir>`.
- Detection at the repository root: `public/index.html`, `openapi.json`, `storybook-static/index.json`, `package.json`. Each of tiny-shop's `old/` and `new/` matches all four (tabs); the generator lab does not match.
- Executable `bin/web --plugin <name>` (node 24 on PATH).
- Trees: `$WEB_TREES` (folder with `old/` and `new/`, the sample's layout) overrides; else the commits of `describe`'s `bundle` or `$PLUGIN_BUNDLE_DIR/bundle.json` fill the first existing pattern of `$WEB_TREE_PATTERN` or `web.json` `trees` (`…/{commit}`, relative to the repo root). No tree → `ok: false`.
- Outputs: `site` → `screens` image-pair; `api` (method, path, body) → `console` image (Scalar screenshot), `response` text-diff `{old, new}` as `status\nbody`, `exchanges` table `[method, path, old, new, same]`; `stories` (story) → `stories` table `[story, old, new]`, `story` image-pair; `plan` → `plan` text-diff, `detected` facts per side (provider, start, install, nixPkgs, source `nixpacks|recorded|unavailable`).
- Every image value is a `data:image/png;base64,…` URL (`image` as `{path: <data URL>}`, `image-pair` as `{old, new}`). No `page` outputs: the servers and Scalar listen inside the plugin's network namespace, unreachable from the reviewer's browser, so the page gets screenshots instead until the contract has a proxy.
- Servers and Chromium stop on `shutdown`, stdin close, SIGTERM/SIGINT/SIGHUP; a test checks that none is left running.

Deviations and fallbacks
- Sample has no git history: old and new are two folders, used through `$WEB_TREES`; the tests also run it through bundle commits with symlinked trees.
- Storybook builds are hand-written in Storybook's static layout (`index.json` v5, `iframe.html?id=`), not built with Storybook (FALLBACKS.md).
- nixpacks is not in the verify shell: `detect` runs `nixpacks plan` when on PATH, else reads `<tree>/../nixpacks-plan.<side>.json` (recorded with nixpacks 1.41.0 for tiny-shop).
- Site preview and API console need loopback inside the host's network namespace (the host is to bring `lo` up); the host test uses `detect`, which needs none.
- Chromium runs with `chromiumSandbox: false` inside the plugin.
- A test checks that none of the excluded components appears in `package-lock.json`.
