3efb84947716c040074aa020ef0af6c3809a0e11
- X2 A: site units are regions of the method (`--site`), widened to statements and to readers of locals they assign; values entering are params.
- X2 B: an unsupported operation marks its own location `not modelled` and its value becomes an opaque `unmodelled` call.
- X3 B: calls with unknown exception behaviour are opaque and shared; null-receiver only for receivers from `as`, `DefaultValue`, `*OrDefault`, not yet null-checked.
- X4: loops (back edges) are not lowered; the whole graph span is `not modelled` (reason `loop`).
- X5 B: lowering reads the worker's Roslyn CFG (types resolved in the worker); no tree-sitter join.
