//! Hand-lowered test inputs. These are test data for the differ, never pipeline output.
#![allow(dead_code)]

use serde_json::{json, Value};

pub const TC: &str = "src/MessagePack.SourceGenerator/CodeAnalysis/TypeCollector.cs";

pub fn loc(line: u64) -> Value {
    json!({"file": TC, "line": line, "endLine": line})
}
pub fn var(n: &str) -> Value {
    json!({"op": "var", "name": n})
}
pub fn lit(v: Value) -> Value {
    json!({"op": "lit", "value": v})
}
pub fn call(n: &str, args: Vec<Value>) -> Value {
    json!({"op": "call", "name": n, "args": args})
}
pub fn op(o: &str, args: Vec<Value>) -> Value {
    json!({"op": o, "args": args})
}
pub fn ite(c: Value, a: Value, b: Value) -> Value {
    op("if", vec![c, a, b])
}
pub fn at(mut e: Value, line: u64) -> Value {
    e["location"] = loc(line);
    e
}
pub fn let_(n: &str, v: Value, body: Value) -> Value {
    json!({"op": "let", "name": n, "args": [v, body]})
}
pub fn nothing() -> Value {
    json!({"op": "nothing"})
}
pub fn report(n: &str, args: Vec<Value>) -> Value {
    json!({"op": "report", "name": n, "args": args})
}
pub fn opaque(name: &str, symbol: &str, args: &[&str], result: &str) -> Value {
    json!({"name": name, "symbol": symbol, "args": args, "result": result})
}
pub fn model(unit: &str, version: &str, params: &[(&str, &str)], opaque: Vec<Value>, contracts: Vec<Value>, body: Value) -> Value {
    fn lines(e: &Value, out: &mut Vec<u64>) {
        if let Some(l) = e["location"]["line"].as_u64() {
            out.push(l);
        }
        for a in e["args"].as_array().into_iter().flatten() {
            lines(a, out);
        }
    }
    let mut modelled = vec![];
    lines(&body, &mut modelled);
    modelled.sort();
    modelled.dedup();
    json!({"unit": unit, "version": version,
        "params": params.iter().map(|(n, s)| json!({"name": n, "sort": s})).collect::<Vec<_>>(),
        "opaque": opaque, "contracts": contracts, "body": body,
        "lines": {"modelled": modelled, "notModelled": []}})
}

const DSR: &str = "P:Microsoft.CodeAnalysis.ISymbol.DeclaringSyntaxReferences";
const FOD: &str = "M:System.Linq.ImmutableArrayExtensions.FirstOrDefault``1(System.Collections.Immutable.ImmutableArray{``0})";
const IDX: &str = "System.Collections.Immutable.ImmutableArray<Microsoft.CodeAnalysis.SyntaxReference>.this[int]";
const CAST: &str = "(Microsoft.CodeAnalysis.CSharp.Syntax.PropertyDeclarationSyntax)Microsoft.CodeAnalysis.SyntaxNode";
pub const MEMBER_NEEDS_KEY: &str = "MsgPack00xMessagePackAnalyzer.MemberNeedsKey";
pub const BOTH_NULL: &str = "MsgPack00xMessagePackAnalyzer.BothStringAndIntKeyAreNull";

const P2283: &[(&str, &str)] = &[
    ("isProperty", "Bool"), ("reportIsNull", "Bool"), ("nullKey", "Bool"), ("derivedNull", "Bool"), ("item", "Sym"), ("comp", "Comp"),
];

fn dsr_item() -> Value {
    call("dsr", vec![var("item")])
}
fn fod() -> Value {
    call("firstOrDefault", vec![dsr_item()])
}

/// 2283, both lookup sites of GetObjectInfo, lowered by hand from TC.base.cs:985 and :1077
/// (research/spikes/src/solver). The `isProperty` input selects the property loop or the field loop.
pub fn tc2283_old() -> Value {
    let idx0 = json!({"op": "index", "contract": "index-empty", "args": [dsr_item(), lit(json!(0))]});
    let prop = ite(var("reportIsNull"), nothing(), ite(var("nullKey"),
        at(report(BOTH_NULL, vec![call("identLoc", vec![
            json!({"op": "cast", "name": "PropertyDeclarationSyntax", "contract": "failed-cast",
                   "args": [call("getSyntax", vec![idx0.clone()])]})])]), 985),
        nothing()));
    let field = ite(var("derivedNull"),
        at(let_("loc", call("nodeLoc", vec![call("getSyntax", vec![idx0])]),
            ite(var("reportIsNull"), nothing(), report(MEMBER_NEEDS_KEY, vec![var("loc")]))), 1077),
        nothing());
    model("TypeCollector.GetObjectInfo", "old", &P2283[..5],
        vec![opaque("dsr", DSR, &["Sym"], "RefArray"),
             opaque("getSyntax", "M:Microsoft.CodeAnalysis.SyntaxReference.GetSyntax(System.Threading.CancellationToken)", &["Ref"], "Syn"),
             opaque("identLoc", "M:Microsoft.CodeAnalysis.SyntaxToken.GetLocation", &["Syn"], "Loc"),
             opaque("nodeLoc", "M:Microsoft.CodeAnalysis.SyntaxNode.GetLocation", &["Syn"], "Loc")],
        vec![json!({"contract": "index-empty", "symbol": IDX}), json!({"contract": "failed-cast", "symbol": CAST})],
        ite(var("isProperty"), prop, field))
}

/// The inlined `ReportIfLocatedInCompilation` wrapper installed at TC.2283.cs:239.
pub fn wrapper(loc_var: &str, joiner: &str) -> Value {
    at(op(joiner, vec![call("isInSource", vec![var(loc_var)]),
        call("containsTree", vec![var("comp"), call("sourceTree", vec![var(loc_var)])])]), 239)
}

pub fn tc2283_new_with(joiner: &str) -> Value {
    let prop_loc = ite(op("isnull", vec![fod()]), op("none", vec![]),
        ite(call("isPropertyDeclaration", vec![call("getSyntax", vec![fod()])]),
            op("some", vec![call("identLoc", vec![call("getSyntax", vec![fod()])])]), op("none", vec![])));
    let prop = ite(var("reportIsNull"), nothing(), ite(var("nullKey"),
        let_("loc", prop_loc, ite(wrapper("loc", joiner), at(report(BOTH_NULL, vec![var("loc")]), 985), nothing())),
        nothing()));
    let field_loc = ite(op("isnull", vec![fod()]), op("none", vec![]),
        op("some", vec![call("nodeLoc", vec![call("getSyntax", vec![fod()])])]));
    let field = ite(var("derivedNull"),
        at(let_("loc", field_loc, ite(var("reportIsNull"), nothing(),
            ite(wrapper("loc", joiner), report(MEMBER_NEEDS_KEY, vec![var("loc")]), nothing()))), 1077),
        nothing());
    model("TypeCollector.GetObjectInfo", "new", P2283,
        vec![opaque("dsr", DSR, &["Sym"], "RefArray"),
             opaque("firstOrDefault", FOD, &["RefArray"], "OptRef"),
             opaque("getSyntax", "M:Microsoft.CodeAnalysis.SyntaxReference.GetSyntax(System.Threading.CancellationToken)", &["Ref"], "Syn"),
             opaque("isPropertyDeclaration", "T:Microsoft.CodeAnalysis.CSharp.Syntax.PropertyDeclarationSyntax", &["Syn"], "Bool"),
             opaque("identLoc", "M:Microsoft.CodeAnalysis.SyntaxToken.GetLocation", &["Syn"], "Loc"),
             opaque("nodeLoc", "M:Microsoft.CodeAnalysis.SyntaxNode.GetLocation", &["Syn"], "Loc"),
             opaque("isInSource", "P:Microsoft.CodeAnalysis.Location.IsInSource", &["OptLoc"], "Bool"),
             opaque("sourceTree", "P:Microsoft.CodeAnalysis.Location.SourceTree", &["OptLoc"], "Tree"),
             opaque("containsTree", "M:Microsoft.CodeAnalysis.Compilation.ContainsSyntaxTree(Microsoft.CodeAnalysis.SyntaxTree)", &["Comp", "Tree"], "Bool")],
        vec![], ite(var("isProperty"), prop, field))
}

pub fn tc2283_new() -> Value {
    tc2283_new_with("and")
}

// ---------------------------------------------------------------- 2234, property loop (spike L34)

const CK: &str = "M:System.Collections.Generic.Dictionary`2.ContainsKey(`0)";
const ADD: &str = "M:System.Collections.Generic.Dictionary`2.Add(`0,`1)";

fn rep(d: &str) -> Value {
    ite(var("reportIsNull"), nothing(), report(d, vec![]))
}

fn guarded_add(head: bool, contains: Value, add: Value) -> Value {
    if head { ite(contains, nothing(), add) } else { add }
}

fn rest(head: bool) -> Value {
    let int_contains = || call("intContains", vec![var("intMembers"), var("v")]);
    let str_contains = || call("strContains", vec![var("strMembers"), var("strKey")]);
    let int_add = json!({"op": "call", "name": "intAdd", "contract": "dictionary-duplicate-add", "args": [var("intMembers"), var("v")]});
    let str_add = json!({"op": "call", "name": "strAdd", "contract": "dictionary-duplicate-add", "args": [var("strMembers"), var("strKey")]});
    op("seq", vec![
        ite(var("isIntKey"),
            let_("v", json!({"op": "value", "sort": "Int", "contract": "nullable-value-null", "args": [var("intKey")]}),
                op("seq", vec![ite(int_contains(), rep("KeysMustBeUnique"), nothing()), guarded_add(head, int_contains(), int_add)])),
            ite(op("not", vec![op("isnull", vec![var("strKey")])]),
                op("seq", vec![ite(str_contains(), rep("KeysMustBeUnique"), nothing()), call("hiddenInc", vec![]),
                    guarded_add(head, str_contains(), str_add)]),
                nothing())),
        ite(var("fmtNull"), call("collectCore", vec![var("item")]), nothing()),
    ])
}

/// 2234's property loop body from `if (searchFirst)` on, plus the AOT init check that reads
/// `suppressSourceGeneration` identically in both versions. `head` adds the `continue` and the
/// guarded `Add`s.
pub fn tc2234(head: bool) -> Value {
    let mix = op("or", vec![op("and", vec![var("isIntKey"), op("isnull", vec![var("intKey")])]),
        op("and", vec![op("not", vec![var("isIntKey")]), op("isnull", vec![var("strKey")])])]);
    let not_first = if head {
        ite(mix, op("seq", vec![rep("DoNotMixStringAndIntKeys"), nothing()]), rest(true))
    } else {
        op("seq", vec![ite(mix, rep("DoNotMixStringAndIntKeys"), nothing()), rest(false)])
    };
    let body = op("seq", vec![
        ite(op("and", vec![op("not", vec![var("suppress")]), var("isInitOnly")]), rep("AOTInitProperty"), nothing()),
        at(ite(var("searchFirst"), let_("isIntKey", op("not", vec![op("isnull", vec![var("intKey")])]), rest(head)), not_first), 998),
    ]);
    model("TypeCollector.GetObjectInfo", if head { "new" } else { "old" },
        &[("members", "Members"), ("searchFirst", "Bool"), ("isIntKey", "Bool"), ("intKey", "OptInt"), ("strKey", "OptString"),
          ("intMembers", "Dict"), ("strMembers", "Dict"), ("fmtNull", "Bool"), ("reportIsNull", "Bool"), ("suppress", "Bool"), ("isInitOnly", "Bool")],
        vec![opaque("intContains", CK, &["Dict", "Int"], "Bool"), opaque("strContains", CK, &["Dict", "OptString"], "Bool"),
             opaque("intAdd", ADD, &["Dict", "Int"], "Void"), opaque("strAdd", ADD, &["Dict", "OptString"], "Void"),
             opaque("hiddenInc", "hiddenIntKey++", &[], "Void"),
             opaque("collectCore", "M:MessagePack.SourceGenerator.CodeAnalysis.TypeCollector.CollectCore(Microsoft.CodeAnalysis.ITypeSymbol)", &["Sym"], "Void")],
        vec![json!({"contract": "nullable-value-null", "symbol": "P:System.Nullable`1.Value"}),
             json!({"contract": "dictionary-duplicate-add", "symbol": ADD})],
        json!({"op": "foreach", "name": "item", "args": [var("members"), body]}))
}

/// Swap the arms of the k-th `if` (pre-order). Returns false when there is no k-th `if`.
pub fn flip_if(v: &mut Value, k: usize) -> bool {
    fn go(v: &mut Value, k: usize, seen: &mut usize) -> bool {
        if v["op"] == "if" {
            if *seen == k {
                let a = v["args"].as_array_mut().unwrap();
                if a.len() == 3 {
                    a.swap(1, 2);
                }
                return true;
            }
            *seen += 1;
        }
        if let Some(a) = v.get_mut("args").and_then(Value::as_array_mut) {
            for x in a {
                if go(x, k, seen) {
                    return true;
                }
            }
        }
        false
    }
    go(&mut v["body"], k, &mut 0)
}

/// Replace every node matching `pred` with `f(node)`.
pub fn rewrite(v: &mut Value, pred: &dyn Fn(&Value) -> bool, f: &dyn Fn(&mut Value)) {
    if let Some(b) = v.get_mut("body") {
        return rewrite(b, pred, f);
    }
    if pred(v) {
        f(v);
    }
    if let Some(a) = v.get_mut("args").and_then(Value::as_array_mut) {
        for x in a {
            rewrite(x, pred, f);
        }
    }
}
