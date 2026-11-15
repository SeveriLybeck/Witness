using System.Diagnostics;
using System.Text.Json.Nodes;
using Json.Schema;
using Witness.Worker;
using Xunit;
using Xunit.Abstractions;

namespace Witness.Worker.Tests;

/// Paths prepared by worker/setup.sh, passed in by worker/verify.sh.
static class Env
{
    public static readonly string Root = Environment.GetEnvironmentVariable("WITNESS_SB") ?? throw new InvalidOperationException("WITNESS_SB not set");
    public static string Wt(string v) => Path.Combine(Root, "wt", v);
    public static string Commit(string v) => File.ReadAllText(Path.Combine(Root, "wt", v + ".commit")).Trim();
    public static JsonObject Checkout(string v) => new() { ["worktree"] = Wt(v), ["commit"] = Commit(v) };
    public const string TypeCollector = "src/MessagePack.SourceGenerator/CodeAnalysis/TypeCollector.cs";
    public static string Contracts => Path.Combine(Root, "contracts");

    static readonly Lazy<JsonSchema> ReplySchema = new(() =>
    {
        var dir = Path.Combine(Contracts, "messages");
        // verify.sh copies common.schema.json beside the message schemas, which reference it as a sibling
        SchemaRegistry.Global.Register(JsonSchema.FromFile(Path.Combine(dir, "common.schema.json")));
        return JsonSchema.FromFile(Path.Combine(dir, "worker-reply.schema.json"));
    });

    public static void AssertValid(JsonNode reply)
    {
        var r = ReplySchema.Value.Evaluate(reply, new EvaluationOptions { OutputFormat = OutputFormat.List });
        if (!r.IsValid)
        {
            var errs = string.Join("\n", (r.Details ?? new List<EvaluationResults>()).Where(d => d.HasErrors).Take(10)
                .Select(d => $"{d.InstanceLocation}: {string.Join("; ", d.Errors!.Values)}"));
            Assert.Fail("reply does not validate against worker-reply.schema.json:\n" + errs);
        }
    }

    public static JsonObject Call(int id, string method, JsonObject p)
    {
        var reply = Worker.Handle(new JsonObject { ["id"] = id, ["method"] = method, ["params"] = p });
        Assert.True(reply["error"] == null, reply["error"]?.ToJsonString());
        return reply;
    }
}

public class SandboxTests
{
    [Fact] public void Empty() { }

    [Fact]
    public void FixturesValidate()
    {
        foreach (var f in Directory.GetFiles(Path.Combine(Env.Contracts, "fixtures/2283/inputs"), "worker-reply.*.json"))
            Env.AssertValid(JsonNode.Parse(File.ReadAllText(f))!);
    }

    [Fact]
    public void NoNetwork()
    {
        using var c = new System.Net.Sockets.TcpClient();
        Assert.ThrowsAny<Exception>(() => c.Connect("1.1.1.1", 443));
    }
}

public class BindingTests(ITestOutputHelper log)
{
    [Theory]
    [InlineData("old")]
    [InlineData("h2283")]
    [InlineData("h2234")]
    public void TypeCollectorBinds(string v)
    {
        var proj = ProjectLoader.ForFile(Env.Wt(v), Env.TypeCollector);
        var tree = proj.Tree(Env.TypeCollector)!;
        Assert.NotNull(tree);
        var errors = proj.Errors(tree);
        var all = proj.Compilation.GetDiagnostics().Count(d => d.Severity == Microsoft.CodeAnalysis.DiagnosticSeverity.Error);
        log.WriteLine($"{v}: {proj.Compilation.SyntaxTrees.Count()} trees, errors in TypeCollector.cs {errors.Count}, in project {all}; problems: {string.Join("; ", proj.Problems)}");
        Assert.Empty(errors.Select(e => e.ToString()));
        Assert.Equal(0, all);
        Assert.Empty(proj.Problems);
    }
}

public class CfgTests(ITestOutputHelper log)
{
    static int CountOps(JsonNode? op) => op == null ? 0 : 1 + (op["children"]?.AsArray().Sum(CountOps) ?? 0);

    static (int blocks, int ops, int graphs) Count(JsonNode g)
    {
        var blocks = g["blocks"]!.AsArray();
        int b = blocks.Count, o = blocks.Sum(x => x!["operations"]!.AsArray().Sum(CountOps) + CountOps(x["branch"])), n = 1;
        foreach (var l in g["lambdas"]?.AsArray() ?? new JsonArray()) { var (b2, o2, n2) = Count(l!); b += b2; o += o2; n += n2; }
        return (b, o, n);
    }

    [Theory]
    [InlineData("old")]
    [InlineData("h2283")]
    [InlineData("h2234")]
    public void GetObjectInfo(string v)
    {
        var sw = Stopwatch.StartNew();
        var reply = Env.Call(1, "cfg", new JsonObject
        {
            ["checkout"] = Env.Checkout(v),
            ["methods"] = new JsonArray(new JsonObject { ["file"] = Env.TypeCollector, ["name"] = "TypeCollector.GetObjectInfo" }),
        });
        var result = reply["result"]!;
        var methods = result["methods"]!.AsArray();
        Assert.Single(methods);
        var (blocks, ops, graphs) = Count(methods[0]!);
        log.WriteLine($"{v}: {blocks} blocks, {ops} operations, {graphs} graphs, {sw.ElapsedMilliseconds} ms, {reply.ToJsonString().Length / 1024} KiB");
        Assert.Empty(result["errors"]!.AsArray().Select(e => (string)e!));
        Assert.True(blocks > 600 && ops > 3000 && graphs > 40);
        Env.AssertValid(reply);
    }

    static IEnumerable<JsonNode> Ops(JsonNode? n)
    {
        if (n is JsonObject o)
        {
            if (o["kind"] is JsonValue && o["syntax"] != null) yield return o;
            foreach (var kv in o) foreach (var x in Ops(kv.Value)) yield return x;
        }
        else if (n is JsonArray a) foreach (var e in a) foreach (var x in Ops(e)) yield return x;
    }

    [Fact]
    public void DiagnosticIdsOnDescriptorFields()
    {
        var reply = Env.Call(3, "cfg", new JsonObject
        {
            ["checkout"] = Env.Checkout("h2283"),
            ["methods"] = new JsonArray(new JsonObject { ["file"] = Env.TypeCollector, ["name"] = "TypeCollector.GetObjectInfo" }),
        });
        Env.AssertValid(reply);
        var ids = Ops(reply["result"]).Where(o => (string?)o["kind"] == "FieldReference" && o["diagnosticId"] != null)
            .GroupBy(o => (string)o["symbol"]!).ToDictionary(g => g.Key.Split('.').Last(), g => (string)g.First()["diagnosticId"]!);
        // MsgPack00xMessagePackAnalyzer.cs: BothStringAndIntKeyAreNull uses InvalidMessagePackObjectId = "MsgPack005"; MemberNeedsKey uses "MsgPack004"
        Assert.Equal("MsgPack005", ids["BothStringAndIntKeyAreNull"]);
        Assert.Equal("MsgPack004", ids["MemberNeedsKey"]);
        Assert.Equal("MsgPack005", ids["KeysMustBeUnique"]);
        Assert.DoesNotContain(Ops(reply["result"]), o => (string?)o["kind"] == "FieldReference" && o["diagnosticId"] != null && (string?)o["type"] != "Microsoft.CodeAnalysis.DiagnosticDescriptor");
    }

    static JsonNode Cfg(string v, string file, string name) => Env.Call(20, "cfg", new JsonObject
    {
        ["checkout"] = Env.Checkout(v),
        ["methods"] = new JsonArray(new JsonObject { ["file"] = file, ["name"] = name }),
    });

    [Fact]
    public void ThrownTypeOf2129GetMemberType()
    {
        var reply = Cfg("h2129", "src/MessagePack.SourceGenerator/CodeAnalysis/GeneratedFormatterCollector.cs", "GeneratedFormatterCollector.GetMemberType");
        Env.AssertValid(reply);
        var blocks = reply["result"]!["methods"]![0]!["blocks"]!.AsArray();
        var thrown = blocks.Where(b => (bool?)b!["throws"] == true).Select(b => (string?)b!["thrownType"]).ToList();
        log.WriteLine(string.Join(", ", thrown));
        // the `_ =>` arm, plus the compiler's own no-arm-matched throw of the switch expression
        Assert.Equal("System.ArgumentOutOfRangeException", thrown[0]);
        Assert.All(thrown, t => Assert.NotNull(t));
    }

    [Fact]
    public void LambdaHashIgnoresPosition()
    {
        static Dictionary<string, string> Hashes(JsonNode reply) => Ops(reply["result"])
            .Where(o => o["graphHash"] != null).GroupBy(o => (string)o["graph"]!).ToDictionary(g => g.Key, g => (string)g.First()["graphHash"]!);
        var old = Hashes(Cfg("old", Env.TypeCollector, "TypeCollector.GetObjectInfo"));
        var neu = Hashes(Cfg("h2234", Env.TypeCollector, "TypeCollector.GetObjectInfo"));
        log.WriteLine($"lambdas old {old.Count}, 2234 {neu.Count}; names in common {old.Keys.Intersect(neu.Keys).Count()}; hashes in common {old.Values.Intersect(neu.Values).Count()}");
        Assert.True(old.Keys.Except(neu.Keys).Any(), "some lambda moved");
        Assert.True(old.Values.Distinct().Count(h => neu.Values.Contains(h)) >= old.Keys.Intersect(neu.Keys).Count());
        // the graph carries the same hash as the leaf that points at it
        var reply = Cfg("h2283", Env.TypeCollector, "TypeCollector..ctor");
        var lambda = reply["result"]!["methods"]![0]!["lambdas"]![0]!;
        Assert.Equal((string)lambda["hash"]!, Ops(reply["result"]).First(o => (string?)o["graph"] == (string)lambda["name"]!)["graphHash"]!.GetValue<string>());
    }

    [Fact]
    public void PositionalRecordProperty()
    {
        var reply = Cfg("h2234", Env.TypeCollector, "TypeCollector.GetObjectInfo");
        var intKey = Ops(reply["result"]).Where(o => (string?)o["kind"] == "PropertyReference" && ((string?)o["symbol"])?.EndsWith("MemberSerializationInfo.IntKey") == true).ToList();
        Assert.NotEmpty(intKey);
        var src = File.ReadAllLines(Path.Combine(Env.Wt("h2234"), "src/MessagePack.SourceGenerator/CodeAnalysis/MemberSerializationInfo.cs"));
        var parms = string.Join(" ", src).Split("record MemberSerializationInfo(")[1].Split(')')[0].Split(',').Select(p => p.Trim().Split(' ').Last()).ToList();
        Assert.All(intKey, o => Assert.Equal(parms.IndexOf("IntKey"), (int)o["positionalParameter"]!));
        // an ordinary property is never marked
        Assert.DoesNotContain(Ops(reply["result"]), o => o["positionalParameter"] != null && ((string?)o["symbol"])?.EndsWith(".DeclaringSyntaxReferences") == true);
    }

    [Fact]
    public void MaybeNullStringLocal()
    {
        var reply = Cfg("old", Env.TypeCollector, "TypeCollector.GetObjectInfo");
        var g = reply["result"]!["methods"]![0]!;
        var names = g["maybeNullLocals"]!.AsArray().Select(x => (string)x!).ToList();
        log.WriteLine(string.Join(", ", names));
        Assert.Contains("stringKey", names);
        Assert.DoesNotContain("intKey", names);
        Assert.Contains(Ops(reply["result"]), o => (string?)o["kind"] == "LocalReference" && (string?)o["name"] == "stringKey" && (bool?)o["maybeNull"] == true);
    }

    [Fact]
    public void LambdaIsSeparateGraph()
    {
        var reply = Env.Call(2, "cfg", new JsonObject
        {
            ["checkout"] = Env.Checkout("h2283"),
            ["methods"] = new JsonArray(new JsonObject { ["file"] = Env.TypeCollector, ["name"] = "TypeCollector..ctor" },
                new JsonObject { ["file"] = Env.TypeCollector, ["name"] = "TypeCollector.GetPropertyIdentifierLocation" }),
        });
        Env.AssertValid(reply);
        var methods = reply["result"]!["methods"]!.AsArray();
        var ctor = methods.First(m => ((string)m!["name"]!).EndsWith(".ctor"))!;
        var lambda = ctor["lambdas"]!.AsArray().Single(l => ((string)l!["name"]!).Contains("lambda@239"))!;
        Assert.Contains("ReportIfLocatedInCompilation", lambda.ToJsonString());
        var helper = methods.First(m => ((string)m!["name"]!).EndsWith("GetPropertyIdentifierLocation"))!;
        Assert.Equal("Microsoft.CodeAnalysis.IPropertySymbol", (string)helper["parameters"]![0]!["type"]!);
        Assert.Contains("\"kind\":\"IsNull\"", helper.ToJsonString());
    }
}

public class LinkTests(ITestOutputHelper log)
{
    static JsonObject L(int a, int b) => new() { ["file"] = Env.TypeCollector, ["line"] = a, ["endLine"] = b };

    [Fact]
    public void References2283()
    {
        var reply = Env.Call(2, "references", new JsonObject
        {
            ["checkout"] = Env.Checkout("h2283"),
            ["changedMembers"] = new JsonArray("GetPropertyIdentifierLocation", "ReportIfLocatedInCompilation"),
            ["parts"] = new JsonArray(new JsonObject { ["id"] = "p1", ["location"] = L(239, 239) }, new JsonObject { ["id"] = "p2", ["location"] = L(985, 985) },
                new JsonObject { ["id"] = "p3", ["location"] = L(1077, 1077) }, new JsonObject { ["id"] = "p4", ["location"] = L(1429, 1435) },
                new JsonObject { ["id"] = "p5", ["location"] = L(1436, 1451) }),
        });
        Env.AssertValid(reply);
        log.WriteLine(reply.ToJsonString());
        var parts = reply["result"]!["parts"]!.AsArray().ToDictionary(p => (string)p!["id"]!, p => p!["references"]!.AsArray().Select(x => (string)x!).ToList());
        Assert.Equal(new[] { "ReportIfLocatedInCompilation" }, parts["p1"]);
        Assert.Equal(new[] { "GetPropertyIdentifierLocation" }, parts["p2"]);
        Assert.Empty(parts["p3"]);
        Assert.Empty(parts["p4"]);
        Assert.Empty(parts["p5"]);
    }

    [Fact]
    public void Dependencies2283()
    {
        var reply = Env.Call(3, "dependencies", new JsonObject
        {
            ["old"] = Env.Checkout("old"),
            ["new"] = Env.Checkout("h2283"),
            ["blocks"] = new JsonArray(
                new JsonObject { ["id"] = "b1", ["old"] = new JsonArray(L(239, 239)), ["new"] = new JsonArray(L(239, 239), L(1436, 1451)) },
                new JsonObject { ["id"] = "b2", ["old"] = new JsonArray(L(985, 985)), ["new"] = new JsonArray(L(985, 985), L(1429, 1435)) },
                new JsonObject { ["id"] = "b3", ["old"] = new JsonArray(L(1077, 1077)), ["new"] = new JsonArray(L(1077, 1077)) }),
        });
        Env.AssertValid(reply);
        log.WriteLine(reply.ToJsonString());
        var r = reply["result"]!;
        var reach = r["reach"]!.AsArray().ToDictionary(x => (string)x!["block"]!, x => (int)x!["callSites"]!);
        Assert.Equal(28, reach["b1"]);
        var deps = r["dependencies"]!.AsArray();
        Assert.Contains(deps, d => (string)d!["from"]! == "b1" && (string)d["to"]! == "b2" && (bool)d["changed"]! && d["sites"]!.ToJsonString().Contains("\"line\":985"));
        Assert.Contains(deps, d => (string)d!["from"]! == "b1" && (string)d["to"]! == "b3" && (bool)d["changed"]! && d["sites"]!.ToJsonString().Contains("\"line\":1080"));
        Assert.DoesNotContain(deps, d => (string)d!["to"]! == "b1");
    }

    [Fact]
    public void Facts2283()
    {
        var reply = Env.Call(4, "facts", new JsonObject { ["old"] = Env.Checkout("old"), ["new"] = Env.Checkout("h2283") });
        Env.AssertValid(reply);
        log.WriteLine(reply.ToJsonString());
        var facts = reply["result"]!["facts"]!.AsArray();
        Assert.Contains(facts, f => (string)f!["kind"]! == "changed-wiring" && ((string)f["symbol"]!).EndsWith("TypeCollector.reportDiagnostic") && ((string)f["new"]!).Contains("ReportIfLocatedInCompilation"));
        Assert.DoesNotContain(facts, f => (string)f!["kind"]! is "public-type" or "public-member" or "new-diagnostic" or "new-project");
    }

    [Fact]
    public void Facts2129()
    {
        var reply = Env.Call(6, "facts", new JsonObject { ["old"] = Env.Checkout("old"), ["new"] = Env.Checkout("h2129") });
        Env.AssertValid(reply);
        var facts = reply["result"]!["facts"]!.AsArray().Select(f => f!.AsObject()).ToList();
        foreach (var g in facts.GroupBy(f => (string)f["kind"]!).OrderBy(g => g.Key)) log.WriteLine($"2129 {g.Key}: {g.Count()}");
        foreach (var f in facts) log.WriteLine(f.ToJsonString());
        var diags = facts.Where(f => (string)f["kind"]! == "new-diagnostic").ToList();
        Assert.Equal(4, diags.Count);
        Assert.All(diags, d => Assert.Matches("^MsgPack\\d+$", (string)d["diagnosticId"]!));
        Assert.Equal(4, diags.Select(d => (string)d["diagnosticId"]!).Distinct().Count());
        // every public member that belongs to a new diagnostic is marked with it
        foreach (var d in diags)
            Assert.Equal(2, facts.Count(f => (string)f["kind"]! == "public-member" && (string?)f["diagnostic"] == (string)d["diagnosticId"]!));
        // a new type spans its declaration, and its members name it as container and lie inside it
        var type = facts.First(f => (string)f["kind"]! == "public-type");
        var loc = type["location"]!;
        Assert.True((int)loc["endLine"]! > (int)loc["line"]!);
        var members = facts.Where(f => (string?)f["container"] == (string)type["symbol"]!).ToList();
        Assert.NotEmpty(members);
        Assert.All(members, m => Assert.True((string)m["location"]!["file"]! == (string)loc["file"]! && (int)m["location"]!["line"]! >= (int)loc["line"]! && (int)m["location"]!["endLine"]! <= (int)loc["endLine"]!));
        Assert.Contains(facts, f => (string)f["kind"]! == "new-file" && ((string)f["symbol"]!).StartsWith("src/") && ((string)f["symbol"]!).Contains("Collector"));
        Assert.Contains(facts, f => (string)f["kind"]! == "new-project" && ((string)f["symbol"]!).StartsWith("tests/"));
    }

    [Fact]
    public void Facts2234()
    {
        var reply = Env.Call(5, "facts", new JsonObject { ["old"] = Env.Checkout("old"), ["new"] = Env.Checkout("h2234") });
        Env.AssertValid(reply);
        log.WriteLine(reply.ToJsonString());
    }
}

public class LabTests(ITestOutputHelper log)
{
    const string Lib = "using MessagePack;\nnamespace Lib;\n[MessagePackObject]\npublic struct S<T>\n{\n    [Key(0)] public string Key { get; set; }\n    public int Unkeyed { get; set; }\n}\n";
    const string App = "using MessagePack; using Lib;\nnamespace App;\n[MessagePackObject]\npublic sealed class Holder { [Key(0)] public S<int> Member { get; set; } }\npublic static class Probe { public static object Make() => new Holder { Member = new S<int> { Key = \"k\", Unkeyed = 42 } }; }\n";

    static JsonObject Side(string v) => new() { ["generator"] = Path.Combine(Env.Wt(v), "bin/MessagePack.SourceGenerator/Release/netstandard2.0/MessagePack.SourceGenerator.dll"), ["runtime"] = Path.Combine(Env.Wt(v), "bin/MessagePack/Release/net8.0") };

    static JsonObject Req(bool roundTrip, string lib = Lib, string app = App, string noWarn = "") => new()
    {
        ["old"] = Side("old"), ["new"] = Side("h2283"), ["reverted"] = null,
        ["library"] = lib, ["app"] = app, ["buildProperties"] = new JsonObject { ["NoWarn"] = noWarn },
        ["roundTrip"] = roundTrip, ["generatedResolverOnly"] = true,
    };

    static List<string> Ids(JsonNode side) => side["diagnostics"]!.AsArray().Select(d => (string)d!["id"]!).ToList();

    [Fact]
    public void UnkeyedMember2283()
    {
        var reply = Env.Call(5, "run", Req(true));
        Env.AssertValid(reply);
        var r = reply["result"]!;
        log.WriteLine("new roundTrip " + r["new"]!["roundTrip"]?.ToJsonString());
        log.WriteLine("old diagnostics " + string.Join(", ", Ids(r["old"]!)) + "; new diagnostics " + string.Join(", ", Ids(r["new"]!)));
        Assert.Contains("MsgPack004", Ids(r["old"]!));
        Assert.Empty(Ids(r["new"]!));
        var rt = r["new"]!["roundTrip"]!;
        Assert.Null((string?)rt["error"]);
        Assert.Equal("generated-and-builtin", (string)rt["resolver"]!);
        Assert.Equal("[[\"k\"]]", (string)rt["wire"]!);
        Assert.Contains("\"Unkeyed\":42", (string)rt["sent"]!);
        Assert.Contains("\"Unkeyed\":0", (string)rt["received"]!);
        Assert.Null(r["old"]!["roundTrip"]);
        var pid = (int?)r["new"]!["pid"];
        Assert.NotNull(pid);
        Assert.NotEqual((int)r["workerPid"]!, pid);
        Assert.Equal(Environment.ProcessId, (int)r["workerPid"]!);
    }

    const string ClosedLib = "using MessagePack;\nnamespace Lib;\n[MessagePackObject]\npublic struct Item<T>\n{\n    [Key(0)] public string Key { get; set; }\n}\n";
    const string ClosedApp = "using MessagePack; using Lib;\nnamespace App;\n[MessagePackObject]\npublic sealed class Cmd { [Key(0)] public string Name { get; set; } [Key(1)] public Item<int> Member { get; set; } }\npublic static class Probe { public static object Make() => new Cmd { Name = \"s\", Member = new Item<int> { Key = \"k\" } }; }\n";

    [Fact]
    public void ClosedGenericStruct2283()
    {
        var reply = Env.Call(6, "run", Req(true, ClosedLib, ClosedApp));
        Env.AssertValid(reply);
        var r = reply["result"]!;
        log.WriteLine("new roundTrip " + r["new"]!["roundTrip"]?.ToJsonString());
        log.WriteLine("old diagnostics " + string.Join(", ", Ids(r["old"]!)) + "; new diagnostics " + string.Join(", ", Ids(r["new"]!)));
        Assert.Contains("CS8785", Ids(r["old"]!));
        Assert.Contains("AD0001", Ids(r["old"]!));
        Assert.Empty(Ids(r["new"]!));
        var rt = r["new"]!["roundTrip"]!;
        log.WriteLine(rt.ToJsonString());
        Assert.Null((string?)rt["error"]);
        Assert.Equal((string)rt["sent"]!, (string)rt["received"]!);
    }

    [Fact]
    public void GenerateOnlyIsFastWhenWarm()
    {
        Env.Call(7, "run", Req(false));
        var sw = Stopwatch.StartNew();
        var reply = Env.Call(8, "run", Req(false));
        var ms = sw.ElapsedMilliseconds;
        Env.AssertValid(reply);
        log.WriteLine($"warm generate-only, both sides: {ms} ms; old {(double)reply["result"]!["old"]!["milliseconds"]!:F0} ms, new {(double)reply["result"]!["new"]!["milliseconds"]!:F0} ms");
        Assert.Null(reply["result"]!["new"]!["pid"]?.GetValue<int?>());
        Assert.True(ms < 1000, $"{ms} ms");
    }

    [Fact]
    public void MissingFormatterReported()
    {
        const string app = "using MessagePack;\nnamespace App;\n[MessagePackObject] public sealed class Known { [Key(0)] public int X { get; set; } }\npublic sealed class Plain { public int X { get; set; } }\npublic static class Probe { public static object Make() => new Plain { X = 1 }; }\n";
        var reply = Env.Call(10, "run", Req(true, "namespace Lib; public static class Marker { }", app));
        var rt = reply["result"]!["new"]!["roundTrip"]!;
        log.WriteLine(rt.ToJsonString());
        Assert.Contains("not registered", (string)rt["error"]!);
        Assert.Null((string?)rt["received"]);
    }

    [Fact]
    public void NoWarnApplies()
    {
        var reply = Env.Call(9, "run", Req(false, noWarn: "MsgPack004"));
        Assert.DoesNotContain("MsgPack004", Ids(reply["result"]!["old"]!));
    }
}

public class ProtocolTests
{
    static JsonNode Line(JsonObject req) => JsonNode.Parse(Worker.HandleLine(req.ToJsonString()))!;

    [Fact]
    public void Cfg2297TransformUtilities()
    {
        var reply = Line(new JsonObject
        {
            ["id"] = 11, ["method"] = "cfg",
            ["params"] = new JsonObject
            {
                ["checkout"] = Env.Checkout("old"),
                ["methods"] = new JsonArray(new JsonObject { ["file"] = "src/MessagePack.SourceGenerator/Transforms/TransformUtilities.cs", ["name"] = "TransformUtilities.EmitNestingTypesAndNamespaces" }),
            },
        });
        Assert.True(reply["error"] == null, reply["error"]?.ToJsonString());
        Assert.Equal(11, (int)reply["id"]!);
        Assert.Equal(3, reply["result"]!["methods"]!.AsArray().Count);
        Env.AssertValid(reply);
    }

    [Theory]
    [InlineData("{\"id\":12,\"method\":\"cfg\",\"params\":{\"checkout\":{\"worktree\":\"/nonexistent\",\"commit\":\"0000000000000000000000000000000000000000\"},\"methods\":[{\"file\":\"x.cs\",\"name\":\"A.B\"}]}}")]
    [InlineData("{\"id\":12,\"method\":\"facts\",\"params\":{\"old\":42}}")]
    [InlineData("{\"id\":12,\"method\":\"run\",\"params\":{}}")]
    [InlineData("{\"id\":12,\"method\":\"nosuch\",\"params\":{}}")]
    public void FailingRequestGivesErrorReply(string line)
    {
        var reply = JsonNode.Parse(Worker.HandleLine(line))!;
        Assert.Equal(12, (int)reply["id"]!);
        Assert.NotNull(reply["error"]);
        Env.AssertValid(reply);
    }

    [Fact]
    public void WrongCommitRejected()
    {
        var co = Env.Checkout("h2283");
        co["commit"] = Env.Commit("old");
        var reply = Line(new JsonObject { ["id"] = 13, ["method"] = "facts", ["params"] = new JsonObject { ["old"] = Env.Checkout("old"), ["new"] = co } });
        Assert.Equal(13, (int)reply["id"]!);
        Assert.Contains("is at " + Env.Commit("h2283"), (string)reply["error"]!["message"]!);
        Env.AssertValid(reply);
    }

    [Fact]
    public void OneProcessManyLines()
    {
        var dll = typeof(Worker).Assembly.Location;
        var dotnet = Lab.Which("dotnet", "WITNESS_DOTNET")!;
        var psi = new ProcessStartInfo(dotnet) { RedirectStandardInput = true, RedirectStandardOutput = true, UseShellExecute = false };
        psi.ArgumentList.Add(dll);
        using var p = Process.Start(psi)!;
        var req = new JsonObject { ["id"] = 1, ["method"] = "facts", ["params"] = new JsonObject { ["old"] = Env.Checkout("old"), ["new"] = Env.Checkout("h2283") } };
        p.StandardInput.WriteLine(req.ToJsonString());
        req["id"] = 2;
        p.StandardInput.WriteLine(req.ToJsonString());
        p.StandardInput.Close();
        var a = JsonNode.Parse(p.StandardOutput.ReadLine()!)!;
        var b = JsonNode.Parse(p.StandardOutput.ReadLine()!)!;
        Assert.Null(p.StandardOutput.ReadLine());
        p.WaitForExit();
        Assert.Equal(1, (int)a["id"]!);
        Assert.Equal(2, (int)b["id"]!);
        Env.AssertValid(a);
        Assert.Equal(a["result"]!.ToJsonString(), b["result"]!.ToJsonString());
    }
}
