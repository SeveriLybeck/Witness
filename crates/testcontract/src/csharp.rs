//! A small C# reader: masks comments and string literals, then finds xUnit test methods
//! (`[Fact]`, `[Theory]` and their `Skippable`/`Combinatorial` kin) with their enclosing type.

/// Replace the contents of comments, strings and char literals with spaces, keeping newlines,
/// so braces and keywords can be counted on what is left. Delimiting quotes stay.
pub fn mask(src: &str) -> String {
    let c: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let n = c.len();
    let mut i = 0;
    let blank = |ch: char| if ch == '\n' { '\n' } else { ' ' };
    while i < n {
        let ch = c[i];
        if ch == '/' && i + 1 < n && c[i + 1] == '/' {
            while i < n && c[i] != '\n' {
                out.push(' ');
                i += 1;
            }
        } else if ch == '/' && i + 1 < n && c[i + 1] == '*' {
            out.push_str("  ");
            i += 2;
            while i < n && !(c[i] == '*' && i + 1 < n && c[i + 1] == '/') {
                out.push(blank(c[i]));
                i += 1;
            }
            if i < n {
                out.push_str("  ");
                i += 2;
            }
        } else if ch == '\'' {
            out.push('\'');
            i += 1;
            while i < n && c[i] != '\'' && c[i] != '\n' {
                if c[i] == '\\' && i + 1 < n {
                    out.push(' ');
                    i += 1;
                }
                out.push(' ');
                i += 1;
            }
            if i < n && c[i] == '\'' {
                out.push('\'');
                i += 1;
            }
        } else if ch == '"' || ((ch == '@' || ch == '$') && starts_string(&c, i)) {
            // prefix: any of $ and @
            let mut verbatim = false;
            while i < n && (c[i] == '$' || c[i] == '@') {
                verbatim |= c[i] == '@';
                out.push(c[i]);
                i += 1;
            }
            let mut q = 0;
            while i + q < n && c[i + q] == '"' {
                q += 1;
            }
            if q >= 3 {
                for _ in 0..q {
                    out.push('"');
                }
                i += q;
                loop {
                    if i >= n {
                        break;
                    }
                    let mut k = 0;
                    while i + k < n && c[i + k] == '"' {
                        k += 1;
                    }
                    if k >= q {
                        for _ in 0..k {
                            out.push('"');
                        }
                        i += k;
                        break;
                    }
                    if k > 0 {
                        for _ in 0..k {
                            out.push(' ');
                        }
                        i += k;
                    } else {
                        out.push(blank(c[i]));
                        i += 1;
                    }
                }
            } else {
                // q is 1 or 2; 2 means the empty string "".
                out.push('"');
                i += 1;
                if q == 2 && !(verbatim && i + 1 < n && c[i + 1] == '"') {
                    out.push('"');
                    i += 1;
                    continue;
                }
                while i < n {
                    if verbatim && c[i] == '"' && i + 1 < n && c[i + 1] == '"' {
                        out.push_str("  ");
                        i += 2;
                    } else if c[i] == '"' {
                        out.push('"');
                        i += 1;
                        break;
                    } else if !verbatim && c[i] == '\\' && i + 1 < n {
                        out.push(' ');
                        out.push(blank(c[i + 1]));
                        i += 2;
                    } else if !verbatim && c[i] == '\n' {
                        break;
                    } else {
                        out.push(blank(c[i]));
                        i += 1;
                    }
                }
            }
        } else {
            out.push(ch);
            i += 1;
        }
    }
    out
}

fn starts_string(c: &[char], mut i: usize) -> bool {
    let start = i;
    while i < c.len() && (c[i] == '$' || c[i] == '@') {
        i += 1;
    }
    i > start && i < c.len() && c[i] == '"'
}

/// One test method found in a file.
#[derive(Clone, Debug, PartialEq)]
pub struct TestMethod {
    /// Fully qualified name: namespace, enclosing types and method, joined by dots.
    pub name: String,
    pub method: String,
    /// 1-based line of the method name.
    pub line: usize,
    /// 1-based line of the closing brace.
    pub end_line: usize,
    /// 1-based line where the declaration starts (first attribute).
    pub start_line: usize,
    /// Source text from the first attribute line to the closing brace.
    pub text: String,
    /// Lines of the body (from the signature to the closing brace).
    pub body_start: usize,
}

const TEST_ATTRS: [&str; 6] = ["Fact", "Theory", "SkippableFact", "SkippableTheory", "CombinatorialData", "StaFact"];

fn has_test_attr(header: &str) -> bool {
    // attribute lists inside [...]
    let mut rest = header;
    while let Some(p) = rest.find('[') {
        let after = &rest[p + 1..];
        let end = after.find(']').unwrap_or(after.len());
        for a in after[..end].split(',') {
            let a = a.trim();
            let id: String = a.chars().take_while(|ch| ch.is_alphanumeric() || *ch == '_' || *ch == '.').collect();
            let id = id.rsplit('.').next().unwrap_or("").trim_end_matches("Attribute");
            if TEST_ATTRS.contains(&id) {
                return true;
            }
        }
        rest = &after[end.min(after.len())..];
    }
    false
}

fn ident_before_paren(header: &str) -> Option<String> {
    // the method name is the identifier right before the first '(' outside of attributes
    let mut depth = 0i32;
    let b: Vec<char> = header.chars().collect();
    for (k, &ch) in b.iter().enumerate() {
        match ch {
            '[' => depth += 1,
            ']' => depth -= 1,
            '(' if depth == 0 => {
                let mut j = k;
                while j > 0 && b[j - 1].is_whitespace() {
                    j -= 1;
                }
                // skip generic arguments  Name<T>(
                if j > 0 && b[j - 1] == '>' {
                    let mut d = 0;
                    while j > 0 {
                        j -= 1;
                        if b[j] == '>' {
                            d += 1;
                        } else if b[j] == '<' {
                            d -= 1;
                            if d == 0 {
                                break;
                            }
                        }
                    }
                }
                let end = j;
                while j > 0 && (b[j - 1].is_alphanumeric() || b[j - 1] == '_') {
                    j -= 1;
                }
                if j < end {
                    return Some(b[j..end].iter().collect());
                }
                return None;
            }
            _ => {}
        }
    }
    None
}

fn word_after(header: &str, kw: &str) -> Option<String> {
    let toks: Vec<&str> = header.split(|ch: char| ch.is_whitespace() || ch == '(' || ch == ':' || ch == '<' || ch == '{').filter(|t| !t.is_empty()).collect();
    let k = toks.iter().position(|t| *t == kw)?;
    toks.get(k + 1).map(|s| s.to_string())
}

/// All test methods of a C# file, in source order.
pub fn test_methods(src: &str) -> Vec<TestMethod> {
    let m: Vec<char> = mask(src).chars().collect();
    let orig_lines: Vec<&str> = src.lines().collect();
    #[derive(Clone)]
    enum Scope {
        Ns(String),
        Type(String),
        Other,
    }
    let mut stack: Vec<Scope> = Vec::new();
    let mut file_ns: Option<String> = None;
    let mut header = String::new();
    let mut header_line = 0usize; // line of first non-space char of header
    let mut line = 1usize;
    let mut out = Vec::new();
    let mut i = 0;
    while i < m.len() {
        let ch = m[i];
        match ch {
            '{' => {
                let h = header.clone();
                let toks = h.split_whitespace().collect::<Vec<_>>();
                let in_type = matches!(stack.last(), Some(Scope::Type(_)));
                if toks.contains(&"namespace") {
                    stack.push(Scope::Ns(word_after(&h, "namespace").unwrap_or_default()));
                } else if let Some(kw) = ["class", "struct", "record", "interface"].iter().find(|k| toks.contains(k)) {
                    let mut name = word_after(&h, kw).unwrap_or_default();
                    if *kw == "record" && (name == "class" || name == "struct") {
                        name = word_after(&h, &name).unwrap_or_default();
                    }
                    stack.push(Scope::Type(name));
                } else if in_type && has_test_attr(&h) && h.contains('(') {
                    // a test method: find its end
                    let method = ident_before_paren(&h).unwrap_or_default();
                    let mut depth = 0;
                    let mut j = i;
                    let mut l = line;
                    while j < m.len() {
                        match m[j] {
                            '{' => depth += 1,
                            '}' => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            '\n' => l += 1,
                            _ => {}
                        }
                        j += 1;
                    }
                    let start_line = header_line;
                    // the line holding the method name
                    let mut name_line = start_line;
                    for (k, ol) in orig_lines.iter().enumerate().take(line).skip(start_line.saturating_sub(1)) {
                        if !ol.trim_start().starts_with('[') && ol.contains(&format!("{method}(")) {
                            name_line = k + 1;
                            break;
                        }
                    }
                    let mut parts: Vec<String> = Vec::new();
                    if let Some(ns) = file_ns.clone() {
                        parts.push(ns);
                    }
                    for s in &stack {
                        match s {
                            Scope::Ns(n) | Scope::Type(n) if !n.is_empty() => parts.push(n.clone()),
                            _ => {}
                        }
                    }
                    parts.push(method.clone());
                    let text = orig_lines[start_line.saturating_sub(1)..l.min(orig_lines.len())].join("\n");
                    out.push(TestMethod { name: parts.join("."), method, line: name_line, end_line: l, start_line, text, body_start: name_line });
                    line = l;
                    i = j + 1;
                    header.clear();
                    header_line = 0;
                    continue;
                } else {
                    stack.push(Scope::Other);
                }
                header.clear();
                header_line = 0;
            }
            '}' => {
                stack.pop();
                header.clear();
                header_line = 0;
            }
            ';' => {
                let toks: Vec<&str> = header.split_whitespace().collect();
                if toks.first() == Some(&"namespace") && file_ns.is_none() && stack.is_empty() {
                    file_ns = toks.get(1).map(|s| s.to_string());
                }
                header.clear();
                header_line = 0;
            }
            _ => {
                if !ch.is_whitespace() && header_line == 0 {
                    header_line = line;
                }
                header.push(ch);
            }
        }
        if ch == '\n' {
            line += 1;
        }
        i += 1;
    }
    out
}

/// The full statement that starts on or contains 1-based `line` (up to its `;`), trimmed.
pub fn statement_at(src: &str, line: usize) -> Option<String> {
    let lines: Vec<&str> = src.lines().collect();
    let masked = mask(src);
    let mlines: Vec<&str> = masked.lines().collect();
    if line == 0 || line > lines.len() {
        return None;
    }
    let mut end = line;
    while end <= lines.len() {
        if mlines.get(end - 1).map(|l| l.trim_end().ends_with(';')).unwrap_or(false) {
            break;
        }
        end += 1;
    }
    let end = end.min(lines.len());
    let text: Vec<&str> = lines[line - 1..end].iter().map(|l| l.trim()).collect();
    Some(text.join("\n"))
}

/// Lines (1-based) of statements in `[from, to]` that start with an assertion call.
pub fn assertion_lines(src: &str, from: usize, to: usize) -> Vec<usize> {
    let masked = mask(src);
    masked
        .lines()
        .enumerate()
        .filter(|(k, l)| {
            let n = k + 1;
            let t = l.trim_start();
            n >= from && n <= to && (t.starts_with("Assert.") || t.contains(" Assert.") || t.contains(".Should()"))
        })
        .map(|(k, _)| k + 1)
        .collect()
}

/// Names of methods called in a text (identifier followed by `(`).
pub fn called_names(text: &str) -> Vec<String> {
    let m: Vec<char> = mask(text).chars().collect();
    let mut out = Vec::new();
    let mut k = 0;
    while k < m.len() {
        if m[k].is_alphabetic() || m[k] == '_' {
            let s = k;
            while k < m.len() && (m[k].is_alphanumeric() || m[k] == '_') {
                k += 1;
            }
            if k < m.len() && m[k] == '(' {
                let w: String = m[s..k].iter().collect();
                if !out.contains(&w) {
                    out.push(w);
                }
            }
        } else {
            k += 1;
        }
    }
    out
}

/// Line range of a (non-test) method declaration `name(` in the file, by brace matching.
pub fn method_range(src: &str, name: &str) -> Option<(usize, usize)> {
    let masked = mask(src);
    let ml: Vec<&str> = masked.lines().collect();
    for (k, l) in ml.iter().enumerate() {
        let t = l.trim_start();
        let decl = (t.starts_with("private") || t.starts_with("public") || t.starts_with("internal") || t.starts_with("protected") || t.starts_with("static"))
            && l.contains(&format!(" {name}("));
        if decl {
            let mut depth = 0i32;
            let mut seen = false;
            for (j, l2) in ml.iter().enumerate().skip(k) {
                for ch in l2.chars() {
                    if ch == '{' {
                        depth += 1;
                        seen = true;
                    } else if ch == '}' {
                        depth -= 1;
                    }
                }
                if seen && depth == 0 {
                    return Some((k + 1, j + 1));
                }
                if !seen && l2.trim_end().ends_with(';') {
                    return Some((k + 1, j + 1));
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r#"
namespace A.B;

public class T
{
    // { not a brace
    string s = "}";
    string r = """
    { raw }
    """;

    [Fact]
    public async Task One()
    {
        var x = new { A = 1 };
        Assert.Equal(1, x.A);
    }

    [Theory, InlineData(1)]
    public void Two(int a) { Assert.True(a > 0); }

    public void Helper() { }
}
"#;

    #[test]
    fn finds_tests_with_names_and_lines() {
        let t = test_methods(SRC);
        assert_eq!(t.len(), 2, "{t:?}");
        assert_eq!(t[0].name, "A.B.T.One");
        assert_eq!(t[0].line, 13);
        assert_eq!(t[0].end_line, 17);
        assert_eq!(t[1].name, "A.B.T.Two");
        assert_eq!(statement_at(SRC, 16).unwrap(), "Assert.Equal(1, x.A);");
        assert_eq!(assertion_lines(SRC, 13, 17), vec![16]);
    }

    #[test]
    fn mask_keeps_lines() {
        assert_eq!(mask(SRC).lines().count(), SRC.lines().count());
        assert!(!mask("var a = @\"x\"\"}\";").contains('}'));
        assert!(!mask("var a = $\"{b}\";").contains('{'));
    }
}
