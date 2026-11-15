using System.Text.Json.Nodes;
using System.Text.RegularExpressions;
using System.Xml.Linq;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;

namespace Witness.Worker;

/// `facts`: architectural facts of old against new, from syntax over the production sources (D33).
public static class Facts
{
    static readonly HashSet<string> SkipDirs = new(StringComparer.OrdinalIgnoreCase) { "bin", "obj", ".git", "tests", "test", "benchmark", "benchmarks", "sandbox", "samples" };

    static bool Production(string rel) => !rel.Split('/').SkipLast(1).Any(s => SkipDirs.Contains(s) || s.EndsWith(".Tests", StringComparison.OrdinalIgnoreCase));

    static List<string> Files(string wt, string pattern, bool productionOnly) =>
        Directory.EnumerateFiles(wt, pattern, SearchOption.AllDirectories)
            .Select(f => Path.GetRelativePath(wt, f).Replace('\\', '/'))
            .Where(r => !r.Split('/').SkipLast(1).Any(s => s is "bin" or "obj" or ".git" or "work"))
            .Where(r => !productionOnly || Production(r))
            .OrderBy(r => r, StringComparer.Ordinal).ToList();

    /// Value: the string a const field holds, or the diagnostic ID a DiagnosticDescriptor field is created with.
    sealed record Api(string Id, string Name, string Signature, string File, int Line, bool IsType, int EndLine, string? Container, string? Value);
    sealed record Wiring(string Key, string Symbol, string Value, string File, int Line);
    sealed record Diag(string Id, string Field, string File, int Line, int EndLine);

    /// The `id` argument of `new DiagnosticDescriptor(...)` (named or first positional), or a string literal, resolved through string constants.
    static string? IdOf(ExpressionSyntax? e, Dictionary<string, string> consts) => e switch
    {
        LiteralExpressionSyntax { RawKind: (int)SyntaxKind.StringLiteralExpression } lit => lit.Token.ValueText,
        IdentifierNameSyntax idn => consts.GetValueOrDefault(idn.Identifier.Text),
        MemberAccessExpressionSyntax ma => consts.GetValueOrDefault(ma.Name.Identifier.Text),
        BaseObjectCreationExpressionSyntax oc when (oc is not ObjectCreationExpressionSyntax o || o.Type.ToString().EndsWith("DiagnosticDescriptor", StringComparison.Ordinal))
            && oc.ArgumentList is { Arguments.Count: > 0 } al
            => IdOf((al.Arguments.FirstOrDefault(a => a.NameColon?.Name.Identifier.Text == "id") ?? al.Arguments.FirstOrDefault(a => a.NameColon == null))?.Expression, consts),
        _ => null,
    };

    sealed class Side
    {
        public readonly Dictionary<string, Api> Api = new();
        public readonly Dictionary<string, Wiring> Wiring = new();
        public readonly Dictionary<string, Diag> Diagnostics = new();
        public HashSet<string> CsFiles = new();
        public HashSet<string> Projects = new();
        public readonly Dictionary<string, HashSet<string>> Deps = new();
    }

    static string Norm(string s) => Regex.Replace(s, @"\s+", " ").Trim();

    static string TypePath(SyntaxNode n)
    {
        var parts = new List<string>();
        foreach (var a in n.AncestorsAndSelf())
        {
            if (a is BaseTypeDeclarationSyntax t) parts.Add(t.Identifier.Text + (t is TypeDeclarationSyntax { TypeParameterList: { } tp } ? "`" + tp.Parameters.Count : ""));
            else if (a is DelegateDeclarationSyntax dd) parts.Add(dd.Identifier.Text);
            else if (a is BaseNamespaceDeclarationSyntax ns) parts.Add(ns.Name.ToString());
        }
        parts.Reverse();
        return string.Join(".", parts);
    }

    static bool Public(SyntaxTokenList mods) => mods.Any(m => m.IsKind(SyntaxKind.PublicKeyword)) || mods.Any(m => m.IsKind(SyntaxKind.ProtectedKeyword)) && !mods.Any(m => m.IsKind(SyntaxKind.PrivateKeyword));

    static bool VisibleChain(SyntaxNode n)
    {
        foreach (var t in n.Ancestors().OfType<BaseTypeDeclarationSyntax>())
            if (!Public(t.Modifiers)) return false;
        return true;
    }

    static Side Read(string wt)
    {
        var side = new Side();
        var cs = Files(wt, "*.cs", true);
        side.CsFiles = cs.ToHashSet();
        var trees = cs.Select(rel => (rel, root: CSharpSyntaxTree.ParseText(File.ReadAllText(Path.Combine(wt, rel)), ProjectLoader.Parse, rel).GetRoot())).ToList();
        var consts = trees.SelectMany(t => t.root.DescendantNodes().OfType<VariableDeclaratorSyntax>())
            .Where(v => v.Initializer?.Value is LiteralExpressionSyntax { RawKind: (int)SyntaxKind.StringLiteralExpression })
            .GroupBy(v => v.Identifier.Text).ToDictionary(g => g.Key, g => ((LiteralExpressionSyntax)g.First().Initializer!.Value).Token.ValueText);
        foreach (var (rel, root) in trees)
        {
            int L(SyntaxNode n) => n.GetLocation().GetLineSpan().StartLinePosition.Line + 1;
            int E(SyntaxNode n) => n.GetLocation().GetLineSpan().EndLinePosition.Line + 1;
            foreach (var n in root.DescendantNodes())
            {
                switch (n)
                {
                    case BaseTypeDeclarationSyntax t when Public(t.Modifiers) && VisibleChain(t):
                        var tid = "T:" + TypePath(t);
                        side.Api.TryAdd(tid, new Api(tid, tid, Norm($"{t.Modifiers} {(t as TypeDeclarationSyntax)?.Keyword.Text ?? "enum"} {t.Identifier}{(t is TypeDeclarationSyntax td ? td.TypeParameterList?.ToString() : "")}{t.BaseList}"), rel, L(t), true, E(t), t.Parent is BaseTypeDeclarationSyntax pt ? "T:" + TypePath(pt) : null, null));
                        break;
                    case MemberDeclarationSyntax m when m.Parent is TypeDeclarationSyntax owner && (Public(m.Modifiers) || owner is InterfaceDeclarationSyntax) && VisibleChain(m) && (Public(owner.Modifiers)):
                        var (prefix, name, sig) = m switch
                        {
                            MethodDeclarationSyntax md => ("M:", md.Identifier.Text + md.TypeParameterList, $"{md.ReturnType} {md.Identifier}{md.TypeParameterList}{md.ParameterList}"),
                            ConstructorDeclarationSyntax cd => ("M:", "#ctor", $"{cd.Identifier}{cd.ParameterList}"),
                            PropertyDeclarationSyntax pd => ("P:", pd.Identifier.Text, $"{pd.Type} {pd.Identifier} {pd.AccessorList}"),
                            IndexerDeclarationSyntax ix => ("P:", "Item", $"{ix.Type} this{ix.ParameterList}"),
                            FieldDeclarationSyntax fd => ("F:", string.Join(",", fd.Declaration.Variables.Select(v => v.Identifier.Text)), $"{fd.Declaration.Type} {string.Join(", ", fd.Declaration.Variables.Select(v => v.Identifier.Text))}"),
                            EventFieldDeclarationSyntax ef => ("E:", string.Join(",", ef.Declaration.Variables.Select(v => v.Identifier.Text)), $"event {ef.Declaration.Type}"),
                            _ => ("", "", ""),
                        };
                        if (prefix == "") break;
                        var parms = m switch
                        {
                            BaseMethodDeclarationSyntax bm => "(" + string.Join(",", bm.ParameterList.Parameters.Select(p => Norm(p.Type?.ToString() ?? ""))) + ")",
                            IndexerDeclarationSyntax ix2 => "(" + string.Join(",", ix2.ParameterList.Parameters.Select(p => Norm(p.Type?.ToString() ?? ""))) + ")",
                            _ => "",
                        };
                        var baseName = prefix + TypePath(owner) + "." + name;
                        var id = baseName + parms;
                        var full = Norm($"{m.Modifiers} {sig}");
                        string? value = m is FieldDeclarationSyntax { Declaration.Variables: [{ Initializer.Value: var iv }] } ? IdOf(iv, consts) : null;
                        var api = new Api(id, baseName, full, rel, L(m), false, E(m), "T:" + TypePath(owner), value);
                        if (!side.Api.TryAdd(id, api)) side.Api[id + "#" + L(m)] = api;
                        break;
                }
                // one entry per DiagnosticDescriptor field, keyed by the field
                if (n is VariableDeclaratorSyntax { Initializer.Value: BaseObjectCreationExpressionSyntax init } dv
                    && dv.Parent?.Parent is FieldDeclarationSyntax { Parent: TypeDeclarationSyntax dt } fdecl
                    && fdecl.Declaration.Type.ToString().EndsWith("DiagnosticDescriptor", StringComparison.Ordinal)
                    && IdOf(init, consts) is { } did)
                {
                    var field = "F:" + TypePath(dt) + "." + dv.Identifier.Text;
                    side.Diagnostics.TryAdd(field, new Diag(did, field, rel, L(fdecl), E(fdecl)));
                }
                switch (n)
                {
                    case ConstructorDeclarationSyntax ctor when ctor.Parent is TypeDeclarationSyntax ct:
                        var ctorKey = TypePath(ct) + ".#ctor(" + string.Join(",", ctor.ParameterList.Parameters.Select(p => Norm(p.Type?.ToString() ?? ""))) + ")";
                        foreach (var ae in ctor.DescendantNodes().OfType<AssignmentExpressionSyntax>().Where(a => a.Parent is ExpressionStatementSyntax && a.IsKind(SyntaxKind.SimpleAssignmentExpression)))
                        {
                            var target = ae.Left is MemberAccessExpressionSyntax { Expression: ThisExpressionSyntax } me ? me.Name.Identifier.Text
                                : ae.Left is IdentifierNameSyntax iname ? iname.Identifier.Text : null;
                            if (target == null) continue;
                            var key = ctorKey + "/" + target;
                            side.Wiring.TryAdd(key, new Wiring(key, "F:" + TypePath(ct) + "." + target, Norm(ae.Right.ToString()), rel, L(ae)));
                        }
                        break;
                }
            }
        }
        foreach (var proj in Files(wt, "*.csproj", false).Concat(Files(wt, "*.props", false)))
        {
            if (proj.EndsWith(".csproj")) side.Projects.Add(proj);
            try
            {
                var x = XDocument.Load(Path.Combine(wt, proj));
                side.Deps[proj] = x.Descendants().Where(e => e.Name.LocalName is "PackageReference" or "ProjectReference" or "PackageVersion")
                    .Select(e => (string?)e.Attribute("Include")).Where(s => s != null).Select(s => s!.Replace('\\', '/')).ToHashSet();
            }
            catch { /* not XML */ }
        }
        return side;
    }

    static JsonObject Loc(string file, int line, int? endLine = null) => new() { ["file"] = file, ["line"] = line, ["endLine"] = endLine ?? line };

    static JsonObject ApiFact(string kind, Api a, string? old, string? @new, bool located, HashSet<string> newIds)
    {
        var f = Fact(kind, a.Id, old, @new, located ? Loc(a.File, a.Line, a.EndLine) : null);
        if (a.Container != null) f["container"] = a.Container;
        if (a.Value != null && newIds.Contains(a.Value)) f["diagnostic"] = a.Value;
        return f;
    }

    static JsonObject Fact(string kind, string symbol, string? old, string? @new, JsonObject? loc) =>
        new() { ["kind"] = kind, ["symbol"] = symbol, ["old"] = old, ["new"] = @new, ["location"] = loc };

    public static JsonObject Run(JsonObject p)
    {
        var o = Read((string)p["old"]!["worktree"]!);
        var n = Read((string)p["new"]!["worktree"]!);
        var facts = new JsonArray();
        var newDescriptors = n.Diagnostics.Values.Where(d => !o.Diagnostics.ContainsKey(d.Field)).OrderBy(d => d.Field, StringComparer.Ordinal).ToList();
        var newIds = newDescriptors.Select(d => d.Id).ToHashSet();
        var oldByName = o.Api.Values.GroupBy(a => a.Name).ToDictionary(g => g.Key, g => g.ToList());
        var newByName = n.Api.Values.GroupBy(a => a.Name).ToDictionary(g => g.Key, g => g.ToList());
        foreach (var (key, a) in n.Api.OrderBy(k => k.Key, StringComparer.Ordinal))
        {
            if (o.Api.TryGetValue(key, out var was))
            {
                if (was.Signature != a.Signature) facts.Add(ApiFact("changed-signature", a, was.Signature, a.Signature, true, newIds));
                continue;
            }
            if (!a.IsType && oldByName.TryGetValue(a.Name, out var olds) && olds.Count == 1 && newByName[a.Name].Count == 1)
                facts.Add(ApiFact("changed-signature", a, olds[0].Signature, a.Signature, true, newIds));
            else facts.Add(ApiFact(a.IsType ? "public-type" : "public-member", a, null, a.Signature, true, newIds));
        }
        foreach (var (key, a) in o.Api.OrderBy(k => k.Key, StringComparer.Ordinal))
        {
            if (n.Api.ContainsKey(key)) continue;
            if (!a.IsType && newByName.TryGetValue(a.Name, out var news) && news.Count == 1 && oldByName[a.Name].Count == 1) continue; // reported as changed-signature
            facts.Add(ApiFact(a.IsType ? "public-type" : "public-member", a, a.Signature, null, false, newIds));
        }
        foreach (var f in n.CsFiles.Except(o.CsFiles).OrderBy(x => x, StringComparer.Ordinal)) facts.Add(Fact("new-file", f, null, f, Loc(f, 1)));
        foreach (var f in n.Projects.Except(o.Projects).OrderBy(x => x, StringComparer.Ordinal)) facts.Add(Fact("new-project", f, null, f, Loc(f, 1)));
        foreach (var (proj, deps) in n.Deps.OrderBy(k => k.Key, StringComparer.Ordinal))
        {
            var before = o.Deps.GetValueOrDefault(proj) ?? new HashSet<string>();
            foreach (var d in deps.Except(before).OrderBy(x => x, StringComparer.Ordinal)) facts.Add(Fact("new-dependency", d, null, proj, Loc(proj, 1)));
        }
        foreach (var d in newDescriptors)
        {
            var f = Fact("new-diagnostic", d.Field, null, d.Id, Loc(d.File, d.Line, d.EndLine));
            f["diagnosticId"] = d.Id;
            f["diagnostic"] = d.Id;
            facts.Add(f);
        }
        foreach (var (key, w) in n.Wiring.OrderBy(k => k.Key, StringComparer.Ordinal))
            if (o.Wiring.TryGetValue(key, out var was) && was.Value != w.Value)
                facts.Add(Fact("changed-wiring", w.Symbol, was.Value, w.Value, Loc(w.File, w.Line)));
        return new JsonObject { ["facts"] = facts };
    }
}
