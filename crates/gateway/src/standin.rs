//! Local stand-in for the two GitHub REST endpoints the gateway uses:
//! `GET /repos/{owner}/{repo}/pulls/{n}` and `POST /repos/{owner}/{repo}/pulls/{n}/reviews`.
use crate::http::{Request, Response};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct Pull {
    pub head: String,
    pub author: String,
}

/// A request the stand-in received: method, path, Authorization header, JSON body.
#[derive(Debug, Clone)]
pub struct Received {
    pub method: String,
    pub path: String,
    pub authorization: Option<String>,
    pub body: Value,
}

#[derive(Default)]
pub struct State {
    /// `"{owner}/{repo}#{n}"` → pull request.
    pub pulls: Mutex<HashMap<String, Pull>>,
    /// token → login.
    pub tokens: Mutex<HashMap<String, String>>,
    pub received: Mutex<Vec<Received>>,
    pub base: Mutex<String>,
}

impl State {
    pub fn set_pull(&self, repo: &str, n: u64, head: &str, author: &str) {
        self.pulls.lock().unwrap().insert(format!("{repo}#{n}"), Pull { head: head.into(), author: author.into() });
    }
    pub fn add_token(&self, token: &str, login: &str) {
        self.tokens.lock().unwrap().insert(token.into(), login.into());
    }
    /// Received review submissions (POSTs) only.
    pub fn reviews(&self) -> Vec<Received> {
        self.received.lock().unwrap().iter().filter(|r| r.method == "POST").cloned().collect()
    }
}

fn handle(st: &State, req: &Request) -> Response {
    let auth = req.header("authorization").map(String::from);
    st.received.lock().unwrap().push(Received {
        method: req.method.clone(),
        path: req.path.clone(),
        authorization: auth.clone(),
        body: req.json().unwrap_or(Value::Null),
    });
    let seg: Vec<&str> = req.path.trim_matches('/').split('/').collect();
    let not_found = || Response::json(404, &json!({"message": "Not Found"}));
    if seg.len() < 5 || seg[0] != "repos" || seg[3] != "pulls" {
        return not_found();
    }
    let repo = format!("{}/{}", seg[1], seg[2]);
    let Ok(n) = seg[4].parse::<u64>() else { return not_found() };
    let Some(pull) = st.pulls.lock().unwrap().get(&format!("{repo}#{n}")).cloned() else { return not_found() };
    match (req.method.as_str(), seg.len()) {
        ("GET", 5) => Response::json(200, &json!({"number": n, "head": {"sha": pull.head}, "user": {"login": pull.author}})),
        ("POST", 6) if seg[5] == "reviews" => {
            let token = auth.as_deref().and_then(|a| a.strip_prefix("Bearer ").or_else(|| a.strip_prefix("token "))).map(str::trim);
            let Some(login) = token.and_then(|t| st.tokens.lock().unwrap().get(t).cloned()) else {
                return Response::json(401, &json!({"message": "Bad credentials"}));
            };
            let body = req.json().unwrap_or(Value::Null);
            let event = body.get("event").and_then(Value::as_str).unwrap_or("");
            if event == "APPROVE" && login == pull.author {
                return Response::json(422, &json!({"message": "Unprocessable Entity", "errors": ["Can not approve your own pull request"]}));
            }
            let id = st.reviews().len();
            let base = st.base.lock().unwrap().clone();
            Response::json(
                200,
                &json!({"id": id, "user": {"login": login}, "state": if event == "APPROVE" {"APPROVED"} else {"CHANGES_REQUESTED"},
                        "commit_id": body.get("commit_id").cloned().unwrap_or(Value::Null),
                        "html_url": format!("{base}/repos/{repo}/pulls/{n}#pullrequestreview-{id}")}),
            )
        }
        _ => not_found(),
    }
}

/// Serve the stand-in on `listener` in a background thread.
pub fn start(listener: TcpListener) -> Arc<State> {
    let st = Arc::new(State::default());
    *st.base.lock().unwrap() = format!("http://{}", listener.local_addr().unwrap());
    let s2 = st.clone();
    std::thread::spawn(move || crate::http::serve(listener, move |r| handle(&s2, r)));
    st
}
