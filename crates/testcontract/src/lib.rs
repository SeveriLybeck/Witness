//! Witness test contract (group 8; D26, D27, D28).
//!
//! From the cutter's `parts.json`/`blocks.json` and the two commits it finds the pull request's
//! test methods in the test-file diff, runs them on old and new production code through the
//! sandbox queue (network on for the test step, D43), sorts them into categories, reverts each
//! production block alone to link declared tests to blocks, keeps the reverted builds, and
//! generates an xUnit test for each undeclared difference that has a replay input.

pub mod csharp;
pub mod gen;
pub mod revert;
pub mod trx;

use sandbox::{Net, Queue, Sandbox, Step, StepRecord, WorkingCopy};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Strings in a failed test's output that mean the run broke, not the test (playbook C2).
pub const INFRA_MARKERS: [&str; 5] = ["NU1301", "api.nuget.org", "HttpRequestException", "Unable to load the service index", "Name or service not known"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Change {
    Added,
    Edited,
    Removed,
}
impl Change {
    pub fn as_str(self) -> &'static str {
        match self {
            Change::Added => "added",
            Change::Edited => "edited",
            Change::Removed => "removed",
        }
    }
}

/// `pass` | `fail` | `not-compiled` | `not-run`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail,
    NotCompiled,
    NotRun,
}
impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Pass => "pass",
            Outcome::Fail => "fail",
            Outcome::NotCompiled => "not-compiled",
            Outcome::NotRun => "not-run",
        }
    }
}

/// D26 categories. A test that does not pass on the new code fits none of the four run
/// categories; it is `not-run` (never declared). See STATUS.md.
pub fn categorize(change: Change, loosened: bool, old: Outcome, new: Outcome) -> &'static str {
    if change == Change::Removed || (change == Change::Edited && loosened) {
        return "withdrawn";
    }
    if new != Outcome::Pass {
        return "not-run";
    }
    match old {
        Outcome::NotCompiled => "new-api",
        Outcome::Fail => "declared",
        Outcome::Pass => "hanging",
        Outcome::NotRun => "not-run",
    }
}

/// One of the pull request's tests, as found in the diff.
#[derive(Clone, Debug)]
pub struct PrTest {
    pub name: String,
    pub file: String,
    pub change: Change,
    pub loosened: bool,
    pub old: Option<csharp::TestMethod>,
    pub new: Option<csharp::TestMethod>,
}

fn norm(s: &str) -> String {
    s.lines().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n")
}

/// Assertion statements of a method and of the same-file helpers it calls (one level).
fn assertions(src: &str, m: &csharp::TestMethod) -> Vec<String> {
    let mut lines = csharp::assertion_lines(src, m.line, m.end_line);
    if lines.is_empty() {
        for name in csharp::called_names(&m.text) {
            if name == m.method {
                continue;
            }
            if let Some((a, b)) = csharp::method_range(src, &name) {
                lines.extend(csharp::assertion_lines(src, a, b));
            }
        }
    }
    lines.iter().filter_map(|l| csharp::statement_at(src, *l)).collect()
}

/// Test methods added, edited or removed between two versions of one test file (D26).
pub fn diff_tests(file: &str, old: Option<&str>, new: Option<&str>) -> Vec<PrTest> {
    let om = old.map(csharp::test_methods).unwrap_or_default();
    let nm = new.map(csharp::test_methods).unwrap_or_default();
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for n in &nm {
        if !seen.insert(n.name.clone()) {
            continue;
        }
        match om.iter().find(|o| o.name == n.name) {
            None => out.push(PrTest { name: n.name.clone(), file: file.into(), change: Change::Added, loosened: false, old: None, new: Some(n.clone()) }),
            Some(o) if norm(&o.text) != norm(&n.text) => {
                let before = assertions(old.unwrap(), o);
                let after = assertions(new.unwrap(), n);
                let loosened = before.iter().any(|b| !after.contains(b)) && after.len() <= before.len();
                out.push(PrTest { name: n.name.clone(), file: file.into(), change: Change::Edited, loosened, old: Some(o.clone()), new: Some(n.clone()) })
            }
            _ => {}
        }
    }
    for o in &om {
        if !nm.iter().any(|n| n.name == o.name) && seen.insert(o.name.clone()) {
            out.push(PrTest { name: o.name.clone(), file: file.into(), change: Change::Removed, loosened: false, old: Some(o.clone()), new: None });
        }
    }
    out
}

/// Result of one test in one run.
#[derive(Clone, Debug)]
pub struct RunResult {
    pub outcome: Outcome,
    pub message: Option<String>,
    pub stack: Option<String>,
}

/// Results by fully qualified name; theory cases fold into their method (any failure fails).
pub fn fold_trx(results: &[trx::TrxResult]) -> BTreeMap<String, RunResult> {
    let mut m: BTreeMap<String, RunResult> = BTreeMap::new();
    for r in results {
        let name = r.test_name.split('(').next().unwrap_or("").trim().to_string();
        let text = format!("{}\n{}", r.message.clone().unwrap_or_default(), r.stack.clone().unwrap_or_default());
        let outcome = match r.outcome.as_str() {
            "Passed" => Outcome::Pass,
            "Failed" if INFRA_MARKERS.iter().any(|k| text.contains(k)) => Outcome::NotRun,
            "Failed" => Outcome::Fail,
            _ => Outcome::NotRun,
        };
        let e = m.entry(name).or_insert(RunResult { outcome, message: r.message.clone(), stack: r.stack.clone() });
        let rank = |o: Outcome| match o {
            Outcome::Pass => 0,
            Outcome::Fail => 2,
            _ => 1,
        };
        if rank(outcome) > rank(e.outcome) {
            *e = RunResult { outcome, message: r.message.clone(), stack: r.stack.clone() };
        }
    }
    m
}

/// First stack frame line number inside `file` (`... in /path/<file>:line N`).
pub fn frame_line(stack: &str, file: &str) -> Option<usize> {
    stack.lines().find_map(|l| {
        let p = l.find(":line ")?;
        if !l[..p].ends_with(file) {
            return None;
        }
        l[p + 6..].trim().parse().ok()
    })
}

// ---------------------------------------------------------------- sandbox plumbing

fn git(sb: &Sandbox, cwd: &str, args: &[&str]) -> Result<StepRecord, String> {
    let mut a = vec!["git", "-c", "core.hooksPath=/dev/null"];
    a.extend_from_slice(args);
    let r = sb.run(&Step::new("git", Net::Off, &a).cwd(cwd));
    if r.ok() {
        Ok(r)
    } else {
        Err(format!("git {:?} in {cwd}: exit {:?}: {}", args, r.exit, r.stderr.trim()))
    }
}

/// A detached working copy at `<root>/<rel>` for `commit`; reset to the commit when it exists
/// (untracked files removed, ignored build output kept for incremental builds).
pub fn worktree(sb: &Sandbox, rel: &str, commit: &str) -> Result<WorkingCopy, String> {
    let abs = sb.root.join(rel);
    if abs.join(".git").exists() {
        git(sb, rel, &["checkout", "-q", "-f", "--detach", commit])?;
        git(sb, rel, &["clean", "-fdq"])?;
    } else {
        if let Some(p) = abs.parent() {
            fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        let _g = locked(&sb.root.join(".sb/clone.lock"));
        git(sb, "repo", &["worktree", "add", "-q", "-f", "--detach", &format!("../{rel}"), commit])?;
    }
    Ok(WorkingCopy { commit: commit.into(), rel: rel.into(), abs })
}

/// Nearest directory above `file` holding a `.csproj`, as a path relative to the working copy.
pub fn project_of(wc_abs: &Path, file: &str) -> Option<String> {
    let mut d = Path::new(file).parent();
    while let Some(dir) = d {
        if let Ok(rd) = fs::read_dir(wc_abs.join(dir)) {
            let mut v: Vec<String> = rd.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.ends_with(".csproj")).collect();
            v.sort();
            if let Some(p) = v.first() {
                return Some(dir.join(p).to_string_lossy().to_string());
            }
        }
        d = dir.parent();
    }
    None
}

/// An exclusive lock on `path`, held across threads (each opens its own file) and processes.
pub fn locked(path: &Path) -> Option<fs::File> {
    if let Some(p) = path.parent() {
        let _ = fs::create_dir_all(p);
    }
    let f = fs::OpenOptions::new().create(true).truncate(false).write(true).open(path).ok()?;
    f.lock().ok()?;
    Some(f)
}

/// Clone and working copies of one sandbox root, created once under a lock.
pub fn working_copy_locked(sb: &Sandbox, rev: &str) -> Result<WorkingCopy, String> {
    let _g = locked(&sb.root.join(".sb/clone.lock"));
    sb.working_copy(rev)
}

fn restore_build(sb: &Sandbox, q: &Queue, wc: &WorkingCopy, proj: &str) -> StepRecord {
    // concurrent cold restores into the shared NuGet folder collide (sandbox T4): one at a time
    // across threads and processes, plus retries
    let _g = locked(&sb.nuget.join(".testcontract-restore.lock"));
    let mut r = sb.restore(q, wc, proj);
    for _ in 0..2 {
        if r.ok() {
            break;
        }
        r = sb.restore(q, wc, proj);
    }
    if !r.ok() {
        return r;
    }
    sb.build(q, wc, proj, &["-c", "Release"])
}

fn stem(f: &str) -> &str {
    match f.rfind('.') {
        Some(d) if d > f.rfind('/').map(|s| s + 1).unwrap_or(0) => &f[..d],
        _ => f,
    }
}

/// Parts of category `generated` whose file is the checked-in twin (same path, other
/// extension) of a file the block changes, such as `X.tt` → `X.cs`. They revert with the block.
pub fn generated_twins<'a>(block_parts: &[&Value], all: &'a [Value]) -> Vec<&'a Value> {
    let files: BTreeSet<String> = block_parts.iter().filter(|p| p["category"] != "generated").filter_map(|p| loc_file(p)).collect();
    all.iter()
        .filter(|p| p["category"] == "generated")
        .filter(|p| loc_file(p).is_some_and(|g| files.iter().any(|f| *f != g && stem(f) == stem(&g))))
        .collect()
}

struct RevertRun {
    id: String,
    compiles: bool,
    failing: Vec<String>,
    rel: String,
    /// first compiler error lines when the revert does not compile
    errors: Vec<String>,
    /// the build could not be run (worktree, restore, timeout, non-compiler failure)
    infra: Option<String>,
}

/// Compiler error lines (`: error CS…`) of a build output, deduplicated.
fn compile_errors(build: &StepRecord) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for l in build.stdout.lines().chain(build.stderr.lines()) {
        if l.contains(": error CS") {
            let t = l.trim().to_string();
            if !v.contains(&t) {
                v.push(t);
            }
        }
    }
    v
}

/// Error lines of a build output that name `file`.
fn errors_in(build: &StepRecord, file: &str) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for l in build.stdout.lines().chain(build.stderr.lines()) {
        if l.contains(": error ") && l.contains(file) {
            let t = l.trim().to_string();
            if !v.contains(&t) {
                v.push(t);
            }
        }
    }
    v
}

/// Run the named tests of one project (`dotnet test --no-build`, network on), results from TRX.
fn run_tests(sb: &Sandbox, q: &Queue, wc: &WorkingCopy, proj: &str, names: &[String], tag: &str) -> BTreeMap<String, RunResult> {
    if names.is_empty() {
        return BTreeMap::new();
    }
    let filter = names.iter().map(|n| format!("FullyQualifiedName={n}")).collect::<Vec<_>>().join("|");
    let trx_name = format!("{tag}.trx");
    let dir = wc.abs.join("TestResults");
    let _ = fs::remove_file(dir.join(&trx_name));
    let logger = format!("trx;LogFileName={trx_name}");
    let _r = sb.test(q, wc, proj, &["-c", "Release", "--filter", &filter, "--logger", &logger, "--results-directory", "TestResults"]);
    let xml = fs::read_to_string(dir.join(&trx_name)).unwrap_or_default();
    fold_trx(&trx::parse(&xml))
}

// ---------------------------------------------------------------- the pipeline

/// Inputs: the cutter's records and the commits.
pub struct Inputs {
    /// Directory holding `parts.json` and `blocks.json`; `differences.json`, `replays.json`,
    /// `groups.json` and `scope.json` are read when present.
    pub records: PathBuf,
    pub old: String,
    pub new: String,
}

/// What `prepare` produced; also written to the output directory.
pub struct Output {
    pub tests: Value,
    pub links: Value,
    pub snapshots: Value,
    pub suggested: Value,
    pub reverted: Value,
    pub old_commit: String,
    pub new_commit: String,
}

fn read_json(dir: &Path, name: &str) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(dir.join(name)).ok()?).ok()
}

fn loc_file(p: &Value) -> Option<String> {
    p["new"]["file"].as_str().or(p["old"]["file"].as_str()).map(String::from)
}

fn lines_of(v: &Value) -> BTreeSet<usize> {
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_u64()).map(|x| x as usize).collect()).unwrap_or_default()
}

struct SideRun {
    results: BTreeMap<String, RunResult>,
    /// test file -> compile error lines when it was left out of this run
    excluded: BTreeMap<String, Vec<String>>,
    /// the run as a whole failed to build
    broken: bool,
}

fn side_run(sb: &Sandbox, q: &Queue, wc: &WorkingCopy, swapped: &[String], by_proj: &BTreeMap<String, Vec<String>>, tests: &[PrTest], tag: &str, old_side: Option<&WorkingCopy>) -> SideRun {
    let mut results = BTreeMap::new();
    let mut excluded: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut broken = false;
    for (proj, files) in by_proj {
        let mut tries = 0;
        let ok = loop {
            let b = restore_build(sb, q, wc, proj);
            if b.ok() {
                break true;
            }
            tries += 1;
            // old side: leave out the swapped test files that do not compile
            let bad: Vec<(String, Vec<String>)> = files.iter().filter(|f| swapped.contains(f) && !excluded.contains_key(*f)).map(|f| (f.clone(), errors_in(&b, f))).filter(|(_, e)| !e.is_empty()).collect();
            if old_side.is_none() || bad.is_empty() || tries > files.len() {
                break false;
            }
            for (f, e) in bad {
                let o = old_side.unwrap();
                match fs::read_to_string(o.abs.join(&f)) {
                    Ok(t) => {
                        let _ = fs::write(wc.abs.join(&f), t);
                    }
                    Err(_) => {
                        let _ = fs::remove_file(wc.abs.join(&f));
                    }
                }
                excluded.insert(f, e);
            }
        };
        if !ok {
            broken = true;
            continue;
        }
        let names: Vec<String> = tests.iter().filter(|t| t.change != Change::Removed && files.contains(&t.file) && !excluded.contains_key(&t.file)).map(|t| t.name.clone()).collect();
        results.extend(run_tests(sb, q, wc, proj, &names, tag));
    }
    SideRun { results, excluded, broken }
}

/// The whole test contract for one (old, new) pair. `work` is a directory under the sandbox
/// root for this module's working copies (for example `tc`).
pub fn prepare(sb: &Sandbox, q: &Queue, inputs: &Inputs, work: &str) -> Result<Output, String> {
    let parts = read_json(&inputs.records, "parts.json").ok_or("parts.json missing")?;
    let blocks = read_json(&inputs.records, "blocks.json").ok_or("blocks.json missing")?;
    let differences = read_json(&inputs.records, "differences.json").unwrap_or(json!({"differences": []}));
    let replays = read_json(&inputs.records, "replays.json").unwrap_or(json!({"replays": []}));
    let groups = read_json(&inputs.records, "groups.json").unwrap_or(json!({"groups": []}));
    let scope = read_json(&inputs.records, "scope.json").unwrap_or(json!({}));
    let parts = parts["parts"].as_array().cloned().unwrap_or_default();
    let blocks = blocks["blocks"].as_array().cloned().unwrap_or_default();

    let old_wc = working_copy_locked(sb, &inputs.old)?;
    let new_wc = working_copy_locked(sb, &inputs.new)?;
    let (old_c, new_c) = (old_wc.commit.clone(), new_wc.commit.clone());
    let key = format!("{}-{}", &old_c[..12], &new_c[..12]);

    // 1. the pull request's tests, from the diff of the test files (D26)
    let mut test_files: Vec<String> = parts.iter().filter(|p| p["category"] == "test").filter_map(loc_file).collect();
    test_files.sort();
    test_files.dedup();
    let mut tests: Vec<PrTest> = Vec::new();
    for f in &test_files {
        if !f.ends_with(".cs") {
            continue;
        }
        let o = fs::read_to_string(old_wc.abs.join(f)).ok();
        let n = fs::read_to_string(new_wc.abs.join(f)).ok();
        tests.extend(diff_tests(f, o.as_deref(), n.as_deref()));
    }
    let mut by_proj: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for f in &test_files {
        if let Some(p) = project_of(&new_wc.abs, f).or_else(|| project_of(&old_wc.abs, f)) {
            by_proj.entry(p).or_default().push(f.clone());
        }
    }
    let run_proj: BTreeMap<String, Vec<String>> = by_proj.iter().filter(|(_, fs_)| tests.iter().any(|t| t.change != Change::Removed && fs_.contains(&t.file))).map(|(k, v)| (k.clone(), v.clone())).collect();

    // 2. two runs: new code, and old production code with the new test files (D26)
    let mix_rel = format!("{work}/mix/{key}");
    let (new_run, old_run) = std::thread::scope(|s| -> Result<(SideRun, SideRun), String> {
        let hn = s.spawn(|| side_run(sb, q, &new_wc, &[], &run_proj, &tests, "new", None));
        let mix = worktree(sb, &mix_rel, &old_c)?;
        let mut swapped = Vec::new();
        for f in &test_files {
            match fs::read_to_string(new_wc.abs.join(f)) {
                Ok(t) => {
                    if let Some(p) = mix.abs.join(f).parent() {
                        let _ = fs::create_dir_all(p);
                    }
                    fs::write(mix.abs.join(f), t).map_err(|e| e.to_string())?;
                    swapped.push(f.clone());
                }
                Err(_) => {
                    let _ = fs::remove_file(mix.abs.join(f));
                }
            }
        }
        let old_run = side_run(sb, q, &mix, &swapped, &run_proj, &tests, "old", Some(&old_wc));
        Ok((hn.join().map_err(|_| "new run panicked")?, old_run))
    })?;

    // 3. categories, verbatim failure and assertion (D26)
    let mut entries: Vec<(String, usize, Value)> = Vec::new();
    let mut declared_names: Vec<String> = Vec::new();
    for t in &tests {
        let res = |run: &SideRun| -> (Outcome, Option<RunResult>) {
            if t.change == Change::Removed {
                return (Outcome::NotRun, None);
            }
            if run.excluded.contains_key(&t.file) {
                return (Outcome::NotCompiled, None);
            }
            if run.broken && !run.results.contains_key(&t.name) {
                return (Outcome::NotRun, None);
            }
            match run.results.get(&t.name) {
                Some(r) => (r.outcome, Some(r.clone())),
                None => (Outcome::NotRun, None),
            }
        };
        let (old_o, old_r) = res(&old_run);
        let (new_o, _) = res(&new_run);
        let cat = categorize(t.change, t.loosened, old_o, new_o);
        if cat == "declared" || cat == "new-api" {
            declared_names.push(t.name.clone());
        }
        let new_src = fs::read_to_string(new_wc.abs.join(&t.file)).unwrap_or_default();
        let old_src = fs::read_to_string(old_wc.abs.join(&t.file)).unwrap_or_default();
        let after = t.new.as_ref().and_then(|m| {
            let hit = old_r.as_ref().and_then(|r| r.stack.as_deref()).and_then(|s| frame_line(s, &t.file)).filter(|l| *l >= m.start_line && *l <= new_src.lines().count());
            match hit {
                Some(l) => csharp::statement_at(&new_src, l),
                None => assertions(&new_src, m).into_iter().next(),
            }
        });
        let before = t.old.as_ref().and_then(|m| assertions(&old_src, m).into_iter().next());
        let failure = match cat {
            "declared" => old_r.as_ref().and_then(|r| r.message.clone()),
            "new-api" => old_run.excluded.get(&t.file).and_then(|e| e.first().cloned()),
            _ => None,
        };
        let m = t.new.as_ref().or(t.old.as_ref()).unwrap();
        entries.push((t.file.clone(), m.line, json!({
            "name": t.name,
            "location": {"file": t.file, "line": m.line, "endLine": m.end_line},
            "change": t.change.as_str(), "category": cat,
            "old": old_o.as_str(), "new": new_o.as_str(),
            "failureMessage": failure,
            "assertion": {"before": before, "after": after},
        })));
    }
    entries.sort_by(|a, b| (&a.0, a.1).cmp(&(&b.0, b.1)));
    let tests_json = json!({"provenance": "pipeline", "tests": entries.into_iter().map(|e| e.2).collect::<Vec<_>>()});

    // 4. revert each production block alone, rebuild, re-run the declared tests (D27)
    let diff = git(sb, "repo", &["diff", "-U0", "--no-color", "--no-ext-diff", "--no-textconv", &old_c, &new_c])?.stdout;
    let hunks = revert::parse_hunks(&diff);
    let part_by_id: BTreeMap<String, &Value> = parts.iter().filter_map(|p| Some((p["id"].as_str()?.to_string(), p))).collect();
    let prod: Vec<&Value> = blocks.iter().filter(|b| b["category"] == "production").collect();
    let decl_proj: Vec<(String, Vec<String>)> = run_proj
        .iter()
        .map(|(p, fs_)| (p.clone(), tests.iter().filter(|t| fs_.contains(&t.file) && declared_names.contains(&t.name)).map(|t| t.name.clone()).collect::<Vec<_>>()))
        .collect();
    let link_results: Vec<RevertRun> = std::thread::scope(|s| {
        let hs: Vec<_> = prod
            .iter()
            .map(|b| {
                let id = b["id"].as_str().unwrap_or("").to_string();
                let mut bparts: Vec<&Value> = b["parts"].as_array().map(|a| a.iter().filter_map(|p| part_by_id.get(p.as_str()?).copied()).collect()).unwrap_or_default();
                bparts.extend(generated_twins(&bparts, &parts));
                let (hunks, old_wc, new_wc, decl_proj, new_c) = (&hunks, &old_wc, &new_wc, &decl_proj, &new_c);
                let rel = format!("{work}/rev/{}/{id}", &new_c[..12]);
                s.spawn(move || -> RevertRun {
                    let wc = match worktree(sb, &rel, new_c) {
                        Ok(w) => w,
                        Err(e) => return RevertRun { id, compiles: false, failing: vec![], rel, errors: vec![], infra: Some(e) },
                    };
                    let mut files: BTreeMap<String, (BTreeSet<usize>, BTreeSet<usize>)> = BTreeMap::new();
                    for p in &bparts {
                        if let Some(f) = loc_file(p) {
                            let e = files.entry(f).or_default();
                            e.0.extend(lines_of(&p["addedLines"]));
                            e.1.extend(lines_of(&p["deletedLines"]));
                        }
                    }
                    for (f, (a, d)) in &files {
                        let hs: Vec<revert::Hunk> = hunks.iter().filter(|(hf, _)| hf == f).map(|(_, h)| h.clone()).collect();
                        let o = fs::read_to_string(old_wc.abs.join(f)).unwrap_or_default();
                        let n = fs::read_to_string(new_wc.abs.join(f)).unwrap_or_default();
                        let (t, _) = revert::revert_file(&o, &n, &hs, a, d);
                        if t.is_empty() && !new_wc.abs.join(f).exists() {
                            continue;
                        }
                        let _ = fs::write(wc.abs.join(f), t);
                    }
                    let compiles = true;
                    let mut failing = Vec::new();
                    // only projects holding declared tests: they built and passed on new, so a
                    // build failure here is the revert's (another test project may not build at all)
                    let projs: Vec<(String, Vec<String>)> = decl_proj.iter().filter(|(_, n)| !n.is_empty()).cloned().collect();
                    for (p, names) in &projs {
                        let b = restore_build(sb, q, &wc, p);
                        if !b.ok() {
                            let errs = compile_errors(&b);
                            if b.step != "build" || b.timed_out || errs.is_empty() {
                                let tail: Vec<&str> = b.stdout.lines().chain(b.stderr.lines()).filter(|l| !l.trim().is_empty()).collect();
                                let tail = tail[tail.len().saturating_sub(8)..].join("\n");
                                return RevertRun { id, compiles: false, failing: vec![], rel, errors: vec![], infra: Some(format!("{} exit {:?}{}: {tail}", b.step, b.exit, if b.timed_out { " (timed out)" } else { "" })) };
                            }
                            return RevertRun { id, compiles: false, failing: vec![], rel, errors: errs.into_iter().take(5).collect(), infra: None };
                        }
                        let r = run_tests(sb, q, &wc, p, names, &format!("revert-{id}"));
                        for n in names {
                            if r.get(n).map(|x| x.outcome == Outcome::Fail).unwrap_or(false) {
                                failing.push(n.clone());
                            }
                        }
                    }
                    RevertRun { id, compiles, failing, rel, errors: vec![], infra: None }
                })
            })
            .collect();
        hs.into_iter().filter_map(|h| h.join().ok()).collect()
    });
    // a revert build that could not be run is infrastructure, not `not-linked`: the module
    // fails and the bundle builder marks the tests section `not-run` with this detail
    let infra: Vec<String> = link_results.iter().filter_map(|r| r.infra.as_ref().map(|e| format!("{}: {e}", r.id))).collect();
    if !infra.is_empty() {
        return Err(format!("revert build not run: {}", infra.join("; ")));
    }

    let diffs = differences["differences"].as_array().cloned().unwrap_or_default();
    let not_modelled = scope["notModelled"].as_array().cloned().unwrap_or_default();
    let block_has_not_modelled = |b: &Value| -> bool {
        b["parts"].as_array().map(|ps| ps.iter().any(|p| {
            let Some(p) = p.as_str().and_then(|p| part_by_id.get(p)) else { return false };
            let (Some(f), Some(l), Some(e)) = (p["new"]["file"].as_str(), p["new"]["line"].as_u64(), p["new"]["endLine"].as_u64()) else { return false };
            not_modelled.iter().any(|n| n["location"]["file"] == f && n["location"]["line"].as_u64().unwrap_or(0) <= e && n["location"]["endLine"].as_u64().unwrap_or(0) >= l)
        })).unwrap_or(false)
    };
    let mut links = Vec::new();
    let mut state_of: BTreeMap<String, String> = BTreeMap::new();
    let mut reverted = Vec::new();
    for b in &prod {
        let id = b["id"].as_str().unwrap_or("");
        let Some(rr) = link_results.iter().find(|r| r.id == id) else { return Err(format!("revert of {id} panicked")) };
        let (compiles, failing, rel, errors) = (rr.compiles, rr.failing.clone(), rr.rel.clone(), rr.errors.clone());
        let state = if !compiles { "not-linked" } else if failing.is_empty() { "not-declared" } else { "declared" };
        let has_diff = diffs.iter().any(|d| d["block"] == id);
        let placement = match state {
            "declared" => "declared-region",
            "not-linked" => "outside",
            _ if has_diff || block_has_not_modelled(b) => "outside",
            _ => "neither",
        };
        let tests_v: Vec<String> = if state == "declared" { failing } else { vec![] };
        links.push(json!({"block": id, "state": state, "tests": tests_v, "placement": placement}));
        state_of.insert(id.into(), state.into());
        reverted.push(json!({"block": id, "compiles": compiles, "errors": errors, "path": if compiles { Value::from(sb.root.join(&rel).to_string_lossy().to_string()) } else { Value::Null }}));
    }
    let links_json = json!({"provenance": "pipeline", "links": links});

    // 5. edited snapshots: "output changed" with before and after; never declare a block (D26)
    let grouped: BTreeSet<String> = groups["groups"].as_array().map(|g| g.iter().flat_map(|x| x["parts"].as_array().cloned().unwrap_or_default()).filter_map(|p| p.as_str().map(String::from)).collect()).unwrap_or_default();
    let snapshots = snapshot_entries(&parts, &grouped, &|f| fs::read_to_string(old_wc.abs.join(f)).ok(), &|f| fs::read_to_string(new_wc.abs.join(f)).ok());

    // 6. generated tests for undeclared differences with a replay input (D28)
    let replay_list = replays["replays"].as_array().cloned().unwrap_or_default();
    let test_dir = run_proj.keys().next().and_then(|p| Path::new(p).parent().map(|d| d.to_string_lossy().to_string())).unwrap_or_else(|| "tests/MessagePack.SourceGenerator.Tests".into());
    let mut suggestions = Vec::new();
    for d in &diffs {
        let did = d["id"].as_str().unwrap_or("");
        let block = d["block"].as_str().unwrap_or("");
        let undeclared = matches!(state_of.get(block).map(String::as_str), Some("not-declared") | Some("not-linked"));
        let Some(r) = replay_list.iter().find(|r| r["difference"] == did) else { continue };
        let Some(text) = r["input"]["text"].as_str() else { continue };
        if !undeclared {
            continue;
        }
        let (class, src) = gen::test_source(did, d["number"].as_u64().unwrap_or(0), block, text, r["new"].as_str().unwrap_or(""));
        let path = format!("{test_dir}/Witness/{class}.cs");
        suggestions.push((did.to_string(), block.to_string(), class, path.clone(), gen::new_file_patch(&path, &src)));
    }
    let mut suggested = Vec::new();
    if !suggestions.is_empty() {
        let gen_rel = format!("{work}/gen/{}", &new_c[..12]);
        let pdir_rel = format!("{work}/patches/{}", &new_c[..12]);
        fs::create_dir_all(sb.root.join(&pdir_rel)).map_err(|e| e.to_string())?;
        let wc = worktree(sb, &gen_rel, &new_c)?;
        let mut applied = Vec::new();
        for (did, _, _, _, patch) in &suggestions {
            let pf = format!("{pdir_rel}/{did}.patch");
            fs::write(sb.root.join(&pf), patch).map_err(|e| e.to_string())?;
            applied.push(git(sb, &gen_rel, &["apply", &format!("{}/{pf}", sb.root.display())]).is_ok());
        }
        let proj = project_of(&wc.abs, &suggestions[0].3).unwrap_or_default();
        let b = restore_build(sb, q, &wc, &proj);
        for (k, (did, block, class, path, patch)) in suggestions.iter().enumerate() {
            let compiles = applied[k] && b.ok() && errors_in(&b, &format!("{class}.cs")).is_empty();
            suggested.push(json!({"difference": did, "block": block, "file": path, "test": format!("{class}.Difference{}_ReplayInput", did.trim_start_matches('d')), "compiles": compiles, "patch": patch}));
        }
    }
    let suggested_json = json!({"provenance": "pipeline", "suggested": suggested});
    let mut reverted_json = json!({"provenance": "pipeline", "reverted": reverted});
    let (mut tests_json, mut snapshots, mut suggested_json) = (tests_json, snapshots, suggested_json);
    for v in [&mut tests_json, &mut snapshots, &mut suggested_json, &mut reverted_json] {
        relativize_json(v, work, "");
    }
    Ok(Output { tests: tests_json, links: links_json, snapshots, suggested: suggested_json, reverted: reverted_json, old_commit: old_c, new_commit: new_c })
}

/// Snapshot parts outside mechanical groups, as "output changed" entries with the file's
/// before and after text (`checker: file-comparison`). Not a contract record; the bundle
/// builder numbers them into `differences.json`.
pub fn snapshot_entries(parts: &[Value], grouped: &BTreeSet<String>, old: &dyn Fn(&str) -> Option<String>, new: &dyn Fn(&str) -> Option<String>) -> Value {
    let mut out = Vec::new();
    for p in parts.iter().filter(|p| p["category"] == "snapshot") {
        let id = p["id"].as_str().unwrap_or("");
        if grouped.contains(id) {
            continue;
        }
        let Some(file) = loc_file(p) else { continue };
        let pick = |text: Option<String>, loc: &Value| -> Option<String> {
            let t = text?;
            let (l, e) = (loc["line"].as_u64()? as usize, loc["endLine"].as_u64()? as usize);
            Some(t.lines().skip(l.saturating_sub(1)).take(e + 1 - l).collect::<Vec<_>>().join("\n"))
        };
        out.push(json!({
            "part": id, "kind": "output-changed", "checker": "file-comparison",
            "location": p["new"].clone(), "file": file,
            "before": pick(old(p["old"]["file"].as_str().unwrap_or(&file)), &p["old"]),
            "after": pick(new(&file), &p["new"]),
        }));
    }
    json!({"provenance": "pipeline", "snapshots": out})
}

/// Write the outputs: `tests.json`, `links.json` (contract records), `snapshots.json`,
/// `suggested-tests.json`, `reverted.json` and `patches/<difference>.patch`.
pub fn write(out: &Output, dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir.join("patches")).map_err(|e| e.to_string())?;
    let w = |n: &str, v: &Value| fs::write(dir.join(n), serde_json::to_string_pretty(v).unwrap() + "\n").map_err(|e| e.to_string());
    w("tests.json", &out.tests)?;
    w("links.json", &out.links)?;
    w("snapshots.json", &out.snapshots)?;
    w("suggested-tests.json", &out.suggested)?;
    w("reverted.json", &out.reverted)?;
    for s in out.suggested["suggested"].as_array().into_iter().flatten() {
        if let (Some(d), Some(p)) = (s["difference"].as_str(), s["patch"].as_str()) {
            fs::write(dir.join("patches").join(format!("{d}.patch")), p).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn categories() {
        assert_eq!(categorize(Change::Added, false, Outcome::Fail, Outcome::Pass), "declared");
        assert_eq!(categorize(Change::Added, false, Outcome::NotCompiled, Outcome::Pass), "new-api");
        assert_eq!(categorize(Change::Added, false, Outcome::Pass, Outcome::Pass), "hanging");
        assert_eq!(categorize(Change::Removed, false, Outcome::NotRun, Outcome::NotRun), "withdrawn");
        assert_eq!(categorize(Change::Edited, true, Outcome::Pass, Outcome::Pass), "withdrawn");
        assert_eq!(categorize(Change::Added, false, Outcome::NotRun, Outcome::NotRun), "not-run");
        assert_eq!(categorize(Change::Added, false, Outcome::Fail, Outcome::Fail), "not-run");
    }

    #[test]
    fn diff_finds_added_edited_removed() {
        let old = "public class C {\n [Fact]\n public void A() { Assert.True(x); }\n [Fact]\n public void B() { Assert.Equal(1, y); }\n [Fact]\n public void D() { }\n}\n";
        let new = "public class C {\n [Fact]\n public void A() { Assert.True(x); }\n [Fact]\n public void B() { }\n [Fact]\n public void E() { Assert.False(z); }\n}\n";
        let t = diff_tests("C.cs", Some(old), Some(new));
        let got: Vec<(String, Change, bool)> = t.iter().map(|t| (t.name.clone(), t.change, t.loosened)).collect();
        assert_eq!(got, vec![("C.B".into(), Change::Edited, true), ("C.E".into(), Change::Added, false), ("C.D".into(), Change::Removed, false)]);
    }

    #[test]
    fn infra_failure_is_not_a_fail() {
        let r = vec![trx::TrxResult { test_name: "C.A".into(), outcome: "Failed".into(), message: Some("System.Net.Http.HttpRequestException: x".into()), stack: None }];
        assert_eq!(fold_trx(&r)["C.A"].outcome, Outcome::NotRun);
        assert_eq!(frame_line("   at C.A() in /x/tests/T.cs:line 42\n", "tests/T.cs"), Some(42));
    }
}

/// Parsed command line: `--records D --old R --new R --out D [--root N]`, or the module form
/// `[--root N] <stageDir> <bundleDir> <repo> <old> <new>` (repo ignored; `extra` = `<bundleDir>/tests-extra`).
#[derive(Debug, PartialEq)]
pub struct Args {
    pub records: PathBuf,
    pub out: PathBuf,
    pub old: String,
    pub new: String,
    pub root: Option<String>,
    pub extra: Option<PathBuf>,
}

pub fn parse_args(a: &[String]) -> Result<Args, String> {
    let mut root = None;
    let mut named: BTreeMap<String, String> = BTreeMap::new();
    let mut pos = Vec::new();
    let mut i = 0;
    while i < a.len() {
        if let Some(k) = a[i].strip_prefix("--") {
            let v = a.get(i + 1).cloned().ok_or(format!("--{k} needs a value"))?;
            if k == "root" {
                root = Some(v)
            } else {
                named.insert(k.into(), v);
            }
            i += 2;
        } else {
            pos.push(a[i].clone());
            i += 1;
        }
    }
    if pos.len() == 5 {
        let bundle = PathBuf::from(&pos[1]);
        return Ok(Args { records: bundle.clone(), out: PathBuf::from(&pos[0]), old: pos[3].clone(), new: pos[4].clone(), root, extra: Some(bundle.join("tests-extra")) });
    }
    match (named.get("records"), named.get("old"), named.get("new"), named.get("out")) {
        (Some(r), Some(o), Some(n), Some(d)) if pos.is_empty() => Ok(Args { records: r.into(), out: d.into(), old: o.clone(), new: n.clone(), root, extra: None }),
        _ => Err("usage: testcontract --records <dir> --old <rev> --new <rev> --out <dir> [--root NAME] | [--root NAME] <stageDir> <bundleDir> <repo> <old> <new>".into()),
    }
}

/// Default sandbox root name: `testcontract-<first 12 of the new commit>`.
pub fn default_root(new_commit: &str) -> String {
    format!("testcontract-{}", &new_commit[..new_commit.len().min(12)])
}

/// The extra outputs only (module form: `<bundleDir>/tests-extra/`).
pub fn write_extra(out: &Output, dir: &Path) -> Result<(), String> {
    write(out, dir)?;
    let _ = fs::remove_file(dir.join("tests.json"));
    let _ = fs::remove_file(dir.join("links.json"));
    Ok(())
}

#[cfg(test)]
mod twin_tests {
    use super::*;
    #[test]
    fn tt_block_takes_generated_twin() {
        let p = |id: &str, cat: &str, f: &str| json!({"id": id, "category": cat, "new": {"file": f, "line": 1, "endLine": 1}, "old": null});
        let all = vec![p("p1", "production", "T/A.tt"), p("p2", "generated", "T/A.cs"), p("p3", "generated", "T/B.cs"), p("p4", "production", "T/B2.tt")];
        let got: Vec<&str> = generated_twins(&[&all[0]], &all).iter().map(|v| v["id"].as_str().unwrap()).collect();
        assert_eq!(got, vec!["p2"]);
        assert!(generated_twins(&[&all[3]], &all).is_empty());
    }
}

#[cfg(test)]
mod args_tests {
    use super::*;
    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }
    #[test]
    fn module_and_named_forms() {
        let a = parse_args(&s(&["--root", "x", "st", "bd", "repo", "o1", "n1"])).unwrap();
        assert_eq!(a, Args { records: "bd".into(), out: "st".into(), old: "o1".into(), new: "n1".into(), root: Some("x".into()), extra: Some(PathBuf::from("bd/tests-extra")) });
        let b = parse_args(&s(&["--records", "r", "--old", "o", "--new", "n", "--out", "d"])).unwrap();
        assert_eq!((b.records, b.extra, b.root), (PathBuf::from("r"), None, None));
        assert!(parse_args(&s(&["a", "b"])).is_err());
        assert_eq!(default_root("0123456789abcdef"), "testcontract-0123456789ab");
    }
}

/// Rewrites every absolute path under a sandbox working copy (`<any root>/<work>/{mix,gen}/<id>/…`
/// or `<any root>/<work>/rev/<id>/<block>/…`) to its repository-relative form. Nothing else changes.
pub fn relativize(s: &str, work: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    loop {
        let mut best: Option<(usize, usize)> = None; // (path start, relative part start)
        for kind in ["mix", "gen", "rev"] {
            let needle = format!("/{work}/{kind}/");
            let Some(i) = rest.find(&needle) else { continue };
            let start = rest[..i].rfind(|c: char| c.is_whitespace() || "\"'(<=,;`".contains(c)).map_or(0, |j| j + 1);
            if !rest[start..].starts_with('/') {
                continue;
            }
            let mut end = i + needle.len();
            let mut ok = true;
            for _ in 0..if kind == "rev" { 2 } else { 1 } {
                match rest[end..].find('/') {
                    Some(j) if j > 0 && !rest[end..end + j].chars().any(char::is_whitespace) => end += j + 1,
                    _ => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok && best.map_or(true, |(b, _)| start < b) {
                best = Some((start, end));
            }
        }
        match best {
            Some((start, end)) => {
                out.push_str(&rest[..start]);
                rest = &rest[end..];
            }
            None => {
                out.push_str(rest);
                return out;
            }
        }
    }
}

fn relativize_json(v: &mut Value, work: &str, key: &str) {
    match v {
        // reverted.json `path` is the kept build's host path by contract (read by the builder, never shown).
        Value::String(s) if key != "path" => *s = relativize(s, work),
        Value::Array(a) => a.iter_mut().for_each(|x| relativize_json(x, work, "")),
        Value::Object(o) => o.iter_mut().for_each(|(k, x)| relativize_json(x, work, k)),
        _ => {}
    }
}

#[cfg(test)]
mod relativize_tests {
    use super::relativize;

    #[test]
    fn stack_trace_paths_become_repository_relative() {
        let t = "System.Exception : boom\n   at MessagePack.SourceGenerator.CodeAnalysis.TypeCollector.Get() in /srv/x/work/sandbox/testcontract-abc/tc/mix/111111111111-222222222222/src/MessagePack.SourceGenerator/CodeAnalysis/TypeCollector.cs:line 1075\n   at T.M() in /home/u/w/sandbox/r/tc/rev/222222222222/b3/tests/A/B.cs:line 9\nerror CS0103: (/opt/s/tc/gen/222222222222/src/X.cs(4,5)) keep /other/path.cs";
        assert_eq!(
            relativize(t, "tc"),
            "System.Exception : boom\n   at MessagePack.SourceGenerator.CodeAnalysis.TypeCollector.Get() in src/MessagePack.SourceGenerator/CodeAnalysis/TypeCollector.cs:line 1075\n   at T.M() in tests/A/B.cs:line 9\nerror CS0103: (src/X.cs(4,5)) keep /other/path.cs"
        );
    }
}
