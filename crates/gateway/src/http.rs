//! Minimal HTTP/1.1 over std: one request per connection, `Connection: close`.
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}
impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    pub fn param(&self, name: &str) -> Option<&str> {
        self.query.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }
    pub fn json(&self) -> Option<Value> {
        serde_json::from_slice(&self.body).ok()
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
    pub content_type: &'static str,
}
impl Response {
    pub fn json(status: u16, v: &Value) -> Response {
        Response { status, body: serde_json::to_vec(v).unwrap(), content_type: "application/json" }
    }
    pub fn raw_json(status: u16, body: Vec<u8>) -> Response {
        Response { status, body, content_type: "application/json" }
    }
}

pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16) {
                    Ok(v) => {
                        out.push(v);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let mut r = BufReader::new(stream.try_clone().ok()?);
    let mut line = String::new();
    r.read_line(&mut line).ok()?;
    let mut it = line.split_whitespace();
    let method = it.next()?.to_string();
    let target = it.next()?.to_string();
    let mut headers = Vec::new();
    loop {
        let mut h = String::new();
        if r.read_line(&mut h).ok()? == 0 {
            break;
        }
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    let len: usize = headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("content-length")).and_then(|(_, v)| v.parse().ok()).unwrap_or(0);
    if len > 16 << 20 {
        return None;
    }
    let mut body = vec![0; len];
    r.read_exact(&mut body).ok()?;
    let (path, q) = target.split_once('?').unwrap_or((&target, ""));
    let query = q
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (percent_decode(k), percent_decode(v))
        })
        .collect();
    Some(Request { method, path: percent_decode(path), query, headers, body })
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        409 => "Conflict",
        422 => "Unprocessable Entity",
        502 => "Bad Gateway",
        _ => "Status",
    }
}

/// Serve `handler` on `listener` until the process ends, one thread per connection.
pub fn serve<F>(listener: TcpListener, handler: F)
where
    F: Fn(&Request) -> Response + Send + Sync + 'static,
{
    let handler = Arc::new(handler);
    for stream in listener.incoming().flatten() {
        let h = handler.clone();
        std::thread::spawn(move || {
            let mut s = stream;
            let _ = s.set_read_timeout(Some(Duration::from_secs(30)));
            let resp = match read_request(&mut s) {
                Some(req) => h(&req),
                None => Response::json(400, &serde_json::json!({"error": "bad request", "gate": null})),
            };
            let head = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                resp.status,
                reason(resp.status),
                resp.content_type,
                resp.body.len()
            );
            let _ = s.write_all(head.as_bytes());
            let _ = s.write_all(&resp.body);
            let _ = s.flush();
        });
    }
}

/// A blocking HTTP client call: `url` is `http://host:port/path`. Returns status and body.
pub fn call(method: &str, url: &str, headers: &[(&str, &str)], body: Option<&[u8]>) -> Result<(u16, Vec<u8>), String> {
    let rest = url.strip_prefix("http://").ok_or("only http:// urls")?;
    let (host, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let mut s = TcpStream::connect(host).map_err(|e| format!("{host}: {e}"))?;
    s.set_read_timeout(Some(Duration::from_secs(30))).ok();
    let body = body.unwrap_or(&[]);
    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Length: {}\r\n", body.len());
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    s.write_all(body).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let split = buf.windows(4).position(|w| w == b"\r\n\r\n").ok_or("no header end")?;
    let head = String::from_utf8_lossy(&buf[..split]).to_string();
    let status = head.split_whitespace().nth(1).and_then(|x| x.parse().ok()).ok_or("bad status line")?;
    Ok((status, buf[split + 4..].to_vec()))
}
