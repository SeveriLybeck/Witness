3efb84947716c040074aa020ef0af6c3809a0e11

- T5 (free): Playwright browsers from nix `playwright-driver.browsers`, npm `playwright-core` pinned to 1.59.1; node 24 with npm in place of bun (no bun lockfile).
- T2 (free): `nixpacks` is not in the verify shell; the detect viewer uses `nixpacks plan` when on PATH, else the plans recorded with nixpacks 1.41.0 (`samples/tiny-shop/nixpacks-plan.{old,new}.json`).
- No playbook entry for Storybook: the two static Storybook builds of tiny-shop are hand-written in the static build layout (`index.json` v5, `iframe.html?id=<story>`) instead of built by Storybook (its toolchain is several hundred MB and network-bound).
