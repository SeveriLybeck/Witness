//! Reverting one block: `git diff -U0` hunks restricted to the block's changed lines (D27).

use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq)]
pub struct Hunk {
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
}

fn range(s: &str) -> (usize, usize) {
    let mut it = s.splitn(2, ',');
    let a = it.next().unwrap_or("0").parse().unwrap_or(0);
    let b = it.next().map(|x| x.parse().unwrap_or(0)).unwrap_or(1);
    (a, b)
}

/// Hunks per new-side file path of a `git diff -U0` output.
pub fn parse_hunks(diff: &str) -> Vec<(String, Hunk)> {
    let mut file = String::new();
    let mut out = Vec::new();
    for l in diff.lines() {
        if let Some(p) = l.strip_prefix("+++ ") {
            if p != "/dev/null" {
                file = p.strip_prefix("b/").unwrap_or(p).to_string();
            }
        } else if let Some(p) = l.strip_prefix("--- ") {
            file = if p == "/dev/null" { String::new() } else { p.strip_prefix("a/").unwrap_or(p).to_string() };
        } else if let Some(h) = l.strip_prefix("@@ -") {
            let mut parts = h.split_whitespace();
            let (os, oc) = range(parts.next().unwrap_or("0"));
            let (ns, nc) = range(parts.next().unwrap_or("+0").trim_start_matches('+'));
            out.push((file.clone(), Hunk { old_start: os, old_count: oc, new_start: ns, new_count: nc }));
        }
    }
    out
}

/// New text of one file with the block's lines reverted. `added` are new-file lines of the
/// block, `deleted` old-file lines. Lines of a hunk outside the block stay as in the new file.
pub fn revert_file(old: &str, new: &str, hunks: &[Hunk], added: &BTreeSet<usize>, deleted: &BTreeSet<usize>) -> (String, bool) {
    let ol: Vec<&str> = old.lines().collect();
    let mut nl: Vec<String> = new.lines().map(String::from).collect();
    let mut touched = false;
    let mut hs: Vec<&Hunk> = hunks.iter().collect();
    hs.sort_by_key(|h| std::cmp::Reverse(h.new_start));
    for h in hs {
        let nrange: Vec<usize> = (h.new_start..h.new_start + h.new_count).collect();
        let orange: Vec<usize> = (h.old_start..h.old_start + h.old_count).collect();
        let a_in: Vec<usize> = nrange.iter().copied().filter(|n| added.contains(n)).collect();
        let d_in: Vec<usize> = orange.iter().copied().filter(|n| deleted.contains(n)).collect();
        if a_in.is_empty() && d_in.is_empty() {
            continue;
        }
        touched = true;
        let pivot = a_in.first().copied().unwrap_or(usize::MAX);
        let mut rep: Vec<String> = Vec::new();
        for n in nrange.iter().filter(|n| !added.contains(n) && **n < pivot) {
            rep.push(nl[n - 1].clone());
        }
        for o in &d_in {
            rep.push(ol.get(o - 1).copied().unwrap_or("").to_string());
        }
        for n in nrange.iter().filter(|n| !added.contains(n) && **n > pivot && pivot != usize::MAX) {
            rep.push(nl[n - 1].clone());
        }
        let (at, len) = if h.new_count == 0 { (h.new_start, 0) } else { (h.new_start - 1, h.new_count) };
        let at = at.min(nl.len());
        nl.splice(at..(at + len).min(nl.len()), rep);
    }
    let mut s = nl.join("\n");
    if new.ends_with('\n') {
        s.push('\n');
    }
    (s, touched)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reverts_only_block_lines() {
        let old = "a\nb\nc\nd\n";
        let new = "a\nB\nc\nd\nX\nY\n";
        let diff = "--- a/f\n+++ b/f\n@@ -2 +2 @@\n-b\n+B\n@@ -4,0 +5,2 @@\n+X\n+Y\n";
        let h: Vec<Hunk> = parse_hunks(diff).into_iter().map(|(_, h)| h).collect();
        assert_eq!(h.len(), 2);
        let (t, touched) = revert_file(old, new, &h, &[2].into(), &[2].into());
        assert!(touched);
        assert_eq!(t, "a\nb\nc\nd\nX\nY\n");
        let (t, _) = revert_file(old, new, &h, &[5, 6].into(), &BTreeSet::new());
        assert_eq!(t, "a\nB\nc\nd\n");
        let (t, _) = revert_file(old, new, &h, &[2, 5, 6].into(), &[2].into());
        assert_eq!(t, old);
    }
}
