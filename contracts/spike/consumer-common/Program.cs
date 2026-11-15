using System.Reflection;
using MessagePack;
using MessagePack.Resolvers;

static class Runner
{
    static IFormatterResolver? Gen(Assembly a) =>
        a.GetTypes().FirstOrDefault(t => t.Name == "GeneratedMessagePackResolver")?.GetField("Instance", BindingFlags.Public | BindingFlags.Static)?.GetValue(null) as IFormatterResolver;

    static void Main()
    {
        var appGen = Gen(typeof(Runner).Assembly);
        var libGen = Gen(typeof(Lib.Marker).Assembly);
        Console.WriteLine($"app GeneratedMessagePackResolver: {(appGen is null ? "ABSENT" : "present")}; lib GeneratedMessagePackResolver: {(libGen is null ? "ABSENT" : "present")}");
        var configs = new List<(string, IFormatterResolver?)>
        {
            ("app-generated-only", appGen),
            ("generated(app+lib)+builtin, no dynamic", appGen is null ? null : CompositeResolver.Create(new[] { appGen, libGen, BuiltinResolver.Instance }.Where(r => r is not null).ToArray()!)),
            ("StandardResolver (default)", StandardResolver.Instance),
        };
        object v = App.Probe.Make();
        Console.WriteLine("input : " + App.Probe.Show(v));
        foreach (var (name, r) in configs)
        {
            if (r is null) { Console.WriteLine($"[{name}] SKIPPED: no generated resolver in app"); continue; }
            try
            {
                var o = MessagePackSerializerOptions.Standard.WithResolver(r);
                byte[] b = MessagePackSerializer.Serialize(v.GetType(), v, o);
                object? back = MessagePackSerializer.Deserialize(v.GetType(), b, o);
                Console.WriteLine($"[{name}] wire={MessagePackSerializer.ConvertToJson(b)} back={App.Probe.Show(back!)}");
            }
            catch (Exception e)
            {
                var inner = e; while (inner.InnerException is not null) inner = inner.InnerException;
                Console.WriteLine($"[{name}] THREW {e.GetType().Name}: {inner.GetType().Name}: {inner.Message.Split('\n')[0]}");
            }
        }
    }
}
