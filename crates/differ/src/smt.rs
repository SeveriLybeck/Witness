//! One long-lived `z3 -in` process fed SMT-LIB text (playbook D1, option A).

use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sat {
    Sat,
    Unsat,
    Unknown,
}

pub struct Z3 {
    child: Child,
    stdin: ChildStdin,
    out: BufReader<ChildStdout>,
    declared: HashSet<String>,
    /// Declarations in the order sent; copied into solver-rows.json.
    pub decls: Vec<String>,
    pub queries: usize,
}

impl Z3 {
    pub fn start() -> Result<Z3, String> {
        let mut child = Command::new("z3")
            .args(["-in", "-smt2"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("cannot start z3: {e}"))?;
        let stdin = child.stdin.take().unwrap();
        let out = BufReader::new(child.stdout.take().unwrap());
        let mut z = Z3 { child, stdin, out, declared: HashSet::new(), decls: vec![], queries: 0 };
        z.send("(set-option :timeout 2000)\n(set-option :produce-models true)\n")?;
        Ok(z)
    }

    fn send(&mut self, s: &str) -> Result<(), String> {
        self.stdin.write_all(s.as_bytes()).and_then(|_| self.stdin.flush()).map_err(|e| format!("z3 write: {e}"))
    }

    fn line(&mut self) -> Result<String, String> {
        let mut l = String::new();
        if self.out.read_line(&mut l).map_err(|e| e.to_string())? == 0 {
            return Err("z3 exited".into());
        }
        Ok(l.trim().to_string())
    }

    /// Send a declaration once; `key` is the declared symbol.
    pub fn declare(&mut self, key: &str, decl: String) -> Result<(), String> {
        if self.declared.insert(key.to_string()) {
            self.send(&format!("{decl}\n"))?;
            self.decls.push(decl);
        }
        Ok(())
    }

    /// `(push) (assert a)… (check-sat) [(get-value …)] (pop)`.
    pub fn check(&mut self, asserts: &[String], get: &[String]) -> Result<(Sat, Option<String>), String> {
        self.queries += 1;
        let mut q = String::from("(push 1)\n");
        for a in asserts {
            q.push_str(&format!("(assert {a})\n"));
        }
        q.push_str("(check-sat)\n");
        self.send(&q)?;
        let r = loop {
            let l = self.line()?;
            match l.as_str() {
                "sat" => break Sat::Sat,
                "unsat" => break Sat::Unsat,
                "unknown" | "timeout" => break Sat::Unknown,
                "" => continue,
                _ => {
                    let _ = self.send("(pop 1)\n");
                    return Err(format!("z3: {l} in {}", asserts.join(" ")));
                }
            }
        };
        let mut val = None;
        if r == Sat::Sat && !get.is_empty() {
            self.send(&format!("(get-value ({}))\n", get.join(" ")))?;
            let mut acc = String::new();
            let mut depth: i64 = 0;
            loop {
                let l = self.line()?;
                depth += l.matches('(').count() as i64 - l.matches(')').count() as i64;
                acc.push_str(&l);
                acc.push(' ');
                if depth <= 0 && !acc.trim().is_empty() {
                    break;
                }
            }
            val = Some(acc.trim().to_string());
        }
        self.send("(pop 1)\n")?;
        Ok((r, val))
    }
}

impl Drop for Z3 {
    fn drop(&mut self) {
        let _ = self.send("(exit)\n");
        let _ = self.child.wait();
    }
}

/// `z3 --version` output, e.g. "Z3 version 4.16.0 - 64 bit".
pub fn version() -> Result<String, String> {
    let o = Command::new("z3").arg("--version").output().map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Quote a symbol when it is not a plain SMT-LIB simple symbol.
pub fn q(name: &str) -> String {
    let plain = !name.is_empty()
        && !name.chars().next().unwrap().is_ascii_digit()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if plain {
        name.to_string()
    } else {
        format!("|{}|", name.replace('|', "_"))
    }
}
