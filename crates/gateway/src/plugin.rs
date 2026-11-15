//! Forwarding to the plugin host: one long-lived host process per (bundle, plugin), started as
//! `<host command...> <plugin> <bundle dir>`, exchanging one JSON message per line
//! (`plugin-request.schema.json` in, `plugin-reply.schema.json` out).
//! The table lock is held only to look up or insert a process; each process has its own lock.
//! A request without a reply within the timeout kills the process and drops it from the table.
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(90);

struct Proc {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    next_id: u64,
}

impl Proc {
    fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

type Shared = Arc<Mutex<Proc>>;

#[derive(Default)]
pub struct Hosts {
    procs: Mutex<HashMap<String, Shared>>,
}

impl Hosts {
    fn get_or_start(&self, cmd: &[String], plugin: &str, bundle_dir: &Path, name: &str) -> Result<Shared, String> {
        let mut procs = self.procs.lock().unwrap();
        if let Some(p) = procs.get(name) {
            // try_lock: a busy process is alive by definition.
            let alive = match p.try_lock() {
                Ok(mut g) => matches!(g.child.try_wait(), Ok(None)),
                Err(_) => true,
            };
            if alive {
                return Ok(p.clone());
            }
            procs.remove(name);
        }
        let mut child = Command::new(&cmd[0])
            .args(&cmd[1..])
            .arg(plugin)
            .arg(bundle_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("plugin host: {e}"))?;
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in stdout.lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let p = Arc::new(Mutex::new(Proc { child, stdin, lines: rx, next_id: 1 }));
        procs.insert(name.to_string(), p.clone());
        Ok(p)
    }

    fn drop_proc(&self, name: &str, p: &Shared) {
        let mut procs = self.procs.lock().unwrap();
        if procs.get(name).map(|q| Arc::ptr_eq(q, p)).unwrap_or(false) {
            procs.remove(name);
        }
    }

    /// Send one request and read lines until the reply with the same id. The page's id is
    /// replaced by the gateway's own on the way in and restored on the way out.
    pub fn forward(&self, cmd: &[String], plugin: &str, bundle_dir: &Path, key: &str, mut request: Value, timeout: Duration) -> Result<Value, String> {
        if cmd.is_empty() {
            return Err("no plugin host configured".into());
        }
        let page_id = request.get("id").cloned().unwrap_or(Value::from(1));
        let name = format!("{key}/{plugin}");
        let shared = self.get_or_start(cmd, plugin, bundle_dir, &name)?;
        let mut p = shared.lock().unwrap();
        let id = p.next_id;
        p.next_id += 1;
        request["id"] = Value::from(id);
        let line = serde_json::to_string(&request).unwrap();
        let deadline = Instant::now() + timeout;
        let pr: &mut Proc = &mut p;
        let (stdin, lines) = (&mut pr.stdin, &pr.lines);
        let res = (|| {
            writeln!(stdin, "{line}").map_err(|e| e.to_string())?;
            stdin.flush().map_err(|e| e.to_string())?;
            loop {
                let left = deadline.saturating_duration_since(Instant::now());
                let out = match lines.recv_timeout(left) {
                    Ok(l) => l,
                    Err(RecvTimeoutError::Timeout) => return Err(format!("plugin host timed out after {} s", timeout.as_secs())),
                    Err(RecvTimeoutError::Disconnected) => return Err("plugin host closed its output".to_string()),
                };
                let Ok(mut v) = serde_json::from_str::<Value>(out.trim()) else { continue };
                if v.get("id").and_then(Value::as_u64) == Some(id) {
                    v["id"] = page_id.clone();
                    return Ok(v);
                }
            }
        })();
        if res.is_err() {
            p.kill();
            drop(p);
            self.drop_proc(&name, &shared);
        }
        res
    }
}

impl Drop for Hosts {
    fn drop(&mut self) {
        for (_, p) in self.procs.lock().unwrap().drain() {
            if let Ok(mut p) = p.lock() {
                let id = p.next_id;
                let _ = writeln!(p.stdin, "{{\"id\":{id},\"op\":\"shutdown\"}}");
                p.kill();
            }
        }
    }
}
