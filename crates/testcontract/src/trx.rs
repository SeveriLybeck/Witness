//! Reading a VSTest `.trx` result file (string scanning; the format is flat enough).

#[derive(Clone, Debug, PartialEq)]
pub struct TrxResult {
    /// xUnit display name: the fully qualified method name, plus `(args)` for theory cases.
    pub test_name: String,
    /// `Passed`, `Failed`, `NotExecuted`, ...
    pub outcome: String,
    pub message: Option<String>,
    pub stack: Option<String>,
}

pub fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('&') {
        out.push_str(&rest[..p]);
        let after = &rest[p..];
        let end = after.find(';').unwrap_or(0);
        let ent = &after[1..end.max(1)];
        let rep = match ent {
            "lt" => Some("<".to_string()),
            "gt" => Some(">".to_string()),
            "amp" => Some("&".to_string()),
            "quot" => Some("\"".to_string()),
            "apos" => Some("'".to_string()),
            e if e.starts_with("#x") => u32::from_str_radix(&e[2..], 16).ok().and_then(char::from_u32).map(|c| c.to_string()),
            e if e.starts_with('#') => e[1..].parse::<u32>().ok().and_then(char::from_u32).map(|c| c.to_string()),
            _ => None,
        };
        match rep {
            Some(r) if end > 0 => {
                out.push_str(&r);
                rest = &after[end + 1..];
            }
            _ => {
                out.push('&');
                rest = &after[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let key = format!(" {name}=\"");
    let p = tag.find(&key)? + key.len();
    let e = tag[p..].find('"')?;
    Some(unescape(&tag[p..p + e]))
}

fn element(body: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let p = body.find(&open)? + open.len();
    let e = body[p..].find(&format!("</{name}>"))?;
    Some(unescape(&body[p..p + e]))
}

pub fn parse(xml: &str) -> Vec<TrxResult> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(p) = rest.find("<UnitTestResult ") {
        let after = &rest[p..];
        let tag_end = after.find('>').unwrap_or(after.len());
        let tag = &after[..tag_end];
        let self_closing = tag.ends_with('/');
        let (body, next) = if self_closing {
            ("", &after[tag_end..])
        } else {
            match after.find("</UnitTestResult>") {
                Some(e) => (&after[tag_end..e], &after[e..]),
                None => (&after[tag_end..], ""),
            }
        };
        if let (Some(n), Some(o)) = (attr(tag, "testName"), attr(tag, "outcome")) {
            out.push(TrxResult { test_name: n, outcome: o, message: element(body, "Message"), stack: element(body, "StackTrace") });
        }
        rest = if next.len() < after.len() { &next[next.len().min(1)..] } else { "" };
        if next.is_empty() {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_results() {
        let x = r#"<Results><UnitTestResult testId="1" testName="C.A" outcome="Passed" /><UnitTestResult testName="C.B(x: 1)" outcome="Failed"><Output><ErrorInfo><Message>Assert.Empty() Failure: &lt;x&gt; &amp; MsgPack004</Message><StackTrace>   at C.B() in /w/T.cs:line 12</StackTrace></ErrorInfo></Output></UnitTestResult></Results>"#;
        let r = parse(x);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].outcome, "Passed");
        assert_eq!(r[1].test_name, "C.B(x: 1)");
        assert_eq!(r[1].message.as_deref(), Some("Assert.Empty() Failure: <x> & MsgPack004"));
        assert!(r[1].stack.as_deref().unwrap().contains(":line 12"));
    }
}
