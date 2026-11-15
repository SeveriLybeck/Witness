// TypeScript front end (D24): lowers pure functions to the model (contracts/messages/model.schema.json)
// and lists React render causes as a dependency analysis (D30).
// Usage: node lower.js lower <file> <function> <old|new> [unitId]
//        node lower.js render <file>
"use strict";
const ts = require("typescript");
const path = require("path");

function program(file) {
  const p = ts.createProgram([file], {
    strict: true, noEmit: true, jsx: ts.JsxEmit.Preserve, target: ts.ScriptTarget.ES2022,
    allowJs: false, skipLibCheck: true, types: [],
  });
  return { checker: p.getTypeChecker(), sf: p.getSourceFile(file) };
}

function findFunction(sf, name) {
  let found = null;
  const visit = (n) => {
    if (found) return;
    if (ts.isFunctionDeclaration(n) && n.name && n.name.text === name) found = n;
    else if (ts.isVariableDeclaration(n) && ts.isIdentifier(n.name) && n.name.text === name && n.initializer &&
      (ts.isArrowFunction(n.initializer) || ts.isFunctionExpression(n.initializer))) found = n.initializer;
    else ts.forEachChild(n, visit);
  };
  visit(sf);
  return found;
}

function lowerFunction(file, fname, version, unitId) {
  const { checker, sf } = program(file);
  if (!sf) throw new Error(`cannot read ${file}`);
  const fn = findFunction(sf, fname);
  if (!fn) throw new Error(`no function ${fname} in ${file}`);
  const rel = path.basename(file);
  const loc = (n) => {
    const a = sf.getLineAndCharacterOfPosition(n.getStart(sf)).line + 1;
    const b = sf.getLineAndCharacterOfPosition(n.getEnd()).line + 1;
    return { file: rel, line: a, endLine: b };
  };
  const sortOf = (t) => {
    if (t.isUnion()) {
      const ss = [...new Set(t.types.map(sortOf))];
      return ss.length === 1 ? ss[0] : "Obj";
    }
    if (t.flags & (ts.TypeFlags.NumberLike)) return "Int";
    if (t.flags & (ts.TypeFlags.BooleanLike)) return "Bool";
    if (t.flags & (ts.TypeFlags.StringLike)) return "String";
    return "Obj";
  };
  const sortNode = (n) => sortOf(checker.getTypeAtLocation(n));
  const opaque = new Map();
  const notModelled = [];
  const env = new Map();
  const useOpaque = (name, symbol, args, result) => {
    if (!opaque.has(name)) opaque.set(name, { name, symbol, args, result });
    return name;
  };
  const unmodelled = (n, reason) => {
    notModelled.push({ location: loc(n), reason });
    const s = sortNode(n);
    const name = useOpaque(`unmodelled_${s}`, `unmodelled:${s}`, [], s);
    return { op: "call", name, args: [], location: loc(n) };
  };
  const binNames = {
    [ts.SyntaxKind.PlusToken]: "add", [ts.SyntaxKind.MinusToken]: "sub", [ts.SyntaxKind.AsteriskToken]: "mul",
    [ts.SyntaxKind.SlashToken]: "div", [ts.SyntaxKind.PercentToken]: "mod", [ts.SyntaxKind.LessThanToken]: "lt",
    [ts.SyntaxKind.LessThanEqualsToken]: "le", [ts.SyntaxKind.GreaterThanToken]: "gt",
    [ts.SyntaxKind.GreaterThanEqualsToken]: "ge",
  };
  const isNullish = (e) => e.kind === ts.SyntaxKind.NullKeyword || (ts.isIdentifier(e) && e.text === "undefined");

  function expr(e) {
    if (ts.isParenthesizedExpression(e) || ts.isAsExpression(e) || ts.isNonNullExpression(e)) return expr(e.expression);
    if (ts.isNumericLiteral(e)) {
      const v = Number(e.text);
      return Number.isInteger(v) ? { op: "lit", value: v } : unmodelled(e, "non-integer number");
    }
    if (ts.isStringLiteral(e) || ts.isNoSubstitutionTemplateLiteral(e)) return { op: "lit", value: e.text };
    if (e.kind === ts.SyntaxKind.TrueKeyword) return { op: "lit", value: true };
    if (e.kind === ts.SyntaxKind.FalseKeyword) return { op: "lit", value: false };
    if (isNullish(e)) return { op: "none" };
    if (ts.isIdentifier(e)) {
      if (!env.has(e.text)) return unmodelled(e, `free name ${e.text}`);
      const s = env.get(e.text);
      return s ? { op: "var", name: e.text, sort: s } : { op: "var", name: e.text };
    }
    if (ts.isConditionalExpression(e)) return { op: "if", args: [expr(e.condition), expr(e.whenTrue), expr(e.whenFalse)], location: loc(e) };
    if (ts.isPrefixUnaryExpression(e)) {
      if (e.operator === ts.SyntaxKind.ExclamationToken) return { op: "not", args: [expr(e.operand)] };
      if (e.operator === ts.SyntaxKind.MinusToken && ts.isNumericLiteral(e.operand)) return { op: "lit", value: -Number(e.operand.text) };
      if (e.operator === ts.SyntaxKind.MinusToken) {
        const name = useOpaque("op_neg", "op:-(Int)", ["Int"], "Int");
        return { op: "call", name, args: [expr(e.operand)] };
      }
      return unmodelled(e, "unary operator");
    }
    if (ts.isBinaryExpression(e)) {
      const k = e.operatorToken.kind;
      const l = e.left, r = e.right;
      if (k === ts.SyntaxKind.AmpersandAmpersandToken) return { op: "and", args: [expr(l), expr(r)] };
      if (k === ts.SyntaxKind.BarBarToken) return { op: "or", args: [expr(l), expr(r)] };
      const eqK = [ts.SyntaxKind.EqualsEqualsEqualsToken, ts.SyntaxKind.EqualsEqualsToken];
      const neK = [ts.SyntaxKind.ExclamationEqualsEqualsToken, ts.SyntaxKind.ExclamationEqualsToken];
      if (eqK.includes(k) || neK.includes(k)) {
        let t;
        if (isNullish(r)) t = { op: "isnull", args: [expr(l)] };
        else if (isNullish(l)) t = { op: "isnull", args: [expr(r)] };
        else t = { op: "eq", args: [expr(l), expr(r)] };
        return neK.includes(k) ? { op: "not", args: [t] } : t;
      }
      if (binNames[k]) {
        const ls = sortNode(l), rs = sortNode(r), res = sortNode(e);
        const n = `op_${binNames[k]}_${ls}_${rs}`;
        const name = useOpaque(n, `op:${e.operatorToken.getText(sf)}(${ls},${rs})`, [ls, rs], res);
        return { op: "call", name, args: [expr(l), expr(r)], location: loc(e) };
      }
      return unmodelled(e, "binary operator");
    }
    if (ts.isCallExpression(e)) {
      const sym = checker.getSymbolAtLocation(e.expression);
      const target = sym ? checker.getFullyQualifiedName(sym) : e.expression.getText(sf);
      const args = e.arguments.map((a) => sortNode(a));
      const res = sortNode(e);
      const short = target.replace(/^".*"\./, "").replace(/[^A-Za-z0-9_]/g, "_");
      const name = useOpaque(`${short}_${args.length}`, `fn:${target.replace(/^".*"\./, "")}(${args.join(",")})`, args, res);
      return { op: "call", name, args: e.arguments.map(expr), location: loc(e) };
    }
    return unmodelled(e, ts.SyntaxKind[e.kind]);
  }

  function throwOf(s) {
    const x = s.expression;
    let name = "Error";
    if (x && ts.isNewExpression(x) && ts.isIdentifier(x.expression)) name = x.expression.text;
    return { op: "throw", name, location: loc(s) };
  }

  // A statement list as one expression: the function's result, a throw, or nothing.
  function stmts(list, i) {
    if (i >= list.length) return { op: "nothing" };
    const s = list[i];
    if (ts.isReturnStatement(s)) return s.expression ? expr(s.expression) : { op: "nothing" };
    if (ts.isThrowStatement(s)) return throwOf(s);
    if (ts.isBlock(s)) return stmts([...s.statements, ...list.slice(i + 1)], 0);
    if (ts.isIfStatement(s)) {
      const rest = list.slice(i + 1);
      const arm = (b) => (b ? stmts([...(ts.isBlock(b) ? b.statements : [b]), ...rest], 0) : stmts(rest, 0));
      return { op: "if", args: [expr(s.expression), arm(s.thenStatement), arm(s.elseStatement)], location: loc(s) };
    }
    if (ts.isVariableStatement(s) && (s.declarationList.flags & ts.NodeFlags.Const)) {
      const decls = s.declarationList.declarations;
      const build = (j) => {
        if (j >= decls.length) return stmts(list, i + 1);
        const d = decls[j];
        if (!ts.isIdentifier(d.name) || !d.initializer) return unmodelled(d, "declaration form");
        const v = expr(d.initializer);
        env.set(d.name.text, sortNode(d.name));
        return { op: "let", name: d.name.text, args: [v, build(j + 1)] };
      };
      return build(0);
    }
    if (ts.isEmptyStatement(s)) return stmts(list, i + 1);
    // X2: a statement outside the closed list stands in as an opaque value and ends the unit.
    return unmodelled(s, ts.SyntaxKind[s.kind]);
  }

  const params = fn.parameters.map((p) => {
    const name = p.name.getText(sf);
    const sort = sortNode(p.name);
    env.set(name, sort);
    return { name, sort };
  });
  const body = fn.body && ts.isBlock(fn.body) ? stmts(fn.body.statements, 0) : expr(fn.body);
  const span = loc(fn);
  const modelled = [];
  for (let l = span.line; l <= span.endLine; l++) modelled.push(l);
  const notLines = new Set(notModelled.flatMap((n) => { const r = []; for (let l = n.location.line; l <= n.location.endLine; l++) r.push(l); return r; }));
  const startLine = sf.getLineAndCharacterOfPosition((fn.name || fn).getStart(sf)).line + 1;
  return {
    unit: unitId || `${fname}#${startLine}`,
    version,
    params,
    opaque: [...opaque.values()],
    contracts: [],
    body,
    lines: { modelled: modelled.filter((l) => !notLines.has(l)), notModelled },
  };
}

// React render causes: what a component reads that may make it render again.
function renderCauses(file) {
  const { sf } = program(file);
  if (!sf) throw new Error(`cannot read ${file}`);
  const rel = path.basename(file);
  const line = (n) => sf.getLineAndCharacterOfPosition(n.getStart(sf)).line + 1;
  const out = [];
  const hookName = (c) => {
    const x = c.expression;
    if (ts.isIdentifier(x)) return x.text;
    if (ts.isPropertyAccessExpression(x)) return x.name.text;
    return null;
  };
  const returnsJsx = (fn) => {
    let j = false;
    const v = (n) => { if (ts.isJsxElement(n) || ts.isJsxSelfClosingElement(n) || ts.isJsxFragment(n)) j = true; else ts.forEachChild(n, v); };
    v(fn);
    return j;
  };
  const memoed = new Set();
  const comps = [];
  const visit = (n) => {
    if (ts.isCallExpression(n) && ["memo"].includes(hookName(n) || "")) {
      const a = n.arguments[0];
      if (a && ts.isIdentifier(a)) memoed.add(a.text);
      if (a && (ts.isArrowFunction(a) || ts.isFunctionExpression(a)) && ts.isVariableDeclaration(n.parent))
        comps.push({ name: n.parent.name.getText(sf), fn: a, memo: true });
    }
    if (ts.isFunctionDeclaration(n) && n.name && /^[A-Z]/.test(n.name.text) && returnsJsx(n)) comps.push({ name: n.name.text, fn: n, memo: false });
    if (ts.isVariableDeclaration(n) && ts.isIdentifier(n.name) && /^[A-Z]/.test(n.name.text) && n.initializer &&
      (ts.isArrowFunction(n.initializer) || ts.isFunctionExpression(n.initializer)) && returnsJsx(n.initializer))
      comps.push({ name: n.name.text, fn: n.initializer, memo: false });
    ts.forEachChild(n, visit);
  };
  visit(sf);
  for (const c of comps) {
    const causes = [];
    const memo = c.memo || memoed.has(c.name);
    const p = c.fn.parameters[0];
    if (p) {
      if (ts.isObjectBindingPattern(p.name)) {
        for (const el of p.name.elements) causes.push({ cause: "prop", name: (el.propertyName || el.name).getText(sf), line: line(el) });
      } else {
        const pn = p.name.getText(sf);
        const seen = new Set();
        const v = (n) => {
          if (ts.isPropertyAccessExpression(n) && ts.isIdentifier(n.expression) && n.expression.text === pn && !seen.has(n.name.text)) {
            seen.add(n.name.text);
            causes.push({ cause: "prop", name: n.name.text, line: line(n) });
          }
          ts.forEachChild(n, v);
        };
        if (c.fn.body) v(c.fn.body);
      }
    }
    const v = (n) => {
      if (n !== c.fn && (ts.isFunctionDeclaration(n) || ts.isArrowFunction(n) || ts.isFunctionExpression(n))) return;
      if (ts.isCallExpression(n)) {
        const h = hookName(n);
        const decl = ts.isVariableDeclaration(n.parent) ? n.parent.name : null;
        const first = decl && ts.isArrayBindingPattern(decl) && decl.elements[0] ? decl.elements[0].getText(sf) : decl ? decl.getText(sf) : h;
        if (h === "useState" || h === "useReducer") causes.push({ cause: "state", name: first, line: line(n) });
        else if (h === "useContext") causes.push({ cause: "context", name: n.arguments[0] ? n.arguments[0].getText(sf) : "?", line: line(n) });
        else if (h === "useSyncExternalStore") causes.push({ cause: "store", name: first, line: line(n) });
      }
      ts.forEachChild(n, v);
    };
    v(c.fn);
    if (!memo) causes.push({ cause: "parent", name: "parent render", line: line(c.fn) });
    out.push({
      component: c.name,
      file: rel,
      memo,
      heading: "may re-render when",
      causes: causes.map((x) => ({ ...x, label: label(x) })),
    });
  }
  return { components: out };
}

function label(x) {
  switch (x.cause) {
    case "prop": return `prop ${x.name} changes`;
    case "state": return `state ${x.name} changes`;
    case "context": return `context ${x.name} changes`;
    case "store": return `store ${x.name} changes`;
    default: return "parent re-renders";
  }
}

function main() {
  const [cmd, file, ...rest] = process.argv.slice(2);
  let r;
  if (cmd === "lower") r = lowerFunction(path.resolve(file), rest[0], rest[1] || "new", rest[2]);
  else if (cmd === "render") r = renderCauses(path.resolve(file));
  else { process.stderr.write("usage: lower.js lower <file> <function> <old|new> [unit] | render <file>\n"); process.exit(2); }
  process.stdout.write(JSON.stringify(r, null, 1) + "\n");
}
main();
