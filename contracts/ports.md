# Ports

Group N owns the block `9000 + 20·N` to `9000 + 20·N + 19` (group numbers from `tasks.md`). Tests bind port 0.
Never use 8790 or 8791. Check a port is free with `ss -ltnp` before binding.

| Group | Module | Block |
|---|---|---|
| 0 | Contracts | 9000–9019 |
| 1 | Sandbox runner | 9020–9039 |
| 2 | Bundle builder | 9040–9059 |
| 3 | Block cutter | 9060–9079 |
| 4 | C# worker | 9080–9099 |
| 5 | C# translator | 9100–9119 |
| 6 | Differ | 9120–9139 |
| 7 | Lean certifier | 9140–9159 |
| 8 | Test contract | 9160–9179 |
| 9 | Rules | 9180–9199 |
| 10 | Plugin host | 9200–9219 |
| 11 | Generator lab | 9220–9239 |
| 12 | Model interface | 9240–9259 |
| 13 | Gateway (9260 serves the API; 9261 the local GitHub stand-in) | 9260–9279 |
| 14–18 | Page (dev server of group N at its block's first port) | 9280–9379 |
| 19 | Integration | 9380–9399 |
| 20–25 | Later groups | 9400–9519 |
