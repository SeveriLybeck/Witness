using System.Text.Json.Nodes;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;

namespace Witness.Worker;

/// `references` (members a part references) and `dependencies` (block edges and reach), from Roslyn symbols (D29, D30, D33).
public static class Links
{
    public readonly record struct Range(string File, int Line, int EndLine)
    {
        public static Range Of(JsonNode n) => new((string)n["file"]!, (int)n["line"]!, (int)n["endLine"]!);
        public bool Holds(string file, int line) => file == File && line >= Line && line <= EndLine;
    }

    static int Line(SyntaxNode n) => n.GetLocation().GetLineSpan().StartLinePosition.Line + 1;

    /// Nodes of one file whose first character lies inside the range.
    static IEnumerable<SyntaxNode> NodesIn(SyntaxTree tree, Range r)
    {
        var text = tree.GetText();
        if (r.Line > text.Lines.Count) yield break;
        var start = text.Lines[r.Line - 1].Start;
        var end = text.Lines[Math.Min(r.EndLine, text.Lines.Count) - 1].End;
        var span = Microsoft.CodeAnalysis.Text.TextSpan.FromBounds(start, end);
        foreach (var n in tree.GetRoot().DescendantNodes(span))
            if (span.Contains(n.SpanStart)) yield return n;
    }

    /// Stable key of a symbol within one version (locals by their declaration).
    public static string? Key(ISymbol? s)
    {
        if (s == null) return null;
        s = s.OriginalDefinition;
        if (s is IMethodSymbol m && m.ReducedFrom != null) s = m.ReducedFrom;
        return s switch
        {
            ILocalSymbol l => $"local:{l.DeclaringSyntaxReferences.FirstOrDefault()?.SyntaxTree.FilePath}:{l.DeclaringSyntaxReferences.FirstOrDefault()?.Span.Start}:{l.Name}",
            IParameterSymbol p => $"param:{Cfg.SymbolId(p.ContainingSymbol)}:{p.Name}",
            _ => s.GetDocumentationCommentId(),
        };
    }

    static ISymbol? Resolve(SemanticModel sm, SyntaxNode n)
    {
        var info = sm.GetSymbolInfo(n);
        return info.Symbol ?? info.CandidateSymbols.FirstOrDefault();
    }

    public static JsonObject References(JsonObject p)
    {
        var co = p["checkout"]!.AsObject();
        var wt = (string)co["worktree"]!;
        var changed = p["changedMembers"]?.AsArray().Select(x => (string)x!).ToHashSet();
        var parts = new JsonArray();
        foreach (var part in p["parts"]!.AsArray())
        {
            var r = Range.Of(part!["location"]!);
            var names = new SortedSet<string>(StringComparer.Ordinal);
            var symbols = new SortedSet<string>(StringComparer.Ordinal);
            if (r.File.EndsWith(".cs", StringComparison.Ordinal) && File.Exists(Path.Combine(wt, r.File)))
            {
                var proj = ProjectLoader.ForFile(wt, r.File);
                var tree = proj.Tree(r.File);
                if (tree != null)
                {
                    var sm = proj.Compilation.GetSemanticModel(tree);
                    foreach (var n in NodesIn(tree, r).OfType<SimpleNameSyntax>())
                    {
                        var name = n.Identifier.Text;
                        if (changed != null && !changed.Contains(name)) continue;
                        var s = Resolve(sm, n);
                        if (s == null) { if (changed != null) names.Add(name); continue; }
                        if (s is not (IMethodSymbol or IPropertySymbol or IFieldSymbol or IEventSymbol or INamedTypeSymbol)) continue;
                        if (changed == null && !s.Locations.Any(l => l.IsInSource)) continue;
                        names.Add(s.Name);
                        if (Cfg.SymbolId(s) is { } id) symbols.Add(id);
                    }
                }
            }
            parts.Add(new JsonObject
            {
                ["id"] = (string)part["id"]!,
                ["references"] = new JsonArray(names.Select(x => (JsonNode)x).ToArray()),
                ["symbols"] = new JsonArray(symbols.Select(x => (JsonNode)x).ToArray()),
            });
        }
        return new JsonObject { ["parts"] = parts };
    }

    sealed class BlockFacts
    {
        public readonly HashSet<string> Declared = new();
        public readonly HashSet<string> Written = new();
        public readonly Dictionary<string, string> Names = new();
        public readonly List<(string key, string file, int line)> Uses = new();
        /// Statements outside the block that read a local the block writes, with every symbol they use.
        public readonly List<(string file, int line, HashSet<string> keys)> Plumb = new();
        public string Text = "";
    }

    /// The variable an expression in the range is stored into, when the range holds only part of that statement.
    static ISymbol? EnclosingTarget(SemanticModel sm, SyntaxNode top)
    {
        for (SyntaxNode? n = top; n != null && n is not StatementSyntax and not MemberDeclarationSyntax; n = n.Parent)
        {
            if (n.Parent is EqualsValueClauseSyntax { Parent: VariableDeclaratorSyntax vd }) return sm.GetDeclaredSymbol(vd);
            if (n.Parent is AssignmentExpressionSyntax ae && ae.Right == n) return Resolve(sm, ae.Left);
        }
        return null;
    }

    static BlockFacts Analyse(string wt, List<Range> ranges)
    {
        var f = new BlockFacts();
        var text = new System.Text.StringBuilder();
        foreach (var r in ranges)
        {
            if (!r.File.EndsWith(".cs", StringComparison.Ordinal) || !File.Exists(Path.Combine(wt, r.File))) continue;
            var proj = ProjectLoader.ForFile(wt, r.File);
            var tree = proj.Tree(r.File);
            if (tree == null) continue;
            var sm = proj.Compilation.GetSemanticModel(tree);
            var nodes = NodesIn(tree, r).ToList();
            var nodeSet = nodes.ToHashSet();
            var tops = nodes.Where(n => n.Parent is not null && !nodeSet.Contains(n.Parent)).ToList();
            foreach (var n in tops) text.Append(n.ToString().Trim()).Append('\n');
            var localsWritten = new HashSet<ILocalSymbol>(SymbolEqualityComparer.Default);
            void Write(ISymbol? s)
            {
                if (Key(s) is not { } k) return;
                f.Written.Add(k);
                f.Names[k] = s!.Name;
                if (s is ILocalSymbol l) localsWritten.Add(l);
            }
            foreach (var top in tops) Write(EnclosingTarget(sm, top));
            foreach (var n in nodes)
            {
                switch (n)
                {
                    case MemberDeclarationSyntax md when md is BaseMethodDeclarationSyntax or BasePropertyDeclarationSyntax:
                        if (sm.GetDeclaredSymbol(md) is { } ds && Key(ds) is { } k1) { f.Declared.Add(k1); f.Names[k1] = ds.Name; }
                        break;
                    case VariableDeclaratorSyntax vd:
                        if (sm.GetDeclaredSymbol(vd) is { } vs && Key(vs) is { } k2)
                        {
                            if (vs is IFieldSymbol) f.Declared.Add(k2);
                            if (vd.Initializer != null || vs is IFieldSymbol) Write(vs);
                        }
                        break;
                    case AssignmentExpressionSyntax ae:
                        Write(Resolve(sm, ae.Left));
                        break;
                    case SimpleNameSyntax sn:
                        if (IsAssignTarget(sn)) break;
                        if (Resolve(sm, sn) is { } us && Key(us) is { } k4) f.Uses.Add((k4, r.File, Line(sn)));
                        break;
                }
            }
            // reads of the block's locals outside the block: the statement that consumes the value
            foreach (var local in localsWritten)
            {
                var body = local.DeclaringSyntaxReferences.FirstOrDefault()?.GetSyntax().FirstAncestorOrSelf<MemberDeclarationSyntax>();
                if (body == null || body.SyntaxTree != tree) continue;
                foreach (var id in body.DescendantNodes().OfType<IdentifierNameSyntax>().Where(i => i.Identifier.Text == local.Name))
                {
                    var line = Line(id);
                    if (ranges.Any(x => x.Holds(r.File, line)) || IsAssignTarget(id)) continue;
                    if (!SymbolEqualityComparer.Default.Equals(Resolve(sm, id), local)) continue;
                    var stmt = id.FirstAncestorOrSelf<StatementSyntax>();
                    if (stmt == null) continue;
                    var keys = stmt.DescendantNodesAndSelf().OfType<SimpleNameSyntax>().Select(x => Key(Resolve(sm, x))).OfType<string>().ToHashSet();
                    f.Plumb.Add((r.File, line, keys));
                }
            }
        }
        f.Text = text.ToString();
        return f;
    }

    static bool IsAssignTarget(SimpleNameSyntax sn)
    {
        SyntaxNode n = sn;
        if (n.Parent is MemberAccessExpressionSyntax ma && ma.Name == sn) n = ma;
        return n.Parent is AssignmentExpressionSyntax ae && ae.Left == n && ae.IsKind(SyntaxKind.SimpleAssignmentExpression);
    }

    /// Is this use of a symbol a call site: a method invoked, or a delegate invoked (`d(...)`, `d.Invoke(...)`, `d?.Invoke(...)`).
    static bool IsCallSite(SimpleNameSyntax sn, ISymbol s)
    {
        SyntaxNode n = sn;
        if (n.Parent is MemberAccessExpressionSyntax ma && ma.Name == sn) n = ma;
        if (s is IMethodSymbol) return n.Parent is InvocationExpressionSyntax inv && inv.Expression == n;
        var t = s switch { IFieldSymbol fs => fs.Type, IPropertySymbol ps => ps.Type, ILocalSymbol ls => ls.Type, _ => null };
        if (t?.TypeKind != TypeKind.Delegate) return !IsAssignTarget(sn);
        if (n.Parent is InvocationExpressionSyntax di && di.Expression == n) return true;
        if (n.Parent is MemberAccessExpressionSyntax inv2 && inv2.Name.Identifier.Text == "Invoke" && inv2.Parent is InvocationExpressionSyntax) return true;
        if (n.Parent is ConditionalAccessExpressionSyntax ca && ca.Expression == n && ca.WhenNotNull is InvocationExpressionSyntax { Expression: MemberBindingExpressionSyntax { Name.Identifier.Text: "Invoke" } }) return true;
        return false;
    }

    public static JsonObject Dependencies(JsonObject p)
    {
        var oldWt = (string)p["old"]!["worktree"]!;
        var newWt = (string)p["new"]!["worktree"]!;
        var blocks = p["blocks"]!.AsArray().Select(b => (
            id: (string)b!["id"]!,
            old: b["old"]!.AsArray().Select(x => Range.Of(x!)).ToList(),
            @new: b["new"]!.AsArray().Select(x => Range.Of(x!)).ToList())).ToList();

        var facts = new Dictionary<(string ver, string id), BlockFacts>();
        foreach (var b in blocks)
        {
            facts[("old", b.id)] = Analyse(oldWt, b.old);
            facts[("new", b.id)] = Analyse(newWt, b.@new);
        }
        var edges = new Dictionary<(string from, string to), Dictionary<string, (bool reference, SortedSet<(string file, int line)> lines)>>();
        void Add(string ver, string from, string to, bool reference, string file, int line)
        {
            if (!edges.TryGetValue((from, to), out var perVer)) edges[(from, to)] = perVer = new();
            if (!perVer.TryGetValue(ver, out var e)) e = (false, new SortedSet<(string file, int line)>());
            e.reference |= reference;
            e.lines.Add((file, line));
            perVer[ver] = e;
        }
        foreach (var ver in new[] { "old", "new" })
            foreach (var a in blocks)
                foreach (var b in blocks)
                {
                    if (a.id == b.id) continue;
                    var fa = facts[(ver, a.id)];
                    var fb = facts[(ver, b.id)];
                    var aRanges = ver == "old" ? a.old : a.@new;
                    // B uses a member A declares or a value A writes
                    foreach (var (key, file, line) in fb.Uses)
                    {
                        bool decl = fa.Declared.Contains(key), wr = fa.Written.Contains(key);
                        if (decl || wr) Add(ver, a.id, b.id, decl && !wr, file, line);
                    }
                    // a value B computes is consumed through A's member, in a statement outside both blocks
                    foreach (var (file, line, keys) in fb.Plumb)
                    {
                        if (aRanges.Any(x => x.Holds(file, line))) continue;
                        bool decl = keys.Overlaps(fa.Declared), wr = keys.Overlaps(fa.Written.Where(k => !k.StartsWith("local:")));
                        if (decl || wr) Add(ver, a.id, b.id, decl && !wr, file, line);
                    }
                }
        var deps = new JsonArray();
        foreach (var ((from, to), perVer) in edges.OrderBy(e => e.Key.from, StringComparer.Ordinal).ThenBy(e => e.Key.to, StringComparer.Ordinal))
        {
            var version = perVer.Count == 2 ? "both" : perVer.Keys.Single();
            var e = perVer.TryGetValue("new", out var en) ? en : perVer["old"];
            bool changed = version != "both"
                || !perVer["old"].lines.SetEquals(perVer["new"].lines)
                || facts[("old", from)].Text != facts[("new", from)].Text;
            deps.Add(new JsonObject
            {
                ["from"] = from, ["to"] = to, ["version"] = version,
                ["via"] = e.reference ? "reference" : "value",
                ["changed"] = changed,
                ["sites"] = new JsonArray(e.lines.Select(l => (JsonNode)new JsonObject { ["file"] = l.file, ["line"] = l.line, ["endLine"] = l.line }).ToArray()),
            });
        }

        var reach = new JsonArray();
        foreach (var b in blocks)
        {
            var (ver, wt, ranges) = b.@new.Count > 0 ? ("new", newWt, b.@new) : ("old", oldWt, b.old);
            var f = facts[(ver, b.id)];
            var targets = f.Declared.Union(f.Written).Where(k => !k.StartsWith("local:") && !k.StartsWith("param:")).ToHashSet();
            var sites = new SortedSet<(string file, int line)>();
            var projects = ranges.Where(r => r.File.EndsWith(".cs") && File.Exists(Path.Combine(wt, r.File)))
                .Select(r => ProjectLoader.ForFile(wt, r.File)).Distinct().ToList();
            var names = targets.Select(k => f.Names.GetValueOrDefault(k)).Where(n => n != null).ToHashSet();
            foreach (var proj in projects)
                foreach (var tree in proj.Compilation.SyntaxTrees)
                {
                    SemanticModel? sm = null;
                    var rel = proj.Rel(tree.FilePath);
                    foreach (var sn in tree.GetRoot().DescendantNodes().OfType<SimpleNameSyntax>())
                    {
                        if (!names.Contains(sn.Identifier.Text)) continue;
                        var line = Line(sn);
                        if (ranges.Any(r => r.Holds(rel, line))) continue;
                        sm ??= proj.Compilation.GetSemanticModel(tree);
                        var s = Resolve(sm, sn);
                        if (s == null || !targets.Contains(Key(s) ?? "")) continue;
                        if (IsCallSite(sn, s)) sites.Add((rel, line));
                    }
                }
            reach.Add(new JsonObject
            {
                ["block"] = b.id,
                ["callSites"] = sites.Count,
                ["sites"] = new JsonArray(sites.Select(s => (JsonNode)new JsonObject { ["file"] = s.file, ["line"] = s.line, ["endLine"] = s.line }).ToArray()),
            });
        }
        return new JsonObject { ["dependencies"] = deps, ["reach"] = reach };
    }
}
