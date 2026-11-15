3efb84947716c040074aa020ef0af6c3809a0e11
Contract hash above (playbook §14 item 6). Fallbacks taken:
- HTTP server and client hand-written over std (no network crates); one request per connection.
- Plugin host argv is not in the contract: gateway runs `<host cmd> <plugin> <bundle dir>`, one long-lived process per bundle and plugin.
- New bundle on a moved head: optional `--prepare` command, else a placeholder `bundle.json` with every section `not-run`.
