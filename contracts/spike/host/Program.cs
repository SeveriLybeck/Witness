// usage: host <lib.cs> <app.cs> <iterations> name=wtBinDir [name=wtBinDir ...]
using System.Collections.Immutable;
using System.Diagnostics;
using System.Reflection;
using System.Runtime.Loader;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.Diagnostics;

var total = Stopwatch.StartNew();
string libSrc = File.ReadAllText(args[0]), appSrc = File.ReadAllText(args[1]);
int iterations = int.Parse(args[2]);
var parse = new CSharpParseOptions(LanguageVersion.CSharp12);
var bcl = ((string)AppContext.GetData("TRUSTED_PLATFORM_ASSEMBLIES")!).Split(Path.PathSeparator)
    .Where(p => Path.GetFileName(p).StartsWith("System.") || Path.GetFileName(p) is "mscorlib.dll" or "netstandard.dll")
    .Select(p => (MetadataReference)MetadataReference.CreateFromFile(p)).ToList();
Console.WriteLine($"host Roslyn: {typeof(CSharpCompilation).Assembly.GetName().Version}; runtime {Environment.Version}");

foreach (var spec in args.Skip(3))
{
    var (name, bin) = (spec.Split('=')[0], spec.Split('=')[1]);
    var sw = Stopwatch.StartNew();
    var refs = bcl.Concat(new[] { "MessagePack.dll", "MessagePack.Annotations.dll" }
        .Select(f => (MetadataReference)MetadataReference.CreateFromFile(Path.Combine(bin, "MessagePack/Release/net8.0", f)))).ToList();
    var opts = new CSharpCompilationOptions(OutputKind.DynamicallyLinkedLibrary, nullableContextOptions: NullableContextOptions.Enable);
    // lib -> metadata image (plain compile, no analyzer: the "annotations-only library" situation)
    var lib = CSharpCompilation.Create("Lib", new[] { CSharpSyntaxTree.ParseText(libSrc, parse, "Lib.cs") }, refs, opts);
    using var ms = new MemoryStream();
    var emit = lib.Emit(ms);
    if (!emit.Success) { Console.WriteLine("lib failed: " + string.Join("\n", emit.Diagnostics)); return 1; }
    var libRef = MetadataReference.CreateFromImage(ms.ToArray());
    var app = CSharpCompilation.Create("App", new[] { CSharpSyntaxTree.ParseText(appSrc, parse, "App.cs") }, refs.Append(libRef), opts);

    var alc = new SideAlc(name);
    var genAsm = alc.LoadFromAssemblyPath(Path.Combine(bin, "MessagePack.SourceGenerator/Release/netstandard2.0/MessagePack.SourceGenerator.dll"));
    var roslynRef = genAsm.GetReferencedAssemblies().First(a => a.Name == "Microsoft.CodeAnalysis");
    var types = genAsm.GetTypes();
    var generators = types.Where(t => !t.IsAbstract && typeof(IIncrementalGenerator).IsAssignableFrom(t)).Select(t => ((IIncrementalGenerator)Activator.CreateInstance(t)!).AsSourceGenerator()).ToArray();
    var analyzers = types.Where(t => !t.IsAbstract && typeof(DiagnosticAnalyzer).IsAssignableFrom(t)).Select(t => (DiagnosticAnalyzer)Activator.CreateInstance(t)!).ToImmutableArray();
    Console.WriteLine($"\n=== {name}: generator references Roslyn {roslynRef.Version}; {generators.Length} generator(s), {analyzers.Length} analyzer(s); setup+load {sw.ElapsedMilliseconds} ms");

    for (int i = 0; i < iterations; i++)
    {
        sw.Restart();
        // fresh tree each iteration so nothing is served from the incremental cache
        var comp = app.ReplaceSyntaxTree(app.SyntaxTrees[0], CSharpSyntaxTree.ParseText(appSrc + "\n// " + i, parse, "App.cs"));
        GeneratorDriver driver = CSharpGeneratorDriver.Create(generators, parseOptions: parse);
        driver = driver.RunGeneratorsAndUpdateCompilation(comp, out var outComp, out var genDiags);
        var run = driver.GetRunResult();
        var anDiags = outComp.WithAnalyzers(analyzers).GetAnalyzerDiagnosticsAsync().Result;
        var compDiags = outComp.GetDiagnostics().Where(d => d.Severity >= DiagnosticSeverity.Warning).ToArray();
        long ms1 = sw.ElapsedMilliseconds;
        if (i == 0)
        {
            foreach (var r in run.Results)
            {
                if (r.Exception is not null) Console.WriteLine($"  GENERATOR EXCEPTION {r.Exception.GetType().Name}: {r.Exception.Message}  @ {r.Exception.StackTrace?.Split('\n')[0].Trim()}");
                foreach (var s in r.GeneratedSources) Console.WriteLine($"  generated: {s.HintName} ({s.SourceText.Length} chars)");
            }
            foreach (var (kind, ds) in new[] { ("generator", genDiags.AsEnumerable()), ("analyzer", anDiags), ("compiler", compDiags) })
                foreach (var d in ds)
                    Console.WriteLine($"  [{kind}] {d.Id} {d.Severity} {(d.Location.IsInSource ? d.Location.GetLineSpan().ToString() : "no-location")}: {d.GetMessage().Split('\n')[0]}");
        }
        Console.WriteLine($"  run {i}: {ms1} ms");
    }
    alc.Unload();
}
Console.WriteLine($"\ntotal process time {total.ElapsedMilliseconds} ms (+ runtime startup)");
return 0;

sealed class SideAlc(string name) : AssemblyLoadContext(name, isCollectible: true)
{
    // Everything except the generator itself (Roslyn, System.*) resolves from the host's default context.
    protected override Assembly? Load(AssemblyName assemblyName) => null;
}
