// Pure helpers for the code panel: old/new line mapping and the breadcrumb.

export interface LineMap { oldToNew: (number | null)[]; newToOld: (number | null)[] }

/** Aligns old and new using the exact changed line lists (1-based). */
export function alignLines(nOld: number, nNew: number, deleted: Set<number>, added: Set<number>): LineMap {
  const oldToNew: (number | null)[] = Array.from({ length: nOld + 1 }, () => null);
  const newToOld: (number | null)[] = Array.from({ length: nNew + 1 }, () => null);
  let i = 1;
  let j = 1;
  while (i <= nOld || j <= nNew) {
    if (i <= nOld && deleted.has(i)) i++;
    else if (j <= nNew && added.has(j)) j++;
    else if (i <= nOld && j <= nNew) { oldToNew[i] = j; newToOld[j] = i; i++; j++; }
    else if (i <= nOld) i++;
    else j++;
  }
  return { oldToNew, newToOld };
}

/** Maps a line to the other side; an unmatched line goes to the nearest matched line above it. */
export function mapLine(map: (number | null)[], line: number): { line: number; exact: boolean } {
  const exact = map[line];
  if (exact != null) return { line: exact, exact: true };
  for (let k = line - 1; k >= 1; k--) {
    const m = map[k];
    if (m != null) return { line: m + 1, exact: false };
  }
  return { line: 1, exact: false };
}

/** One line of the unified diff: a context line has both numbers, a removed line only `old`, an added one only `new`. */
export interface DiffRow { kind: 'ctx' | 'del' | 'add'; old: number | null; new: number | null; text: string }

/**
 * The whole file as one unified diff from the exact changed line lists: walking both sides, a deleted old line is
 * removed, then an added new line is added, else the two lines are the same line. Removed lines of a hunk come
 * before its added lines.
 */
export function unifiedRows(oldLines: string[], newLines: string[], deleted: Set<number>, added: Set<number>): DiffRow[] {
  const out: DiffRow[] = [];
  let i = 1;
  let j = 1;
  while (i <= oldLines.length || j <= newLines.length) {
    if (i <= oldLines.length && (deleted.has(i) || j > newLines.length)) { out.push({ kind: 'del', old: i, new: null, text: oldLines[i - 1] }); i++; }
    else if (j <= newLines.length && (added.has(j) || i > oldLines.length)) { out.push({ kind: 'add', old: null, new: j, text: newLines[j - 1] }); j++; }
    else { out.push({ kind: 'ctx', old: i, new: j, text: newLines[j - 1] }); i++; j++; }
  }
  return out;
}

/** Index of the row showing `line` on `side`; a line missing on that side falls to the next row that has one. */
export function rowOf(rows: DiffRow[], side: 'old' | 'new', line: number): number {
  const exact = rows.findIndex((r) => r[side] === line);
  if (exact >= 0) return exact;
  const next = rows.findIndex((r) => (r[side] ?? 0) > line);
  return next >= 0 ? next : Math.max(0, rows.length - 1);
}

/** The rows first shown: the changed hunk at (or nearest after) `at`, whole, with `context` lines on each side. */
export function hunkWindow(rows: DiffRow[], at: number, context = 6): { from: number; to: number } {
  if (!rows.length) return { from: 0, to: -1 };
  const changed = (k: number) => rows[k] && rows[k].kind !== 'ctx';
  let a = at;
  if (!changed(a)) {
    // the nearest change within the context, else the target line alone with its context
    let best = -1;
    for (let d = 1; d <= context && best < 0; d++) best = changed(at + d) ? at + d : changed(at - d) ? at - d : -1;
    if (best < 0) return { from: Math.max(0, at - context), to: Math.min(rows.length - 1, at + context) };
    a = best;
  }
  let s = a, e = a;
  while (changed(s - 1)) s--;
  while (changed(e + 1)) e++;
  return { from: Math.max(0, Math.min(s, at) - context), to: Math.min(rows.length - 1, Math.max(e, at) + context) };
}

/** Side by side: context on both sides, a hunk's removed lines beside its added lines, blanks where one side is shorter. */
export function splitRows(rows: DiffRow[]): { left: DiffRow | null; right: DiffRow | null; index: number }[] {
  const out: { left: DiffRow | null; right: DiffRow | null; index: number }[] = [];
  let k = 0;
  while (k < rows.length) {
    if (rows[k].kind === 'ctx') { out.push({ left: rows[k], right: rows[k], index: k }); k++; continue; }
    const dels: number[] = [];
    const adds: number[] = [];
    while (k < rows.length && rows[k].kind === 'del') dels.push(k++);
    while (k < rows.length && rows[k].kind === 'add') adds.push(k++);
    for (let m = 0; m < Math.max(dels.length, adds.length); m++) {
      const l = dels[m], r = adds[m];
      out.push({ left: l != null ? rows[l] : null, right: r != null ? rows[r] : null, index: l ?? r });
    }
  }
  return out;
}

/** Leading whitespace all non-blank lines share, in characters. */
export function commonIndent(lines: string[]): number {
  let min = Infinity;
  for (const l of lines) {
    if (!l.trim()) continue;
    min = Math.min(min, l.length - l.trimStart().length);
  }
  return Number.isFinite(min) ? min : 0;
}

const typeDecl = /\b(class|struct|record|interface|enum)\s+([A-Za-z_]\w*)/;
const methodDecl =
  /^\s*(?:\[[^\]]*\]\s*)*(?:(?:public|private|protected|internal|static|async|override|virtual|sealed|abstract|readonly|unsafe|partial|extern|new)\s+)+[\w<>[\],.?() ]*?\b([A-Za-z_]\w*)\s*(?:<[^>]*>)?\s*\(/;

function stripStrings(s: string): string {
  return s.replace(/\/\/.*$/, '').replace(/"(?:\\.|[^"\\])*"/g, '""').replace(/'(?:\\.|[^'\\])'/g, "''");
}

/** Enclosing type and member names at `line` (1-based), outermost first. */
export function enclosing(lines: string[], line: number): { types: string[]; member: string | null } {
  const stack: { kind: 'type' | 'member'; name: string; depth: number }[] = [];
  let pending: { kind: 'type' | 'member'; name: string } | null = null;
  let depth = 0;
  for (let i = 0; i < Math.min(line, lines.length); i++) {
    const text = stripStrings(lines[i]);
    const t = typeDecl.exec(text);
    const m = t ? null : methodDecl.exec(text);
    if (t) pending = { kind: 'type', name: t[2] };
    else if (m && !/^\s*(if|for|foreach|while|switch|using|lock|return|catch)\b/.test(text)) pending = { kind: 'member', name: m[1] };
    for (const ch of text) {
      if (ch === '{') {
        if (pending) { stack.push({ ...pending, depth }); pending = null; }
        depth++;
      } else if (ch === '}') {
        depth--;
        while (stack.length && stack[stack.length - 1].depth >= depth) stack.pop();
      } else if (ch === ';' && pending?.kind === 'member') pending = null;
    }
  }
  const types = stack.filter((s) => s.kind === 'type').map((s) => s.name);
  const members = stack.filter((s) => s.kind === 'member');
  return { types, member: members.length ? members[members.length - 1].name : (pending?.kind === 'member' ? pending.name : null) };
}
