#!/bin/sh
# npm test: Playwright browsers from nix (playwright-driver.browsers, chromium-1217 for playwright-core 1.59.1).
set -e
export PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1
export PLAYWRIGHT_BROWSERS_PATH="${PLAYWRIGHT_BROWSERS_PATH:-$(nix-build '<nixpkgs>' -A playwright-driver.browsers --no-out-link)}"
exec node --test --test-concurrency=1 --test-timeout=90000 test/*.test.ts
