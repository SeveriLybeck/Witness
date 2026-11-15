# intake — status

Works: pasted link to a plan. Parses `<repo>/pull/<n>` and `<repo>/compare/<old>...<new>` (any git URL or path, `file://` included), clones into `<work>/clone`, fetches `refs/pull/<n>/head`, takes old as merge-base with `origin/HEAD`, sorts changed files by language, plans map (parser) and proof (front end), offers a playground for recognised stacks, starts it only on a click, runs `witness prepare` with streamed progress and no time cap.
Run: `nix-shell -p cargo rustc gcc pkg-config git --run 'cargo test -p intake'` (4 tests, local repositories only, no network).
CLI: `witness-intake [--work DIR] [--playground] <link> [--prepare CMD...]`; progress lines on stderr (`fetch: clone`, `fetch: refs`, `prepare: <line>`, `prepare: exit N`), plan JSON on stdout. `--prepare witness prepare --wait` appends `<clone> <old> <new>`; env `WITNESS_FRONT_ENDS` lists the planned front ends.

Interface
- Rust: `intake::{parse_link, Link, Target, fetch, Fetched, plan, start_playground, run_prepare, language, stack, git}`.
- Plan: `{link, repository, old, new, languages: [{language, files, map: planned|"no parser", proof: planned|"no front end", frontEnd}], otherFiles, playground: {stack, offered, started}}`.
- Front ends: C# `translator`, TypeScript `translator-ts`. Parsers: C#, TS, JS, Rust, Go, Java, C, C++, Ruby, Python, JSON.
- Stacks: `dotnet` (`*.sln`/`*.csproj` at the new commit), `node` (`package.json`); anything else offers none, `start_playground` returns `no playground for this stack`; without a click it returns `playground waits for a click`.

Deviations and fallbacks
- The plan is not a contract record; it is not schema-validated. Section states remain `witness prepare`'s.
- The parser table states which languages the map is planned for; actual parsing is the map modules' job and a module without a parser leaves its section `not-run`.
- GitHub links need network and a `refs/pull/*` ref on the remote; only local URLs are tested.
- Playground start records `started: true` and returns the stack; launching the plugin is the plugin host's job.
- No playbook fallback taken (`FALLBACKS.md`).
