//! Reduced Roslyn CFG worker replies for 2283, shaped as the C# worker serialises them
//! (worker-reply.schema.json) and following the dumps in research/spikes/roslyn-cfg-check.md §3.
//! Only the changed sites of `GetObjectInfo` (:985, :1077) are kept; the rest of the 720-line method is cut.

use serde_json::{json, Value};

pub const F: &str = "src/MessagePack.SourceGenerator/CodeAnalysis/TypeCollector.cs";
pub const TC: &str = "MessagePack.SourceGenerator.CodeAnalysis.TypeCollector";
pub const ACTION: &str = "System.Action<Microsoft.CodeAnalysis.Diagnostic>";

pub fn loc(a: u64, b: u64) -> Value {
    json!({"file": F, "line": a, "endLine": b})
}

pub fn op(kind: &str, ty: Option<&str>, sym: Option<&str>, syntax: &str, line: u64, ch: Vec<Value>) -> Value {
    json!({"kind": kind, "type": ty, "symbol": sym, "syntax": syntax, "location": loc(line, line), "children": ch})
}

pub fn block(ord: u64, ops: Vec<Value>, branch: Option<Value>, jt: bool, ft: Option<u64>, cond: Option<u64>) -> Value {
    json!({"ordinal": ord, "kind": "block", "operations": ops, "branch": branch, "jumpIfTrue": jt, "fallThrough": ft, "conditional": cond})
}

pub fn entry() -> Value {
    json!({"ordinal": 0, "kind": "entry", "operations": [], "branch": null, "fallThrough": 1, "conditional": null})
}

pub fn exit(ord: u64) -> Value {
    json!({"ordinal": ord, "kind": "exit", "operations": [], "branch": null, "fallThrough": null, "conditional": null})
}

pub fn graph(name: &str, a: u64, b: u64, params: &[(&str, &str)], blocks: Vec<Value>, lambdas: Vec<Value>) -> Value {
    let ps: Vec<Value> = params.iter().map(|(n, t)| json!({"name": n, "type": t})).collect();
    json!({"name": name, "location": loc(a, b), "parameters": ps, "blocks": blocks, "lambdas": lambdas})
}

pub fn reply(methods: Vec<Value>) -> Value {
    json!({"id": 1, "method": "cfg", "result": {"errors": [], "methods": methods}})
}

fn p(name: &str, ty: &str, line: u64) -> Value {
    op("ParameterReference", Some(ty), None, name, line, vec![])
}

fn field_report(line: u64) -> Value {
    op("FlowCapture", Some(ACTION), None, "this.reportDiagnostic", line,
        vec![op("FieldReference", Some(ACTION), Some(&format!("F:{TC}.reportDiagnostic")), "this.reportDiagnostic", line, vec![])])
}

fn is_null_report(line: u64) -> Value {
    op("IsNull", Some("System.Boolean"), None, "this.reportDiagnostic", line, vec![])
}

fn invoke(line: u64, diag: Value) -> Value {
    op("ExpressionStatement", Some("System.Void"), None, "this.reportDiagnostic?.Invoke(...)", line, vec![
        op("Invocation", Some("System.Void"), Some("M:System.Action`1.Invoke(`0)"), ".Invoke(...)", line, vec![
            op("FlowCaptureReference", Some(ACTION), None, "this.reportDiagnostic", line, vec![]), diag])])
}

fn create(line: u64, desc: &str, location: Value) -> Value {
    op("Invocation", Some("Microsoft.CodeAnalysis.Diagnostic"),
        Some("M:Microsoft.CodeAnalysis.Diagnostic.Create(Microsoft.CodeAnalysis.DiagnosticDescriptor,Microsoft.CodeAnalysis.Location,System.Object[])"),
        "Diagnostic.Create(...)", line, vec![
            op("FieldReference", Some("Microsoft.CodeAnalysis.DiagnosticDescriptor"), Some(&format!("F:MessagePack.SourceGenerator.MsgPack00xMessagePackAnalyzer.{desc}")),
                &format!("MsgPack00xMessagePackAnalyzer.{desc}"), line, vec![]),
            location])
}

fn dsr(line: u64) -> Value {
    op("PropertyReference", Some("System.Collections.Immutable.ImmutableArray<Microsoft.CodeAnalysis.SyntaxReference>"),
        Some("P:Microsoft.CodeAnalysis.ISymbol.DeclaringSyntaxReferences"), "item.DeclaringSyntaxReferences", line,
        vec![p("item", "Microsoft.CodeAnalysis.IPropertySymbol", line)])
}

fn index0(line: u64) -> Value {
    op("PropertyReference", Some("Microsoft.CodeAnalysis.SyntaxReference"),
        Some("P:System.Collections.Immutable.ImmutableArray`1.Item(System.Int32)"), "item.DeclaringSyntaxReferences[0]", line,
        vec![dsr(line), op("Literal", Some("System.Int32"), None, "0", line, vec![])])
}

fn get_syntax(line: u64, recv: Value) -> Value {
    op("Invocation", Some("Microsoft.CodeAnalysis.SyntaxNode"),
        Some("M:Microsoft.CodeAnalysis.SyntaxReference.GetSyntax(System.Threading.CancellationToken)"), "GetSyntax()", line, vec![recv])
}

fn get_location(line: u64, recv: Value, sym: &str) -> Value {
    op("Invocation", Some("Microsoft.CodeAnalysis.Location"), Some(sym), "GetLocation()", line, vec![recv])
}

const NODE_LOC: &str = "M:Microsoft.CodeAnalysis.SyntaxNode.GetLocation";
const TOKEN_LOC: &str = "M:Microsoft.CodeAnalysis.SyntaxToken.GetLocation";

fn identifier(line: u64, recv: Value) -> Value {
    op("PropertyReference", Some("Microsoft.CodeAnalysis.SyntaxToken"),
        Some("P:Microsoft.CodeAnalysis.CSharp.Syntax.PropertyDeclarationSyntax.Identifier"), "Identifier", line, vec![recv])
}

fn params() -> Vec<(&'static str, &'static str)> {
    vec![("item", "Microsoft.CodeAnalysis.IPropertySymbol"), ("nullKey", "System.Boolean"), ("needsKey", "System.Boolean")]
}

/// Old: `:985` reports with `((PropertyDeclarationSyntax)item.DeclaringSyntaxReferences[0].GetSyntax()).Identifier.GetLocation()`,
/// `:1077` with `item.DeclaringSyntaxReferences[0].GetSyntax().GetLocation()`.
pub fn old_reply() -> Value {
    let loc985 = get_location(985, identifier(985, op("Conversion", Some("Microsoft.CodeAnalysis.CSharp.Syntax.PropertyDeclarationSyntax"), None,
        "((PropertyDeclarationSyntax)item.DeclaringSyntaxReferences[0].GetSyntax())", 985,
        vec![get_syntax(985, index0(985))])), TOKEN_LOC);
    let g = graph(&format!("TypeCollector.GetObjectInfo"), 700, 1420, &params(), vec![
        entry(),
        block(1, vec![], Some(p("nullKey", "System.Boolean", 983)), false, Some(2), Some(4)),
        block(2, vec![field_report(985)], Some(is_null_report(985)), true, Some(3), Some(4)),
        block(3, vec![invoke(985, create(985, "BothStringAndIntKeyAreNull", loc985))], None, false, Some(4), None),
        block(4, vec![], Some(p("needsKey", "System.Boolean", 1076)), false, Some(5), Some(7)),
        block(5, vec![field_report(1080)], Some(is_null_report(1080)), true, Some(6), Some(7)),
        block(6, vec![invoke(1080, create(1077, "MemberNeedsKey", get_location(1077, get_syntax(1077, index0(1077)), NODE_LOC)))], None, false, Some(7), None),
        exit(7),
    ], vec![]);
    let ctor = graph("TypeCollector..ctor", 236, 260, &[("reportAnalyzerDiagnostic", "System.Action<Microsoft.CodeAnalysis.Diagnostic>?")], vec![
        entry(),
        block(1, vec![op("SimpleAssignment", Some(ACTION), None, "this.reportDiagnostic = reportAnalyzerDiagnostic", 239, vec![
            op("FieldReference", Some(ACTION), Some(&format!("F:{TC}.reportDiagnostic")), "this.reportDiagnostic", 239, vec![]),
            p("reportAnalyzerDiagnostic", ACTION, 239)])], None, false, Some(2), None),
        exit(2)], vec![]);
    reply(vec![g, ctor])
}

/// New: `:985` calls the added helper; `:1077` uses `FirstOrDefault()?.GetSyntax().GetLocation()`;
/// the ctor at `:239` installs `diagnostic => ReportIfLocatedInCompilation(reportAnalyzerDiagnostic, diagnostic)`.
pub fn new_reply() -> Value {
    let helper_sym = format!("M:{TC}.GetPropertyIdentifierLocation(Microsoft.CodeAnalysis.IPropertySymbol)");
    let call985 = op("Invocation", Some("Microsoft.CodeAnalysis.Location?"), Some(&helper_sym), "GetPropertyIdentifierLocation(item)", 985,
        vec![p("item", "Microsoft.CodeAnalysis.IPropertySymbol", 985)]);
    let fod = |line: u64, recv: Value| op("Invocation", Some("Microsoft.CodeAnalysis.SyntaxReference?"),
        Some("M:System.Linq.ImmutableArrayExtensions.FirstOrDefault``1(System.Collections.Immutable.ImmutableArray{``0})"),
        "item.DeclaringSyntaxReferences.FirstOrDefault()", line, vec![recv]);
    let fod_syn = "item.DeclaringSyntaxReferences.FirstOrDefault()";
    let g = graph("TypeCollector.GetObjectInfo", 700, 1420, &params(), vec![
        entry(),
        block(1, vec![], Some(p("nullKey", "System.Boolean", 983)), false, Some(2), Some(4)),
        block(2, vec![field_report(985)], Some(is_null_report(985)), true, Some(3), Some(4)),
        block(3, vec![invoke(985, create(985, "BothStringAndIntKeyAreNull", call985))], None, false, Some(4), None),
        block(4, vec![], Some(p("needsKey", "System.Boolean", 1076)), false, Some(5), Some(11)),
        block(5, vec![op("FlowCapture", Some("Microsoft.CodeAnalysis.SyntaxReference?"), None, fod_syn, 1077, vec![fod(1077, dsr(1077))])],
            Some(op("IsNull", Some("System.Boolean"), None, fod_syn, 1077, vec![])), true, Some(6), Some(7)),
        block(6, vec![op("FlowCapture", Some("Microsoft.CodeAnalysis.Location?"), None, "loc1077", 1077, vec![
            get_location(1077, get_syntax(1077, op("FlowCaptureReference", Some("Microsoft.CodeAnalysis.SyntaxReference"), None, fod_syn, 1077, vec![])), NODE_LOC)])],
            None, false, Some(8), None),
        block(7, vec![op("FlowCapture", Some("Microsoft.CodeAnalysis.Location?"), None, "loc1077", 1077, vec![
            op("DefaultValue", Some("Microsoft.CodeAnalysis.Location?"), None, "null", 1077, vec![])])], None, false, Some(8), None),
        block(8, vec![field_report(1080)], Some(is_null_report(1080)), true, Some(9), Some(11)),
        block(9, vec![invoke(1080, create(1077, "MemberNeedsKey", op("FlowCaptureReference", Some("Microsoft.CodeAnalysis.Location?"), None, "loc1077", 1077, vec![])))],
            None, false, Some(11), None),
        exit(11),
    ], vec![]);
    // GetPropertyIdentifierLocation(property) => (property.DeclaringSyntaxReferences.FirstOrDefault()?.GetSyntax() as PropertyDeclarationSyntax)?.Identifier.GetLocation();
    let pr = |l| op("ParameterReference", Some("Microsoft.CodeAnalysis.IPropertySymbol"), None, "property", l, vec![]);
    let pdsr = op("PropertyReference", Some("System.Collections.Immutable.ImmutableArray<Microsoft.CodeAnalysis.SyntaxReference>"),
        Some("P:Microsoft.CodeAnalysis.ISymbol.DeclaringSyntaxReferences"), "property.DeclaringSyntaxReferences", 1434, vec![pr(1434)]);
    let psyn = "property.DeclaringSyntaxReferences.FirstOrDefault()";
    let pds = "(... as PropertyDeclarationSyntax)";
    let helper = graph("TypeCollector.GetPropertyIdentifierLocation", 1434, 1434, &[("property", "Microsoft.CodeAnalysis.IPropertySymbol")], vec![
        entry(),
        block(1, vec![op("FlowCapture", Some("Microsoft.CodeAnalysis.SyntaxReference?"), None, psyn, 1434, vec![fod(1434, pdsr)])],
            Some(op("IsNull", Some("System.Boolean"), None, psyn, 1434, vec![])), true, Some(2), Some(5)),
        block(2, vec![op("FlowCapture", Some("Microsoft.CodeAnalysis.CSharp.Syntax.PropertyDeclarationSyntax?"), None, pds, 1434, vec![
            op("Conversion", Some("Microsoft.CodeAnalysis.CSharp.Syntax.PropertyDeclarationSyntax?"), None, "?.GetSyntax() as PropertyDeclarationSyntax", 1434, vec![
                get_syntax(1434, op("FlowCaptureReference", Some("Microsoft.CodeAnalysis.SyntaxReference"), None, psyn, 1434, vec![]))])])],
            Some(op("IsNull", Some("System.Boolean"), None, pds, 1434, vec![])), true, Some(3), Some(5)),
        block(3, vec![op("FlowCapture", Some("Microsoft.CodeAnalysis.Location?"), None, "result", 1434, vec![
            get_location(1434, identifier(1434, op("FlowCaptureReference", Some("Microsoft.CodeAnalysis.CSharp.Syntax.PropertyDeclarationSyntax"), None, pds, 1434, vec![])), TOKEN_LOC)])],
            None, false, Some(6), None),
        block(5, vec![op("FlowCapture", Some("Microsoft.CodeAnalysis.Location?"), None, "result", 1434, vec![
            op("DefaultValue", Some("Microsoft.CodeAnalysis.Location?"), None, "null", 1434, vec![])])], None, false, Some(6), None),
        exit(6),
    ], vec![]);
    // ReportIfLocatedInCompilation(report, diagnostic) { if (diagnostic.Location.IsInSource && this.compilation.ContainsSyntaxTree(diagnostic.Location.SourceTree)) report(diagnostic); }
    let dref = |l| op("ParameterReference", Some("Microsoft.CodeAnalysis.Diagnostic"), None, "diagnostic", l, vec![]);
    let dloc = |l| op("PropertyReference", Some("Microsoft.CodeAnalysis.Location"), Some("P:Microsoft.CodeAnalysis.Diagnostic.Location"), "diagnostic.Location", l, vec![dref(l)]);
    let filter = graph("TypeCollector.ReportIfLocatedInCompilation", 1443, 1449, &[("report", ACTION), ("diagnostic", "Microsoft.CodeAnalysis.Diagnostic")], vec![
        entry(),
        block(1, vec![], Some(op("PropertyReference", Some("System.Boolean"), Some("P:Microsoft.CodeAnalysis.Location.IsInSource"), "diagnostic.Location.IsInSource", 1445, vec![dloc(1445)])), false, Some(2), Some(4)),
        block(2, vec![], Some(op("Invocation", Some("System.Boolean"), Some("M:Microsoft.CodeAnalysis.Compilation.ContainsSyntaxTree(Microsoft.CodeAnalysis.SyntaxTree)"),
            "this.compilation.ContainsSyntaxTree(diagnostic.Location.SourceTree)", 1445, vec![
                op("FieldReference", Some("Microsoft.CodeAnalysis.Compilation"), Some(&format!("F:{TC}.compilation")), "this.compilation", 1445, vec![]),
                op("PropertyReference", Some("Microsoft.CodeAnalysis.SyntaxTree?"), Some("P:Microsoft.CodeAnalysis.Location.SourceTree"), "diagnostic.Location.SourceTree", 1445, vec![dloc(1445)])])),
            false, Some(3), Some(4)),
        block(3, vec![op("ExpressionStatement", Some("System.Void"), None, "report(diagnostic);", 1447, vec![
            op("Invocation", Some("System.Void"), Some("M:System.Action`1.Invoke(`0)"), "report(diagnostic)", 1447, vec![
                op("ParameterReference", Some(ACTION), None, "report", 1447, vec![]), dref(1447)])])], None, false, Some(4), None),
        exit(4),
    ], vec![]);
    let lambda = graph("lambda@239", 239, 239, &[("diagnostic", "Microsoft.CodeAnalysis.Diagnostic")], vec![
        entry(),
        block(1, vec![op("ExpressionStatement", Some("System.Void"), None, "ReportIfLocatedInCompilation(reportAnalyzerDiagnostic, diagnostic)", 239, vec![
            op("Invocation", Some("System.Void"), Some(&format!("M:{TC}.ReportIfLocatedInCompilation(System.Action{{Microsoft.CodeAnalysis.Diagnostic}},Microsoft.CodeAnalysis.Diagnostic)")),
                "ReportIfLocatedInCompilation(reportAnalyzerDiagnostic, diagnostic)", 239, vec![
                    op("InstanceReference", Some(TC), None, "this", 239, vec![]),
                    op("ParameterReference", Some(ACTION), None, "reportAnalyzerDiagnostic", 239, vec![]),
                    dref(239)])])], None, false, Some(2), None),
        exit(2),
    ], vec![]);
    let ctor = graph("TypeCollector..ctor", 236, 260, &[("reportAnalyzerDiagnostic", "System.Action<Microsoft.CodeAnalysis.Diagnostic>?")], vec![
        entry(),
        block(1, vec![], Some(op("IsPattern", Some("System.Boolean"), None, "reportAnalyzerDiagnostic is null", 239, vec![
            p("reportAnalyzerDiagnostic", ACTION, 239), op("ConstantPattern", None, None, "null", 239, vec![])])), false, Some(3), Some(2)),
        block(2, vec![op("FlowCapture", Some(ACTION), None, "c0", 239, vec![op("DefaultValue", Some(ACTION), None, "null", 239, vec![])])], None, false, Some(4), None),
        block(3, vec![op("FlowCapture", Some(ACTION), None, "c0", 239, vec![
            op("DelegateCreation", Some(ACTION), None, "diagnostic => ReportIfLocatedInCompilation(reportAnalyzerDiagnostic, diagnostic)", 239, vec![
                op("FlowAnonymousFunction", None, None, "diagnostic => ...", 239, vec![])])])], None, false, Some(4), None),
        block(4, vec![op("SimpleAssignment", Some(ACTION), None, "this.reportDiagnostic = ...", 239, vec![
            op("FieldReference", Some(ACTION), Some(&format!("F:{TC}.reportDiagnostic")), "this.reportDiagnostic", 239, vec![]),
            op("FlowCaptureReference", Some(ACTION), None, "c0", 239, vec![])])], None, false, Some(5), None),
        exit(5),
    ], vec![lambda]);
    reply(vec![g, helper, filter, ctor])
}
