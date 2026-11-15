using System.Collections.Immutable;
using System.Diagnostics;
using System.Reflection;
using System.Runtime.Loader;
using System.Text.Json.Nodes;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.Diagnostics;

namespace Witness.Worker;

/// `run`: the two-version generator host (D37). Each generator loads in its own load context; reviewer code
/// is only compiled here, and executed (round trip) in a bwrap child process.
public static class Lab
{
    sealed class SideAlc(string name) : AssemblyLoadContext(name, isCollectible: true)
    {
        protected override Assembly? Load(AssemblyName assemblyName) => null; // Roslyn and the BCL come from the worker
    }

    sealed record Generator(SideAlc Alc, ISourceGenerator[] Generators, ImmutableArray<DiagnosticAnalyzer> Analyzers);

    static readonly Dictionary<string, Generator> Loaded = new();
    static readonly CSharpParseOptions Parse = new(LanguageVersion.CSharp12);

    static Generator Load(string path)
    {
        path = Path.GetFullPath(path);
        lock (Loaded)
        {
            if (Loaded.TryGetValue(path, out var g)) return g;
            var alc = new SideAlc(path);
            var asm = alc.LoadFromAssemblyPath(path);
            var types = asm.GetTypes().Where(t => !t.IsAbstract).ToList();
            var gens = types.Where(t => typeof(IIncrementalGenerator).IsAssignableFrom(t)).Select(t => ((IIncrementalGenerator)Activator.CreateInstance(t)!).AsSourceGenerator())
                .Concat(types.Where(t => typeof(ISourceGenerator).IsAssignableFrom(t)).Select(t => (ISourceGenerator)Activator.CreateInstance(t)!)).ToArray();
            var analyzers = types.Where(t => typeof(DiagnosticAnalyzer).IsAssignableFrom(t) && t.GetCustomAttributes(typeof(DiagnosticAnalyzerAttribute), false).Length > 0)
                .Select(t => (DiagnosticAnalyzer)Activator.CreateInstance(t)!).ToImmutableArray();
            return Loaded[path] = new Generator(alc, gens, analyzers);
        }
    }

    sealed class Options(ImmutableDictionary<string, string> values) : AnalyzerConfigOptions
    {
        public override bool TryGetValue(string key, out string value) => values.TryGetValue(key, out value!);
    }

    sealed class OptionsProvider(ImmutableDictionary<string, string> global) : AnalyzerConfigOptionsProvider
    {
        static readonly Options Empty = new(ImmutableDictionary<string, string>.Empty);
        public override AnalyzerConfigOptions GlobalOptions { get; } = new Options(global);
        public override AnalyzerConfigOptions GetOptions(SyntaxTree tree) => Empty;
        public override AnalyzerConfigOptions GetOptions(AdditionalText textFile) => Empty;
    }

    static readonly Lazy<List<MetadataReference>> Bcl = new(() =>
        ((string)AppContext.GetData("TRUSTED_PLATFORM_ASSEMBLIES")!).Split(Path.PathSeparator)
            .Where(p => Path.GetFileName(p).StartsWith("System.", StringComparison.Ordinal) || Path.GetFileName(p) is "mscorlib.dll" or "netstandard.dll")
            .OrderBy(p => p, StringComparer.Ordinal)
            .Select(p => (MetadataReference)MetadataReference.CreateFromFile(p)).ToList());

    static readonly Dictionary<string, MetadataReference> RefCache = new();
    static MetadataReference Ref(string path)
    {
        lock (RefCache) return RefCache.TryGetValue(path, out var r) ? r : RefCache[path] = MetadataReference.CreateFromFile(path);
    }

    /// The runtime folder holding MessagePack.dll: given, or found beside the generator's build output.
    static string Runtime(JsonObject side)
    {
        if (side["runtime"] is JsonValue rv && (string?)rv is { Length: > 0 } r) return Path.GetFullPath(r);
        var gen = Path.GetFullPath((string)side["generator"]!);
        var bin = gen;
        for (int i = 0; i < 4; i++) bin = Path.GetDirectoryName(bin)!;
        return Path.Combine(bin, "MessagePack", "Release", "net8.0");
    }

    public static JsonObject Run(JsonObject p)
    {
        var props = p["buildProperties"]?.AsObject().ToDictionary(kv => kv.Key, kv => (string?)kv.Value ?? "") ?? new();
        var lib = (string)p["library"]!;
        var app = (string)p["app"]!;
        bool roundTrip = (bool)p["roundTrip"]!;
        bool genOnly = (bool)p["generatedResolverOnly"]!;
        var result = new JsonObject
        {
            ["old"] = Side(p["old"]!.AsObject(), lib, app, props, roundTrip, genOnly),
            ["new"] = Side(p["new"]!.AsObject(), lib, app, props, roundTrip, genOnly),
            ["reverted"] = p["reverted"] is JsonObject rev ? Side(rev, lib, app, props, roundTrip, genOnly) : null,
            ["workerPid"] = Environment.ProcessId,
        };
        return result;
    }

    static string Severity(DiagnosticSeverity s) => s switch
    {
        DiagnosticSeverity.Error => "error", DiagnosticSeverity.Warning => "warning", DiagnosticSeverity.Info => "info", _ => "hidden",
    };

    static JsonObject Side(JsonObject side, string libSrc, string appSrc, Dictionary<string, string> props, bool roundTrip, bool genOnly)
    {
        var sw = Stopwatch.StartNew();
        var gen = Load((string)side["generator"]!);
        var runtime = Runtime(side);
        var refs = Bcl.Value.Concat(new[] { "MessagePack.dll", "MessagePack.Annotations.dll" }.Select(f => Path.Combine(runtime, f)).Where(File.Exists).Select(Ref)).ToList();

        var noWarn = (props.GetValueOrDefault("NoWarn") ?? "").Split(new[] { ';', ',', ' ' }, StringSplitOptions.RemoveEmptyEntries).ToHashSet(StringComparer.OrdinalIgnoreCase);
        var nullable = (props.GetValueOrDefault("Nullable") ?? "").ToLowerInvariant() switch
        {
            "enable" => NullableContextOptions.Enable, "warnings" => NullableContextOptions.Warnings, "annotations" => NullableContextOptions.Annotations, _ => NullableContextOptions.Disable,
        };
        var opts = new CSharpCompilationOptions(OutputKind.DynamicallyLinkedLibrary, nullableContextOptions: nullable)
            .WithSpecificDiagnosticOptions(noWarn.Select(id => KeyValuePair.Create(id, ReportDiagnostic.Suppress)));
        var provider = new OptionsProvider(props.ToImmutableDictionary(kv => "build_property." + kv.Key, kv => kv.Value));
        var diags = new List<Diagnostic>();

        var libComp = CSharpCompilation.Create("Lib", new[] { CSharpSyntaxTree.ParseText(libSrc, Parse, "Lib.cs") }, refs, opts);
        using var libImage = new MemoryStream();
        var libEmit = libComp.Emit(libImage);
        var generated = new JsonArray();
        CSharpCompilation? outComp = null;
        if (!libEmit.Success) diags.AddRange(libEmit.Diagnostics.Where(d => d.Severity >= DiagnosticSeverity.Warning));
        else
        {
            var libRef = MetadataReference.CreateFromImage(libImage.ToArray());
            var appComp = CSharpCompilation.Create("App", new[] { CSharpSyntaxTree.ParseText(appSrc, Parse, "App.cs") }, refs.Append(libRef), opts);
            GeneratorDriver driver = CSharpGeneratorDriver.Create(gen.Generators, parseOptions: Parse, optionsProvider: provider);
            driver = driver.RunGeneratorsAndUpdateCompilation(appComp, out var oc, out var genDiags);
            outComp = (CSharpCompilation)oc;
            foreach (var r in driver.GetRunResult().Results)
                foreach (var s in r.GeneratedSources)
                    generated.Add(new JsonObject { ["hintName"] = s.HintName, ["text"] = s.SourceText.ToString() });
            diags.AddRange(genDiags);
            if (gen.Analyzers.Length > 0)
            {
                var an = outComp.WithAnalyzers(gen.Analyzers, new AnalyzerOptions(ImmutableArray<AdditionalText>.Empty, provider));
                diags.AddRange(an.GetAnalyzerDiagnosticsAsync().GetAwaiter().GetResult());
            }
            diags.AddRange(outComp.GetDiagnostics().Where(d => d.Severity >= DiagnosticSeverity.Warning));
        }
        var kept = diags.Where(d => !d.IsSuppressed && !noWarn.Contains(d.Id) && d.Severity != DiagnosticSeverity.Hidden)
            .Select(d => (d.Id, sev: Severity(d.Severity), msg: d.GetMessage(), loc: d.Location.IsInSource ? $"{d.Location.GetLineSpan().Path}({d.Location.GetLineSpan().StartLinePosition.Line + 1},{d.Location.GetLineSpan().StartLinePosition.Character + 1})" : null))
            .Distinct().ToList();
        var jd = new JsonArray(kept.Select(d => (JsonNode)new JsonObject { ["id"] = d.Id, ["severity"] = d.sev, ["message"] = d.msg, ["location"] = d.loc }).ToArray());

        JsonNode? rt = null;
        int? pid = null;
        if (roundTrip && outComp != null && !kept.Any(d => d.sev == "error"))
            (rt, pid) = RoundTrip(outComp, libImage.ToArray(), runtime, genOnly ? "generated-and-builtin" : "standard");
        return new JsonObject
        {
            ["diagnostics"] = jd,
            ["generated"] = generated,
            ["roundTrip"] = rt,
            ["milliseconds"] = sw.Elapsed.TotalMilliseconds,
            ["pid"] = pid,
        };
    }

    static readonly Lazy<string> RunnerSource = new(() => RoundTripSource.Text);

    public static string? Which(string name, string envVar)
    {
        var env = Environment.GetEnvironmentVariable(envVar);
        if (!string.IsNullOrEmpty(env) && File.Exists(env)) return env;
        foreach (var d in (Environment.GetEnvironmentVariable("PATH") ?? "").Split(':', StringSplitOptions.RemoveEmptyEntries))
            if (File.Exists(Path.Combine(d, name))) return Path.Combine(d, name);
        return null;
    }

    static JsonObject RtError(string resolver, string sent, string error) =>
        new() { ["resolver"] = resolver, ["sent"] = sent, ["received"] = null, ["wire"] = null, ["error"] = error };

    static (JsonNode, int?) RoundTrip(CSharpCompilation app, byte[] libImage, string runtime, string resolver)
    {
        var comp = app.AddSyntaxTrees(CSharpSyntaxTree.ParseText(RunnerSource.Value, Parse, "WitnessRoundTrip.cs"))
            .WithOptions(app.Options.WithOutputKind(OutputKind.ConsoleApplication).WithMainTypeName("WitnessRoundTrip.Runner"));
        var dir = Directory.CreateTempSubdirectory("witness-run-").FullName;
        try
        {
            using (var fs = File.Create(Path.Combine(dir, "App.dll")))
            {
                var emit = comp.Emit(fs);
                if (!emit.Success)
                    return (RtError(resolver, "", "app does not build: " + string.Join("; ", emit.Diagnostics.Where(d => d.Severity == DiagnosticSeverity.Error).Take(3).Select(d => d.Id + " " + d.GetMessage()))), null);
            }
            File.WriteAllBytes(Path.Combine(dir, "Lib.dll"), libImage);
            foreach (var f in Directory.GetFiles(runtime, "*.dll")) File.Copy(f, Path.Combine(dir, Path.GetFileName(f)), true);
            if (ProjectLoader.Package("microsoft.net.stringtools") is { } st)
            {
                var dll = Directory.GetFiles(st, "Microsoft.NET.StringTools.dll", SearchOption.AllDirectories).OrderBy(f => f.Contains("net8.0") ? 0 : f.Contains("netstandard2.0") ? 1 : 2).FirstOrDefault();
                if (dll != null && !File.Exists(Path.Combine(dir, "Microsoft.NET.StringTools.dll"))) File.Copy(dll, Path.Combine(dir, "Microsoft.NET.StringTools.dll"));
            }
            File.WriteAllText(Path.Combine(dir, "App.runtimeconfig.json"),
                $"{{\"runtimeOptions\":{{\"tfm\":\"net8.0\",\"framework\":{{\"name\":\"Microsoft.NETCore.App\",\"version\":\"{Environment.Version.Major}.{Environment.Version.Minor}.0\"}}}}}}");
            File.WriteAllText(Path.Combine(dir, "passwd"), "u:x:1000:100:u:/tmp:/bin/sh\n");

            var bwrap = Which("bwrap", "WITNESS_BWRAP");
            var dotnet = Which("dotnet", "WITNESS_DOTNET") ?? (Path.GetFileNameWithoutExtension(Environment.ProcessPath) == "dotnet" ? Environment.ProcessPath : null);
            if (bwrap == null) return (RtError(resolver, "", "no bwrap: reviewer code is not executed"), null);
            if (dotnet == null) return (RtError(resolver, "", "no dotnet"), null);
            var real = new FileInfo(dotnet).ResolveLinkTarget(true)?.FullName ?? dotnet;
            var psi = new ProcessStartInfo(bwrap) { RedirectStandardOutput = true, RedirectStandardError = true, UseShellExecute = false };
            foreach (var a in new[] { "--unshare-all", "--die-with-parent", "--new-session", "--clearenv",
                         "--ro-bind", "/nix/store", "/nix/store", "--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp" })
                psi.ArgumentList.Add(a);
            foreach (var extra in new[] { Path.GetDirectoryName(real)!, Path.GetDirectoryName(dotnet)! }.Distinct())
                if (!extra.StartsWith("/nix/store/", StringComparison.Ordinal)) { psi.ArgumentList.Add("--ro-bind"); psi.ArgumentList.Add(extra); psi.ArgumentList.Add(extra); }
            foreach (var a in new[] { "--ro-bind", dir, dir, "--ro-bind", Path.Combine(dir, "passwd"), "/etc/passwd",
                         "--setenv", "HOME", "/tmp", "--setenv", "DOTNET_CLI_HOME", "/tmp", "--setenv", "TMPDIR", "/tmp",
                         "--setenv", "DOTNET_NOLOGO", "1", "--setenv", "DOTNET_CLI_TELEMETRY_OPTOUT", "1", "--setenv", "LANG", "C.UTF-8",
                         "--chdir", dir, dotnet, Path.Combine(dir, "App.dll"), resolver })
                psi.ArgumentList.Add(a);
            using var proc = Process.Start(psi)!;
            var pid = proc.Id;
            var outTask = proc.StandardOutput.ReadToEndAsync();
            var errTask = proc.StandardError.ReadToEndAsync();
            if (!proc.WaitForExit(30_000)) { try { proc.Kill(true); } catch { } return (RtError(resolver, "", "timed out after 30 s"), pid); }
            var stdout = outTask.Result;
            var line = stdout.Split('\n').LastOrDefault(l => l.StartsWith('{'));
            if (line == null) return (RtError(resolver, "", $"exit {proc.ExitCode}: {Trim(errTask.Result + stdout)}"), pid);
            return (JsonNode.Parse(line)!, pid);
        }
        finally { try { Directory.Delete(dir, true); } catch { } }
    }

    static string Trim(string s) { s = s.Trim(); return s.Length > 500 ? s[..500] : s; }
}
