//! Pull request intake by pasted link (D44).
//!
//! A pasted link names a repository and a pull request (`<repo>/pull/<n>`) or a
//! range (`<repo>/compare/<old>...<new>`). Intake fetches it, sorts the changed
//! files by language, plans the map for every language with a parser and the
//! proof for every language with a front end, and offers a playground only for
//! recognised stacks; the playground starts only on an explicit click.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Pull(u64),
    Compare(String, String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub repository: String,
    pub target: Target,
}

/// Parse a pasted link. Accepts any git URL or path followed by
/// `/pull/<n>` (trailing `/files`, `/commits`, query or fragment ignored) or
/// `/compare/<old>...<new>` (also `..`).
pub fn parse_link(text: &str) -> Result<Link, String> {
    let s = text.trim();
    let s = s.split(['?', '#']).next().unwrap_or("");
    let s = s.trim_end_matches('/');
    if let Some(i) = s.rfind("/pull/") {
        let repo = &s[..i];
        let num = s[i + 6..].split('/').next().unwrap_or("");
        let n: u64 = num.parse().map_err(|_| format!("bad pull number: {num}"))?;
        return Ok(Link { repository: clean_repo(repo)?, target: Target::Pull(n) });
    }
    if let Some(i) = s.rfind("/compare/") {
        let repo = &s[..i];
        let range = &s[i + 9..];
        let (a, b) = range
            .split_once("...")
            .or_else(|| range.split_once(".."))
            .ok_or_else(|| format!("bad range: {range}"))?;
        if a.is_empty() || b.is_empty() {
            return Err(format!("bad range: {range}"));
        }
        return Ok(Link {
            repository: clean_repo(repo)?,
            target: Target::Compare(a.to_string(), b.to_string()),
        });
    }
    Err("link needs /pull/<n> or /compare/<old>...<new>".into())
}

fn clean_repo(r: &str) -> Result<String, String> {
    if r.is_empty() {
        return Err("no repository in link".into());
    }
    Ok(r.to_string())
}

/// Language of a path: (language, has parser, front end).
pub fn language(path: &str) -> Option<(&'static str, bool, Option<&'static str>)> {
    let ext = Path::new(path).extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "cs" => ("csharp", true, Some("translator")),
        "ts" | "tsx" | "mts" | "cts" => ("typescript", true, Some("translator-ts")),
        "js" | "jsx" | "mjs" | "cjs" => ("javascript", true, None),
        "rs" => ("rust", true, None),
        "go" => ("go", true, None),
        "java" => ("java", true, None),
        "c" | "h" => ("c", true, None),
        "cpp" | "cc" | "hpp" | "cxx" => ("cpp", true, None),
        "rb" => ("ruby", true, None),
        "py" => ("python", true, None),
        "json" => ("json", true, None),
        "md" => ("markdown", false, None),
        _ => return None,
    })
}

/// Recognised stack at a commit, from its file list.
pub fn stack(files: &[String]) -> Option<&'static str> {
    let base = |f: &String| f.rsplit('/').next().unwrap_or("").to_string();
    if files.iter().any(|f| f.ends_with(".sln") || f.ends_with(".csproj")) {
        Some("dotnet")
    } else if files.iter().any(|f| base(f) == "package.json") {
        Some("node")
    } else {
        None
    }
}

pub fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "core.hooksPath=/dev/null"])
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub struct Fetched {
    pub clone: PathBuf,
    pub old: String,
    pub new: String,
}

/// Fetch the link into `<work>/clone` (reused when present).
pub fn fetch(link: &Link, work: &Path, progress: &mut dyn FnMut(&str)) -> Result<Fetched, String> {
    let clone = work.join("clone");
    std::fs::create_dir_all(work).map_err(|e| e.to_string())?;
    if !clone.join(".git").exists() && !clone.join("HEAD").exists() {
        progress("fetch: clone");
        let c = clone.to_str().ok_or("bad path")?;
        git(work, &["clone", "--quiet", "--no-checkout", &link.repository, c])?;
    }
    progress("fetch: refs");
    let (old, new) = match &link.target {
        Target::Pull(n) => {
            let r = format!("+refs/pull/{n}/head:refs/witness/pull-{n}");
            git(&clone, &["fetch", "--quiet", "origin", &r])?;
            let new = git(&clone, &["rev-parse", &format!("refs/witness/pull-{n}^{{commit}}")])?;
            let base = git(&clone, &["rev-parse", "refs/remotes/origin/HEAD^{commit}"])?;
            let old = git(&clone, &["merge-base", &base, &new])?;
            (old, new)
        }
        Target::Compare(a, b) => {
            let _ = git(&clone, &["fetch", "--quiet", "origin"]);
            (resolve(&clone, a)?, resolve(&clone, b)?)
        }
    };
    Ok(Fetched { clone, old, new })
}

fn resolve(clone: &Path, r: &str) -> Result<String, String> {
    git(clone, &["rev-parse", &format!("refs/remotes/origin/{r}^{{commit}}")])
        .or_else(|_| git(clone, &["rev-parse", &format!("{r}^{{commit}}")]))
}

/// Build the plan for a fetched change.
pub fn plan(link_text: &str, f: &Fetched) -> Result<Value, String> {
    let changed: Vec<String> = git(&f.clone, &["diff", "--name-only", &f.old, &f.new])?
        .lines()
        .map(str::to_string)
        .collect();
    let tree: Vec<String> = git(&f.clone, &["ls-tree", "-r", "--name-only", &f.new])?
        .lines()
        .map(str::to_string)
        .collect();
    let mut langs: Vec<(String, bool, Option<&str>, Vec<String>)> = Vec::new();
    let mut other = Vec::new();
    for p in &changed {
        match language(p) {
            Some((l, parser, fe)) => match langs.iter_mut().find(|x| x.0 == l) {
                Some(x) => x.3.push(p.clone()),
                None => langs.push((l.to_string(), parser, fe, vec![p.clone()])),
            },
            None => other.push(p.clone()),
        }
    }
    let languages: Vec<Value> = langs
        .iter()
        .map(|(l, parser, fe, files)| {
            json!({
                "language": l,
                "files": files,
                "map": if *parser { "planned" } else { "no parser" },
                "proof": match fe { Some(_) => "planned", None => "no front end" },
                "frontEnd": fe,
            })
        })
        .collect();
    let st = stack(&tree);
    Ok(json!({
        "link": link_text.trim(),
        "repository": f.clone.to_string_lossy(),
        "old": f.old,
        "new": f.new,
        "languages": languages,
        "otherFiles": other,
        "playground": {"stack": st, "offered": st.is_some(), "started": false},
    }))
}

/// Start the playground. Only on an explicit click and a recognised stack.
pub fn start_playground(plan: &mut Value, clicked: bool) -> Result<String, String> {
    if !clicked {
        return Err("playground waits for a click".into());
    }
    let st = plan["playground"]["stack"].as_str().map(str::to_string);
    match st {
        Some(s) => {
            plan["playground"]["started"] = json!(true);
            Ok(s)
        }
        None => Err("no playground for this stack".into()),
    }
}

/// Run `witness prepare` (or any command) on the fetched change, passing each
/// output line to `progress`. No time cap. Returns the exit code.
pub fn run_prepare(cmd: &[String], f: &Fetched, plan: &Value, progress: &mut dyn FnMut(&str)) -> Result<i32, String> {
    use std::io::{BufRead, BufReader};
    let (prog, rest) = cmd.split_first().ok_or("empty prepare command")?;
    let fronts: Vec<&str> = plan["languages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|l| l["frontEnd"].as_str())
        .collect();
    let mut child = Command::new(prog)
        .args(rest)
        .arg(&f.clone)
        .arg(&f.old)
        .arg(&f.new)
        .env("WITNESS_FRONT_ENDS", fronts.join(","))
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("{prog}: {e}"))?;
    progress("prepare: started");
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            progress(&format!("prepare: {line}"));
        }
    }
    let code = child.wait().map_err(|e| e.to_string())?.code().unwrap_or(-1);
    progress(&format!("prepare: exit {code}"));
    Ok(code)
}
