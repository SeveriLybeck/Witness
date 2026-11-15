using System.Diagnostics;
using System.Text;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;
using Microsoft.CodeAnalysis.FlowAnalysis;
using Microsoft.CodeAnalysis.Operations;

// args: <worktree root> <pkgs dir> <level 0..3> <dumpfile> <changed ranges "239,985,1077,1429-1451"> <methods csv> [--gen]
var root = args[0]; var pkgs = args[1]; int level = int.Parse(args[2]); var dumpFile = args[3];
var ranges = args[4].Split(',').Select(s => s.Split('-')).Select(p => (int.Parse(p[0]), int.Parse(p[^1]))).ToList();
var methods = args[5].Split(',').ToHashSet();
bool gen = args.Contains("--gen");
var sw = Stopwatch.StartNew();
var proj = Path.Combine(root, "src/MessagePack.SourceGenerator");
var tcPath = Path.Combine(proj, "CodeAnalysis/TypeCollector.cs");
var parse = new CSharpParseOptions(LanguageVersion.Latest);
IEnumerable<string> files;
if (level == 0) files = new[] { tcPath };
else {
  files = Directory.EnumerateFiles(proj, "*.cs", SearchOption.AllDirectories).Where(f => !f.Contains("/obj/") && !f.Contains("/bin/")).ToList();
  files = files.Concat(new[] { "src/MessagePack/Internal/AutomataKeyGen.cs", "src/MessagePack/SafeBitConverter.cs", "src/MessagePack/MessagePackSerializationException.cs" }.Select(f => Path.Combine(root, f)));
}
var trees = new List<SyntaxTree>();
foreach (var f in files) trees.Add(CSharpSyntaxTree.ParseText(File.ReadAllText(f), parse, f));
if (level >= 2) // implicit usings (as the SDK generates them for ImplicitUsings=enable on netstandard2.0)
  trees.Add(CSharpSyntaxTree.ParseText("global using global::System;global using global::System.Collections.Generic;global using global::System.IO;global using global::System.Linq;global using global::System.Net.Http;global using global::System.Threading;global using global::System.Threading.Tasks;", parse, "GlobalUsings.g.cs"));
if (level >= 4) trees.Add(CSharpSyntaxTree.ParseText(File.ReadAllText(Path.Combine(root, "obj/src/MessagePack.SourceGenerator/Release/netstandard2.0/MessagePack.SourceGenerator.Version.cs")), parse, "Version.g.cs"));
var refs = new List<MetadataReference>();
foreach (var d in Directory.GetFiles(Path.Combine(pkgs, "netstandard.library/2.0.3/build/netstandard2.0/ref"), "*.dll")) refs.Add(MetadataReference.CreateFromFile(d));
foreach (var d in new[] { "microsoft.codeanalysis.common/4.12.0/lib/netstandard2.0/Microsoft.CodeAnalysis.dll", "microsoft.codeanalysis.csharp/4.12.0/lib/netstandard2.0/Microsoft.CodeAnalysis.CSharp.dll",
  "system.collections.immutable/8.0.0/lib/netstandard2.0/System.Collections.Immutable.dll", "system.reflection.metadata/8.0.0/lib/netstandard2.0/System.Reflection.Metadata.dll" })
  refs.Add(MetadataReference.CreateFromFile(Path.Combine(pkgs, d)));
if (level >= 3) foreach (var d in new[] { "system.memory/4.5.5/lib/netstandard2.0/System.Memory.dll", "system.runtime.compilerservices.unsafe/6.0.0/lib/netstandard2.0/System.Runtime.CompilerServices.Unsafe.dll", "system.buffers/4.5.1/lib/netstandard2.0/System.Buffers.dll", "system.numerics.vectors/4.5.0/lib/netstandard2.0/System.Numerics.Vectors.dll", "system.threading.tasks.extensions/4.5.4/lib/netstandard2.0/System.Threading.Tasks.Extensions.dll" })
  refs.Add(MetadataReference.CreateFromFile(Path.Combine(pkgs, d)));
var opts = new CSharpCompilationOptions(OutputKind.DynamicallyLinkedLibrary, nullableContextOptions: NullableContextOptions.Enable, allowUnsafe: true);
var comp = CSharpCompilation.Create("MessagePack.SourceGenerator", trees, refs, opts);
var tCreate = sw.ElapsedMilliseconds; sw.Restart();
long tGen = 0;
if (level >= 3) {
  var asm = System.Reflection.Assembly.LoadFrom(Path.Combine(pkgs, "polysharp/1.15.0/analyzers/dotnet/cs/PolySharp.SourceGenerators.dll"));
  var gens = asm.GetTypes().Where(t => typeof(IIncrementalGenerator).IsAssignableFrom(t) && !t.IsAbstract).Select(t => ((IIncrementalGenerator)Activator.CreateInstance(t)).AsSourceGenerator()).ToList();
  var addl = new List<AdditionalText>();
  var drv = CSharpGeneratorDriver.Create(gens, addl, parse);
  drv.RunGeneratorsAndUpdateCompilation(comp, out var comp2, out var gd);
  Console.WriteLine($"PolySharp generated {comp2.SyntaxTrees.Count() - comp.SyntaxTrees.Count()} trees; gen diags={gd.Length}");
  if (level >= 4) { // stand-in for Microsoft.CodeAnalysis.ResxSourceGenerator: Strings class with one property per resx entry
    var names = System.Xml.Linq.XDocument.Load(Path.Combine(proj, "Strings.resx")).Root.Elements("data").Select(e => (string)e.Attribute("name"));
    var stub = "namespace MessagePack.SourceGenerator { internal static class Strings { public static System.Resources.ResourceManager ResourceManager => null; " + string.Join(" ", names.Select(n => $"public static string {n} => \"\";")) + " } }";
    comp2 = comp2.AddSyntaxTrees(CSharpSyntaxTree.ParseText(stub, parse, "Strings.stub.cs"));
    Console.WriteLine($"resx stub with {names.Count()} entries added");
  }
  comp = (CSharpCompilation)comp2; tGen = sw.ElapsedMilliseconds; sw.Restart();
}

var o = new StringBuilder();
if (gen) { // run PolySharp + resx generators in-process
  // handled by caller extension point: not implemented here
}
var tcTree = trees.First(t => t.FilePath == tcPath);
var sm = comp.GetSemanticModel(tcTree);
var tSm = sw.ElapsedMilliseconds; sw.Restart();
var diags = comp.GetDiagnostics().Where(d => d.Severity == DiagnosticSeverity.Error).ToList();
var tDiag = sw.ElapsedMilliseconds; sw.Restart();
var tcDiags = diags.Where(d => d.Location.SourceTree == tcTree).ToList();
Console.WriteLine($"files={trees.Count} errors total={diags.Count} in TypeCollector.cs={tcDiags.Count}");
foreach (var g in diags.GroupBy(d => d.Id)) Console.WriteLine($"  {g.Key} x{g.Count()}  e.g. {g.First().GetMessage()} @ {Path.GetFileName(g.First().Location.SourceTree?.FilePath)}");
Console.WriteLine($"ms: create compilation={tCreate} polysharp-generators={tGen} getSemanticModel={tSm} full GetDiagnostics={tDiag}");

bool Changed(SyntaxNode n) { var ls = n.GetLocation().GetLineSpan(); int a = ls.StartLinePosition.Line + 1, b = ls.EndLinePosition.Line + 1; return ranges.Any(r => a <= r.Item2 && b >= r.Item1 && a >= r.Item1 - 0 && b <= r.Item2 + 0) || ranges.Any(r => a >= r.Item1 && a <= r.Item2); }
bool ChangedStrict(SyntaxNode n) { int a = n.GetLocation().GetLineSpan().StartLinePosition.Line + 1; return ranges.Any(r => a >= r.Item1 && a <= r.Item2); }
string Snip(IOperation op) { var s = op.Syntax?.ToString().Replace("\r", "").Replace("\n", " ") ?? ""; s = System.Text.RegularExpressions.Regex.Replace(s, @"\s+", " "); return s.Length > 90 ? s[..90] + "..." : s; }
void DumpOp(IOperation op, int ind, StringBuilder sb, int maxDepth = 6) {
  sb.Append(new string(' ', ind * 2)).Append(op.Kind).Append(op.Type != null ? $" : {op.Type.ToDisplayString(SymbolDisplayFormat.MinimallyQualifiedFormat)}" : "");
  if (op is IInvocationOperation inv) sb.Append($" [{inv.TargetMethod.Name}]");
  if (op is IFlowCaptureOperation fc) sb.Append($" [cap#{fc.Id.GetHashCode() & 0xff}]");
  if (op is IFlowCaptureReferenceOperation fr) sb.Append($" [ref#{fr.Id.GetHashCode() & 0xff}]");
  if (op is IBinaryOperation bo) sb.Append($" [{bo.OperatorKind}]");
  if (op is ILocalReferenceOperation lr) sb.Append($" [{lr.Local.Name}]");
  if (op is IParameterReferenceOperation pr) sb.Append($" [{pr.Parameter.Name}]");
  if (op is IPropertyReferenceOperation prp) sb.Append($" [{prp.Property.Name}]");
  if (op is IFieldReferenceOperation fld) sb.Append($" [{fld.Field.Name}]");
  if (op.Kind is OperationKind.None or OperationKind.Invalid) sb.Append($"  <<{op.Syntax?.Kind()}>> `{Snip(op)}`");
  else if (!op.ChildOperations.Any()) sb.Append($"  `{Snip(op)}`");
  sb.AppendLine();
  if (ind < maxDepth) foreach (var c in op.ChildOperations) DumpOp(c, ind + 1, sb, maxDepth);
}
var kindHist = new SortedDictionary<string, int>(); var regionHist = new SortedDictionary<string, int>();
var report = new StringBuilder(); var dump = new StringBuilder();
foreach (var m in tcTree.GetRoot().DescendantNodes().OfType<BaseMethodDeclarationSyntax>().Where(m => (m is MethodDeclarationSyntax md ? md.Identifier.Text : ".ctor") is var name && methods.Contains(name) && (m is not ConstructorDeclarationSyntax || methods.Contains(".ctor")))) {
  var name = m is MethodDeclarationSyntax md2 ? md2.Identifier.Text : ".ctor";
  var op = sm.GetOperation(m);
  var swm = Stopwatch.StartNew();
  long mem0 = GC.GetTotalMemory(true);
  var cfg = (op is IConstructorBodyOperation cb ? ControlFlowGraph.Create(cb) : ControlFlowGraph.Create((IMethodBodyOperation)op));
  var tCfg = swm.ElapsedMilliseconds;
  var all = new List<(ControlFlowGraph g, string tag)> { (cfg, name) };
  for (int i = 0; i < all.Count; i++) { // nested lambdas and local functions
    var g = all[i].g;
    foreach (var lf in g.LocalFunctions) all.Add((g.GetLocalFunctionControlFlowGraph(lf), $"{all[i].tag}/local:{lf.Name}"));
    foreach (var b in g.Blocks) foreach (var top in b.Operations.Concat(b.BranchValue != null ? new[] { b.BranchValue } : new IOperation[0])) foreach (var d in top.DescendantsAndSelf().OfType<IFlowAnonymousFunctionOperation>())
      all.Add((g.GetAnonymousFunctionControlFlowGraph(d), $"{all[i].tag}/lambda@{d.Syntax.GetLocation().GetLineSpan().StartLinePosition.Line + 1}"));
  }
  int nBlocks = 0, nOps = 0, noneInv = 0, noneInvChanged = 0, chBlocks = 0, chOps = 0;
  var hist = new SortedDictionary<string, int>(); var reg = new SortedDictionary<string, int>();
  var noneList = new List<string>();
  void Walk(ControlFlowRegion r) { reg[r.Kind.ToString()] = reg.GetValueOrDefault(r.Kind.ToString()) + 1; foreach (var c in r.NestedRegions) Walk(c); }
  foreach (var (g, tag) in all) {
    Walk(g.Root);
    foreach (var b in g.Blocks) {
      nBlocks++;
      var tops = b.Operations.Concat(b.BranchValue != null ? new[] { b.BranchValue } : new IOperation[0]).ToList();
      bool blockChanged = tops.Any(t => t.DescendantsAndSelf().Any(x => x.Syntax != null && ChangedStrict(x.Syntax)));
      if (blockChanged) chBlocks++;
      foreach (var top in tops) foreach (var x in top.DescendantsAndSelf()) {
        nOps++; var k = x.Kind.ToString(); hist[k] = hist.GetValueOrDefault(k) + 1;
        if (x.Kind is OperationKind.None or OperationKind.Invalid) {
          noneInv++; if (blockChanged) noneInvChanged++;
          noneList.Add($"{tag} B{b.Ordinal}{(blockChanged ? "*" : "")} {x.Kind} {x.Syntax?.Kind()} L{x.Syntax?.GetLocation().GetLineSpan().StartLinePosition.Line + 1} `{Snip(x)}`");
        }
        if (x.Syntax != null && ChangedStrict(x.Syntax)) chOps++;
      }
    }
  }
  report.AppendLine($"== {name} (line {m.GetLocation().GetLineSpan().StartLinePosition.Line + 1}, {m.GetLocation().GetLineSpan().EndLinePosition.Line - m.GetLocation().GetLineSpan().StartLinePosition.Line + 1} lines)  CFG ms={tCfg}  graphs(incl. lambdas/local fns)={all.Count} blocks={nBlocks} ops={nOps} None/Invalid={noneInv} (in changed blocks={noneInvChanged}) changedBlocks={chBlocks} opsOnChangedLines={chOps}");
  report.AppendLine("   regions: " + string.Join(", ", reg.Select(kv => $"{kv.Key}={kv.Value}")));
  report.AppendLine("   kinds: " + string.Join(", ", hist.OrderByDescending(kv => kv.Value).Select(kv => $"{kv.Key}={kv.Value}")));
  foreach (var s in noneList.Take(25)) report.AppendLine("   NONE/INVALID " + s);
  if (noneList.Count > 25) report.AppendLine($"   ... {noneList.Count - 25} more");
  // dump changed blocks
  dump.AppendLine($"################ {name}");
  foreach (var (g, tag) in all) foreach (var b in g.Blocks) {
    var tops = b.Operations.Concat(b.BranchValue != null ? new[] { b.BranchValue } : new IOperation[0]).ToList();
    if (!tops.Any(t => t.DescendantsAndSelf().Any(x => x.Syntax != null && ChangedStrict(x.Syntax)))) continue;
    dump.AppendLine($"--- [{tag}] Block B{b.Ordinal} kind={b.Kind} reachable={b.IsReachable} region={b.EnclosingRegion.Kind}");
    foreach (var t in b.Operations) DumpOp(t, 1, dump);
    if (b.BranchValue != null) { dump.AppendLine($"  BRANCHVALUE:"); DumpOp(b.BranchValue, 2, dump); }
    var cs = b.ConditionalSuccessor; var fs = b.FallThroughSuccessor;
    dump.AppendLine($"  cond={b.ConditionKind}  -> conditional: {(cs == null ? "-" : $"{cs.Semantics} B{cs.Destination?.Ordinal}")}  fallthrough: {(fs == null ? "-" : $"{fs.Semantics} B{fs.Destination?.Ordinal}")}");
  }
  foreach (var kv in hist) kindHist[kv.Key] = kindHist.GetValueOrDefault(kv.Key) + kv.Value;
  foreach (var kv in reg) regionHist[kv.Key] = regionHist.GetValueOrDefault(kv.Key) + kv.Value;
}
Console.WriteLine(report);
File.WriteAllText(dumpFile, dump.ToString());
Console.WriteLine($"peakWS MB={Process.GetCurrentProcess().PeakWorkingSet64 / 1048576} managed MB={GC.GetTotalMemory(false) / 1048576}");
