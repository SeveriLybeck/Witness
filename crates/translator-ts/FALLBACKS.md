3efb84947716c040074aa020ef0af6c3809a0e11
- X2: a TypeScript construct outside the closed list (loops, `let`, assignments, objects, JSX in pure functions) is marked not modelled at its location and stands in as an opaque `unmodelled_<Sort>` call.
- Arithmetic and comparison operators are opaque calls `op:<tok>(<sorts>)` (the model has no arithmetic op); the differ compares them as uninterpreted functions shared by old and new.
- Calls to other functions are opaque `fn:<qualified name>(<sorts>)`, assumed not to throw (model rule for opaques).
- Sorts from the type checker: number = Int, boolean = Bool, string = String, everything else Obj. Non-integer number literals are not modelled.
- React render causes are syntactic (props read, useState/useReducer, useContext, useSyncExternalStore, parent render unless `memo`); confirmation by counting renders belongs to the playground, not here.
- Node is taken from `nodejs` in nix (24.21.0), not bun; `npm ci` runs on first use and needs the network once.
