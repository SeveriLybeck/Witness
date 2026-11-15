using System.Reflection;
using System.Xml.Linq;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;

namespace Witness.Worker;

/// One compiled C# project of a checkout, loaded so that types bind (recipe of research/spikes/roslyn-cfg-check.md).
public sealed class LoadedProject
{
    public required string Worktree { get; init; }
    public required string ProjectDir { get; init; }
    public required CSharpCompilation Compilation { get; init; }
    public required List<string> Problems { get; init; }

    public string Rel(string path) => Path.GetRelativePath(Worktree, path).Replace('\\', '/');

    public SyntaxTree? Tree(string relFile)
    {
        var full = Path.GetFullPath(Path.Combine(Worktree, relFile));
        return Compilation.SyntaxTrees.FirstOrDefault(t => t.FilePath == full);
    }

    /// Error diagnostics located in one file.
    public List<Diagnostic> Errors(SyntaxTree tree) =>
        Compilation.GetSemanticModel(tree).GetDiagnostics().Where(d => d.Severity == DiagnosticSeverity.Error).ToList();
}

public static class ProjectLoader
{
    static readonly Dictionary<string, LoadedProject> Cache = new();
    static readonly object Gate = new();
    public static readonly CSharpParseOptions Parse = new(LanguageVersion.Latest);

    public static string PackagesDir()
    {
        var env = Environment.GetEnvironmentVariable("NUGET_PACKAGES");
        if (!string.IsNullOrEmpty(env)) return env;
        return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), ".nuget", "packages");
    }

    /// Highest installed version directory of a package, or the pinned one when given.
    public static string? Package(string id, string? pinned = null)
    {
        var dir = Path.Combine(PackagesDir(), id);
        if (!Directory.Exists(dir)) return null;
        if (pinned != null) return Directory.Exists(Path.Combine(dir, pinned)) ? Path.Combine(dir, pinned) : null;
        return Directory.GetDirectories(dir)
            .Select(d => (d, v: Version.TryParse(Path.GetFileName(d).Split('-')[0], out var v) ? v : new Version(0, 0)))
            .OrderByDescending(x => x.v).Select(x => x.d).FirstOrDefault();
    }

    static readonly (string id, string? pin, string dll)[] NetStandardPackages =
    {
        ("microsoft.codeanalysis.common", "4.12.0", "Microsoft.CodeAnalysis.dll"),
        ("microsoft.codeanalysis.csharp", "4.12.0", "Microsoft.CodeAnalysis.CSharp.dll"),
        ("system.collections.immutable", null, "System.Collections.Immutable.dll"),
        ("system.reflection.metadata", null, "System.Reflection.Metadata.dll"),
        ("system.memory", null, "System.Memory.dll"),
        ("system.runtime.compilerservices.unsafe", null, "System.Runtime.CompilerServices.Unsafe.dll"),
        ("system.buffers", null, "System.Buffers.dll"),
        ("system.numerics.vectors", null, "System.Numerics.Vectors.dll"),
        ("system.threading.tasks.extensions", null, "System.Threading.Tasks.Extensions.dll"),
    };

    /// The project that owns a file of a checkout (nearest directory with a .csproj), compiled and cached.
    public static LoadedProject ForFile(string worktree, string relFile)
    {
        var wt = Path.GetFullPath(worktree);
        var full = Path.GetFullPath(Path.Combine(wt, relFile));
        var dir = Path.GetDirectoryName(full)!;
        while (dir.StartsWith(wt, StringComparison.Ordinal) && dir.Length >= wt.Length && !Directory.EnumerateFiles(dir, "*.csproj").Any())
            dir = Path.GetDirectoryName(dir)!;
        if (!dir.StartsWith(wt, StringComparison.Ordinal) || !Directory.EnumerateFiles(dir, "*.csproj").Any())
            dir = Path.GetDirectoryName(full)!; // no project: the file's folder alone
        lock (Gate)
        {
            if (Cache.TryGetValue(dir, out var hit)) return hit;
            var p = Load(wt, dir);
            Cache[dir] = p;
            return p;
        }
    }

    static IEnumerable<string> SourceFiles(string dir) =>
        Directory.EnumerateFiles(dir, "*.cs", SearchOption.AllDirectories)
            .Where(f => !Path.GetRelativePath(dir, f).Split('/', '\\').Any(s => s is "bin" or "obj"))
            .OrderBy(f => f, StringComparer.Ordinal);

    static LoadedProject Load(string wt, string dir)
    {
        var problems = new List<string>();
        var csprojPath = Directory.EnumerateFiles(dir, "*.csproj").OrderBy(f => f).FirstOrDefault();
        XDocument? csproj = null;
        if (csprojPath != null) try { csproj = XDocument.Load(csprojPath); } catch (Exception e) { problems.Add($"csproj: {e.Message}"); }
        string Prop(string name) => csproj?.Descendants().FirstOrDefault(e => e.Name.LocalName == name)?.Value.Trim() ?? "";
        var projectName = csprojPath != null ? Path.GetFileNameWithoutExtension(csprojPath) : Path.GetFileName(dir);
        var rootNs = Prop("RootNamespace") is { Length: > 0 } rn ? rn : projectName;

        var files = SourceFiles(dir).ToList();
        foreach (var c in csproj?.Descendants().Where(e => e.Name.LocalName == "Compile") ?? Enumerable.Empty<XElement>())
        {
            var inc = (string?)c.Attribute("Include");
            if (inc == null || inc.Contains('*')) continue;
            var path = Path.GetFullPath(Path.Combine(dir, inc.Replace('\\', '/')));
            if (File.Exists(path) && !files.Contains(path)) files.Add(path);
            else if (!File.Exists(path)) problems.Add($"linked file missing: {inc}");
        }
        var trees = files.Select(f => CSharpSyntaxTree.ParseText(File.ReadAllText(f), Parse, f)).ToList();

        // build outputs: the repository may put obj/ at its root (obj/<project path>) or beside the project
        var relDir = Path.GetRelativePath(wt, dir);
        var objDirs = new[] { Path.Combine(wt, "obj", relDir), Path.Combine(dir, "obj") }.Where(Directory.Exists).ToList();
        string? Obj(string pattern) => objDirs.SelectMany(o => Directory.EnumerateFiles(o, pattern, SearchOption.AllDirectories))
            .OrderBy(f => f.Contains("/Release/") ? 0 : 1).ThenBy(f => f).FirstOrDefault();

        var implicitUsings = Prop("ImplicitUsings").Equals("enable", StringComparison.OrdinalIgnoreCase) || Prop("ImplicitUsings").Equals("true", StringComparison.OrdinalIgnoreCase);
        if (implicitUsings)
        {
            var gu = Obj("*.GlobalUsings.g.cs");
            var text = gu != null ? File.ReadAllText(gu)
                : "global using global::System;global using global::System.Collections.Generic;global using global::System.IO;global using global::System.Linq;global using global::System.Net.Http;global using global::System.Threading;global using global::System.Threading.Tasks;";
            trees.Add(CSharpSyntaxTree.ParseText(text, Parse, Path.Combine(dir, "obj", "GlobalUsings.g.cs")));
        }
        if (Obj("*.Version.cs") is { } version)
            trees.Add(CSharpSyntaxTree.ParseText(File.ReadAllText(version), Parse, version));
        else if (csprojPath != null)
            problems.Add("no Version.cs in obj (project not built)");

        var refs = new List<MetadataReference>();
        var nsLib = Package("netstandard.library", "2.0.3");
        if (nsLib == null) problems.Add("package netstandard.library 2.0.3 missing");
        else refs.AddRange(Directory.GetFiles(Path.Combine(nsLib, "build/netstandard2.0/ref"), "*.dll").OrderBy(f => f).Select(f => MetadataReference.CreateFromFile(f)));
        foreach (var (id, pin, dll) in NetStandardPackages)
        {
            var pkg = Package(id, pin);
            var path = pkg == null ? null : Path.Combine(pkg, "lib/netstandard2.0", dll);
            if (path == null || !File.Exists(path)) { problems.Add($"package {id} {pin} missing"); continue; }
            refs.Add(MetadataReference.CreateFromFile(path));
        }

        var nullable = Prop("Nullable").Equals("enable", StringComparison.OrdinalIgnoreCase) ? NullableContextOptions.Enable : NullableContextOptions.Disable;
        var opts = new CSharpCompilationOptions(OutputKind.DynamicallyLinkedLibrary, nullableContextOptions: nullable, allowUnsafe: true);
        var comp = CSharpCompilation.Create(projectName, trees, refs, opts);

        // PolySharp, run in-process as the build would
        if (csproj?.ToString().Contains("PolySharp", StringComparison.Ordinal) == true)
        {
            var poly = Package("polysharp");
            var dll = poly == null ? null : Path.Combine(poly, "analyzers/dotnet/cs/PolySharp.SourceGenerators.dll");
            if (dll == null || !File.Exists(dll)) problems.Add("package polysharp missing");
            else
            {
                var asm = Assembly.LoadFrom(dll);
                var gens = asm.GetTypes().Where(t => typeof(IIncrementalGenerator).IsAssignableFrom(t) && !t.IsAbstract)
                    .Select(t => ((IIncrementalGenerator)Activator.CreateInstance(t)!).AsSourceGenerator()).ToList();
                CSharpGeneratorDriver.Create(gens, parseOptions: Parse).RunGeneratorsAndUpdateCompilation(comp, out var c2, out _);
                comp = (CSharpCompilation)c2;
            }
        }
        // stand-in for the resx generator: one string property per entry
        foreach (var resx in Directory.EnumerateFiles(dir, "*.resx", SearchOption.TopDirectoryOnly))
        {
            try
            {
                var names = XDocument.Load(resx).Root!.Elements("data").Select(e => (string?)e.Attribute("name")).Where(n => n != null && SyntaxFacts.IsValidIdentifier(n));
                var cls = Path.GetFileNameWithoutExtension(resx);
                var stub = $"namespace {rootNs} {{ internal static class {cls} {{ public static System.Resources.ResourceManager ResourceManager => null!; public static System.Globalization.CultureInfo? Culture {{ get; set; }} "
                    + string.Join(" ", names.Select(n => $"public static string {n} => \"\";")) + " } }";
                comp = comp.AddSyntaxTrees(CSharpSyntaxTree.ParseText(stub, Parse, resx + ".stub.cs"));
            }
            catch (Exception e) { problems.Add($"resx {Path.GetFileName(resx)}: {e.Message}"); }
        }
        return new LoadedProject { Worktree = wt, ProjectDir = dir, Compilation = comp, Problems = problems };
    }
}
