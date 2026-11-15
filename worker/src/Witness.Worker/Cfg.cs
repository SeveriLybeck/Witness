using System.Text.Json.Nodes;
using System.Text.RegularExpressions;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp.Syntax;
using Microsoft.CodeAnalysis.FlowAnalysis;
using Microsoft.CodeAnalysis.Operations;

namespace Witness.Worker;

/// `cfg`: each requested method's ControlFlowGraph with IOperation trees and resolved symbols; lambdas and local functions as nested graphs (D16).
public static class Cfg
{
    public static readonly SymbolDisplayFormat TypeFormat = SymbolDisplayFormat.FullyQualifiedFormat
        .WithGlobalNamespaceStyle(SymbolDisplayGlobalNamespaceStyle.Omitted)
        .RemoveMiscellaneousOptions(SymbolDisplayMiscellaneousOptions.UseSpecialTypes);

    public static JsonObject Run(JsonObject p)
    {
        var co = p["checkout"]!.AsObject();
        var wt = (string)co["worktree"]!;
        var methods = new JsonArray();
        var errors = new JsonArray();
        foreach (var m in p["methods"]!.AsArray())
        {
            var file = (string)m!["file"]!;
            var name = (string)m["name"]!;
            var proj = ProjectLoader.ForFile(wt, file);
            var tree = proj.Tree(file);
            if (tree == null) { errors.Add($"{file}: not found in the project"); continue; }
            foreach (var d in proj.Errors(tree)) errors.Add($"{file}:{d.Location.GetLineSpan().StartLinePosition.Line + 1}: {d.Id} {d.GetMessage()}");
            var sm = proj.Compilation.GetSemanticModel(tree);
            var decls = FindMethods(tree, name).ToList();
            if (decls.Count == 0) { errors.Add($"{file}: method {name} not found"); continue; }
            foreach (var decl in decls)
            {
                var op = sm.GetOperation(decl);
                ControlFlowGraph? g = op switch
                {
                    IMethodBodyOperation mb => ControlFlowGraph.Create(mb),
                    IConstructorBodyOperation cb => ControlFlowGraph.Create(cb),
                    _ => null,
                };
                if (g == null) { errors.Add($"{file}: {name} has no body"); continue; }
                var sym = sm.GetDeclaredSymbol(decl) as IMethodSymbol;
                var w = new Writer(proj, errors);
                methods.Add(w.Graph(g, name, decl, sym?.Parameters ?? ImmutableParams));
            }
        }
        return new JsonObject { ["methods"] = methods, ["errors"] = errors };
    }

    static readonly System.Collections.Immutable.ImmutableArray<IParameterSymbol> ImmutableParams = System.Collections.Immutable.ImmutableArray<IParameterSymbol>.Empty;

    /// "Type.Method", "Outer.Inner.Method" or "Type..ctor".
    public static IEnumerable<BaseMethodDeclarationSyntax> FindMethods(SyntaxTree tree, string name)
    {
        string typePath, member;
        if (name.EndsWith("..ctor", StringComparison.Ordinal)) { typePath = name[..^6]; member = ".ctor"; }
        else { var i = name.LastIndexOf('.'); typePath = i < 0 ? "" : name[..i]; member = i < 0 ? name : name[(i + 1)..]; }
        foreach (var d in tree.GetRoot().DescendantNodes().OfType<BaseMethodDeclarationSyntax>())
        {
            var n = d switch { MethodDeclarationSyntax md => md.Identifier.Text, ConstructorDeclarationSyntax => ".ctor", _ => null };
            if (n != member) continue;
            var types = string.Join(".", d.Ancestors().OfType<BaseTypeDeclarationSyntax>().Reverse().Select(t => t.Identifier.Text));
            if (typePath.Length == 0 || types == typePath || types.EndsWith("." + typePath, StringComparison.Ordinal)) yield return d;
        }
    }

    public static JsonObject Loc(LoadedProject proj, SyntaxNode? node) => Loc(proj, node?.GetLocation());

    public static JsonObject Loc(LoadedProject proj, Location? loc)
    {
        if (loc == null || !loc.IsInSource) return new JsonObject { ["file"] = "?", ["line"] = 1, ["endLine"] = 1 };
        var s = loc.GetLineSpan();
        return new JsonObject { ["file"] = proj.Rel(s.Path), ["line"] = s.StartLinePosition.Line + 1, ["endLine"] = s.EndLinePosition.Line + 1 };
    }

    public static string? SymbolId(ISymbol? s)
    {
        if (s == null) return null;
        if (s is IMethodSymbol m && m.ReducedFrom != null) s = m.ReducedFrom;
        return s.OriginalDefinition.GetDocumentationCommentId();
    }

    /// The constant `id` argument of a DiagnosticDescriptor field's initializer, resolved through the semantic model.
    public static string? DiagnosticId(LoadedProject proj, IFieldSymbol field)
    {
        if (field.Type.Name != "DiagnosticDescriptor" || field.Type.ContainingNamespace?.ToDisplayString() != "Microsoft.CodeAnalysis") return null;
        foreach (var r in field.DeclaringSyntaxReferences)
        {
            if (r.GetSyntax() is not VariableDeclaratorSyntax { Initializer.Value: BaseObjectCreationExpressionSyntax { ArgumentList: { } args } }) continue;
            if (!proj.Compilation.SyntaxTrees.Contains(r.SyntaxTree)) continue;
            var arg = args.Arguments.FirstOrDefault(a => a.NameColon?.Name.Identifier.Text == "id")
                ?? args.Arguments.FirstOrDefault(a => a.NameColon == null);
            if (arg == null) continue;
            var cv = proj.Compilation.GetSemanticModel(r.SyntaxTree).GetConstantValue(arg.Expression);
            if (cv.HasValue && cv.Value is string s) return s;
        }
        return null;
    }

    sealed class Writer(LoadedProject proj, JsonArray errors)
    {
        readonly Dictionary<CaptureId, int> captures = new();
        int Capture(CaptureId id) { if (!captures.TryGetValue(id, out var n)) captures[id] = n = captures.Count + 1; return n; }

        HashSet<ILocalSymbol> maybeNull = new(SymbolEqualityComparer.Default);

        static IOperation Unwrap(IOperation op) { while (op is IConversionOperation c) op = c.Operand; return op; }
        static bool IsDefault(IOperation op) => Unwrap(op) is IDefaultValueOperation or ILiteralOperation { ConstantValue: { HasValue: true, Value: null } };

        /// Reference-type locals that some path assigns `default` or `null` (directly or through a flow capture of a conditional).
        static HashSet<ILocalSymbol> MaybeNullLocals(ControlFlowGraph g)
        {
            var all = g.Blocks.SelectMany(b => b.Operations.Concat(b.BranchValue != null ? new[] { b.BranchValue } : Array.Empty<IOperation>()))
                .SelectMany(o => o.DescendantsAndSelf()).ToList();
            var caps = all.OfType<IFlowCaptureOperation>().Where(c => IsDefault(c.Value)).Select(c => c.Id).ToHashSet();
            var set = new HashSet<ILocalSymbol>(SymbolEqualityComparer.Default);
            foreach (var a in all.OfType<ISimpleAssignmentOperation>())
                if (Unwrap(a.Target) is ILocalReferenceOperation lr && lr.Local.Type.IsReferenceType
                    && (IsDefault(a.Value) || Unwrap(a.Value) is IFlowCaptureReferenceOperation fr && caps.Contains(fr.Id)))
                    set.Add(lr.Local);
            return set;
        }

        string? ThrownType(IOperation exception)
        {
            var op = Unwrap(exception);
            if (op.Kind is not (OperationKind.None or OperationKind.Invalid) && op.Type is { TypeKind: not TypeKind.Error } t) return t.ToDisplayString(TypeFormat);
            var syn = exception.Syntax;
            var expr = syn.AncestorsAndSelf().Select(n => n switch { ThrowExpressionSyntax te => te.Expression, ThrowStatementSyntax ts => ts.Expression, _ => null }).FirstOrDefault(e => e != null)
                ?? syn as ExpressionSyntax;
            if (expr == null) return null;
            var ty = proj.Compilation.GetSemanticModel(expr.SyntaxTree).GetTypeInfo(expr).Type;
            return ty is { TypeKind: not TypeKind.Error } ? ty.ToDisplayString(TypeFormat) : null;
        }

        /// SHA-256 of the lambda's text with whitespace collapsed: equal for a lambda that only moved.
        static string ContentHash(SyntaxNode syntax) =>
            Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(System.Text.Encoding.UTF8.GetBytes(Regex.Replace(syntax.ToString(), @"\s+", " ").Trim()))).ToLowerInvariant();

        public JsonObject Graph(ControlFlowGraph g, string name, SyntaxNode decl, IEnumerable<IParameterSymbol> ps)
        {
            var outer = maybeNull;
            maybeNull = MaybeNullLocals(g);
            var lambdas = new JsonArray();
            var blocks = new JsonArray();
            foreach (var b in g.Blocks)
            {
                var ops = new JsonArray();
                foreach (var o in b.Operations) ops.Add(Op(o, g, lambdas, name));
                var fs = b.FallThroughSuccessor;
                var cs = b.ConditionalSuccessor;
                var blk = new JsonObject
                {
                    ["ordinal"] = b.Ordinal,
                    ["kind"] = b.Kind switch { BasicBlockKind.Entry => "entry", BasicBlockKind.Exit => "exit", _ => "block" },
                    ["operations"] = ops,
                    ["branch"] = b.BranchValue == null ? null : Op(b.BranchValue, g, lambdas, name),
                    ["fallThrough"] = fs?.Destination?.Ordinal,
                    ["conditional"] = cs?.Destination?.Ordinal,
                };
                if (b.ConditionKind != ControlFlowConditionKind.None) blk["jumpIfTrue"] = b.ConditionKind == ControlFlowConditionKind.WhenTrue;
                if (fs?.Semantics is ControlFlowBranchSemantics.Throw or ControlFlowBranchSemantics.Rethrow) blk["throws"] = true;
                if (fs?.Semantics == ControlFlowBranchSemantics.Throw && b.BranchValue != null && ThrownType(b.BranchValue) is { } tt) blk["thrownType"] = tt;
                if (fs?.Semantics == ControlFlowBranchSemantics.Rethrow) blk["rethrow"] = true;
                if (fs?.Semantics == ControlFlowBranchSemantics.StructuredExceptionHandling) blk["endOfRegion"] = true;
                blk["region"] = b.EnclosingRegion.Kind.ToString();
                if (!b.IsReachable) blk["reachable"] = false;
                blocks.Add(blk);
            }
            foreach (var lf in g.LocalFunctions)
            {
                var lg = g.GetLocalFunctionControlFlowGraph(lf);
                var syn = lf.DeclaringSyntaxReferences.FirstOrDefault()?.GetSyntax() ?? decl;
                lambdas.Add(Graph(lg, $"{name}/local:{lf.Name}", syn, lf.Parameters));
            }
            var gr = new JsonObject
            {
                ["name"] = name,
                ["location"] = Loc(proj, decl),
                ["parameters"] = new JsonArray(ps.Select(p => (JsonNode)new JsonObject { ["name"] = p.Name, ["type"] = p.Type.ToDisplayString(TypeFormat) }).ToArray()),
                ["blocks"] = blocks,
            };
            if (lambdas.Count > 0) gr["lambdas"] = lambdas;
            if (decl is AnonymousFunctionExpressionSyntax or LocalFunctionStatementSyntax) gr["hash"] = ContentHash(decl);
            if (maybeNull.Count > 0) gr["maybeNullLocals"] = new JsonArray(maybeNull.Select(l => (JsonNode)l.Name).OrderBy(n => (string)n!, StringComparer.Ordinal).ToArray());
            maybeNull = outer;
            return gr;
        }

        static string Snip(SyntaxNode? s)
        {
            var t = Regex.Replace(s?.ToString() ?? "", @"\s+", " ");
            return t.Length > 300 ? t[..300] + "…" : t;
        }

        JsonObject Op(IOperation op, ControlFlowGraph g, JsonArray lambdas, string owner)
        {
            var o = new JsonObject
            {
                ["kind"] = op.Kind.ToString(),
                ["type"] = op.Type?.ToDisplayString(TypeFormat),
                ["symbol"] = null,
                ["syntax"] = Snip(op.Syntax),
                ["location"] = Loc(proj, op.Syntax),
            };
            if (op.Kind is OperationKind.None or OperationKind.Invalid)
                errors.Add($"unrecognised operation {op.Kind} at {proj.Rel(op.Syntax.SyntaxTree.FilePath)}:{op.Syntax.GetLocation().GetLineSpan().StartLinePosition.Line + 1}: {Snip(op.Syntax)}");
            if (op.IsImplicit) o["implicit"] = true;
            if (op.ConstantValue.HasValue) o["constant"] = op.ConstantValue.Value?.ToString();
            switch (op)
            {
                case IInvocationOperation inv: o["symbol"] = SymbolId(inv.TargetMethod); if (inv.IsVirtual) o["virtual"] = true; break;
                case IPropertyReferenceOperation pr:
                    o["symbol"] = SymbolId(pr.Property);
                    // a positional record property: declared by a primary-constructor parameter, reads back that argument
                    if (pr.Property.OriginalDefinition.DeclaringSyntaxReferences.FirstOrDefault()?.GetSyntax() is ParameterSyntax { Parent: ParameterListSyntax { Parent: RecordDeclarationSyntax } pl } psyn)
                        o["positionalParameter"] = pl.Parameters.IndexOf(psyn);
                    break;
                case IFieldReferenceOperation fr:
                    o["symbol"] = SymbolId(fr.Field);
                    if (DiagnosticId(proj, fr.Field) is { } did) o["diagnosticId"] = did;
                    break;
                case IEventReferenceOperation er: o["symbol"] = SymbolId(er.Event); break;
                case IMethodReferenceOperation mr: o["symbol"] = SymbolId(mr.Method); break;
                case IObjectCreationOperation oc: o["symbol"] = SymbolId(oc.Constructor); break;
                case ILocalReferenceOperation lr: o["name"] = lr.Local.Name; if (maybeNull.Contains(lr.Local)) o["maybeNull"] = true; break;
                case IParameterReferenceOperation pa: o["name"] = pa.Parameter.Name; break;
                case IArgumentOperation ar: o["name"] = ar.Parameter?.Name; break;
                case IFlowCaptureOperation fc: o["capture"] = Capture(fc.Id); break;
                case IFlowCaptureReferenceOperation fcr: o["capture"] = Capture(fcr.Id); break;
                case IBinaryOperation bo: o["operator"] = bo.OperatorKind.ToString(); if (bo.OperatorMethod != null) o["symbol"] = SymbolId(bo.OperatorMethod); break;
                case IUnaryOperation uo: o["operator"] = uo.OperatorKind.ToString(); break;
                case ICompoundAssignmentOperation ca: o["operator"] = ca.OperatorKind.ToString(); break;
                case IIncrementOrDecrementOperation id: o["operator"] = id.Kind.ToString(); break;
                case IConversionOperation cv: o["conversion"] = cv.IsTryCast ? "try-cast" : cv.Conversion.IsImplicit ? "implicit" : "explicit"; if (cv.OperatorMethod != null) o["symbol"] = SymbolId(cv.OperatorMethod); break;
                case IDeclarationPatternOperation dp: o["name"] = dp.DeclaredSymbol?.Name; o["matchedType"] = dp.MatchedType?.ToDisplayString(TypeFormat); break;
                case ITypePatternOperation tp: o["matchedType"] = tp.MatchedType.ToDisplayString(TypeFormat); break;
                case IRecursivePatternOperation rp: o["matchedType"] = rp.MatchedType.ToDisplayString(TypeFormat); o["name"] = rp.DeclaredSymbol?.Name; break;
                case IRelationalPatternOperation rel: o["operator"] = rel.OperatorKind.ToString(); break;
                case IBinaryPatternOperation bp: o["operator"] = bp.OperatorKind.ToString(); break;
                case IPropertySubpatternOperation: break;
                case ITypeOfOperation to: o["matchedType"] = to.TypeOperand.ToDisplayString(TypeFormat); break;
                case IIsTypeOperation it: o["matchedType"] = it.TypeOperand.ToDisplayString(TypeFormat); break;
                case IFlowAnonymousFunctionOperation af:
                {
                    var lg = g.GetAnonymousFunctionControlFlowGraph(af);
                    var line = af.Syntax.GetLocation().GetLineSpan().StartLinePosition;
                    var lname = $"{owner}/lambda@{line.Line + 1}:{line.Character + 1}";
                    o["symbol"] = null;
                    o["graph"] = lname;
                    o["graphHash"] = ContentHash(af.Syntax);
                    lambdas.Add(Graph(lg, lname, af.Syntax, af.Symbol.Parameters));
                    break;
                }
            }
            var kids = op.ChildOperations.Select(c => (JsonNode)Op(c, g, lambdas, owner)).ToArray();
            if (kids.Length > 0) o["children"] = new JsonArray(kids);
            return o;
        }
    }
}
